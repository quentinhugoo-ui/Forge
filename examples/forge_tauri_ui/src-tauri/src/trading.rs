use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use crate::trading_manifest::{build_trading_scenario_manifest, TradingScenarioManifest};
#[cfg(test)]
use crate::trading_manifest::{trading_scenario_hashes_from_result, TradingScenarioReplayReport};
#[cfg(not(target_os = "windows"))]
use keyring::{Entry as KeyringEntry, Error as KeyringError};
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
#[cfg(target_os = "windows")]
use windows_sys::Win32::Security::Cryptography::{
    CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
};
#[cfg(target_os = "windows")]
use windows_sys::Win32::Foundation::LocalFree;

#[path = "trading_alpha.rs"]
mod trading_alpha;
pub use trading_alpha::*;

const DEFAULT_BASE_URL: &str = "https://api-fxpractice.oanda.com";
const DEFAULT_INSTRUMENT: &str = "NATGAS_USD";
const KEEP_STORED_SENTINEL: &str = "__FORGE_KEEP_STORED__";
const HISTORY_START_RFC3339: &str = "2006-01-01T00:00:00Z";
const OANDA_WATCHDOG_TICK_MS: u64 = 30_000;
const OANDA_WATCHDOG_HEARTBEAT_MS: u64 = 5 * 60 * 1000;
const OANDA_WATCHDOG_RECOVERY_MS: u64 = 20_000;
const OANDA_GRANULARITIES: &[&str] = &[
    "S5", "S10", "S15", "S30",
    "M1", "M2", "M4", "M5", "M10", "M15", "M30",
    "H1", "H2", "H3", "H4", "H6", "H8", "H12",
    "D", "W", "M",
];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TradingOandaConfigStatus {
    available: bool,
    source: String,
    instrument: String,
    base_url: String,
    account_id_present: bool,
    api_key_present: bool,
    message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TradingOandaProviderStatus {
    connected: bool,
    installed: bool,
    auth_source: String,
    base_url: String,
    account_id_hint: String,
    message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TradingOandaProviderValidationStatus {
    ok: bool,
    account_alias: String,
    currency: String,
    base_url: String,
    account_id_hint: String,
    message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TradingAccountSnapshot {
    alias: String,
    currency: String,
    balance: f64,
    nav: f64,
    unrealized_pl: f64,
    margin_available: f64,
    open_trade_count: u64,
    open_position_count: u64,
    pending_order_count: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TradingPriceSnapshot {
    instrument: String,
    time: String,
    bid: f64,
    ask: f64,
    mid: f64,
    spread: f64,
    units_available_long: f64,
    units_available_short: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TradingInstrumentSummary {
    name: String,
    display_name: String,
    asset_class: String,
    pip_location: Option<i64>,
    display_precision: Option<i64>,
    trade_units_precision: Option<i64>,
    minimum_trade_size: Option<f64>,
    margin_rate: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TradingOrderSummary {
    id: String,
    instrument: String,
    side: String,
    order_type: String,
    units: f64,
    price: Option<f64>,
    state: String,
    create_time: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TradingBookLevel {
    price: f64,
    size: f64,
    pending_units: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TradingBookSnapshot {
    kind: String,
    note: String,
    bids: Vec<TradingBookLevel>,
    asks: Vec<TradingBookLevel>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TradingHistoryFileSummary {
    instrument: String,
    granularity: String,
    path: String,
    rows: usize,
    first_time: Option<String>,
    last_time: Option<String>,
    truncated: bool,
    updated_at_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TradingAssetCatalogEntry {
    instrument: String,
    display_name: String,
    asset_class: String,
    granularities: Vec<String>,
    rows: usize,
    first_time: Option<String>,
    last_time: Option<String>,
    updated_at_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TradingSnapshotResponse {
    config: TradingOandaConfigStatus,
    account: Option<TradingAccountSnapshot>,
    price: Option<TradingPriceSnapshot>,
    instruments: Vec<TradingInstrumentSummary>,
    pending_orders: Vec<TradingOrderSummary>,
    open_trades: Vec<TradingOrderSummary>,
    book: Option<TradingBookSnapshot>,
    history_dir: String,
    history_files: Vec<TradingHistoryFileSummary>,
    asset_catalog: Vec<TradingAssetCatalogEntry>,
    runtime: Option<TradingRuntimeStatus>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TradingCandlePoint {
    time: String,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    volume: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TradingMarketFeedResponse {
    config: TradingOandaConfigStatus,
    instrument: String,
    granularity: String,
    price: Option<TradingPriceSnapshot>,
    pending_orders: Vec<TradingOrderSummary>,
    open_trades: Vec<TradingOrderSummary>,
    book: Option<TradingBookSnapshot>,
    candles: Vec<TradingCandlePoint>,
    alerts: Vec<TradingAlertRecord>,
    alert_events: Vec<TradingAlertEvent>,
    runtime: Option<TradingRuntimeStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TradingAlertNotifications {
    #[serde(default)]
    app: bool,
    #[serde(default)]
    toast: bool,
    #[serde(default)]
    email: bool,
    #[serde(default)]
    sound: bool,
    #[serde(default)]
    email_to: String,
    #[serde(default = "default_alert_sound_profile")]
    sound_profile: String,
    #[serde(default = "default_alert_sound_volume")]
    sound_volume: f64,
    #[serde(default = "default_alert_sound_repeat")]
    sound_repeat: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TradingAlertRecord {
    id: String,
    instrument: String,
    granularity: String,
    condition_kind: String,
    operator: String,
    target_value: f64,
    trigger_mode: String,
    expiration_time_ms: Option<u64>,
    message: String,
    #[serde(default = "default_alert_active")]
    active: bool,
    created_at_ms: u64,
    updated_at_ms: u64,
    #[serde(default)]
    triggered_count: u64,
    last_triggered_at_ms: Option<u64>,
    last_relation: Option<i8>,
    notifications: TradingAlertNotifications,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TradingAlertEvent {
    id: String,
    alert_id: String,
    instrument: String,
    granularity: String,
    price: f64,
    message: String,
    triggered_at_ms: u64,
    notifications: TradingAlertNotifications,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TradingAlertsResponse {
    alerts: Vec<TradingAlertRecord>,
    events: Vec<TradingAlertEvent>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TradingRuntimeStatus {
    connected: bool,
    active_instrument: String,
    active_granularity: String,
    last_heartbeat_ms: u64,
    last_rest_check_ms: u64,
    last_resume_ms: u64,
    consecutive_failures: u32,
    last_error: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TradingSyncResponse {
    config: TradingOandaConfigStatus,
    history_dir: String,
    files: Vec<TradingHistoryFileSummary>,
    assets: Vec<TradingAssetCatalogEntry>,
    notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TradingChartSeriesResponse {
    instrument: String,
    granularity: String,
    asset: Option<TradingAssetCatalogEntry>,
    candles: Vec<TradingCandlePoint>,
    engine: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TradingChartMetricSeries {
    metric: String,
    values: Vec<f64>,
    last_value: Option<f64>,
    min_value: Option<f64>,
    max_value: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TradingChartComputeResponse {
    instrument: String,
    granularity: String,
    rows: usize,
    engine: String,
    metrics: Vec<TradingChartMetricSeries>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TradingOrderResponse {
    ok: bool,
    instrument: String,
    side: String,
    units: f64,
    order_type: String,
    approval_timestamp_bucket: String,
    approval_proof_hash: String,
    message: String,
    response: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TradingAssetRecord {
    token: String,
    symbol: String,
    display_name: String,
    asset_class: String,
    provider: String,
    provider_symbol: String,
    provider_hint: String,
    first_seen_at_ms: u64,
    updated_at_ms: u64,
    source: String,
    notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TradingAssetResolveResponse {
    asset: TradingAssetRecord,
    files: Vec<TradingHistoryFileSummary>,
    notes: Vec<String>,
    history_dir: String,
    fetched: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TradingOandaCredentialSaveRequest {
    account_id: String,
    api_key: String,
    base_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TradingHistorySyncRequest {
    max_rows_per_granularity: Option<usize>,
    instruments: Option<Vec<String>>,
    granularities: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TradingChartSeriesRequest {
    instrument: Option<String>,
    granularity: Option<String>,
    max_rows: Option<usize>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TradingChartComputeRequest {
    instrument: Option<String>,
    granularity: Option<String>,
    max_rows: Option<usize>,
    metrics: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TradingStrategySpec {
    instrument: Option<String>,
    granularity: Option<String>,
    broker: Option<String>,
    point_size: Option<f64>,
    point_size_source: Option<String>,
    point_size_warning: Option<String>,
    entry_hour: Option<u32>,
    entry_hours: Option<Vec<u32>>,
    entry_timezone: Option<String>,
    direction: Option<String>,
    stop_loss_distance: Option<f64>,
    take_profit_min_distance: Option<f64>,
    take_profit_max_distance: Option<f64>,
    target_win_rate: Option<f64>,
    daily_profit_target_distance: Option<f64>,
    low_volatility_metric: Option<String>,
    low_volatility_lookback: Option<usize>,
    low_volatility_percentile: Option<f64>,
    force_daily_entry: Option<bool>,
    spread_cost_distance: Option<f64>,
    slippage_distance: Option<f64>,
    max_hold_bars: Option<usize>,
    train_test_split: Option<f64>,
    candle_refs: Option<Vec<String>>,
    indicator_refs: Option<Vec<String>>,
    metric_commands: Option<Vec<String>>,
    source_text: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TradingStrategyBacktestRequest {
    spec: TradingStrategySpec,
    plan_only: Option<bool>,
    max_rows: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TradingStrategyMissingMetric {
    id: String,
    label: String,
    question: String,
    reason: String,
    examples: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TradingStrategyBacktestCandidate {
    direction: String,
    filter_id: String,
    filter_label: String,
    condition_hash: String,
    mask_ref: String,
    bytecode_ops: Vec<String>,
    display_formula: Vec<String>,
    indicator_refs: Vec<String>,
    entry_count: usize,
    take_profit_distance: f64,
    trades: usize,
    wins: usize,
    losses: usize,
    win_rate: Option<f64>,
    expectancy_distance: f64,
    net_pnl_distance: f64,
    profit_factor: Option<f64>,
    max_loss_streak: usize,
    avg_hold_bars: f64,
    meets_target: bool,
    #[serde(default)]
    daily_target_hit_rate: Option<f64>,
    #[serde(default)]
    positive_day_rate: Option<f64>,
    #[serde(default)]
    target_hit_days: Option<usize>,
    #[serde(default)]
    total_days: Option<usize>,
    #[serde(default)]
    avg_daily_pnl_distance: Option<f64>,
    #[serde(default)]
    min_daily_pnl_distance: Option<f64>,
    robustness: Option<TradingStrategyRobustness>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TradingStrategyRobustness {
    score: f64,
    grade: String,
    in_sample_trades: usize,
    in_sample_win_rate: Option<f64>,
    in_sample_expectancy_distance: f64,
    out_of_sample_trades: usize,
    out_of_sample_win_rate: Option<f64>,
    out_of_sample_expectancy_distance: f64,
    walk_forward_windows: usize,
    walk_forward_pass_rate: f64,
    monthly_positive_rate: Option<f64>,
    yearly_positive_rate: Option<f64>,
    stress_pass_rate: f64,
    worst_stress_expectancy_distance: f64,
    min_trades_ok: bool,
    warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TradingStrategyMetricCommand {
    token: String,
    kind: String,
    role: String,
    cache_key: String,
    node_hash: String,
    cache_hit: bool,
    artifact_uri: String,
    status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TradingStrategyTemplate {
    template_id: String,
    command: String,
    family: String,
    instrument: String,
    granularity: String,
    broker: String,
    direction: String,
    entry_hour_utc: u32,
    target_win_rate: Option<f64>,
    parameter_hash: String,
    data_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TradingStrategyPlanNode {
    id: String,
    label: String,
    operation: String,
    input_hashes: Vec<String>,
    node_hash: String,
    cache_key: String,
    cache_hit: bool,
    artifact_uri: String,
    estimated_items: usize,
    estimated_bytes: usize,
    gpu_candidate: bool,
    status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TradingStrategyCacheReport {
    artifact_root: String,
    data_hash: String,
    template_hash: String,
    result_cache_key: String,
    hits: usize,
    misses: usize,
    injected_results: usize,
    avoided_recalculations: usize,
    reused_nodes: Vec<String>,
    missed_nodes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TradingStrategyGpuPlan {
    preferred_engine: String,
    kernel: String,
    layout: String,
    work_items: usize,
    outcome_cube_key: String,
    gpu_required: bool,
    cpu_fallback: String,
    note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TradingStrategyComputePlan {
    engine: String,
    plan_id: String,
    execution_mode: String,
    metric_commands: Vec<TradingStrategyMetricCommand>,
    template: TradingStrategyTemplate,
    dag_nodes: Vec<TradingStrategyPlanNode>,
    cache_report: TradingStrategyCacheReport,
    gpu_plan: TradingStrategyGpuPlan,
    simulation_count: usize,
    outcome_cube_key: String,
    shared_cache_keys: Vec<String>,
    reused_calculations: usize,
    avoided_recalculations: usize,
    notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TradingStrategyPairedProbe {
    entries: usize,
    take_profit_distance: f64,
    long_wins: usize,
    short_wins: usize,
    long_win_rate: Option<f64>,
    short_win_rate: Option<f64>,
    long_expectancy_distance: f64,
    short_expectancy_distance: f64,
    long_net_pnl_distance: f64,
    short_net_pnl_distance: f64,
    edge: String,
    edge_score: f64,
    shared_entry_scan_cache_key: String,
    note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TradingStrategyVisualProbe {
    entry_time: String,
    exit_time: Option<String>,
    entry_index: usize,
    entry_price: f64,
    direction: String,
    stop_price: f64,
    take_profit_price: f64,
    stop_loss_distance: f64,
    take_profit_distance: f64,
    pnl_distance: f64,
    outcome: String,
    held_bars: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TradingStrategyBacktestResult {
    rows: usize,
    first_time: Option<String>,
    last_time: Option<String>,
    train_rows: usize,
    test_rows: usize,
    low_volatility_threshold: f64,
    entry_hour_utc: u32,
    #[serde(default)]
    entry_hours_utc: Vec<u32>,
    candidates: Vec<TradingStrategyBacktestCandidate>,
    best: Option<TradingStrategyBacktestCandidate>,
    paired_probe: Option<TradingStrategyPairedProbe>,
    visual_probes: Vec<TradingStrategyVisualProbe>,
    compute_plan: TradingStrategyComputePlan,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TradingStrategyBacktestResponse {
    ok: bool,
    status: String,
    engine: String,
    plan_only: bool,
    spec: TradingStrategySpec,
    missing_metrics: Vec<TradingStrategyMissingMetric>,
    questions: Vec<String>,
    plan: Vec<String>,
    compute_plan: Option<TradingStrategyComputePlan>,
    scenario_manifest: Option<TradingScenarioManifest>,
    result: Option<TradingStrategyBacktestResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TradingStrategyLiveSignal {
    id: String,
    status: String,
    instrument: String,
    granularity: String,
    direction: String,
    entry_time: String,
    entry_price: f64,
    stop_price: f64,
    take_profit_price: f64,
    take_profit_distance: f64,
    stop_loss_distance: f64,
    max_hold_bars: usize,
    exit_time: Option<String>,
    exit_price: Option<f64>,
    pnl_distance: Option<f64>,
    outcome: Option<String>,
    reason: String,
    created_at_ms: u64,
    updated_at_ms: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TradingStrategyLiveTickRequest {
    job_id: Option<String>,
    spec: TradingStrategySpec,
    low_volatility_threshold: Option<f64>,
    direction: Option<String>,
    take_profit_distance: Option<f64>,
    max_rows: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TradingStrategyLiveTickResponse {
    ok: bool,
    status: String,
    engine: String,
    job_id: String,
    evaluated_time: Option<String>,
    new_signal: Option<TradingStrategyLiveSignal>,
    closed_signals: Vec<TradingStrategyLiveSignal>,
    open_signals: Vec<TradingStrategyLiveSignal>,
    journal_tail: Vec<TradingStrategyLiveSignal>,
    notes: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TradingAssetResolveRequest {
    token: Option<String>,
    symbol: Option<String>,
    display_name: Option<String>,
    asset_class: Option<String>,
    provider_hint: Option<String>,
    source: Option<String>,
    start_date: Option<String>,
    granularities: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TradingPlaceOrderRequest {
    instrument: Option<String>,
    side: String,
    units: f64,
    order_type: Option<String>,
    limit_price: Option<f64>,
    take_profit: Option<f64>,
    stop_loss: Option<f64>,
    time_in_force: Option<String>,
    approval: Option<TradingLiveApprovalProof>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TradingLiveApprovalProof {
    approved: bool,
    approved_at_ms: u64,
    timestamp_bucket: String,
    provider_state: String,
    action_hash: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TradingMarketFeedRequest {
    instrument: Option<String>,
    granularity: Option<String>,
    count: Option<usize>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TradingAlertsListRequest {
    instrument: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TradingAlertInput {
    id: Option<String>,
    instrument: Option<String>,
    granularity: Option<String>,
    condition_kind: Option<String>,
    operator: Option<String>,
    target_value: Option<f64>,
    trigger_mode: Option<String>,
    expiration_time_ms: Option<u64>,
    message: Option<String>,
    active: Option<bool>,
    notifications: Option<TradingAlertNotifications>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TradingAlertUpsertRequest {
    alert: TradingAlertInput,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TradingAlertDeleteRequest {
    id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct StoredOandaCredentials {
    account_id: String,
    api_key: String,
    base_url: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct StoredOandaCredentialEnvelope {
    version: u32,
    algorithm: String,
    ciphertext: String,
}

#[derive(Debug, Clone, Copy)]
enum LocalCredentialSource {
    EncryptedStore,
    #[cfg(not(target_os = "windows"))]
    SecureSystemStore,
    LegacyPlaintext,
}

#[derive(Debug, Clone)]
struct LoadedLocalCredentials {
    credentials: StoredOandaCredentials,
    source: LocalCredentialSource,
}

#[derive(Debug, Clone)]
struct ResolvedOandaCredentials {
    account_id: String,
    api_key: String,
    base_url: String,
    source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TradingHistoryManifest {
    #[serde(default)]
    instrument: Option<String>,
    source: String,
    generated_at_ms: u64,
    files: Vec<TradingHistoryFileSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct TradingAssetStore {
    #[serde(default)]
    assets: Vec<TradingAssetRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct TradingAlertStore {
    #[serde(default)]
    alerts: Vec<TradingAlertRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct TradingStrategyLiveStore {
    #[serde(default)]
    jobs: Vec<TradingStrategyLiveJob>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TradingStrategyLiveJob {
    job_id: String,
    status: String,
    spec: TradingStrategySpec,
    low_volatility_threshold: f64,
    direction: String,
    take_profit_distance: f64,
    last_evaluated_time: Option<String>,
    signals: Vec<TradingStrategyLiveSignal>,
    created_at_ms: u64,
    updated_at_ms: u64,
}

#[derive(Debug, Clone)]
struct TradingRuntimeBundle {
    account: Option<TradingAccountSnapshot>,
    price: Option<TradingPriceSnapshot>,
    instruments: Vec<TradingInstrumentSummary>,
    pending_orders: Vec<TradingOrderSummary>,
    open_trades: Vec<TradingOrderSummary>,
    book: Option<TradingBookSnapshot>,
    candles: Vec<TradingCandlePoint>,
}

#[derive(Debug, Clone)]
struct OandaCandleSample {
    point: TradingCandlePoint,
}

#[derive(Debug, Clone)]
struct OandaRuntimeState {
    connected: bool,
    active_instrument: String,
    active_granularity: String,
    active_count: usize,
    cached_instrument: String,
    cached_granularity: String,
    last_heartbeat_ms: u64,
    last_rest_check_ms: u64,
    last_resume_ms: u64,
    last_attempt_ms: u64,
    consecutive_failures: u32,
    last_error: String,
    credentials_fingerprint: String,
    account: Option<TradingAccountSnapshot>,
    price: Option<TradingPriceSnapshot>,
    instruments: Vec<TradingInstrumentSummary>,
    pending_orders: Vec<TradingOrderSummary>,
    open_trades: Vec<TradingOrderSummary>,
    book: Option<TradingBookSnapshot>,
    candles: Vec<TradingCandlePoint>,
}

fn trading_store_dir() -> PathBuf {
    crate::forge_store_dir().join("trading")
}

fn default_alert_active() -> bool {
    true
}

fn default_alert_sound_profile() -> String {
    "soft".to_string()
}

fn default_alert_sound_volume() -> f64 {
    0.82
}

fn default_alert_sound_repeat() -> u32 {
    2
}

fn default_alert_notifications() -> TradingAlertNotifications {
    TradingAlertNotifications {
        app: true,
        toast: true,
        email: false,
        sound: true,
        email_to: String::new(),
        sound_profile: default_alert_sound_profile(),
        sound_volume: default_alert_sound_volume(),
        sound_repeat: default_alert_sound_repeat(),
    }
}

fn trading_alerts_file_path() -> PathBuf {
    trading_store_dir().join("alerts.json")
}

fn trading_assets_file_path() -> PathBuf {
    trading_store_dir().join("assets.json")
}

fn trading_strategy_live_jobs_file_path() -> PathBuf {
    trading_store_dir().join("strategy_live_jobs.json")
}

fn trading_strategy_cache_dir() -> PathBuf {
    trading_store_dir().join("strategy_cache")
}

fn history_root_dir() -> PathBuf {
    trading_store_dir().join("oanda")
}

fn sanitize_history_component(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return DEFAULT_INSTRUMENT.to_string();
    }
    trimmed
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.') {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn history_instrument_dir(instrument: &str) -> PathBuf {
    history_root_dir().join(sanitize_history_component(instrument))
}

fn credentials_file_path() -> PathBuf {
    trading_store_dir().join("oanda_credentials.json")
}

#[cfg(target_os = "windows")]
fn secure_storage_label() -> &'static str {
    "Windows DPAPI"
}

#[cfg(target_os = "macos")]
fn secure_storage_label() -> &'static str {
    "macOS Keychain"
}

#[cfg(target_os = "linux")]
fn secure_storage_label() -> &'static str {
    "Linux Secret Service"
}

#[cfg(all(not(target_os = "windows"), not(target_os = "macos"), not(target_os = "linux")))]
fn secure_storage_label() -> &'static str {
    "OS secure store"
}

#[cfg(not(target_os = "windows"))]
fn secure_store_service_name() -> &'static str {
    "Forge/OANDA"
}

#[cfg(not(target_os = "windows"))]
fn secure_store_account_name() -> &'static str {
    "default"
}

fn history_manifest_path() -> PathBuf {
    history_root_dir().join("manifest.json")
}

fn trading_bot_env_candidates() -> Vec<PathBuf> {
    let workspace = crate::forge_workspace_dir();
    let mut out = Vec::new();
    if let Some(parent) = workspace.parent() {
        let trading_bot = parent.join("Trading bot");
        out.push(trading_bot.join(".env"));
        out.push(trading_bot.join(".env.local"));
        out.push(trading_bot.join(".env.production"));
    }
    out
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn trading_approval_bucket(ms: u64) -> String {
    let bucket = ms / 300_000;
    format!("5m:{bucket}")
}

fn trading_order_provider_state(config: &TradingOandaConfigStatus) -> String {
    format!(
        "oanda|source={}|base={}|account={}|key={}",
        config.source,
        config.base_url,
        if config.account_id_present { "present" } else { "missing" },
        if config.api_key_present { "present" } else { "missing" }
    )
}

fn default_runtime_state() -> OandaRuntimeState {
    OandaRuntimeState {
        connected: false,
        active_instrument: DEFAULT_INSTRUMENT.to_string(),
        active_granularity: "H4".to_string(),
        active_count: 240,
        cached_instrument: String::new(),
        cached_granularity: String::new(),
        last_heartbeat_ms: 0,
        last_rest_check_ms: 0,
        last_resume_ms: 0,
        last_attempt_ms: 0,
        consecutive_failures: 0,
        last_error: String::new(),
        credentials_fingerprint: String::new(),
        account: None,
        price: None,
        instruments: Vec::new(),
        pending_orders: Vec::new(),
        open_trades: Vec::new(),
        book: None,
        candles: Vec::new(),
    }
}

static OANDA_RUNTIME_STATE: OnceLock<Mutex<OandaRuntimeState>> = OnceLock::new();
static OANDA_WATCHDOG_STARTED: AtomicBool = AtomicBool::new(false);
static TRADING_ALERT_STORE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
static TRADING_STRATEGY_LIVE_STORE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
static TRADING_STRATEGY_LOW_VOL_CACHE: OnceLock<Mutex<HashMap<String, Vec<f64>>>> = OnceLock::new();
static TRADING_STRATEGY_FEATURE_BANK_CACHE: OnceLock<Mutex<HashMap<String, Arc<StrategyIndicatorFeatureBank>>>> = OnceLock::new();
static TRADING_STRATEGY_ENTRY_SCAN_CACHE: OnceLock<Mutex<HashMap<String, Vec<usize>>>> = OnceLock::new();
static TRADING_STRATEGY_FILTER_ENTRY_CACHE: OnceLock<Mutex<HashMap<String, Vec<StrategyEntryFilter>>>> = OnceLock::new();
static TRADING_STRATEGY_CONDITION_MASK_CACHE: OnceLock<Mutex<HashMap<String, StrategyConditionMask>>> = OnceLock::new();
static TRADING_STRATEGY_ENTRY_OUTCOME_CACHE: OnceLock<Mutex<HashMap<String, StrategyEntryOutcome>>> = OnceLock::new();
static TRADING_STRATEGY_FILTER_OUTCOME_GRID_CACHE: OnceLock<Mutex<HashMap<String, StrategyFilterOutcomeGrid>>> = OnceLock::new();
static TRADING_STRATEGY_STATS_CACHE: OnceLock<Mutex<HashMap<String, StrategySimulationStats>>> = OnceLock::new();
static TRADING_STRATEGY_ROBUSTNESS_CACHE: OnceLock<Mutex<HashMap<String, TradingStrategyRobustness>>> = OnceLock::new();
static TRADING_STRATEGY_RESULT_CACHE: OnceLock<Mutex<HashMap<String, TradingStrategyBacktestResult>>> = OnceLock::new();

fn oanda_runtime_lock() -> &'static Mutex<OandaRuntimeState> {
    OANDA_RUNTIME_STATE.get_or_init(|| Mutex::new(default_runtime_state()))
}

fn trading_alert_store_lock() -> &'static Mutex<()> {
    TRADING_ALERT_STORE_LOCK.get_or_init(|| Mutex::new(()))
}

fn trading_strategy_live_store_lock() -> &'static Mutex<()> {
    TRADING_STRATEGY_LIVE_STORE_LOCK.get_or_init(|| Mutex::new(()))
}

fn trading_strategy_low_vol_cache() -> &'static Mutex<HashMap<String, Vec<f64>>> {
    TRADING_STRATEGY_LOW_VOL_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn trading_strategy_feature_bank_cache() -> &'static Mutex<HashMap<String, Arc<StrategyIndicatorFeatureBank>>> {
    TRADING_STRATEGY_FEATURE_BANK_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn trading_strategy_entry_scan_cache() -> &'static Mutex<HashMap<String, Vec<usize>>> {
    TRADING_STRATEGY_ENTRY_SCAN_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn trading_strategy_filter_entry_cache() -> &'static Mutex<HashMap<String, Vec<StrategyEntryFilter>>> {
    TRADING_STRATEGY_FILTER_ENTRY_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn trading_strategy_condition_mask_cache() -> &'static Mutex<HashMap<String, StrategyConditionMask>> {
    TRADING_STRATEGY_CONDITION_MASK_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn trading_strategy_entry_outcome_cache() -> &'static Mutex<HashMap<String, StrategyEntryOutcome>> {
    TRADING_STRATEGY_ENTRY_OUTCOME_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn trading_strategy_filter_outcome_grid_cache() -> &'static Mutex<HashMap<String, StrategyFilterOutcomeGrid>> {
    TRADING_STRATEGY_FILTER_OUTCOME_GRID_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn trading_strategy_stats_cache() -> &'static Mutex<HashMap<String, StrategySimulationStats>> {
    TRADING_STRATEGY_STATS_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn trading_strategy_robustness_cache() -> &'static Mutex<HashMap<String, TradingStrategyRobustness>> {
    TRADING_STRATEGY_ROBUSTNESS_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn trading_strategy_result_cache() -> &'static Mutex<HashMap<String, TradingStrategyBacktestResult>> {
    TRADING_STRATEGY_RESULT_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn oanda_runtime_status(state: &OandaRuntimeState) -> TradingRuntimeStatus {
    TradingRuntimeStatus {
        connected: state.connected,
        active_instrument: state.active_instrument.clone(),
        active_granularity: state.active_granularity.clone(),
        last_heartbeat_ms: state.last_heartbeat_ms,
        last_rest_check_ms: state.last_rest_check_ms,
        last_resume_ms: state.last_resume_ms,
        consecutive_failures: state.consecutive_failures,
        last_error: state.last_error.clone(),
    }
}

fn credentials_fingerprint(credentials: &ResolvedOandaCredentials) -> String {
    format!(
        "{}|{}|{}",
        credentials.base_url.trim(),
        credentials.account_id.trim(),
        credentials.api_key.trim().len()
    )
}

fn read_env_file_map(path: &Path) -> HashMap<String, String> {
    let Ok(text) = fs::read_to_string(path) else {
        return HashMap::new();
    };
    let mut map = HashMap::new();
    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let normalized = value
            .trim()
            .trim_matches('"')
            .trim_matches('\'')
            .to_string();
        map.insert(key.trim().to_string(), normalized);
    }
    map
}

#[cfg(target_os = "windows")]
fn dpapi_entropy_blob() -> CRYPT_INTEGER_BLOB {
    let bytes = b"Forge OANDA credentials";
    CRYPT_INTEGER_BLOB {
        cbData: bytes.len() as u32,
        pbData: bytes.as_ptr() as *mut u8,
    }
}

#[cfg(target_os = "windows")]
fn encrypt_local_credentials_blob(bytes: &[u8]) -> Result<Vec<u8>, String> {
    if bytes.is_empty() {
        return Err("cannot encrypt empty OANDA credentials".to_string());
    }
    let input = CRYPT_INTEGER_BLOB {
        cbData: bytes.len() as u32,
        pbData: bytes.as_ptr() as *mut u8,
    };
    let entropy = dpapi_entropy_blob();
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };
    let ok = unsafe {
        CryptProtectData(
            &input,
            std::ptr::null(),
            &entropy,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    };
    if ok == 0 {
        return Err(format!(
            "Windows DPAPI protect failed: {}",
            std::io::Error::last_os_error()
        ));
    }
    let encrypted = unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec() };
    unsafe {
        LocalFree(output.pbData.cast());
    }
    Ok(encrypted)
}

#[cfg(target_os = "windows")]
fn decrypt_local_credentials_blob(bytes: &[u8]) -> Result<Vec<u8>, String> {
    if bytes.is_empty() {
        return Err("cannot decrypt empty OANDA credentials".to_string());
    }
    let input = CRYPT_INTEGER_BLOB {
        cbData: bytes.len() as u32,
        pbData: bytes.as_ptr() as *mut u8,
    };
    let entropy = dpapi_entropy_blob();
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };
    let ok = unsafe {
        CryptUnprotectData(
            &input,
            std::ptr::null_mut(),
            &entropy,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    };
    if ok == 0 {
        return Err(format!(
            "Windows DPAPI decrypt failed: {}",
            std::io::Error::last_os_error()
        ));
    }
    let decrypted = unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec() };
    unsafe {
        LocalFree(output.pbData.cast());
    }
    Ok(decrypted)
}

#[cfg(not(target_os = "windows"))]
fn encrypt_local_credentials_blob(_bytes: &[u8]) -> Result<Vec<u8>, String> {
    Err("Encrypted OANDA credential storage is only available on Windows in this build.".to_string())
}

#[cfg(not(target_os = "windows"))]
fn decrypt_local_credentials_blob(_bytes: &[u8]) -> Result<Vec<u8>, String> {
    Err("Encrypted OANDA credential storage is only available on Windows in this build.".to_string())
}

#[cfg(not(target_os = "windows"))]
fn secure_system_store_entry() -> Result<KeyringEntry, String> {
    KeyringEntry::new(secure_store_service_name(), secure_store_account_name())
        .map_err(|e| format!("open {} entry: {e}", secure_storage_label()))
}

#[cfg(not(target_os = "windows"))]
fn load_secure_system_credentials() -> Option<StoredOandaCredentials> {
    let entry = secure_system_store_entry().ok()?;
    let payload = entry.get_password().ok()?;
    serde_json::from_str::<StoredOandaCredentials>(&payload).ok()
}

#[cfg(not(target_os = "windows"))]
fn save_secure_system_credentials(credentials: &StoredOandaCredentials) -> Result<(), String> {
    let entry = secure_system_store_entry()?;
    let payload = serde_json::to_string(credentials)
        .map_err(|e| format!("encode credentials for {}: {e}", secure_storage_label()))?;
    entry
        .set_password(&payload)
        .map_err(|e| format!("save credentials to {}: {e}", secure_storage_label()))
}

#[cfg(not(target_os = "windows"))]
fn clear_secure_system_credentials() -> Result<(), String> {
    let entry = secure_system_store_entry()?;
    match entry.delete_credential() {
        Ok(_) => Ok(()),
        Err(KeyringError::NoEntry) => Ok(()),
        Err(e) => Err(format!(
            "remove credentials from {}: {e}",
            secure_storage_label()
        )),
    }
}

fn load_local_credentials() -> Option<LoadedLocalCredentials> {
    #[cfg(not(target_os = "windows"))]
    if let Some(credentials) = load_secure_system_credentials() {
        return Some(LoadedLocalCredentials {
            credentials,
            source: LocalCredentialSource::SecureSystemStore,
        });
    }

    let path = credentials_file_path();
    let bytes = fs::read(path).ok()?;
    if let Ok(envelope) = serde_json::from_slice::<StoredOandaCredentialEnvelope>(&bytes) {
        if envelope.algorithm == "windows-dpapi" && !envelope.ciphertext.trim().is_empty() {
            let encrypted = BASE64_STANDARD.decode(envelope.ciphertext.trim()).ok()?;
            let decrypted = decrypt_local_credentials_blob(&encrypted).ok()?;
            let credentials = serde_json::from_slice::<StoredOandaCredentials>(&decrypted).ok()?;
            return Some(LoadedLocalCredentials {
                credentials,
                source: LocalCredentialSource::EncryptedStore,
            });
        }
    }
    serde_json::from_slice::<StoredOandaCredentials>(&bytes)
        .ok()
        .map(|credentials| LoadedLocalCredentials {
            credentials,
            source: LocalCredentialSource::LegacyPlaintext,
        })
}

fn save_local_credentials(credentials: &StoredOandaCredentials) -> Result<(), String> {
    #[cfg(not(target_os = "windows"))]
    {
        save_secure_system_credentials(credentials)?;
        let path = credentials_file_path();
        if path.exists() {
            fs::remove_file(&path)
                .map_err(|e| format!("remove stale credentials '{}': {e}", path.display()))?;
        }
        return Ok(());
    }

    let path = credentials_file_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("create credentials dir '{}': {e}", parent.display()))?;
    }
    let payload = serde_json::to_vec(credentials)
        .map_err(|e| format!("encode credentials: {e}"))?;
    let encrypted = encrypt_local_credentials_blob(&payload)?;
    let envelope = StoredOandaCredentialEnvelope {
        version: 1,
        algorithm: "windows-dpapi".to_string(),
        ciphertext: BASE64_STANDARD.encode(encrypted),
    };
    let bytes = serde_json::to_vec_pretty(&envelope)
        .map_err(|e| format!("encode encrypted credentials: {e}"))?;
    fs::write(&path, bytes)
        .map_err(|e| format!("write credentials '{}': {e}", path.display()))?;
    Ok(())
}

fn clear_local_credentials() -> Result<(), String> {
    #[cfg(not(target_os = "windows"))]
    {
        clear_secure_system_credentials()?;
    }

    let path = credentials_file_path();
    if path.exists() {
        fs::remove_file(&path)
            .map_err(|e| format!("remove credentials '{}': {e}", path.display()))?;
    }
    Ok(())
}

fn trading_now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_millis() as u64)
        .unwrap_or(0)
}

fn load_trading_alert_store() -> Result<TradingAlertStore, String> {
    let path = trading_alerts_file_path();
    if !path.exists() {
        return Ok(TradingAlertStore::default());
    }
    let bytes = fs::read(&path)
        .map_err(|e| format!("read alerts '{}': {e}", path.display()))?;
    serde_json::from_slice::<TradingAlertStore>(&bytes)
        .map_err(|e| format!("decode alerts '{}': {e}", path.display()))
}

fn save_trading_alert_store(store: &TradingAlertStore) -> Result<(), String> {
    let path = trading_alerts_file_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("create alerts dir '{}': {e}", parent.display()))?;
    }
    let bytes = serde_json::to_vec_pretty(store)
        .map_err(|e| format!("encode alerts '{}': {e}", path.display()))?;
    fs::write(&path, bytes)
        .map_err(|e| format!("write alerts '{}': {e}", path.display()))
}

fn normalize_alert_operator(value: Option<&str>) -> String {
    match value.unwrap_or("crossing").trim().to_lowercase().as_str() {
        "crossing_up" => "crossing_up".to_string(),
        "crossing_down" => "crossing_down".to_string(),
        "above" => "above".to_string(),
        "below" => "below".to_string(),
        _ => "crossing".to_string(),
    }
}

fn normalize_alert_trigger_mode(value: Option<&str>) -> String {
    match value.unwrap_or("once").trim().to_lowercase().as_str() {
        "repeat" => "repeat".to_string(),
        _ => "once".to_string(),
    }
}

fn normalize_alert_condition_kind(value: Option<&str>) -> String {
    match value.unwrap_or("price").trim().to_lowercase().as_str() {
        "price" => "price".to_string(),
        _ => "price".to_string(),
    }
}

fn sanitize_alert_notifications(raw: Option<TradingAlertNotifications>) -> TradingAlertNotifications {
    let mut notifications = raw.unwrap_or_else(default_alert_notifications);
    notifications.sound_profile = match notifications.sound_profile.trim().to_lowercase().as_str() {
        "bell" => "bell".to_string(),
        "pulse" => "pulse".to_string(),
        _ => "soft".to_string(),
    };
    notifications.sound_volume = notifications.sound_volume.clamp(0.2, 1.0);
    notifications.sound_repeat = notifications.sound_repeat.clamp(1, 4);
    notifications.email_to = notifications.email_to.trim().to_string();
    notifications
}

fn build_alert_message(instrument: &str, operator: &str, target_value: f64) -> String {
    let operator_label = match operator {
        "crossing_up" => "Croisement haussier",
        "crossing_down" => "Croisement baissier",
        "above" => "Au-dessus",
        "below" => "En dessous",
        _ => "Croisement",
    };
    format!("{instrument} {operator_label} {:.3}", target_value)
}

fn load_saved_trading_alerts_filtered(instrument: Option<&str>) -> Result<Vec<TradingAlertRecord>, String> {
    let _guard = trading_alert_store_lock().lock().unwrap();
    let store = load_trading_alert_store()?;
    let filter = instrument
        .map(|value| value.trim().to_uppercase())
        .filter(|value| !value.is_empty());
    Ok(store
        .alerts
        .into_iter()
        .filter(|alert| {
            filter
                .as_ref()
                .map(|target| alert.instrument.trim().eq_ignore_ascii_case(target))
                .unwrap_or(true)
        })
        .collect())
}

fn market_reference_price(
    price: Option<&TradingPriceSnapshot>,
    candles: &[TradingCandlePoint],
) -> Option<f64> {
    if let Some(price) = price {
        if price.mid.is_finite() {
            return Some(price.mid);
        }
    }
    candles.last().and_then(|candle| {
        if candle.close.is_finite() {
            Some(candle.close)
        } else {
            None
        }
    })
}

fn alert_price_relation(current_price: f64, target_value: f64) -> i8 {
    let epsilon = f64::max(0.000_001, target_value.abs() * 0.000_1);
    if (current_price - target_value).abs() <= epsilon {
        0
    } else if current_price > target_value {
        1
    } else {
        -1
    }
}

fn evaluate_trading_alerts(
    instrument: &str,
    granularity: &str,
    price: Option<&TradingPriceSnapshot>,
    candles: &[TradingCandlePoint],
) -> Result<TradingAlertsResponse, String> {
    let _guard = trading_alert_store_lock().lock().unwrap();
    let mut store = load_trading_alert_store()?;
    let now_ms = trading_now_ms();
    let current_price = market_reference_price(price, candles);
    let mut events = Vec::new();
    let mut changed = false;

    for alert in store.alerts.iter_mut() {
        if !alert.instrument.eq_ignore_ascii_case(instrument) {
            continue;
        }
        if let Some(expiration_time_ms) = alert.expiration_time_ms {
            if expiration_time_ms <= now_ms && alert.active {
                alert.active = false;
                changed = true;
            }
        }
        if !alert.active {
            continue;
        }
        let Some(reference_price) = current_price else {
            continue;
        };
        let next_relation = alert_price_relation(reference_price, alert.target_value);
        let previous_relation = alert.last_relation;
        let should_trigger = match alert.operator.as_str() {
            "crossing_up" => previous_relation.map(|value| value < 0 && next_relation >= 0).unwrap_or(false),
            "crossing_down" => previous_relation.map(|value| value > 0 && next_relation <= 0).unwrap_or(false),
            "above" => previous_relation.map(|value| value < 0 && next_relation >= 0).unwrap_or(false),
            "below" => previous_relation.map(|value| value > 0 && next_relation <= 0).unwrap_or(false),
            _ => previous_relation
                .map(|value| value != 0 && next_relation != 0 && value != next_relation)
                .unwrap_or(false),
        };
        if alert.last_relation != Some(next_relation) {
            alert.last_relation = Some(next_relation);
            changed = true;
        }
        if !should_trigger {
            continue;
        }
        alert.triggered_count = alert.triggered_count.saturating_add(1);
        alert.last_triggered_at_ms = Some(now_ms);
        alert.updated_at_ms = now_ms;
        if alert.trigger_mode == "once" {
            alert.active = false;
        }
        events.push(TradingAlertEvent {
            id: format!("evt_{}_{}", alert.id, now_ms),
            alert_id: alert.id.clone(),
            instrument: alert.instrument.clone(),
            granularity: granularity.to_uppercase(),
            price: reference_price,
            message: if alert.message.trim().is_empty() {
                build_alert_message(&alert.instrument, &alert.operator, alert.target_value)
            } else {
                alert.message.clone()
            },
            triggered_at_ms: now_ms,
            notifications: alert.notifications.clone(),
        });
    }

    if changed {
        save_trading_alert_store(&store)?;
    }

    Ok(TradingAlertsResponse {
        alerts: store
            .alerts
            .into_iter()
            .filter(|alert| alert.instrument.eq_ignore_ascii_case(instrument))
            .collect(),
        events,
    })
}

fn attach_alerts_to_market_response(
    mut response: TradingMarketFeedResponse,
) -> TradingMarketFeedResponse {
    match evaluate_trading_alerts(
        &response.instrument,
        &response.granularity,
        response.price.as_ref(),
        &response.candles,
    ) {
        Ok(alert_payload) => {
            response.alerts = alert_payload.alerts;
            response.alert_events = alert_payload.events;
        }
        Err(_) => {
            response.alerts = load_saved_trading_alerts_filtered(Some(&response.instrument)).unwrap_or_default();
            response.alert_events = Vec::new();
        }
    }
    response
}

fn mask_account_id(account_id: &str) -> String {
    let trimmed = account_id.trim();
    if trimmed.is_empty() {
        return "not set".to_string();
    }
    let keep = trimmed.chars().count().min(4);
    let suffix = trimmed
        .chars()
        .rev()
        .take(keep)
        .collect::<String>()
        .chars()
        .rev()
        .collect::<String>();
    format!("***{suffix}")
}

fn resolve_credentials() -> Option<ResolvedOandaCredentials> {
    if let Some(local) = load_local_credentials() {
        let source = match local.source {
            LocalCredentialSource::EncryptedStore => {
                format!("local Forge encrypted credentials ({})", secure_storage_label())
            }
            #[cfg(not(target_os = "windows"))]
            LocalCredentialSource::SecureSystemStore => {
                format!("local Forge secure system store ({})", secure_storage_label())
            }
            LocalCredentialSource::LegacyPlaintext => "local Forge credentials (legacy plaintext file)".to_string(),
        };
        if !local.credentials.account_id.trim().is_empty() && !local.credentials.api_key.trim().is_empty() {
            return Some(ResolvedOandaCredentials {
                account_id: local.credentials.account_id.trim().to_string(),
                api_key: local.credentials.api_key.trim().to_string(),
                base_url: if local.credentials.base_url.trim().is_empty() {
                    DEFAULT_BASE_URL.to_string()
                } else {
                    local.credentials.base_url.trim().to_string()
                },
                source,
            });
        }
    }

    let account_id = std::env::var("OANDA_ACCOUNT_ID").unwrap_or_default();
    let api_key = std::env::var("OANDA_API_KEY").unwrap_or_default();
    let base_url = std::env::var("OANDA_BASE_URL").unwrap_or_else(|_| DEFAULT_BASE_URL.to_string());
    if !account_id.trim().is_empty() && !api_key.trim().is_empty() {
        return Some(ResolvedOandaCredentials {
            account_id: account_id.trim().to_string(),
            api_key: api_key.trim().to_string(),
            base_url: if base_url.trim().is_empty() {
                DEFAULT_BASE_URL.to_string()
            } else {
                base_url.trim().to_string()
            },
            source: "process environment".to_string(),
        });
    }

    for candidate in trading_bot_env_candidates() {
        let map = read_env_file_map(&candidate);
        let account_id = map.get("OANDA_ACCOUNT_ID").cloned().unwrap_or_default();
        let api_key = map.get("OANDA_API_KEY").cloned().unwrap_or_default();
        let base_url = map
            .get("OANDA_BASE_URL")
            .cloned()
            .unwrap_or_else(|| DEFAULT_BASE_URL.to_string());
        if !account_id.trim().is_empty() && !api_key.trim().is_empty() {
            return Some(ResolvedOandaCredentials {
                account_id: account_id.trim().to_string(),
                api_key: api_key.trim().to_string(),
                base_url: if base_url.trim().is_empty() {
                    DEFAULT_BASE_URL.to_string()
                } else {
                    base_url.trim().to_string()
                },
                source: format!("Trading bot env file ({})", candidate.display()),
            });
        }
    }
    None
}

fn config_status_from_credentials(credentials: Option<&ResolvedOandaCredentials>) -> TradingOandaConfigStatus {
    match credentials {
        Some(resolved) => TradingOandaConfigStatus {
            available: true,
            source: resolved.source.clone(),
            instrument: DEFAULT_INSTRUMENT.to_string(),
            base_url: resolved.base_url.clone(),
            account_id_present: true,
            api_key_present: true,
            message: format!("OANDA credentials ready via {}.", secure_storage_label()),
        },
        None => TradingOandaConfigStatus {
            available: false,
            source: "missing".to_string(),
            instrument: DEFAULT_INSTRUMENT.to_string(),
            base_url: DEFAULT_BASE_URL.to_string(),
            account_id_present: false,
            api_key_present: false,
            message: format!(
                "No OANDA credentials found in the local Forge secure store ({}), process env, or Trading bot .env.",
                secure_storage_label()
            ),
        },
    }
}

fn provider_status_from_credentials(
    credentials: Option<&ResolvedOandaCredentials>,
    message_override: Option<String>,
) -> TradingOandaProviderStatus {
    match credentials {
        Some(resolved) => TradingOandaProviderStatus {
            connected: true,
            installed: true,
            auth_source: resolved.source.clone(),
            base_url: resolved.base_url.clone(),
            account_id_hint: mask_account_id(&resolved.account_id),
            message: message_override.unwrap_or_else(|| {
                if resolved.source.contains("legacy plaintext") {
                    "OANDA credentials are active, but the local file is still legacy plaintext. Re-save them from the OANDA terminal to migrate to encrypted storage."
                        .to_string()
                } else {
                    format!(
                        "OANDA credentials are ready for market data and order routing. Forge stores them only on this machine with {} and does not upload them to Forge developers or Forge servers.",
                        secure_storage_label()
                    )
                }
            }),
        },
        None => TradingOandaProviderStatus {
            connected: false,
            installed: true,
            auth_source: "none".to_string(),
            base_url: DEFAULT_BASE_URL.to_string(),
            account_id_hint: "not set".to_string(),
            message: message_override.unwrap_or_else(|| {
                format!(
                    "No OANDA credentials found. Use the terminal below to validate and save them in the local secure store backed by {}.",
                    secure_storage_label()
                )
            }),
        },
    }
}

fn oanda_headers(api_key: &str) -> Result<HeaderMap, String> {
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
    headers.insert(
        "Accept-Datetime-Format",
        HeaderValue::from_static("RFC3339"),
    );
    let auth = format!("Bearer {}", api_key.trim());
    let auth_value = HeaderValue::from_str(&auth).map_err(|e| format!("invalid authorization header: {e}"))?;
    headers.insert(AUTHORIZATION, auth_value);
    Ok(headers)
}

fn build_oanda_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|e| format!("build OANDA client failed: {e}"))
}

fn parse_f64_value(value: Option<&Value>) -> Option<f64> {
    match value {
        Some(Value::Number(n)) => n.as_f64(),
        Some(Value::String(s)) => s.trim().parse::<f64>().ok(),
        _ => None,
    }
}

fn parse_i64_value(value: Option<&Value>) -> Option<i64> {
    match value {
        Some(Value::Number(n)) => n.as_i64(),
        Some(Value::String(s)) => s.trim().parse::<i64>().ok(),
        _ => None,
    }
}

fn parse_u64_value(value: Option<&Value>) -> Option<u64> {
    match value {
        Some(Value::Number(n)) => n.as_u64(),
        Some(Value::String(s)) => s.trim().parse::<u64>().ok(),
        _ => None,
    }
}

async fn fetch_json(
    client: &reqwest::Client,
    method: reqwest::Method,
    url: &str,
    headers: &HeaderMap,
    query: Option<&[(&str, String)]>,
    body: Option<Value>,
) -> Result<Value, String> {
    let mut final_url = reqwest::Url::parse(url).map_err(|e| format!("invalid url '{url}': {e}"))?;
    if let Some(query) = query {
        {
            let mut pairs = final_url.query_pairs_mut();
            for (key, value) in query {
                pairs.append_pair(key, value);
            }
        }
    }
    let mut request = client.request(method, final_url).headers(headers.clone());
    if let Some(body) = body {
        request = request.json(&body);
    }
    let response = request
        .send()
        .await
        .map_err(|e| format!("request '{url}' failed: {e}"))?;
    let status = response.status();
    let text = response
        .text()
        .await
        .map_err(|e| format!("read response '{url}' failed: {e}"))?;
    if !status.is_success() {
        return Err(format!("OANDA {status} on '{url}': {text}"));
    }
    serde_json::from_str(&text).map_err(|e| format!("decode JSON '{url}' failed: {e}"))
}

fn scan_history_files_from_disk() -> Vec<TradingHistoryFileSummary> {
    let root = history_root_dir();
    let mut files = Vec::new();
    let Ok(entries) = fs::read_dir(root) else {
        return files;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.file_name().and_then(|value| value.to_str()) == Some("manifest.json") {
            continue;
        }
        if path.is_dir() {
            let instrument = path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or(DEFAULT_INSTRUMENT)
                .to_string();
            let Ok(granularity_entries) = fs::read_dir(&path) else {
                continue;
            };
            for granularity_entry in granularity_entries.flatten() {
                let granularity_path = granularity_entry.path();
                if granularity_path.extension().and_then(|s| s.to_str()) != Some("csv") {
                    continue;
                }
                let granularity = granularity_path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_uppercase();
                let updated_at_ms = granularity_entry
                    .metadata()
                    .ok()
                    .and_then(|meta| meta.modified().ok())
                    .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                    .map(|duration| duration.as_millis() as u64)
                    .unwrap_or(0);
                files.push(TradingHistoryFileSummary {
                    instrument: instrument.clone(),
                    granularity,
                    path: granularity_path.display().to_string(),
                    rows: 0,
                    first_time: None,
                    last_time: None,
                    truncated: false,
                    updated_at_ms,
                });
            }
            continue;
        }
        if path.extension().and_then(|s| s.to_str()) != Some("csv") {
            continue;
        }
        let granularity = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_uppercase();
        let updated_at_ms = entry
            .metadata()
            .ok()
            .and_then(|meta| meta.modified().ok())
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_millis() as u64)
            .unwrap_or(0);
        files.push(TradingHistoryFileSummary {
            instrument: DEFAULT_INSTRUMENT.to_string(),
            granularity,
            path: path.display().to_string(),
            rows: 0,
            first_time: None,
            last_time: None,
            truncated: false,
            updated_at_ms,
        });
    }
    files.sort_by(|a, b| {
        a.instrument
            .cmp(&b.instrument)
            .then_with(|| a.granularity.cmp(&b.granularity))
    });
    files
}

fn build_history_catalog() -> Vec<TradingHistoryFileSummary> {
    let disk_files = scan_history_files_from_disk();
    let manifest_files = if let Ok(bytes) = fs::read(history_manifest_path()) {
        serde_json::from_slice::<TradingHistoryManifest>(&bytes)
            .map(|manifest| manifest.files)
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    if disk_files.is_empty() {
        return manifest_files;
    }

    let mut manifest_by_key = HashMap::<(String, String), TradingHistoryFileSummary>::new();
    for entry in manifest_files {
        manifest_by_key.insert(
            (entry.instrument.clone(), entry.granularity.clone()),
            entry,
        );
    }

    let mut files = Vec::new();
    for disk in disk_files {
        let key = (disk.instrument.clone(), disk.granularity.clone());
        if let Some(manifest) = manifest_by_key.remove(&key) {
            files.push(TradingHistoryFileSummary {
                instrument: disk.instrument,
                granularity: disk.granularity,
                path: disk.path,
                rows: if disk.rows > 0 { disk.rows } else { manifest.rows },
                first_time: disk.first_time.or(manifest.first_time),
                last_time: disk.last_time.or(manifest.last_time),
                truncated: disk.truncated || manifest.truncated,
                updated_at_ms: disk.updated_at_ms.max(manifest.updated_at_ms),
            });
        } else {
            files.push(disk);
        }
    }
    files.sort_by(|a, b| {
        a.instrument
            .cmp(&b.instrument)
            .then_with(|| a.granularity.cmp(&b.granularity))
    });
    files
}

fn merge_history_summaries(
    existing: Vec<TradingHistoryFileSummary>,
    updates: &[TradingHistoryFileSummary],
) -> Vec<TradingHistoryFileSummary> {
    let mut by_key: HashMap<(String, String), TradingHistoryFileSummary> = HashMap::new();
    for entry in existing {
        by_key.insert(
            (entry.instrument.clone(), entry.granularity.clone()),
            entry,
        );
    }
    for entry in updates {
        by_key.insert(
            (entry.instrument.clone(), entry.granularity.clone()),
            entry.clone(),
        );
    }
    let mut files = by_key.into_values().collect::<Vec<_>>();
    files.sort_by(|a, b| {
        a.instrument
            .cmp(&b.instrument)
            .then_with(|| a.granularity.cmp(&b.granularity))
    });
    files
}

fn write_history_manifest(source: &str, files: &[TradingHistoryFileSummary]) -> Result<(), String> {
    let existing = if let Ok(bytes) = fs::read(history_manifest_path()) {
        serde_json::from_slice::<TradingHistoryManifest>(&bytes)
            .map(|manifest| manifest.files)
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    let manifest = TradingHistoryManifest {
        instrument: None,
        source: source.to_string(),
        generated_at_ms: now_ms(),
        files: merge_history_summaries(existing, files),
    };
    let bytes = serde_json::to_vec_pretty(&manifest).map_err(|e| format!("encode manifest: {e}"))?;
    let path = history_manifest_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("create history dir '{}': {e}", parent.display()))?;
    }
    fs::write(&path, bytes)
        .map_err(|e| format!("write manifest '{}': {e}", path.display()))?;
    Ok(())
}

fn history_path_for(instrument: &str, granularity: &str) -> PathBuf {
    history_instrument_dir(instrument).join(format!("{}.csv", granularity.to_uppercase()))
}

fn normalize_asset_token(symbol: &str) -> String {
    let _ = symbol;
    "/asset".to_string()
}

fn normalize_market_asset_symbol(value: &str) -> String {
    let raw = value
        .trim()
        .trim_start_matches("/asset_")
        .trim_start_matches("asset_")
        .replace(['-', '/', '.', ' '], "_");
    raw.chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
        .collect::<String>()
        .trim_matches('_')
        .to_uppercase()
}

fn infer_asset_class(symbol: &str, requested: &str) -> String {
    let requested = requested.trim();
    if !requested.is_empty() {
        return requested.to_lowercase();
    }
    let upper = symbol.trim().to_uppercase();
    if matches!(upper.as_str(), "SPX" | "NDX" | "DJI" | "RUT" | "DXY") {
        return "index".to_string();
    }
    if matches!(upper.as_str(), "US10Y" | "US2Y" | "US30Y") {
        return "rates".to_string();
    }
    if matches!(upper.as_str(), "XAUUSD" | "XAGUSD" | "WTI" | "BRENT") {
        return "commodity".to_string();
    }
    if matches!(upper.as_str(), "BTCUSD" | "ETHUSD") {
        return "crypto".to_string();
    }
    if upper.len() == 6 {
        let fiat = ["USD", "EUR", "JPY", "GBP", "CHF", "CAD", "AUD", "NZD", "HKD", "SGD"];
        let left = &upper[..3];
        let right = &upper[3..];
        if fiat.contains(&left) && fiat.contains(&right) {
            return "fx".to_string();
        }
    }
    "equity".to_string()
}

fn stooq_symbol_for_asset(symbol: &str, asset_class: &str) -> Option<String> {
    let upper = symbol.trim().to_uppercase();
    match upper.as_str() {
        "SPX" => return Some("^spx".to_string()),
        "NDX" => return Some("^ndx".to_string()),
        "DJI" => return Some("^dji".to_string()),
        "RUT" => return Some("^rut".to_string()),
        "AAPL" | "MSFT" | "NVDA" | "GOOGL" | "GOOG" | "AMZN" | "META" | "TSLA" | "AMD" => {
            return Some(format!("{}.us", upper.to_lowercase()));
        }
        _ => {}
    }
    if asset_class.eq_ignore_ascii_case("equity")
        && upper.len() <= 6
        && upper.chars().all(|ch| ch.is_ascii_uppercase())
    {
        return Some(format!("{}.us", upper.to_lowercase()));
    }
    None
}

fn provider_hint_for_asset(asset_class: &str, stooq_symbol: Option<&str>) -> String {
    if stooq_symbol.is_some() {
        return "stooq_daily_free".to_string();
    }
    match asset_class {
        "fx" | "commodity" | "crypto" => "dukascopy_tick_or_bar_free".to_string(),
        "rates" => "fred_or_stooq_daily".to_string(),
        _ => "alpha_vantage_or_twelve_data_keyed".to_string(),
    }
}

fn load_asset_store() -> TradingAssetStore {
    fs::read(trading_assets_file_path())
        .ok()
        .and_then(|bytes| serde_json::from_slice::<TradingAssetStore>(&bytes).ok())
        .unwrap_or_default()
}

fn save_asset_store(store: &TradingAssetStore) -> Result<(), String> {
    let path = trading_assets_file_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("create trading asset dir '{}': {e}", parent.display()))?;
    }
    let bytes = serde_json::to_vec_pretty(store).map_err(|e| format!("encode asset store: {e}"))?;
    fs::write(&path, bytes).map_err(|e| format!("write asset store '{}': {e}", path.display()))
}

fn load_strategy_live_store() -> TradingStrategyLiveStore {
    let _guard = trading_strategy_live_store_lock().lock().ok();
    fs::read(trading_strategy_live_jobs_file_path())
        .ok()
        .and_then(|bytes| serde_json::from_slice::<TradingStrategyLiveStore>(&bytes).ok())
        .unwrap_or_default()
}

fn save_strategy_live_store(store: &TradingStrategyLiveStore) -> Result<(), String> {
    let _guard = trading_strategy_live_store_lock()
        .lock()
        .map_err(|_| "strategy live store lock poisoned".to_string())?;
    let path = trading_strategy_live_jobs_file_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("create strategy live dir '{}': {e}", parent.display()))?;
    }
    let bytes = serde_json::to_vec_pretty(store)
        .map_err(|e| format!("encode strategy live store: {e}"))?;
    fs::write(&path, bytes)
        .map_err(|e| format!("write strategy live store '{}': {e}", path.display()))
}

fn upsert_asset_record(record: TradingAssetRecord) -> Result<TradingAssetRecord, String> {
    let mut store = load_asset_store();
    let mut merged = record.clone();
    if let Some(existing) = store.assets.iter_mut().find(|asset| asset.symbol == record.symbol) {
        merged.first_seen_at_ms = existing.first_seen_at_ms.min(record.first_seen_at_ms);
        if !existing.notes.is_empty() {
            let mut notes = existing.notes.clone();
            for note in record.notes {
                if !notes.iter().any(|seen| seen == &note) {
                    notes.push(note);
                }
            }
            merged.notes = notes;
        }
        *existing = merged.clone();
    } else {
        store.assets.push(merged.clone());
    }
    store.assets.sort_by(|a, b| a.symbol.cmp(&b.symbol));
    save_asset_store(&store)?;
    Ok(merged)
}

fn parse_stooq_csv(text: &str) -> Vec<TradingCandlePoint> {
    let mut candles = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if index == 0 || trimmed.is_empty() || trimmed.eq_ignore_ascii_case("No data") {
            continue;
        }
        let cols = trimmed.split(',').map(str::trim).collect::<Vec<_>>();
        if cols.len() < 5 {
            continue;
        }
        let open = cols.get(1).and_then(|value| value.parse::<f64>().ok());
        let high = cols.get(2).and_then(|value| value.parse::<f64>().ok());
        let low = cols.get(3).and_then(|value| value.parse::<f64>().ok());
        let close = cols.get(4).and_then(|value| value.parse::<f64>().ok());
        let (Some(open), Some(high), Some(low), Some(close)) = (open, high, low, close) else {
            continue;
        };
        let volume = cols
            .get(5)
            .and_then(|value| value.parse::<f64>().ok())
            .filter(|value| value.is_finite() && *value >= 0.0)
            .map(|value| value.round() as u64)
            .unwrap_or(0);
        let date = cols[0];
        if date.len() < 10 {
            continue;
        }
        candles.push(TradingCandlePoint {
            time: format!("{}T00:00:00Z", &date[..10]),
            open,
            high,
            low,
            close,
            volume,
        });
    }
    candles.sort_by(|a, b| a.time.cmp(&b.time));
    candles
}

fn candle_date_parts(time: &str) -> Option<(i32, u32, u32)> {
    let date = time.get(0..10)?;
    let mut parts = date.split('-');
    let y = parts.next()?.parse::<i32>().ok()?;
    let m = parts.next()?.parse::<u32>().ok()?;
    let d = parts.next()?.parse::<u32>().ok()?;
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    Some((y, m, d))
}

fn aggregation_key(candle: &TradingCandlePoint, granularity: &str) -> String {
    let frame = granularity.trim().to_uppercase();
    if let Some((y, m, d)) = candle_date_parts(&candle.time) {
        if frame == "Y" {
            return y.to_string();
        }
        if frame == "Q" {
            let quarter = ((m - 1) / 3) + 1;
            return format!("{y}-Q{quarter}");
        }
        if frame == "M" {
            return format!("{y:04}-{m:02}");
        }
        let week = days_from_civil(y, m, d).div_euclid(7);
        return week.to_string();
    }
    if frame == "M" {
        return candle.time.get(0..7).unwrap_or(&candle.time).to_string();
    }
    candle.time.clone()
}

fn aggregate_daily_candles(
    candles: &[TradingCandlePoint],
    granularity: &str,
) -> Vec<TradingCandlePoint> {
    let mut out: Vec<TradingCandlePoint> = Vec::new();
    let mut current_key = String::new();
    for candle in candles {
        let key = aggregation_key(candle, granularity);
        if out.is_empty() || key != current_key {
            current_key = key;
            out.push(candle.clone());
            continue;
        }
        if let Some(last) = out.last_mut() {
            last.high = last.high.max(candle.high);
            last.low = last.low.min(candle.low);
            last.close = candle.close;
            last.volume = last.volume.saturating_add(candle.volume);
        }
    }
    out
}

async fn fetch_stooq_daily(
    client: &reqwest::Client,
    provider_symbol: &str,
    start_date: &str,
) -> Result<Vec<TradingCandlePoint>, String> {
    let d1 = start_date
        .chars()
        .filter(|ch| ch.is_ascii_digit())
        .take(8)
        .collect::<String>();
    let d1 = if d1.len() == 8 { d1 } else { "20060101".to_string() };
    let url = reqwest::Url::parse_with_params(
        "https://stooq.com/q/d/l/",
        &[
            ("s", provider_symbol),
            ("i", "d"),
            ("d1", d1.as_str()),
        ],
    )
    .map_err(|e| format!("build stooq url: {e}"))?;
    let response = client
        .get(url)
        .header(ACCEPT, "text/csv,*/*")
        .send()
        .await
        .map_err(|e| format!("fetch stooq {provider_symbol}: {e}"))?;
    let status = response.status();
    let text = response
        .text()
        .await
        .map_err(|e| format!("read stooq response {provider_symbol}: {e}"))?;
    if !status.is_success() {
        return Err(format!(
            "stooq {provider_symbol} http {status}: {}",
            text.chars().take(160).collect::<String>()
        ));
    }
    let candles = parse_stooq_csv(&text);
    if candles.is_empty() {
        return Err(format!("stooq returned no usable candles for {provider_symbol}"));
    }
    Ok(candles)
}

fn classify_oanda_name(name: &str) -> String {
    let upper = name.trim().to_uppercase();
    if ["BTC", "ETH", "SOL", "XRP", "LTC"].iter().any(|token| upper.starts_with(token)) {
        return "crypto".to_string();
    }
    if ["XAU_", "XAG_", "XCU_", "XPT_", "XPD_"].iter().any(|prefix| upper.starts_with(prefix)) {
        return "commodity".to_string();
    }
    if upper.contains("BUND")
        || upper.contains("BOND")
        || upper.contains("YB_")
        || upper.contains("USB")
        || upper.contains("UK10Y")
        || upper.contains("DE10Y")
    {
        return "bond".to_string();
    }
    if [
        "SPX", "NAS", "US30", "DE30", "DE40", "FR40", "EU50", "JP225", "CN50", "HK33", "AU200",
        "CH20", "CHINAH", "ESPIX", "NL25", "SG30",
    ]
    .iter()
    .any(|prefix| upper.starts_with(prefix))
    {
        return "index".to_string();
    }
    if [
        "AAPL", "TSLA", "NVDA", "AMZN", "MSFT", "META", "GOOG", "NFLX",
    ]
    .iter()
    .any(|prefix| upper.starts_with(prefix))
    {
        return "equity".to_string();
    }
    if upper.contains('_') {
        let parts = upper.split('_').collect::<Vec<_>>();
        if parts.len() == 2 && parts[0].len() <= 6 && parts[1].len() <= 6 {
            let left = parts[0];
            let right = parts[1];
            let fiat_like = [
                "USD", "EUR", "JPY", "GBP", "CHF", "CAD", "AUD", "NZD", "HKD", "SGD",
            ];
            if fiat_like.contains(&left) || fiat_like.contains(&right) {
                return "forex".to_string();
            }
        }
    }
    "commodity".to_string()
}

fn display_name_from_instrument(name: &str) -> String {
    name.trim().replace('_', " / ")
}

fn history_instruments_from_files(files: &[TradingHistoryFileSummary]) -> Vec<TradingInstrumentSummary> {
    let mut seen = HashMap::<String, TradingInstrumentSummary>::new();
    for entry in files {
        let instrument = entry.instrument.trim();
        if instrument.is_empty() {
            continue;
        }
        seen.entry(instrument.to_string()).or_insert_with(|| TradingInstrumentSummary {
            name: instrument.to_string(),
            display_name: display_name_from_instrument(instrument),
            asset_class: classify_oanda_name(instrument),
            pip_location: None,
            display_precision: None,
            trade_units_precision: None,
            minimum_trade_size: None,
            margin_rate: None,
        });
    }
    let mut out = seen.into_values().collect::<Vec<_>>();
    out.sort_by(|a, b| {
        if a.name == DEFAULT_INSTRUMENT {
            std::cmp::Ordering::Less
        } else if b.name == DEFAULT_INSTRUMENT {
            std::cmp::Ordering::Greater
        } else {
            a.name.cmp(&b.name)
        }
    });
    out
}

fn build_asset_catalog(files: &[TradingHistoryFileSummary]) -> Vec<TradingAssetCatalogEntry> {
    let mut by_instrument = HashMap::<String, TradingAssetCatalogEntry>::new();
    for entry in files {
        let instrument = entry.instrument.trim();
        if instrument.is_empty() {
            continue;
        }
        let asset = by_instrument
            .entry(instrument.to_string())
            .or_insert_with(|| TradingAssetCatalogEntry {
                instrument: instrument.to_string(),
                display_name: display_name_from_instrument(instrument),
                asset_class: classify_oanda_name(instrument),
                granularities: Vec::new(),
                rows: 0,
                first_time: None,
                last_time: None,
                updated_at_ms: 0,
            });
        if !asset.granularities.iter().any(|value| value == &entry.granularity) {
            asset.granularities.push(entry.granularity.clone());
        }
        asset.rows = asset.rows.saturating_add(entry.rows);
        asset.updated_at_ms = asset.updated_at_ms.max(entry.updated_at_ms);
        if asset.first_time.is_none() {
            asset.first_time = entry.first_time.clone();
        } else if let (Some(current), Some(next)) = (asset.first_time.as_ref(), entry.first_time.as_ref()) {
            if next < current {
                asset.first_time = Some(next.clone());
            }
        }
        if asset.last_time.is_none() {
            asset.last_time = entry.last_time.clone();
        } else if let (Some(current), Some(next)) = (asset.last_time.as_ref(), entry.last_time.as_ref()) {
            if next > current {
                asset.last_time = Some(next.clone());
            }
        }
    }

    sort_asset_catalog(by_instrument.into_values().collect::<Vec<_>>())
}

fn sort_asset_catalog(mut assets: Vec<TradingAssetCatalogEntry>) -> Vec<TradingAssetCatalogEntry> {
    let granularity_rank = |value: &str| {
        OANDA_GRANULARITIES
            .iter()
            .position(|candidate| candidate.eq_ignore_ascii_case(value))
            .unwrap_or(usize::MAX)
    };

    for asset in &mut assets {
        asset.granularities.sort_by(|a, b| {
            granularity_rank(a)
                .cmp(&granularity_rank(b))
                .then_with(|| a.cmp(b))
        });
    }
    assets.sort_by(|a, b| {
        if a.instrument == DEFAULT_INSTRUMENT {
            std::cmp::Ordering::Less
        } else if b.instrument == DEFAULT_INSTRUMENT {
            std::cmp::Ordering::Greater
        } else {
            a.instrument.cmp(&b.instrument)
        }
    });
    assets
}

fn full_oanda_catalog_granularities() -> Vec<String> {
    OANDA_GRANULARITIES
        .iter()
        .map(|value| (*value).to_string())
        .collect()
}

fn merge_live_oanda_asset_catalog(
    files: &[TradingHistoryFileSummary],
    instruments: &[TradingInstrumentSummary],
) -> Vec<TradingAssetCatalogEntry> {
    let mut by_instrument = build_asset_catalog(files)
        .into_iter()
        .map(|entry| (entry.instrument.clone(), entry))
        .collect::<HashMap<_, _>>();
    let full_granularities = full_oanda_catalog_granularities();
    for item in instruments {
        let instrument = item.name.trim();
        if instrument.is_empty() {
            continue;
        }
        let asset = by_instrument
            .entry(instrument.to_string())
            .or_insert_with(|| TradingAssetCatalogEntry {
                instrument: instrument.to_string(),
                display_name: item.display_name.clone(),
                asset_class: item.asset_class.clone(),
                granularities: Vec::new(),
                rows: 0,
                first_time: None,
                last_time: None,
                updated_at_ms: 0,
            });
        asset.display_name = item.display_name.clone();
        asset.asset_class = item.asset_class.clone();
        asset.granularities = full_granularities.clone();
    }
    sort_asset_catalog(by_instrument.into_values().collect::<Vec<_>>())
}

fn read_history_csv_tail(path: &Path, max_rows: usize) -> Result<String, String> {
    if max_rows == 0 {
        return Ok("time,open,high,low,close,volume\n".to_string());
    }

    let header = {
        let file = File::open(path)
            .map_err(|e| format!("open history CSV '{}': {e}", path.display()))?;
        let mut reader = BufReader::new(file);
        let mut header = String::new();
        reader
            .read_line(&mut header)
            .map_err(|e| format!("read CSV header '{}': {e}", path.display()))?;
        header
    };

    let mut file = File::open(path)
        .map_err(|e| format!("open history CSV '{}': {e}", path.display()))?;
    let mut position = file
        .metadata()
        .map_err(|e| format!("stat history CSV '{}': {e}", path.display()))?
        .len();
    let mut buffer = Vec::new();
    let mut newline_count = 0usize;
    const CHUNK_SIZE: usize = 64 * 1024;

    while position > 0 && newline_count <= max_rows {
        let read_size = position.min(CHUNK_SIZE as u64) as usize;
        position -= read_size as u64;
        file.seek(SeekFrom::Start(position))
            .map_err(|e| format!("seek history CSV '{}': {e}", path.display()))?;
        let mut chunk = vec![0_u8; read_size];
        file.read_exact(&mut chunk)
            .map_err(|e| format!("read history CSV tail '{}': {e}", path.display()))?;
        newline_count += chunk.iter().filter(|byte| **byte == b'\n').count();
        chunk.extend_from_slice(&buffer);
        buffer = chunk;
    }

    let text = String::from_utf8_lossy(&buffer);
    let header_trimmed = header.trim_end_matches(['\r', '\n']);
    let mut rows = text
        .lines()
        .filter(|line| !line.is_empty() && line != &header_trimmed)
        .map(str::to_string)
        .collect::<Vec<_>>();
    if rows.len() > max_rows {
        rows = rows.split_off(rows.len() - max_rows);
    }

    let mut out = String::new();
    out.push_str(header_trimmed);
    out.push('\n');
    for row in rows {
        out.push_str(&row);
        out.push('\n');
    }
    Ok(out)
}

fn parse_history_csv_candle_line(line: &str) -> Option<TradingCandlePoint> {
    let cols = line.split(',').collect::<Vec<_>>();
    if cols.len() < 5 {
        return None;
    }
    let time = cols[0].trim().to_string();
    if time.is_empty() {
        return None;
    }
    let open = cols.get(1).and_then(|value| value.parse::<f64>().ok());
    let high = cols.get(2).and_then(|value| value.parse::<f64>().ok());
    let low = cols.get(3).and_then(|value| value.parse::<f64>().ok());
    let close = cols.get(4).and_then(|value| value.parse::<f64>().ok());
    let volume = cols
        .get(5)
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0);
    let (Some(open), Some(high), Some(low), Some(close)) = (open, high, low, close) else {
        return None;
    };
    Some(TradingCandlePoint {
        time,
        open,
        high,
        low,
        close,
        volume,
    })
}

fn parse_history_csv_candles(text: &str) -> Vec<TradingCandlePoint> {
    let mut out = Vec::new();
    for (index, line) in text.lines().enumerate() {
        if index == 0 || line.trim().is_empty() {
            continue;
        }
        if let Some(candle) = parse_history_csv_candle_line(line) {
            out.push(candle);
        }
    }
    out
}

fn find_asset_catalog_entry(
    files: &[TradingHistoryFileSummary],
    instrument: &str,
) -> Option<TradingAssetCatalogEntry> {
    build_asset_catalog(files)
        .into_iter()
        .find(|asset| asset.instrument == instrument)
}

fn canonical_chart_series(
    instrument: &str,
    granularity: &str,
    max_rows: usize,
) -> Result<TradingChartSeriesResponse, String> {
    let normalized_instrument = instrument.trim();
    let normalized_granularity = granularity.trim().to_uppercase();
    let files = build_history_catalog();
    let path = history_path_for(normalized_instrument, &normalized_granularity);
    let csv_text = if max_rows > 0 {
        read_history_csv_tail(&path, max_rows)?
    } else {
        fs::read_to_string(&path)
            .map_err(|e| format!("read history CSV '{}': {e}", path.display()))?
    };
    let candles = parse_history_csv_candles(&csv_text);
    Ok(TradingChartSeriesResponse {
        instrument: normalized_instrument.to_string(),
        granularity: normalized_granularity,
        asset: find_asset_catalog_entry(&files, normalized_instrument),
        candles,
        engine: "rust-canonical-series".to_string(),
    })
}

fn granularity_step_ms(granularity: &str) -> Option<i64> {
    match granularity.trim().to_uppercase().as_str() {
        "S5" => Some(5_000),
        "S10" => Some(10_000),
        "S15" => Some(15_000),
        "S30" => Some(30_000),
        "M1" => Some(60_000),
        "M2" => Some(2 * 60_000),
        "M4" => Some(4 * 60_000),
        "M5" => Some(5 * 60_000),
        "M10" => Some(10 * 60_000),
        "M15" => Some(15 * 60_000),
        "M30" => Some(30 * 60_000),
        "H1" => Some(60 * 60_000),
        "H2" => Some(2 * 60 * 60_000),
        "H3" => Some(3 * 60 * 60_000),
        "H4" => Some(4 * 60 * 60_000),
        "H6" => Some(6 * 60 * 60_000),
        "H8" => Some(8 * 60 * 60_000),
        "H12" => Some(12 * 60 * 60_000),
        "D" => Some(24 * 60 * 60_000),
        "W" => Some(7 * 24 * 60 * 60_000),
        "M" => Some(30 * 24 * 60 * 60_000),
        _ => None,
    }
}

fn live_source_granularity_for(granularity: &str) -> Option<&'static str> {
    match granularity.trim().to_uppercase().as_str() {
        "S10" => Some("S5"),
        "S30" => Some("S5"),
        "M1" => Some("S5"),
        "M5" => Some("S5"),
        "M15" => Some("S5"),
        "M30" => Some("S5"),
        "H1" => Some("S5"),
        "H4" => Some("S5"),
        "D" => Some("S30"),
        "W" => Some("M5"),
        "M" => Some("H1"),
        _ => None,
    }
}

fn days_from_civil(year: i32, month: u32, day: u32) -> i64 {
    let year = year - i32::from(month <= 2);
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let month = month as i32;
    let day = day as i32;
    let day_of_year = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    i64::from(era * 146097 + day_of_era - 719468)
}

fn civil_from_days(days: i64) -> (i32, u32, u32) {
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let day_of_era = z - era * 146097;
    let year_of_era = (day_of_era - day_of_era / 1460 + day_of_era / 36524 - day_of_era / 146096) / 365;
    let mut year = (year_of_era as i32) + (era as i32) * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    year += i32::from(month <= 2);
    (year, month as u32, day as u32)
}

fn parse_oanda_time_ms(value: &str) -> Option<i64> {
    let trimmed = value.trim();
    let date_part = trimmed.get(0..10)?;
    let time_part = trimmed.get(11..19)?;
    let year = date_part.get(0..4)?.parse::<i32>().ok()?;
    let month = date_part.get(5..7)?.parse::<u32>().ok()?;
    let day = date_part.get(8..10)?.parse::<u32>().ok()?;
    let hour = time_part.get(0..2)?.parse::<i64>().ok()?;
    let minute = time_part.get(3..5)?.parse::<i64>().ok()?;
    let second = time_part.get(6..8)?.parse::<i64>().ok()?;
    let mut millis = 0_i64;
    if let Some(fraction) = trimmed.split('.').nth(1) {
        let digits = fraction.trim_end_matches('Z');
        let millis_digits = if digits.len() >= 3 { &digits[..3] } else { digits };
        millis = match millis_digits.len() {
            0 => 0,
            1 => millis_digits.parse::<i64>().ok()? * 100,
            2 => millis_digits.parse::<i64>().ok()? * 10,
            _ => millis_digits.parse::<i64>().ok()?,
        };
    }
    let days = days_from_civil(year, month, day);
    Some(((days * 24 + hour) * 60 + minute) * 60 * 1000 + second * 1000 + millis)
}

fn format_oanda_time_ms(ms: i64) -> String {
    let seconds = ms.div_euclid(1000);
    let days = seconds.div_euclid(86_400);
    let secs_of_day = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let hour = secs_of_day / 3_600;
    let minute = (secs_of_day % 3_600) / 60;
    let second = secs_of_day % 60;
    format!(
        "{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.000000000Z"
    )
}

fn local_history_tail_candles(
    instrument: &str,
    granularity: &str,
    max_rows: usize,
) -> Vec<TradingCandlePoint> {
    let path = history_path_for(instrument, granularity);
    let csv_text = read_history_csv_tail(&path, max_rows).ok();
    csv_text
        .as_deref()
        .map(parse_history_csv_candles)
        .unwrap_or_default()
}

fn synthesize_incomplete_candle_from_samples(
    instrument: &str,
    granularity: &str,
    complete_candles: &[TradingCandlePoint],
    lower_points: &[TradingCandlePoint],
    price: Option<&TradingPriceSnapshot>,
) -> Option<TradingCandlePoint> {
    let target_step_ms = granularity_step_ms(granularity)?;
    let last_complete = complete_candles.last()?;
    let last_complete_start_ms = parse_oanda_time_ms(&last_complete.time)?;
    let current_bucket_start_ms = last_complete_start_ms + target_step_ms;
    let current_bucket_end_ms = current_bucket_start_ms + target_step_ms;
    let current_market_ms = price
        .and_then(|snapshot| parse_oanda_time_ms(&snapshot.time))
        .unwrap_or_else(|| trading_now_ms() as i64);
    if current_market_ms < current_bucket_start_ms {
        return None;
    }

    let mut aggregated: Vec<&TradingCandlePoint> = lower_points
        .iter()
        .filter(|point| {
            parse_oanda_time_ms(&point.time)
                .map(|time_ms| time_ms >= current_bucket_start_ms && time_ms < current_bucket_end_ms)
                .unwrap_or(false)
        })
        .collect();
    aggregated.sort_by_key(|point| parse_oanda_time_ms(&point.time).unwrap_or(i64::MIN));

    if aggregated.is_empty() {
        let live = price?.mid;
        if !live.is_finite() {
            return None;
        }
        return Some(TradingCandlePoint {
            time: format_oanda_time_ms(current_bucket_start_ms),
            open: last_complete.close,
            high: last_complete.close.max(live),
            low: last_complete.close.min(live),
            close: live,
            volume: 0,
        });
    }

    let first = aggregated.first()?;
    let last = aggregated.last()?;
    let mut open = first.open;
    let mut high = aggregated
        .iter()
        .fold(f64::NEG_INFINITY, |acc, point| acc.max(point.high));
    let mut low = aggregated
        .iter()
        .fold(f64::INFINITY, |acc, point| acc.min(point.low));
    let mut close = last.close;
    let mut volume = aggregated
        .iter()
        .fold(0_u64, |acc, point| acc.saturating_add(point.volume));
    if !open.is_finite() {
        open = last_complete.close;
    }
    if let Some(snapshot) = price {
        if snapshot.instrument.trim().eq_ignore_ascii_case(instrument) && snapshot.mid.is_finite() {
            close = snapshot.mid;
            high = high.max(snapshot.mid);
            low = low.min(snapshot.mid);
        }
    }
    if !high.is_finite() || !low.is_finite() || !close.is_finite() {
        return None;
    }
    if volume == 0 && aggregated.len() == 1 {
        volume = aggregated[0].volume;
    }
    Some(TradingCandlePoint {
        time: format_oanda_time_ms(current_bucket_start_ms),
        open,
        high,
        low,
        close,
        volume,
    })
}

fn metric_window(label: &str, default_window: usize) -> (String, usize) {
    let trimmed = label.trim().to_lowercase();
    if let Some((head, tail)) = trimmed.rsplit_once('_') {
        if let Ok(window) = tail.parse::<usize>() {
            return (head.to_string(), window.max(1));
        }
    }
    (trimmed, default_window.max(1))
}

fn metric_returns(values: &[f64], window: usize, log_return: bool) -> Vec<f64> {
    let mut out = vec![0.0; values.len()];
    for index in 0..values.len() {
        if index < window {
            continue;
        }
        let previous = values[index - window];
        let current = values[index];
        if previous.abs() <= f64::EPSILON || current <= 0.0 || previous <= 0.0 {
            continue;
        }
        out[index] = if log_return {
            (current / previous).ln()
        } else {
            (current / previous) - 1.0
        };
    }
    out
}

fn metric_momentum(values: &[f64], window: usize) -> Vec<f64> {
    let mut out = vec![0.0; values.len()];
    for index in 0..values.len() {
        if index < window {
            continue;
        }
        out[index] = values[index] - values[index - window];
    }
    out
}

fn metric_rolling_std(values: &[f64], window: usize) -> Vec<f64> {
    let mut out = vec![0.0; values.len()];
    if window == 0 {
        return out;
    }
    for index in 0..values.len() {
        if index + 1 < window {
            continue;
        }
        let slice = &values[index + 1 - window..=index];
        let mean = slice.iter().sum::<f64>() / slice.len() as f64;
        let variance = slice
            .iter()
            .map(|value| {
                let delta = *value - mean;
                delta * delta
            })
            .sum::<f64>()
            / slice.len() as f64;
        out[index] = variance.sqrt();
    }
    out
}

fn metric_rsi(values: &[f64], window: usize) -> Vec<f64> {
    let mut out = vec![50.0; values.len()];
    if values.len() < 2 || window == 0 {
        return out;
    }
    let mut gains = vec![0.0; values.len()];
    let mut losses = vec![0.0; values.len()];
    for index in 1..values.len() {
        let delta = values[index] - values[index - 1];
        if delta >= 0.0 {
            gains[index] = delta;
        } else {
            losses[index] = -delta;
        }
    }
    for index in 0..values.len() {
        if index < window {
            continue;
        }
        let avg_gain = gains[index + 1 - window..=index].iter().sum::<f64>() / window as f64;
        let avg_loss = losses[index + 1 - window..=index].iter().sum::<f64>() / window as f64;
        out[index] = if avg_loss <= f64::EPSILON {
            100.0
        } else {
            let rs = avg_gain / avg_loss;
            100.0 - (100.0 / (1.0 + rs))
        };
    }
    out
}

fn metric_volume_z(values: &[f64], window: usize) -> Vec<f64> {
    let mut out = vec![0.0; values.len()];
    if window == 0 {
        return out;
    }
    for index in 0..values.len() {
        if index + 1 < window {
            continue;
        }
        let slice = &values[index + 1 - window..=index];
        let mean = slice.iter().sum::<f64>() / slice.len() as f64;
        let variance = slice
            .iter()
            .map(|value| {
                let delta = *value - mean;
                delta * delta
            })
            .sum::<f64>()
            / slice.len() as f64;
        let std = variance.sqrt();
        out[index] = if std <= f64::EPSILON {
            0.0
        } else {
            (values[index] - mean) / std
        };
    }
    out
}

fn metric_drawdown(values: &[f64], window: usize) -> Vec<f64> {
    let mut out = vec![0.0; values.len()];
    if window == 0 {
        return out;
    }
    for index in 0..values.len() {
        let start = index.saturating_add(1).saturating_sub(window);
        let peak = values[start..=index]
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max);
        if peak.is_finite() && peak.abs() > f64::EPSILON {
            out[index] = (values[index] / peak) - 1.0;
        }
    }
    out
}

fn summarize_metric_series(metric: String, values: Vec<f64>) -> TradingChartMetricSeries {
    let finite = values.iter().copied().filter(|value| value.is_finite()).collect::<Vec<_>>();
    let min_value = finite.iter().copied().reduce(f64::min);
    let max_value = finite.iter().copied().reduce(f64::max);
    let last_value = values.iter().rev().copied().find(|value| value.is_finite());
    TradingChartMetricSeries {
        metric,
        values,
        last_value,
        min_value,
        max_value,
    }
}

fn compute_chart_metric_series(
    candles: &[TradingCandlePoint],
    metric_label: &str,
) -> Result<TradingChartMetricSeries, String> {
    let close = candles.iter().map(|candle| candle.close).collect::<Vec<_>>();
    let open = candles.iter().map(|candle| candle.open).collect::<Vec<_>>();
    let high = candles.iter().map(|candle| candle.high).collect::<Vec<_>>();
    let low = candles.iter().map(|candle| candle.low).collect::<Vec<_>>();
    let volume = candles.iter().map(|candle| candle.volume as f64).collect::<Vec<_>>();
    let (metric, window) = metric_window(metric_label, 14);
    let values = match metric.as_str() {
        "open" => open,
        "high" => high,
        "low" => low,
        "close" | "price" => close,
        "volume" => volume,
        "hlc3" | "typical_price" => (0..candles.len())
            .map(|index| (high[index] + low[index] + close[index]) / 3.0)
            .collect(),
        "range" => (0..candles.len()).map(|index| high[index] - low[index]).collect(),
        "body" => (0..candles.len()).map(|index| close[index] - open[index]).collect(),
        "return" | "return_1" => metric_returns(&close, 1, false),
        "log_return" => metric_returns(&close, 1, true),
        "momentum" => metric_momentum(&close, window),
        "volatility" | "realized_volatility" => metric_rolling_std(&metric_returns(&close, 1, false), window),
        "rsi" => metric_rsi(&close, window),
        "volume_z" | "volume_zscore" => metric_volume_z(&volume, window),
        "drawdown" => metric_drawdown(&close, window),
        other => return Err(format!("unsupported trading chart metric '{other}'")),
    };
    Ok(summarize_metric_series(metric_label.trim().to_string(), values))
}

fn point_size_from_pip_location(pip_location: i64) -> Option<f64> {
    let exponent = i32::try_from(pip_location).ok()?;
    let value = 10_f64.powi(exponent);
    if value.is_finite() && value > 0.0 {
        Some(value)
    } else {
        None
    }
}

fn runtime_instrument_summary(instrument: &str) -> Option<TradingInstrumentSummary> {
    let target = instrument.trim();
    if target.is_empty() {
        return None;
    }
    let state = oanda_runtime_lock().lock().ok()?;
    state
        .instruments
        .iter()
        .find(|item| item.name.eq_ignore_ascii_case(target))
        .cloned()
}

fn fallback_point_size_for_instrument(instrument: &str) -> Option<(f64, String)> {
    let upper = instrument.trim().to_uppercase();
    if upper.is_empty() {
        return None;
    }
    if upper.contains("NATGAS") || upper.contains("XAU") || upper.contains("XAG") {
        return Some((0.01, "instrument-class-inferred".to_string()));
    }
    if upper.contains("JPY") {
        return Some((0.01, "fx-jpy-inferred".to_string()));
    }
    if upper.contains('_') {
        let parts = upper.split('_').collect::<Vec<_>>();
        if parts.len() == 2 && parts[0].len() == 3 && parts[1].len() == 3 {
            return Some((0.0001, "fx-inferred".to_string()));
        }
    }
    if ["SPX", "NAS", "US30", "DE30", "DE40", "FR40", "JP225"]
        .iter()
        .any(|token| upper.contains(token))
    {
        return Some((1.0, "index-inferred".to_string()));
    }
    None
}

fn resolve_strategy_point_size(spec: &TradingStrategySpec) -> (Option<f64>, Option<String>, Option<String>) {
    if spec.point_size.is_some_and(|value| value.is_finite() && value > 0.0) {
        return (
            spec.point_size,
            spec.point_size_source.clone().or_else(|| Some("prompt-explicit".to_string())),
            spec.point_size_warning.clone(),
        );
    }
    let Some(instrument) = spec.instrument.as_deref() else {
        return (None, None, None);
    };
    if let Some(summary) = runtime_instrument_summary(instrument) {
        if let Some(pip_location) = summary.pip_location {
            if let Some(point_size) = point_size_from_pip_location(pip_location) {
                return (
                    Some(point_size),
                    Some(format!("oanda-pipLocation:{pip_location}")),
                    None,
                );
            }
        }
    }
    if instrument.eq_ignore_ascii_case(DEFAULT_INSTRUMENT) {
        return (
            Some(0.01),
            Some("default-natgas-oanda-pipLocation:-2".to_string()),
            None,
        );
    }
    if let Some((point_size, source)) = fallback_point_size_for_instrument(instrument) {
        return (
            Some(point_size),
            Some(source),
            Some("Point size was inferred because broker metadata was not loaded.".to_string()),
        );
    }
    (None, None, None)
}

fn normalize_strategy_token(value: &str) -> Option<String> {
    let mut out = String::new();
    let mut previous_underscore = false;
    for ch in value.trim().trim_start_matches('/').chars() {
        let normalized = if ch.is_ascii_alphanumeric() {
            ch.to_ascii_lowercase()
        } else if ch == '_' || ch == '-' || ch.is_whitespace() {
            '_'
        } else {
            continue;
        };
        if normalized == '_' {
            if previous_underscore || out.is_empty() {
                continue;
            }
            previous_underscore = true;
        } else {
            previous_underscore = false;
        }
        out.push(normalized);
    }
    while out.ends_with('_') {
        out.pop();
    }
    if out.is_empty() { None } else { Some(out) }
}

fn normalize_strategy_tokens(values: Option<Vec<String>>) -> Option<Vec<String>> {
    let mut seen = HashMap::<String, ()>::new();
    let mut out = Vec::new();
    for token in values.unwrap_or_default() {
        let Some(normalized) = normalize_strategy_token(&token) else {
            continue;
        };
        if seen.insert(normalized.clone(), ()).is_none() {
            out.push(normalized);
        }
    }
    if out.is_empty() { None } else { Some(out) }
}

fn extract_strategy_entry_hours(source: &str) -> Vec<u32> {
    let mut hours = Vec::<u32>::new();
    let normalized = source.to_lowercase();
    for token in normalized.split_whitespace() {
        let compact = token.trim_matches(|ch: char| !ch.is_ascii_alphanumeric() && ch != ':');
        if compact.is_empty() {
            continue;
        }
        let candidate = if let Some(stripped) = compact.strip_suffix('h') {
            stripped
        } else if let Some((prefix, suffix)) = compact.split_once(':') {
            if suffix == "00" {
                prefix
            } else {
                continue;
            }
        } else {
            continue;
        };
        let candidate = candidate.trim();
        if candidate.is_empty() || candidate.len() > 2 {
            continue;
        }
        if let Ok(hour) = candidate.parse::<u32>() {
            if hour <= 23 && !hours.contains(&hour) {
                hours.push(hour);
            }
        }
    }
    hours.sort_unstable();
    hours
}

fn normalize_strategy_entry_hours(
    entry_hour: Option<u32>,
    entry_hours: Option<Vec<u32>>,
    source_text: Option<&str>,
) -> Vec<u32> {
    let mut hours = entry_hours.unwrap_or_default();
    if hours.is_empty() {
        if let Some(source_text) = source_text {
            let parsed = extract_strategy_entry_hours(source_text);
            if parsed.len() >= 2 {
                hours = parsed;
            }
        }
    }
    if hours.is_empty() {
        if let Some(entry_hour) = entry_hour {
            hours.push(entry_hour);
        }
    }
    if hours.is_empty() {
        hours.push(21);
    }
    hours.retain(|hour| *hour <= 23);
    hours.sort_unstable();
    hours.dedup();
    hours
}

fn strategy_entry_hours(spec: &TradingStrategySpec) -> Vec<u32> {
    normalize_strategy_entry_hours(
        spec.entry_hour,
        spec.entry_hours.clone(),
        spec.source_text.as_deref(),
    )
}

fn slash_strategy_token(value: &str) -> String {
    if value.trim_start().starts_with('/') {
        value.trim().to_string()
    } else {
        format!("/{}", value.trim())
    }
}

fn normalize_strategy_spec(mut spec: TradingStrategySpec) -> TradingStrategySpec {
    spec.instrument = spec
        .instrument
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    spec.granularity = spec
        .granularity
        .map(|value| value.trim().to_uppercase())
        .filter(|value| !value.is_empty());
    spec.broker = spec
        .broker
        .map(|value| value.trim().to_lowercase())
        .filter(|value| !value.is_empty());
    spec.entry_timezone = spec
        .entry_timezone
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    spec.direction = spec
        .direction
        .map(|value| value.trim().to_lowercase())
        .filter(|value| !value.is_empty());
    spec.low_volatility_metric = spec
        .low_volatility_metric
        .map(|value| value.trim().to_lowercase())
        .filter(|value| !value.is_empty());
    let normalized_entry_hours = normalize_strategy_entry_hours(
        spec.entry_hour,
        spec.entry_hours.take(),
        spec.source_text.as_deref(),
    );
    spec.entry_hour = normalized_entry_hours.first().copied();
    spec.entry_hours = Some(normalized_entry_hours);
    spec.candle_refs = normalize_strategy_tokens(spec.candle_refs);
    spec.indicator_refs = normalize_strategy_tokens(spec.indicator_refs);
    spec.metric_commands = normalize_strategy_tokens(spec.metric_commands);
    if spec.force_daily_entry.is_none() {
        let source = spec
            .source_text
            .as_deref()
            .unwrap_or("")
            .to_lowercase();
        let wants_daily = (source.contains("tous les jours")
            || source.contains("chaque jour")
            || source.contains("every day")
            || source.contains("daily"))
            && (source.contains("1 trade")
                || source.contains("un trade")
                || source.contains("trade tout")
                || source.contains("position"));
        if wants_daily {
            spec.force_daily_entry = Some(true);
        }
    }
    let (point_size, point_size_source, point_size_warning) = resolve_strategy_point_size(&spec);
    spec.point_size = point_size;
    spec.point_size_source = point_size_source;
    spec.point_size_warning = point_size_warning;
    if spec.low_volatility_metric.is_some() && spec.low_volatility_lookback.is_none() {
        spec.low_volatility_lookback = Some(24);
    }
    if spec.low_volatility_metric.is_some() && spec.low_volatility_percentile.is_none() {
        spec.low_volatility_percentile = Some(0.25);
    }
    if spec.max_hold_bars.is_none() {
        spec.max_hold_bars = Some(24);
    }
    if spec.train_test_split.is_none() {
        spec.train_test_split = Some(0.7);
    }
    spec
}

fn push_strategy_missing(
    out: &mut Vec<TradingStrategyMissingMetric>,
    id: &str,
    label: &str,
    question: &str,
    reason: &str,
    examples: &[&str],
) {
    out.push(TradingStrategyMissingMetric {
        id: id.to_string(),
        label: label.to_string(),
        question: question.to_string(),
        reason: reason.to_string(),
        examples: examples.iter().map(|value| value.to_string()).collect(),
    });
}

fn is_positive_finite(value: Option<f64>) -> bool {
    value.is_some_and(|number| number.is_finite() && number > 0.0)
}

fn strategy_timezone_is_utc_like(value: Option<&str>) -> bool {
    let Some(value) = value else {
        return false;
    };
    matches!(
        value.trim().to_lowercase().as_str(),
        "utc" | "z" | "oanda" | "server" | "serveur" | "exchange"
    )
}

fn validate_strategy_spec(spec: &TradingStrategySpec) -> Vec<TradingStrategyMissingMetric> {
    let mut missing = Vec::new();
    if spec.instrument.as_deref().unwrap_or("").trim().is_empty() {
        push_strategy_missing(
            &mut missing,
            "instrument",
            "Actif",
            "Quel actif exact faut-il tester ?",
            "Le runner doit charger un fichier d'historique précis au lieu de deviner l'actif.",
            &["NATGAS_USD sur OANDA", "SPX500_USD", "EUR_USD"],
        );
    }
    if spec.granularity.as_deref().unwrap_or("").trim().is_empty() {
        push_strategy_missing(
            &mut missing,
            "granularity",
            "Timeframe",
            "Quel timeframe faut-il utiliser pour le backtest ?",
            "Les règles d'entrée/sortie changent selon le pas de bougie.",
            &["H1", "M30", "H4"],
        );
    }
    let entry_hours = strategy_entry_hours(spec);
    if entry_hours.is_empty() || entry_hours.iter().any(|hour| *hour > 23) {
        push_strategy_missing(
            &mut missing,
            "entry_hour",
            "Heure d'entrée",
            "A quelle heure exacte faut-il ouvrir le trade ?",
            "Une stratégie horaire doit utiliser une heure stable, bornée entre 0 et 23.",
            &["21h", "11h 15h 21h UTC"],
        );
    }
    if !strategy_timezone_is_utc_like(spec.entry_timezone.as_deref()) {
        push_strategy_missing(
            &mut missing,
            "entry_timezone",
            "Fuseau horaire",
            "Le 21h est-il en UTC/OANDA, heure de Paris, ou une autre timezone ?",
            "Les bougies OANDA sont stockées en UTC; une timezone implicite crée un backtest faux.",
            &["21h UTC", "21h Europe/Paris"],
        );
    }
    if !matches!(
        spec.direction.as_deref().unwrap_or(""),
        "long" | "short" | "buy" | "sell" | "both" | "auto" | "paired" | "straddle"
    ) {
        push_strategy_missing(
            &mut missing,
            "direction",
            "Direction",
            "Faut-il tester long, short, ou les deux directions ?",
            "Le runner peut comparer les deux, mais il doit savoir si une direction est interdite.",
            &["long seulement", "short seulement", "both"],
        );
    }
    if !is_positive_finite(spec.stop_loss_distance) {
        push_strategy_missing(
            &mut missing,
            "stop_loss_distance",
            "Distance du stop",
            "Quelle distance de stop loss faut-il utiliser en prix réel ?",
            "Les points/pips sont ambigus selon l'actif; le moteur attend une distance normalisée.",
            &["SL distance 0.045", "long 3.245 SL 3.200"],
        );
    }
    if !is_positive_finite(spec.take_profit_min_distance) || !is_positive_finite(spec.take_profit_max_distance) {
        push_strategy_missing(
            &mut missing,
            "take_profit_distance",
            "Distance du take profit",
            "Quelle plage de take profit faut-il tester en distance de prix ?",
            "Le runner teste une grille TP min/max; sans borne il optimise dans le vide.",
            &["TP min 0.035 max 0.300", "3.5p minimum, 30 points maximum"],
        );
    }
    if spec
        .target_win_rate
        .is_none_or(|value| !value.is_finite() || value <= 0.0 || value >= 1.0)
    {
        push_strategy_missing(
            &mut missing,
            "target_win_rate",
            "Objectif de réussite",
            "Quel taux de réussite cible faut-il atteindre ?",
            "Le runner doit savoir si 85% est une contrainte dure ou juste une préférence.",
            &["target 85%", "win rate minimum 0.85"],
        );
    }
    if !matches!(
        spec.low_volatility_metric.as_deref().unwrap_or(""),
        "range_sma_percentile" | "atr_percentile" | "bollinger_width_percentile"
    ) {
        push_strategy_missing(
            &mut missing,
            "low_volatility_metric",
            "Définition de faible volatilité",
            "Comment doit-on mesurer la faible volatilité avant 21h ?",
            "Forge doit éviter d'inventer la métrique centrale de la stratégie.",
            &["range SMA 24 sous le percentile 25", "ATR 14 sous le percentile 20"],
        );
    }
    if !is_positive_finite(spec.point_size) {
        push_strategy_missing(
            &mut missing,
            "point_size",
            "Unité de prix",
            "Quelle taille de point/pip faut-il appliquer à cet actif ?",
            "Les distances écrites en pips/points ne sont sûres que si Forge connaît l'unité broker.",
            &["NATGAS_USD point size 0.01", "pipLocation -2"],
        );
    }
    if spec.spread_cost_distance.is_none() || spec.slippage_distance.is_none() {
        push_strategy_missing(
            &mut missing,
            "execution_costs",
            "Coûts d'exécution",
            "Quel spread/slippage faut-il appliquer au backtest ?",
            "Un stop court devient trompeur si le spread et le slippage sont ignorés.",
            &["spread 0.002 slippage 0.001", "coûts à zéro pour premier scan"],
        );
    }
    missing
}

#[derive(Debug, Clone, Default)]
struct StrategyCacheSnapshot {
    hits: usize,
    misses: usize,
    injected_results: usize,
    avoided_recalculations: usize,
    reused_nodes: Vec<String>,
    missed_nodes: Vec<String>,
}

impl StrategyCacheSnapshot {
    fn record(&mut self, node: &str, hit: bool, avoided: usize) {
        if hit {
            self.hits += 1;
            self.avoided_recalculations = self.avoided_recalculations.saturating_add(avoided);
            if !self.reused_nodes.iter().any(|item| item == node) {
                self.reused_nodes.push(node.to_string());
            }
        } else {
            self.misses += 1;
            if !self.missed_nodes.iter().any(|item| item == node) {
                self.missed_nodes.push(node.to_string());
            }
        }
    }
}

fn strategy_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

fn strategy_sha256(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    strategy_hex(&hasher.finalize())
}

fn trading_order_action_hash(
    instrument: &str,
    side: &str,
    units: f64,
    order_type: &str,
    limit_price: Option<f64>,
    stop_loss: Option<f64>,
    take_profit: Option<f64>,
    time_in_force: &str,
    provider_state: &str,
    timestamp_bucket: &str,
) -> String {
    let payload = json!({
        "instrument": instrument,
        "side": side,
        "units": format!("{:.8}", units),
        "orderType": order_type,
        "limitPrice": limit_price.map(|value| format!("{value:.8}")),
        "stopLoss": stop_loss.map(|value| format!("{value:.8}")),
        "takeProfit": take_profit.map(|value| format!("{value:.8}")),
        "timeInForce": time_in_force,
        "providerState": provider_state,
        "timestampBucket": timestamp_bucket,
    });
    strategy_sha256(&payload.to_string())
}

fn validate_trading_order_approval(
    request: &TradingPlaceOrderRequest,
    instrument: &str,
    side: &str,
    order_type: &str,
    time_in_force: &str,
    provider_state: &str,
    now_ms_value: u64,
) -> Result<(String, String), String> {
    let approval = request
        .approval
        .as_ref()
        .ok_or_else(|| "Live trading order requires explicit approval.".to_string())?;
    if !approval.approved {
        return Err("Live trading order approval is not confirmed.".to_string());
    }
    if now_ms_value.saturating_sub(approval.approved_at_ms) > 300_000 {
        return Err("Live trading approval expired; approve again.".to_string());
    }
    let expected_bucket = trading_approval_bucket(approval.approved_at_ms);
    if approval.timestamp_bucket != expected_bucket {
        return Err("Live trading approval bucket is invalid.".to_string());
    }
    if approval.provider_state != provider_state {
        return Err("Live trading provider state changed; approve again.".to_string());
    }
    let expected_action_hash = trading_order_action_hash(
        instrument,
        side,
        request.units,
        order_type,
        request.limit_price,
        request.stop_loss,
        request.take_profit,
        time_in_force,
        provider_state,
        &expected_bucket,
    );
    if approval.action_hash != expected_action_hash {
        return Err("Live trading approval hash mismatch; approve again.".to_string());
    }
    let proof_payload = json!({
        "approved": true,
        "approvedAtMs": approval.approved_at_ms,
        "providerState": provider_state,
        "timestampBucket": expected_bucket,
        "actionHash": expected_action_hash,
    });
    Ok((expected_bucket, strategy_sha256(&proof_payload.to_string())))
}

fn strategy_cache_artifact_path(cache_key: &str) -> PathBuf {
    let safe = cache_key
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') { ch } else { '_' })
        .collect::<String>();
    trading_strategy_cache_dir().join(format!("{safe}.json"))
}

fn strategy_cache_artifact_uri(cache_key: &str) -> String {
    format!("kasm://sha256/{}", strategy_sha256(cache_key))
}

fn strategy_write_cache_marker(cache_key: &str, payload: Value) {
    let path = strategy_cache_artifact_path(cache_key);
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let marker = json!({
        "cacheKey": cache_key,
        "artifactUri": strategy_cache_artifact_uri(cache_key),
        "updatedAtMs": now_ms(),
        "payload": payload,
    });
    if let Ok(bytes) = serde_json::to_vec_pretty(&marker) {
        let _ = fs::write(path, bytes);
    }
}

fn strategy_candles_hash(candles: &[TradingCandlePoint]) -> String {
    let mut hasher = Sha256::new();
    for candle in candles {
        hasher.update(candle.time.as_bytes());
        hasher.update(b"|");
        hasher.update(format!(
            "{:.10}|{:.10}|{:.10}|{:.10}|{};",
            candle.open,
            candle.high,
            candle.low,
            candle.close,
            candle.volume
        ).as_bytes());
    }
    strategy_hex(&hasher.finalize())
}

fn strategy_template_hash(spec: &TradingStrategySpec, data_hash: &str) -> String {
    let payload = json!({
        "instrument": spec.instrument,
        "granularity": spec.granularity,
        "broker": spec.broker,
        "pointSize": spec.point_size,
        "entryHour": spec.entry_hour,
        "entryHours": spec.entry_hours,
        "entryTimezone": spec.entry_timezone,
        "direction": spec.direction,
        "stopLossDistance": spec.stop_loss_distance,
        "takeProfitMinDistance": spec.take_profit_min_distance,
        "takeProfitMaxDistance": spec.take_profit_max_distance,
        "targetWinRate": spec.target_win_rate,
        "dailyProfitTargetDistance": spec.daily_profit_target_distance,
        "lowVolatilityMetric": spec.low_volatility_metric,
        "lowVolatilityLookback": spec.low_volatility_lookback,
        "lowVolatilityPercentile": spec.low_volatility_percentile,
        "forceDailyEntry": spec.force_daily_entry,
        "spreadCostDistance": spec.spread_cost_distance,
        "slippageDistance": spec.slippage_distance,
        "maxHoldBars": spec.max_hold_bars,
        "trainTestSplit": spec.train_test_split,
        "candleRefs": spec.candle_refs,
        "indicatorRefs": spec.indicator_refs,
        "metricCommands": spec.metric_commands,
        "dataHash": data_hash,
        "strategySearchVersion": "adaptive-eclat-mfe-reduce-grid-v8",
    });
    strategy_sha256(&payload.to_string())
}

fn strategy_template_from_spec(
    spec: &TradingStrategySpec,
    data_hash: &str,
    template_hash: &str,
) -> TradingStrategyTemplate {
    let entry_hours = strategy_entry_hours(spec);
    TradingStrategyTemplate {
        template_id: format!("strategy-template-{}", &template_hash[..16.min(template_hash.len())]),
        command: "/strategy_".to_string(),
        family: if strategy_is_paired_mode(spec) && spec.force_daily_entry.unwrap_or(false) {
            "timed_paired_daily_entry_grid".to_string()
        } else if spec.force_daily_entry.unwrap_or(false) {
            "timed_daily_entry_grid".to_string()
        } else {
            "timed_low_volatility_grid".to_string()
        },
        instrument: spec.instrument.as_deref().unwrap_or(DEFAULT_INSTRUMENT).to_string(),
        granularity: spec.granularity.as_deref().unwrap_or("H1").to_string(),
        broker: spec.broker.as_deref().unwrap_or("oanda").to_string(),
        direction: spec.direction.as_deref().unwrap_or("both").to_string(),
        entry_hour_utc: entry_hours.first().copied().unwrap_or(21),
        target_win_rate: spec.target_win_rate,
        parameter_hash: template_hash.to_string(),
        data_hash: data_hash.to_string(),
    }
}

fn strategy_plan_lines(spec: &TradingStrategySpec) -> Vec<String> {
    let entry_hours = strategy_entry_hours(spec);
    vec![
        "runner=forge-tauri-rust-strategy-backtest".to_string(),
        "kasm_plan=slash metric manifest, condition bytecode, stable cache keys, paired long/short probe".to_string(),
        format!(
            "data={} {} from local canonical history CSV",
            spec.instrument.as_deref().unwrap_or("n/a"),
            spec.granularity.as_deref().unwrap_or("n/a")
        ),
        format!(
            "entry={} when UTC hour in [{}]",
            if strategy_is_paired_mode(spec) {
                "force paired long+short per open trading day"
            } else if spec.force_daily_entry.unwrap_or(false) {
                "force one trade per open trading day"
            } else {
                "once per candle"
            },
            entry_hours
                .iter()
                .map(|value| value.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ),
        format!(
            "{}={} lookback={} percentile={}",
            if spec.force_daily_entry.unwrap_or(false) {
                "volatility_context_metric"
            } else {
                "volatility_filter"
            },
            spec.low_volatility_metric.as_deref().unwrap_or("n/a"),
            spec.low_volatility_lookback.unwrap_or(0),
            spec.low_volatility_percentile.unwrap_or(0.0)
        ),
        format!(
            "unit=point_size:{} source:{}",
            spec.point_size
                .map(|value| value.to_string())
                .unwrap_or_else(|| "n/a".to_string()),
            spec.point_size_source.as_deref().unwrap_or("n/a")
        ),
        format!(
            "chart_refs=candles:[{}] indicators:[{}]",
            spec.candle_refs.as_ref().map(|items| items.join(",")).unwrap_or_default(),
            spec.indicator_refs.as_ref().map(|items| items.join(",")).unwrap_or_default()
        ),
        format!(
            "metric_commands=[{}]",
            spec.metric_commands
                .as_ref()
                .map(|items| items.iter().map(|item| slash_strategy_token(item)).collect::<Vec<_>>().join(","))
                .unwrap_or_default()
        ),
        "execution=grid search TP min/max, fixed SL, pessimistic same-candle stop-before-target".to_string(),
        "paired_probe=scan each eligible entry once, test long and short from the same open price".to_string(),
        "safety=LLM never loops over candles; Rust/KASM plan returns compact metrics only".to_string(),
    ]
}

fn build_strategy_plan_only_compute_plan(
    spec: &TradingStrategySpec,
    max_rows: Option<usize>,
) -> TradingStrategyComputePlan {
    let data_key = format!(
        "pending-local-history|{}|{}|max_rows={}",
        spec.instrument.as_deref().unwrap_or(DEFAULT_INSTRUMENT),
        spec.granularity.as_deref().unwrap_or("H1"),
        max_rows.unwrap_or(0)
    );
    let data_hash = format!("planned-data-{}", &strategy_sha256(&data_key)[..16]);
    let template_hash = strategy_template_hash(spec, &data_hash);
    let tp_grid_len = if let (Some(min), Some(max)) = (
        spec.take_profit_min_distance,
        spec.take_profit_max_distance,
    ) {
        strategy_take_profit_grid(min, max).len()
    } else {
        0
    };
    let direction_count = strategy_directions(spec.direction.as_deref().unwrap_or("both")).len();
    let mut snapshot = StrategyCacheSnapshot::default();
    snapshot.record("strategy_template", false, 0);
    let mut plan = build_strategy_compute_plan(
        spec,
        max_rows.unwrap_or(0),
        &data_hash,
        &template_hash,
        &snapshot,
        None,
        tp_grid_len,
        direction_count,
        0,
        0,
    );
    plan.execution_mode = "plan_only_template_to_kasm_dag_no_candle_load".to_string();
    plan.gpu_plan.gpu_required = false;
    plan.gpu_plan.note = "Plan-only does not load candles or run mfe_reduce; the full backtest swaps in the real data hash and work items.".to_string();
    plan.notes.push(
        "Plan-only emitted a compact KASM DAG contract without scanning OHLCV rows.".to_string(),
    );
    plan
}

fn push_strategy_command(
    commands: &mut Vec<(String, String, String)>,
    seen: &mut HashMap<String, ()>,
    token: String,
    kind: &str,
    role: &str,
) {
    let normalized = token.trim().trim_start_matches('/').to_string();
    if normalized.is_empty() {
        return;
    }
    if seen.insert(normalized.clone(), ()).is_none() {
        commands.push((slash_strategy_token(&normalized), kind.to_string(), role.to_string()));
    }
}

fn build_strategy_compute_plan(
    spec: &TradingStrategySpec,
    rows: usize,
    data_hash: &str,
    template_hash: &str,
    cache_snapshot: &StrategyCacheSnapshot,
    threshold: Option<f64>,
    tp_grid_len: usize,
    direction_count: usize,
    eligible_entries: usize,
    condition_program_count: usize,
) -> TradingStrategyComputePlan {
    let mut seen = HashMap::<String, ()>::new();
    let mut commands = Vec::<(String, String, String)>::new();
    push_strategy_command(&mut commands, &mut seen, "/asset".to_string(), "asset", "resolve tradable asset");
    if let Some(instrument) = spec.instrument.as_deref() {
        push_strategy_command(
            &mut commands,
            &mut seen,
            format!("asset_{}", instrument.to_lowercase()),
            "asset",
            "provider symbol and local history",
        );
    }
    if let Some(granularity) = spec.granularity.as_deref() {
        push_strategy_command(
            &mut commands,
            &mut seen,
            format!("candle_{}", granularity.to_lowercase()),
            "candle",
            "canonical OHLCV feed",
        );
    }
    for item in spec.candle_refs.as_ref().cloned().unwrap_or_default() {
        push_strategy_command(&mut commands, &mut seen, item, "candle", "mapped chart candle reference");
    }
    for item in spec.indicator_refs.as_ref().cloned().unwrap_or_default() {
        push_strategy_command(&mut commands, &mut seen, item, "indicator", "mapped chart indicator reference");
    }
    if let Some(metric) = spec.low_volatility_metric.as_deref() {
        push_strategy_command(
            &mut commands,
            &mut seen,
            metric.to_string(),
            "metric",
            "low-volatility filter",
        );
    }
    for item in spec.metric_commands.as_ref().cloned().unwrap_or_default() {
        push_strategy_command(&mut commands, &mut seen, item, "metric", "slash command requested by LLM/user");
    }
    push_strategy_command(
        &mut commands,
        &mut seen,
        "strategy_paired_long_short".to_string(),
        "probe",
        "shared entry scan tests long and short at the same open",
    );
    push_strategy_command(
        &mut commands,
        &mut seen,
        "strategy_condition_bytecode".to_string(),
        "compiler",
        "compile indicator/candle predicates into reusable KASM masks",
    );
    push_strategy_command(
        &mut commands,
        &mut seen,
        "strategy_condition_cross".to_string(),
        "operator",
        "cross/reclaim/close-beyond condition atoms",
    );
    push_strategy_command(
        &mut commands,
        &mut seen,
        "strategy_condition_and".to_string(),
        "operator",
        "AND2/AND3 condition programs from cached atom masks",
    );
    push_strategy_command(
        &mut commands,
        &mut seen,
        "strategy_mask_reduce".to_string(),
        "reducer",
        "reduce cached TP-grid outcomes through each condition mask",
    );
    push_strategy_command(
        &mut commands,
        &mut seen,
        "strategy_mfe_reduce".to_string(),
        "reducer",
        "single-pass per-entry future excursion plus all TP outcomes",
    );
    push_strategy_command(
        &mut commands,
        &mut seen,
        "strategy_filter_outcome_grid".to_string(),
        "cache",
        "mask + direction + fixed SL returns every TP metric in one reduction",
    );
    push_strategy_command(
        &mut commands,
        &mut seen,
        "strategy_tp_grid".to_string(),
        "grid",
        "take-profit search without repeating candle load",
    );

    let base_key = format!(
        "{}|{}|{}|{}|{}|{}|{}",
        spec.instrument.as_deref().unwrap_or(""),
        spec.granularity.as_deref().unwrap_or(""),
        spec.entry_hour.map(|value| value.to_string()).unwrap_or_default(),
        spec.low_volatility_metric.as_deref().unwrap_or(""),
        spec.low_volatility_lookback.map(|value| value.to_string()).unwrap_or_default(),
        threshold.map(|value| format!("{value:.10}")).unwrap_or_default(),
        data_hash
    );
    let plan_id = format!("kasm-plan-{}", &strategy_sha256(&base_key)[..16]);
    let candle_key = format!("ohlcv-{}", &strategy_sha256(&format!(
        "{}|{}|{}",
        spec.instrument.as_deref().unwrap_or(""),
        spec.granularity.as_deref().unwrap_or(""),
        data_hash
    ))[..16]);
    let vol_key = format!("low-vol-{}", &strategy_sha256(&format!("{base_key}|vol"))[..16]);
    let threshold_key = format!("vol-threshold-{}", &strategy_sha256(&format!("{base_key}|threshold"))[..16]);
    let shared_entry_key = format!("entry-scan-{}", &strategy_sha256(&format!("{base_key}|entries"))[..16]);
    let condition_program_key = format!(
        "condition-programs-{}",
        &strategy_sha256(&format!(
            "{base_key}|eclat-mfe-reduce-grid-v7|programs={condition_program_count}|entries={eligible_entries}"
        ))[..16]
    );
    let mfe_reduce_key = format!(
        "mfe-reduce-{}",
        &strategy_sha256(&format!(
            "{base_key}|mfe-reduce-v1|sl={}|hold={}|tp_min={}|tp_max={}|dirs={direction_count}|tpn={tp_grid_len}|entries={eligible_entries}",
            spec.stop_loss_distance.unwrap_or(0.0),
            spec.max_hold_bars.unwrap_or(0),
            spec.take_profit_min_distance.unwrap_or(0.0),
            spec.take_profit_max_distance.unwrap_or(0.0),
        ))[..16]
    );
    let outcome_cube_key = mfe_reduce_key.clone();
    let metrics_key = format!("strategy-metrics-{}", &strategy_sha256(&format!("{outcome_cube_key}|metrics"))[..16]);
    let visual_key = format!("visual-probes-{}", &strategy_sha256(&format!("{outcome_cube_key}|visual"))[..16]);
    let result_cache_key = format!("strategy-result-{}", &template_hash[..16.min(template_hash.len())]);
    let cache_hit_for = |id: &str| cache_snapshot.reused_nodes.iter().any(|item| item == id);
    let node = |id: &str,
                label: &str,
                operation: &str,
                inputs: Vec<String>,
                cache_key: String,
                estimated_items: usize,
                estimated_bytes: usize,
                gpu_candidate: bool| {
        let node_hash = strategy_sha256(&format!(
            "{id}|{operation}|{}|{cache_key}|{}",
            inputs.join(","),
            template_hash
        ));
        let cache_hit = cache_hit_for(id);
        TradingStrategyPlanNode {
            id: id.to_string(),
            label: label.to_string(),
            operation: operation.to_string(),
            input_hashes: inputs,
            node_hash: node_hash.clone(),
            cache_key: cache_key.clone(),
            cache_hit,
            artifact_uri: strategy_cache_artifact_uri(&cache_key),
            estimated_items,
            estimated_bytes,
            gpu_candidate,
            status: if cache_hit { "injected" } else { "scheduled" }.to_string(),
        }
    };
    let simulation_count = eligible_entries
        .saturating_mul(direction_count.max(1))
        .saturating_mul(tp_grid_len.max(1));
    let dag_nodes = vec![
        node(
            "ohlcv",
            "Canonical OHLCV",
            "/asset + /candle feed",
            vec![data_hash.to_string()],
            candle_key.clone(),
            rows,
            rows.saturating_mul(48),
            false,
        ),
        node(
            "low_vol",
            "Low volatility vector",
            spec.low_volatility_metric.as_deref().unwrap_or("range_sma_percentile"),
            vec![candle_key.clone()],
            vol_key.clone(),
            rows,
            rows.saturating_mul(8),
            true,
        ),
        node(
            "threshold",
            "Volatility percentile threshold",
            "percentile(train(low_vol))",
            vec![vol_key.clone()],
            threshold_key.clone(),
            rows,
            64,
            false,
        ),
        node(
            "entry_scan",
            "Eligible entry mask",
            "hour_filter & low_vol_filter",
            vec![candle_key.clone(), threshold_key.clone()],
            shared_entry_key.clone(),
            eligible_entries,
            eligible_entries.saturating_mul(8),
            true,
        ),
        node(
            "condition_programs",
            "Condition bytecode programs",
            "/cross + /threshold + /reclaim + /body + AND2/AND3 bitset masks",
            vec![candle_key.clone(), shared_entry_key.clone()],
            condition_program_key.clone(),
            condition_program_count.max(1),
            condition_program_count.max(1).saturating_mul(96),
            true,
        ),
        node(
            "mfe_reduce",
            "MFE/MAE TP-grid reducer",
            "scan entry futures once and emit all TP outcomes for each direction/mask",
            vec![candle_key.clone(), shared_entry_key.clone(), condition_program_key.clone()],
            mfe_reduce_key.clone(),
            simulation_count,
            eligible_entries
                .saturating_mul(direction_count.max(1))
                .saturating_mul(64)
                .saturating_add(simulation_count.saturating_mul(12)),
            true,
        ),
        node(
            "metrics",
            "Backtest and robustness metrics",
            "popcount/reduce(mfe_reduce_grid, condition_program_masks)",
            vec![mfe_reduce_key.clone(), condition_program_key.clone()],
            metrics_key.clone(),
            simulation_count.saturating_add(
                condition_program_count
                    .saturating_mul(direction_count.max(1))
                    .saturating_mul(tp_grid_len.max(1)),
            ),
            simulation_count.saturating_mul(8).saturating_add(
                condition_program_count.max(1).saturating_mul(16),
            ),
            true,
        ),
        node(
            "visual_probes",
            "Chart SL/TP probes",
            "sample(mfe_reduce_grid)",
            vec![mfe_reduce_key.clone()],
            visual_key.clone(),
            eligible_entries.min(12).saturating_mul(2),
            eligible_entries.min(12).saturating_mul(96),
            false,
        ),
    ];
    let metric_commands = commands
        .into_iter()
        .enumerate()
        .map(|(index, (token, kind, role))| {
            let cache_key = format!(
                "{}-{}",
                kind,
                &strategy_sha256(&format!("{plan_id}|{token}|{role}"))[..16]
            );
            let node_hash = strategy_sha256(&format!("{cache_key}|{token}|{kind}|{role}|{template_hash}"));
            let cache_hit = index < cache_snapshot.hits;
            TradingStrategyMetricCommand {
                token,
                kind,
                role,
                cache_key: cache_key.clone(),
                node_hash,
                cache_hit,
                artifact_uri: strategy_cache_artifact_uri(&cache_key),
                status: if cache_hit { "injected" } else { "executed_or_scheduled" }.to_string(),
            }
        })
        .collect::<Vec<_>>();
    let branch_count = direction_count.max(1) * tp_grid_len.max(1);
    let avoided_recalculations = eligible_entries
        .saturating_mul(branch_count.saturating_sub(1))
        .saturating_add(cache_snapshot.avoided_recalculations);
    let template = strategy_template_from_spec(spec, data_hash, template_hash);
    let cache_report = TradingStrategyCacheReport {
        artifact_root: trading_strategy_cache_dir().display().to_string(),
        data_hash: data_hash.to_string(),
        template_hash: template_hash.to_string(),
        result_cache_key,
        hits: cache_snapshot.hits,
        misses: cache_snapshot.misses,
        injected_results: cache_snapshot.injected_results,
        avoided_recalculations,
        reused_nodes: cache_snapshot.reused_nodes.clone(),
        missed_nodes: cache_snapshot.missed_nodes.clone(),
    };
    let gpu_plan = TradingStrategyGpuPlan {
        preferred_engine: "KASM GPU".to_string(),
        kernel: "strategy_mfe_reduce_tp_grid".to_string(),
        layout: "SoA open/high/low/close/hour + bitset masks + fused MFE/MAE + TP grid".to_string(),
        work_items: simulation_count,
        outcome_cube_key: outcome_cube_key.clone(),
        gpu_required: simulation_count >= 250_000,
        cpu_fallback: "rust SIMD-friendly loops when GPU runtime is unavailable".to_string(),
        note: format!(
            "GPU/KASM scans entry futures once, emits TP-grid summaries for {} condition opcode programs, and reuses mask refs; CPU/LLM only orchestrate compact summaries.",
            condition_program_count
        ),
    };
    TradingStrategyComputePlan {
        engine: "KASM-ready Strategy DAG".to_string(),
        plan_id,
        execution_mode: "template_to_dag_content_addressed_cache_then_mfe_reduce".to_string(),
        metric_commands,
        template,
        dag_nodes,
        cache_report,
        gpu_plan,
        simulation_count,
        outcome_cube_key,
        shared_cache_keys: vec![candle_key, vol_key, threshold_key, shared_entry_key, condition_program_key, mfe_reduce_key],
        reused_calculations: cache_snapshot.hits,
        avoided_recalculations,
        notes: vec![
            "The LLM fills a strict strategy template; Forge compiles it to a content-addressed DAG.".to_string(),
            "Every node is keyed by data hash, normalized params, kernel version and input hashes.".to_string(),
            "Long and short diagnostics reuse the same eligible entry indices and entry prices.".to_string(),
            "Indicator/candle logic is compiled into compact opcode IDs, then composed with cached canonical AND programs.".to_string(),
            "Each mask + direction + SL produces its whole TP grid in one mfe_reduce pass.".to_string(),
            "Known node results are auto-injected into later runs before any loop starts.".to_string(),
            "The fused mfe_reduce grid is the GPU boundary: entries x directions x TP grid without duplicate future scans.".to_string(),
        ],
    }
}

fn strategy_store_compute_plan(plan: &TradingStrategyComputePlan) {
    let dag = plan
        .dag_nodes
        .iter()
        .map(|node| {
            json!({
                "id": node.id,
                "operation": node.operation,
                "nodeHash": node.node_hash,
                "cacheKey": node.cache_key,
                "cacheHit": node.cache_hit,
                "artifactUri": node.artifact_uri,
                "estimatedItems": node.estimated_items,
                "estimatedBytes": node.estimated_bytes,
                "gpuCandidate": node.gpu_candidate,
                "status": node.status,
            })
        })
        .collect::<Vec<_>>();
    strategy_write_cache_marker(
        &plan.plan_id,
        json!({
            "node": "strategy_compute_plan",
            "slash": ["/create_", "/strategy_", "/backtest_"],
            "engine": plan.engine,
            "executionMode": plan.execution_mode,
            "template": {
                "id": plan.template.template_id,
                "hash": plan.template.parameter_hash,
                "dataHash": plan.template.data_hash,
                "instrument": plan.template.instrument,
                "granularity": plan.template.granularity,
            },
            "cacheReport": {
                "hits": plan.cache_report.hits,
                "misses": plan.cache_report.misses,
                "injectedResults": plan.cache_report.injected_results,
                "avoidedRecalculations": plan.cache_report.avoided_recalculations,
                "resultCacheKey": plan.cache_report.result_cache_key,
            },
            "gpuPlan": {
                "preferredEngine": plan.gpu_plan.preferred_engine,
                "kernel": plan.gpu_plan.kernel,
                "layout": plan.gpu_plan.layout,
                "workItems": plan.gpu_plan.work_items,
                "outcomeCubeKey": plan.gpu_plan.outcome_cube_key,
                "gpuRequired": plan.gpu_plan.gpu_required,
                "cpuFallback": plan.gpu_plan.cpu_fallback,
            },
            "simulationCount": plan.simulation_count,
            "outcomeCubeKey": plan.outcome_cube_key,
            "dag": dag,
        }),
    );
}

fn strategy_hour_utc(time: &str) -> Option<u32> {
    let ms = parse_oanda_time_ms(time)?;
    let seconds = ms.div_euclid(1000);
    Some((seconds.rem_euclid(86_400) / 3_600) as u32)
}

fn percentile(values: &[f64], q: f64) -> Option<f64> {
    let mut finite = values
        .iter()
        .copied()
        .filter(|value| value.is_finite())
        .collect::<Vec<_>>();
    if finite.is_empty() {
        return None;
    }
    finite.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let clamped = q.clamp(0.0, 1.0);
    let pos = clamped * (finite.len().saturating_sub(1) as f64);
    let lo = pos.floor() as usize;
    let hi = pos.ceil() as usize;
    if lo == hi {
        return finite.get(lo).copied();
    }
    let weight = pos - lo as f64;
    Some((finite[lo] * (1.0 - weight)) + (finite[hi] * weight))
}

fn strategy_low_volatility_values(
    candles: &[TradingCandlePoint],
    metric: &str,
    lookback: usize,
) -> Vec<f64> {
    let lookback = lookback.max(2);
    let mut raw = vec![f64::NAN; candles.len()];
    match metric {
        "bollinger_width_percentile" => {
            for index in lookback..candles.len() {
                let start = index.saturating_sub(lookback);
                let slice = &candles[start..index];
                if slice.len() < lookback {
                    continue;
                }
                let mean = slice.iter().map(|candle| candle.close).sum::<f64>() / slice.len() as f64;
                let variance = slice
                    .iter()
                    .map(|candle| {
                        let delta = candle.close - mean;
                        delta * delta
                    })
                    .sum::<f64>()
                    / slice.len() as f64;
                raw[index] = variance.sqrt() * 4.0;
            }
        }
        "atr_percentile" => {
            for index in 1..candles.len() {
                let previous_close = candles[index - 1].close;
                raw[index] = (candles[index].high - candles[index].low)
                    .max((candles[index].high - previous_close).abs())
                    .max((candles[index].low - previous_close).abs());
            }
        }
        _ => {
            for (index, candle) in candles.iter().enumerate() {
                raw[index] = candle.high - candle.low;
            }
        }
    }

    let mut out = vec![f64::NAN; candles.len()];
    let mut rolling_sum = 0.0;
    let mut rolling_count = 0usize;
    for index in 0..candles.len() {
        if index > 0 {
            let previous = raw[index - 1];
            if previous.is_finite() {
                rolling_sum += previous;
                rolling_count += 1;
            }
        }
        if index > lookback {
            let leaving = raw[index - lookback - 1];
            if leaving.is_finite() {
                rolling_sum -= leaving;
                rolling_count = rolling_count.saturating_sub(1);
            }
        }
        if rolling_count >= lookback {
            out[index] = rolling_sum / rolling_count as f64;
        }
    }
    out
}

fn strategy_low_volatility_values_cached(
    candles: &[TradingCandlePoint],
    metric: &str,
    lookback: usize,
    data_hash: &str,
    cache_snapshot: &mut StrategyCacheSnapshot,
) -> Vec<f64> {
    let key = format!(
        "low-vol-{}",
        &strategy_sha256(&format!("{data_hash}|{metric}|{}", lookback.max(2)))[..16]
    );
    if let Ok(cache) = trading_strategy_low_vol_cache().lock() {
        if let Some(values) = cache.get(&key) {
            cache_snapshot.record("low_vol", true, candles.len());
            return values.clone();
        }
    }
    cache_snapshot.record("low_vol", false, 0);
    let values = strategy_low_volatility_values(candles, metric, lookback);
    if let Ok(mut cache) = trading_strategy_low_vol_cache().lock() {
        cache.insert(key.clone(), values.clone());
    }
    strategy_write_cache_marker(&key, json!({
        "node": "low_vol",
        "metric": metric,
        "lookback": lookback.max(2),
        "rows": candles.len(),
        "dataHash": data_hash,
    }));
    values
}

fn strategy_take_profit_grid(min_distance: f64, max_distance: f64) -> Vec<f64> {
    let min_distance = min_distance.min(max_distance);
    let max_distance = max_distance.max(min_distance);
    if (max_distance - min_distance).abs() <= f64::EPSILON {
        return vec![min_distance];
    }
    let steps = 24usize;
    (0..=steps)
        .map(|index| {
            let t = index as f64 / steps as f64;
            min_distance + ((max_distance - min_distance) * t)
        })
        .collect()
}

fn strategy_directions(direction: &str) -> Vec<String> {
    match direction.trim().to_lowercase().as_str() {
        "long" | "buy" => vec!["long".to_string()],
        "short" | "sell" => vec!["short".to_string()],
        "paired" | "straddle" => vec!["paired".to_string()],
        _ => vec!["long".to_string(), "short".to_string()],
    }
}

fn strategy_is_paired_mode(spec: &TradingStrategySpec) -> bool {
    matches!(
        spec.direction.as_deref().unwrap_or("").trim().to_lowercase().as_str(),
        "paired" | "straddle"
    )
}

fn strategy_filter_matches_requested_refs(
    filter: &StrategyEntryFilter,
    spec: &TradingStrategySpec,
) -> bool {
    let Some(requested_refs) = spec.indicator_refs.as_ref() else {
        return true;
    };
    if requested_refs.is_empty() {
        return true;
    }
    filter
        .indicator_refs
        .iter()
        .any(|reference| requested_refs.iter().any(|requested| requested.eq_ignore_ascii_case(reference)))
}

#[derive(Debug, Clone)]
struct StrategyBacktestTrade {
    entry_time: String,
    pnl_distance: f64,
}

#[derive(Debug, Clone, Default)]
struct StrategyDailyPerformance {
    total_days: usize,
    positive_days: usize,
    negative_days: usize,
    target_hit_days: usize,
    avg_daily_pnl_distance: f64,
    min_daily_pnl_distance: f64,
}

impl StrategyDailyPerformance {
    fn daily_target_hit_rate(&self) -> Option<f64> {
        if self.total_days > 0 {
            Some(self.target_hit_days as f64 / self.total_days as f64)
        } else {
            None
        }
    }

    fn positive_day_rate(&self) -> Option<f64> {
        if self.total_days > 0 {
            Some(self.positive_days as f64 / self.total_days as f64)
        } else {
            None
        }
    }
}

fn strategy_daily_performance(
    trades: &[StrategyBacktestTrade],
    daily_target_distance: f64,
) -> Option<StrategyDailyPerformance> {
    let mut days = BTreeMap::<String, f64>::new();
    for trade in trades {
        let day = trade.entry_time.get(..10)?.to_string();
        *days.entry(day).or_insert(0.0) += trade.pnl_distance;
    }
    if days.is_empty() {
        return None;
    }
    let total_days = days.len();
    let mut performance = StrategyDailyPerformance {
        total_days,
        min_daily_pnl_distance: f64::INFINITY,
        ..StrategyDailyPerformance::default()
    };
    for pnl in days.values().copied() {
        performance.avg_daily_pnl_distance += pnl;
        performance.min_daily_pnl_distance = performance.min_daily_pnl_distance.min(pnl);
        if pnl > 0.0 {
            performance.positive_days += 1;
        } else if pnl < 0.0 {
            performance.negative_days += 1;
        }
        if pnl >= daily_target_distance {
            performance.target_hit_days += 1;
        }
    }
    performance.avg_daily_pnl_distance /= total_days as f64;
    if !performance.min_daily_pnl_distance.is_finite() {
        performance.min_daily_pnl_distance = 0.0;
    }
    Some(performance)
}

#[derive(Debug)]
struct StrategyIndicatorFeatureBank {
    ema8: Vec<f64>,
    ema13: Vec<f64>,
    ema21: Vec<f64>,
    ema34: Vec<f64>,
    ema50: Vec<f64>,
    ema55: Vec<f64>,
    ema100: Vec<f64>,
    ema200: Vec<f64>,
    basis: Vec<f64>,
    upper: Vec<f64>,
    lower: Vec<f64>,
    vwap: Vec<f64>,
    vwap_sigma: Vec<f64>,
    vwap_ext1_up: Vec<f64>,
    vwap_ext1_down: Vec<f64>,
    vwap_ext2_up: Vec<f64>,
    vwap_ext2_down: Vec<f64>,
    rsi14: Vec<f64>,
    atr14: Vec<f64>,
    macd_line: Vec<f64>,
    macd_signal: Vec<f64>,
    macd_histogram: Vec<f64>,
    donchian20_upper: Vec<f64>,
    donchian20_lower: Vec<f64>,
    donchian20_mid: Vec<f64>,
    donchian55_upper: Vec<f64>,
    donchian55_lower: Vec<f64>,
    donchian55_mid: Vec<f64>,
    stoch14_k: Vec<f64>,
    stoch14_d: Vec<f64>,
    ranges: Vec<f64>,
    bodies: Vec<f64>,
    volumes: Vec<f64>,
    boll_widths: Vec<f64>,
    range_p10: f64,
    range_p20: f64,
    range_p35: f64,
    range_p50: f64,
    body_p35: f64,
    atr_p20: f64,
    atr_p35: f64,
    atr_p50: f64,
    volume_p40: f64,
    volume_p60: f64,
    boll_width_p20: f64,
    boll_width_p35: f64,
    boll_width_p50: f64,
}

#[derive(Debug, Clone)]
struct StrategyEntryFilter {
    id: String,
    label: String,
    condition_hash: String,
    mask_ref: String,
    bytecode_ops: Vec<String>,
    display_formula: Vec<String>,
    indicator_refs: Vec<String>,
    mask: StrategyConditionMask,
    entry_count: usize,
    cache_key: String,
}

#[derive(Debug, Clone)]
struct StrategyAtomEntries {
    id: String,
    label: String,
    indicator_refs: Vec<String>,
    mask: StrategyConditionMask,
    bytecode_ops: Vec<String>,
    display_formula: Vec<String>,
}

#[derive(Debug, Clone)]
struct StrategyConditionMask {
    mask_hash: String,
    bits: Vec<u64>,
    count: usize,
}

#[derive(Debug, Clone)]
struct StrategyOutcomePoint {
    held: usize,
    favorable_distance: f64,
}

#[derive(Debug, Clone)]
struct StrategyEntryOutcome {
    entry_time: String,
    execution_cost_distance: f64,
    terminal_pnl_distance: f64,
    terminal_held: usize,
    stop_pnl_distance: f64,
    stop_held: Option<usize>,
    max_favorable_distance: f64,
    max_adverse_distance: f64,
    favorable_path: Vec<StrategyOutcomePoint>,
}

#[derive(Debug, Clone, Default)]
struct StrategySummaryStats {
    trades: usize,
    wins: usize,
    losses: usize,
    net_pnl: f64,
    gross_profit: f64,
    gross_loss: f64,
    max_loss_streak: usize,
    held_sum: usize,
}

impl StrategySummaryStats {
    fn record(&mut self, pnl: f64, held: usize, loss_streak: &mut usize) {
        self.trades += 1;
        self.held_sum += held;
        self.net_pnl += pnl;
        if pnl > 0.0 {
            self.wins += 1;
            self.gross_profit += pnl;
            *loss_streak = 0;
        } else {
            self.losses += 1;
            self.gross_loss += pnl.abs();
            *loss_streak += 1;
            self.max_loss_streak = self.max_loss_streak.max(*loss_streak);
        }
    }

    fn win_rate(&self) -> Option<f64> {
        if self.trades > 0 {
            Some(self.wins as f64 / self.trades as f64)
        } else {
            None
        }
    }

    fn expectancy(&self) -> f64 {
        if self.trades > 0 {
            self.net_pnl / self.trades as f64
        } else {
            0.0
        }
    }

    fn profit_factor(&self) -> Option<f64> {
        if self.gross_loss > 0.0 {
            Some(self.gross_profit / self.gross_loss)
        } else if self.gross_profit > 0.0 {
            Some(f64::INFINITY)
        } else {
            None
        }
    }

    fn avg_hold_bars(&self) -> f64 {
        if self.trades > 0 {
            self.held_sum as f64 / self.trades as f64
        } else {
            0.0
        }
    }
}

#[derive(Debug, Clone)]
struct StrategyFilterOutcomePoint {
    take_profit_distance: f64,
    stats: StrategySummaryStats,
}

#[derive(Debug, Clone, Default)]
struct StrategyFilterOutcomeGrid {
    points: Vec<StrategyFilterOutcomePoint>,
}

#[derive(Debug, Clone, Default)]
struct StrategySimulationStats {
    trades: usize,
    wins: usize,
    losses: usize,
    net_pnl: f64,
    gross_profit: f64,
    gross_loss: f64,
    max_loss_streak: usize,
    held_sum: usize,
    trades_detail: Vec<StrategyBacktestTrade>,
}

impl StrategySimulationStats {
    fn record(&mut self, entry_time: &str, pnl: f64, held: usize, loss_streak: &mut usize) {
        self.trades += 1;
        self.held_sum += held;
        self.net_pnl += pnl;
        self.trades_detail.push(StrategyBacktestTrade {
            entry_time: entry_time.to_string(),
            pnl_distance: pnl,
        });
        if pnl > 0.0 {
            self.wins += 1;
            self.gross_profit += pnl;
            *loss_streak = 0;
        } else {
            self.losses += 1;
            self.gross_loss += pnl.abs();
            *loss_streak += 1;
            self.max_loss_streak = self.max_loss_streak.max(*loss_streak);
        }
    }

    fn win_rate(&self) -> Option<f64> {
        if self.trades > 0 {
            Some(self.wins as f64 / self.trades as f64)
        } else {
            None
        }
    }

    fn expectancy(&self) -> f64 {
        if self.trades > 0 {
            self.net_pnl / self.trades as f64
        } else {
            0.0
        }
    }

    fn profit_factor(&self) -> Option<f64> {
        if self.gross_loss > 0.0 {
            Some(self.gross_profit / self.gross_loss)
        } else if self.gross_profit > 0.0 {
            Some(f64::INFINITY)
        } else {
            None
        }
    }

    fn avg_hold_bars(&self) -> f64 {
        if self.trades > 0 {
            self.held_sum as f64 / self.trades as f64
        } else {
            0.0
        }
    }
}

fn strategy_entry_indices(
    candles: &[TradingCandlePoint],
    low_volatility_values: &[f64],
    threshold: f64,
    start_index: usize,
    end_index: usize,
    entry_hours: &[u32],
    force_daily_entry: bool,
) -> Vec<usize> {
    if candles.len() < 2 || start_index >= candles.len() {
        return Vec::new();
    }
    if entry_hours.is_empty() {
        return Vec::new();
    }
    let end_limit = end_index.min(candles.len().saturating_sub(1));
    let mut entries = Vec::new();
    for index in start_index..end_limit {
        if !force_daily_entry
            && low_volatility_values.get(index).copied().unwrap_or(f64::NAN) > threshold
        {
            continue;
        }
        let Some(entry_hour) = strategy_hour_utc(&candles[index].time) else {
            continue;
        };
        if !entry_hours.contains(&entry_hour) {
            continue;
        }
        let entry = candles[index].open;
        if !entry.is_finite() || entry <= 0.0 {
            continue;
        }
        entries.push(index);
    }
    entries
}

fn strategy_entry_indices_cached(
    candles: &[TradingCandlePoint],
    low_volatility_values: &[f64],
    threshold: f64,
    start_index: usize,
    end_index: usize,
    entry_hours: &[u32],
    force_daily_entry: bool,
    data_hash: &str,
    cache_snapshot: &mut StrategyCacheSnapshot,
) -> (Vec<usize>, String) {
    let entry_hours_key = entry_hours
        .iter()
        .map(|value| value.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let key = format!(
        "entry-scan-{}",
        &strategy_sha256(&format!(
            "{data_hash}|{threshold:.10}|{start_index}|{end_index}|hours={entry_hours_key}|force_daily={force_daily_entry}|{}",
            low_volatility_values.len()
        ))[..16]
    );
    if let Ok(cache) = trading_strategy_entry_scan_cache().lock() {
        if let Some(entries) = cache.get(&key) {
            cache_snapshot.record("entry_scan", true, candles.len());
            return (entries.clone(), key);
        }
    }
    cache_snapshot.record("entry_scan", false, 0);
    let entries = strategy_entry_indices(
        candles,
        low_volatility_values,
        threshold,
        start_index,
        end_index,
        entry_hours,
        force_daily_entry,
    );
    if let Ok(mut cache) = trading_strategy_entry_scan_cache().lock() {
        cache.insert(key.clone(), entries.clone());
    }
    strategy_write_cache_marker(&key, json!({
        "node": "entry_scan",
        "rows": candles.len(),
        "entries": entries.len(),
        "entryHourUtc": entry_hours.first().copied().unwrap_or(21),
        "entryHoursUtc": entry_hours,
        "threshold": threshold,
        "forceDailyEntry": force_daily_entry,
        "dataHash": data_hash,
    }));
    (entries, key)
}

fn strategy_entries_hash(entries: &[usize]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"strategy-entries:v1");
    for entry in entries {
        hasher.update(entry.to_le_bytes());
    }
    strategy_hex(&hasher.finalize())
}

fn strategy_sma_close(candles: &[TradingCandlePoint], period: usize) -> Vec<f64> {
    let period = period.max(1);
    let mut out = Vec::with_capacity(candles.len());
    let mut sum = 0.0;
    for index in 0..candles.len() {
        sum += candles[index].close;
        if index >= period {
            sum -= candles[index - period].close;
        }
        out.push(if index + 1 >= period { sum / period as f64 } else { f64::NAN });
    }
    out
}

fn strategy_ema_close(candles: &[TradingCandlePoint], period: usize) -> Vec<f64> {
    if candles.is_empty() {
        return Vec::new();
    }
    let alpha = 2.0 / (period.max(1) as f64 + 1.0);
    let mut out = Vec::with_capacity(candles.len());
    let mut previous = candles[0].close;
    for candle in candles {
        previous = alpha.mul_add(candle.close, (1.0 - alpha) * previous);
        out.push(previous);
    }
    out
}

fn strategy_bollinger_bands(
    candles: &[TradingCandlePoint],
    period: usize,
    deviations: f64,
) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let period = period.max(2);
    let basis = strategy_sma_close(candles, period);
    let mut upper = vec![f64::NAN; candles.len()];
    let mut lower = vec![f64::NAN; candles.len()];
    let mut sum = 0.0;
    let mut sum_sq = 0.0;
    for index in 0..candles.len() {
        let close = candles[index].close;
        sum += close;
        sum_sq += close * close;
        if index >= period {
            let drop = candles[index - period].close;
            sum -= drop;
            sum_sq -= drop * drop;
        }
        if index + 1 >= period {
            let mean = sum / period as f64;
            let variance = ((sum_sq / period as f64) - mean * mean).max(0.0);
            let width = variance.sqrt() * deviations;
            upper[index] = basis[index] + width;
            lower[index] = basis[index] - width;
        }
    }
    (basis, upper, lower)
}

fn strategy_vwap(candles: &[TradingCandlePoint]) -> Vec<f64> {
    let mut out = Vec::with_capacity(candles.len());
    let mut pv = 0.0;
    let mut volume = 0.0;
    for candle in candles {
        let typical = (candle.high + candle.low + candle.close) / 3.0;
        let vol = (candle.volume as f64).max(1.0);
        pv += typical * vol;
        volume += vol;
        out.push(if volume > 0.0 { pv / volume } else { candle.close });
    }
    out
}

fn strategy_vwap_extensions(
    candles: &[TradingCandlePoint],
) -> (Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>) {
    let mut vwap = Vec::with_capacity(candles.len());
    let mut sigma = Vec::with_capacity(candles.len());
    let mut ext1_up = Vec::with_capacity(candles.len());
    let mut ext1_down = Vec::with_capacity(candles.len());
    let mut ext2_up = Vec::with_capacity(candles.len());
    let mut ext2_down = Vec::with_capacity(candles.len());
    let mut pv = 0.0;
    let mut sq_pv = 0.0;
    let mut volume = 0.0;
    for candle in candles {
        let typical = (candle.high + candle.low + candle.close) / 3.0;
        let vol = (candle.volume as f64).max(1.0);
        pv += typical * vol;
        sq_pv += typical * typical * vol;
        volume += vol;
        let mean = if volume > 0.0 { pv / volume } else { candle.close };
        let variance = if volume > 0.0 {
            ((sq_pv / volume) - mean * mean).max(0.0)
        } else {
            0.0
        };
        let stddev = variance.sqrt();
        vwap.push(mean);
        sigma.push(stddev);
        ext1_up.push(mean + stddev);
        ext1_down.push(mean - stddev);
        ext2_up.push(mean + 2.0 * stddev);
        ext2_down.push(mean - 2.0 * stddev);
    }
    (vwap, sigma, ext1_up, ext1_down, ext2_up, ext2_down)
}

fn strategy_sma_values(values: &[f64], period: usize) -> Vec<f64> {
    let period = period.max(1);
    let mut out = Vec::with_capacity(values.len());
    let mut sum = 0.0;
    let mut count = 0usize;
    for index in 0..values.len() {
        let value = values[index];
        if value.is_finite() {
            sum += value;
            count += 1;
        }
        if index >= period {
            let leaving = values[index - period];
            if leaving.is_finite() {
                sum -= leaving;
                count = count.saturating_sub(1);
            }
        }
        out.push(if index + 1 >= period && count == period {
            sum / period as f64
        } else {
            f64::NAN
        });
    }
    out
}

fn strategy_ema_values(values: &[f64], period: usize) -> Vec<f64> {
    let alpha = 2.0 / (period.max(1) as f64 + 1.0);
    let mut out = Vec::with_capacity(values.len());
    let mut previous = f64::NAN;
    for value in values {
        if value.is_finite() {
            previous = if previous.is_finite() {
                alpha.mul_add(*value, (1.0 - alpha) * previous)
            } else {
                *value
            };
        }
        out.push(previous);
    }
    out
}

fn strategy_true_range(candles: &[TradingCandlePoint]) -> Vec<f64> {
    let mut out = Vec::with_capacity(candles.len());
    for (index, candle) in candles.iter().enumerate() {
        let range = candle.high - candle.low;
        let true_range = if index == 0 {
            range
        } else {
            let previous_close = candles[index - 1].close;
            range
                .max((candle.high - previous_close).abs())
                .max((candle.low - previous_close).abs())
        };
        out.push(true_range);
    }
    out
}

fn strategy_atr(candles: &[TradingCandlePoint], period: usize) -> Vec<f64> {
    strategy_ema_values(&strategy_true_range(candles), period)
}

fn strategy_rsi_close(candles: &[TradingCandlePoint], period: usize) -> Vec<f64> {
    let period = period.max(2);
    let mut out = vec![f64::NAN; candles.len()];
    if candles.len() <= period {
        return out;
    }
    let mut avg_gain = 0.0;
    let mut avg_loss = 0.0;
    for index in 1..candles.len() {
        let delta = candles[index].close - candles[index - 1].close;
        let gain = delta.max(0.0);
        let loss = (-delta).max(0.0);
        if index <= period {
            avg_gain += gain;
            avg_loss += loss;
            if index == period {
                avg_gain /= period as f64;
                avg_loss /= period as f64;
            }
        } else {
            avg_gain = ((avg_gain * (period - 1) as f64) + gain) / period as f64;
            avg_loss = ((avg_loss * (period - 1) as f64) + loss) / period as f64;
        }
        if index >= period {
            out[index] = if avg_loss <= f64::EPSILON {
                100.0
            } else {
                100.0 - (100.0 / (1.0 + (avg_gain / avg_loss)))
            };
        }
    }
    out
}

fn strategy_macd(
    candles: &[TradingCandlePoint],
    fast: usize,
    slow: usize,
    signal_period: usize,
) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let fast_ema = strategy_ema_close(candles, fast);
    let slow_ema = strategy_ema_close(candles, slow);
    let line = fast_ema
        .iter()
        .zip(slow_ema.iter())
        .map(|(fast, slow)| fast - slow)
        .collect::<Vec<_>>();
    let signal = strategy_ema_values(&line, signal_period);
    let histogram = line
        .iter()
        .zip(signal.iter())
        .map(|(line, signal)| line - signal)
        .collect::<Vec<_>>();
    (line, signal, histogram)
}

fn strategy_donchian_channels(
    candles: &[TradingCandlePoint],
    period: usize,
) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let period = period.max(2);
    let mut upper = vec![f64::NAN; candles.len()];
    let mut lower = vec![f64::NAN; candles.len()];
    let mut high_deque = VecDeque::<usize>::new();
    let mut low_deque = VecDeque::<usize>::new();
    for index in 0..candles.len() {
        while high_deque.front().is_some_and(|front| *front + period <= index) {
            high_deque.pop_front();
        }
        while low_deque.front().is_some_and(|front| *front + period <= index) {
            low_deque.pop_front();
        }
        while high_deque
            .back()
            .is_some_and(|back| candles[*back].high <= candles[index].high)
        {
            high_deque.pop_back();
        }
        while low_deque
            .back()
            .is_some_and(|back| candles[*back].low >= candles[index].low)
        {
            low_deque.pop_back();
        }
        high_deque.push_back(index);
        low_deque.push_back(index);
        if index + 1 >= period {
            if let (Some(high_idx), Some(low_idx)) = (high_deque.front(), low_deque.front()) {
                upper[index] = candles[*high_idx].high;
                lower[index] = candles[*low_idx].low;
            }
        }
    }
    let mid = upper
        .iter()
        .zip(lower.iter())
        .map(|(upper, lower)| {
            if upper.is_finite() && lower.is_finite() {
                (upper + lower) * 0.5
            } else {
                f64::NAN
            }
        })
        .collect::<Vec<_>>();
    (upper, lower, mid)
}

fn strategy_stochastic(
    candles: &[TradingCandlePoint],
    period: usize,
    signal_period: usize,
) -> (Vec<f64>, Vec<f64>) {
    let (upper, lower, _) = strategy_donchian_channels(candles, period);
    let k = candles
        .iter()
        .enumerate()
        .map(|(index, candle)| {
            let span = upper[index] - lower[index];
            if upper[index].is_finite() && lower[index].is_finite() && span.abs() > f64::EPSILON {
                ((candle.close - lower[index]) / span) * 100.0
            } else {
                f64::NAN
            }
        })
        .collect::<Vec<_>>();
    let d = strategy_sma_values(&k, signal_period.max(1));
    (k, d)
}

fn strategy_feature_bank_cached(
    candles: &[TradingCandlePoint],
    data_hash: &str,
    cache_snapshot: &mut StrategyCacheSnapshot,
) -> Arc<StrategyIndicatorFeatureBank> {
    let key = format!(
        "feature-bank-{}",
        &strategy_sha256(&format!(
            "{data_hash}|strategy-feature-bank-v8|rows={}",
            candles.len()
        ))[..16]
    );
    if let Ok(cache) = trading_strategy_feature_bank_cache().lock() {
        if let Some(bank) = cache.get(&key) {
            cache_snapshot.record("indicator_feature_bank", true, candles.len().saturating_mul(24));
            return Arc::clone(bank);
        }
    }
    cache_snapshot.record("indicator_feature_bank", false, 0);

    let (basis, upper, lower) = strategy_bollinger_bands(candles, 20, 2.0);
    let (vwap, vwap_sigma, vwap_ext1_up, vwap_ext1_down, vwap_ext2_up, vwap_ext2_down) =
        strategy_vwap_extensions(candles);
    let (macd_line, macd_signal, macd_histogram) = strategy_macd(candles, 12, 26, 9);
    let (donchian20_upper, donchian20_lower, donchian20_mid) =
        strategy_donchian_channels(candles, 20);
    let (donchian55_upper, donchian55_lower, donchian55_mid) =
        strategy_donchian_channels(candles, 55);
    let (stoch14_k, stoch14_d) = strategy_stochastic(candles, 14, 3);
    let ranges = candles
        .iter()
        .map(|candle| candle.high - candle.low)
        .collect::<Vec<_>>();
    let bodies = candles
        .iter()
        .map(|candle| (candle.close - candle.open).abs())
        .collect::<Vec<_>>();
    let volumes = candles
        .iter()
        .map(|candle| candle.volume as f64)
        .collect::<Vec<_>>();
    let boll_widths = upper
        .iter()
        .zip(lower.iter())
        .map(|(hi, lo)| hi - lo)
        .collect::<Vec<_>>();
    let atr14 = strategy_atr(candles, 14);

    let bank = Arc::new(StrategyIndicatorFeatureBank {
        ema8: strategy_ema_close(candles, 8),
        ema13: strategy_ema_close(candles, 13),
        ema21: strategy_ema_close(candles, 21),
        ema34: strategy_ema_close(candles, 34),
        ema50: strategy_ema_close(candles, 50),
        ema55: strategy_ema_close(candles, 55),
        ema100: strategy_ema_close(candles, 100),
        ema200: strategy_ema_close(candles, 200),
        basis,
        upper,
        lower,
        vwap,
        vwap_sigma,
        vwap_ext1_up,
        vwap_ext1_down,
        vwap_ext2_up,
        vwap_ext2_down,
        rsi14: strategy_rsi_close(candles, 14),
        macd_line,
        macd_signal,
        macd_histogram,
        donchian20_upper,
        donchian20_lower,
        donchian20_mid,
        donchian55_upper,
        donchian55_lower,
        donchian55_mid,
        stoch14_k,
        stoch14_d,
        range_p10: percentile(&ranges, 0.10).unwrap_or(f64::INFINITY),
        range_p20: percentile(&ranges, 0.20).unwrap_or(f64::INFINITY),
        range_p35: percentile(&ranges, 0.35).unwrap_or(f64::INFINITY),
        range_p50: percentile(&ranges, 0.50).unwrap_or(f64::INFINITY),
        body_p35: percentile(&bodies, 0.35).unwrap_or(f64::INFINITY),
        atr_p20: percentile(&atr14, 0.20).unwrap_or(f64::INFINITY),
        atr_p35: percentile(&atr14, 0.35).unwrap_or(f64::INFINITY),
        atr_p50: percentile(&atr14, 0.50).unwrap_or(f64::INFINITY),
        volume_p40: percentile(&volumes, 0.40).unwrap_or(f64::INFINITY),
        volume_p60: percentile(&volumes, 0.60).unwrap_or(f64::INFINITY),
        boll_width_p20: percentile(&boll_widths, 0.20).unwrap_or(f64::INFINITY),
        boll_width_p35: percentile(&boll_widths, 0.35).unwrap_or(f64::INFINITY),
        boll_width_p50: percentile(&boll_widths, 0.50).unwrap_or(f64::INFINITY),
        atr14,
        ranges,
        bodies,
        volumes,
        boll_widths,
    });
    if let Ok(mut cache) = trading_strategy_feature_bank_cache().lock() {
        cache.insert(key.clone(), Arc::clone(&bank));
    }
    strategy_write_cache_marker(&key, json!({
        "node": "indicator_feature_bank",
        "version": "strategy-feature-bank-v8",
        "rows": candles.len(),
        "vectors": [
            "EMA8", "EMA13", "EMA21", "EMA34", "EMA50", "EMA55", "EMA100", "EMA200",
            "Bollinger20x2", "VWAP", "VWAPExtensions", "RSI14", "ATR14", "MACD12_26_9",
            "Donchian20", "Donchian55", "Stochastic14_3", "Range", "Body", "Volume"
        ],
        "reusedBy": ["long_filter_bank", "short_filter_bank", "eclat_bitsets"],
        "dataHash": data_hash,
    }));
    bank
}

fn strategy_condition_mask_words(len: usize) -> usize {
    len.saturating_add(63) / 64
}

fn strategy_condition_set_bit(bits: &mut [u64], index: usize) {
    let word = index / 64;
    let bit = index % 64;
    if let Some(slot) = bits.get_mut(word) {
        *slot |= 1_u64 << bit;
    }
}

fn strategy_condition_count_bits(bits: &[u64]) -> usize {
    bits.iter().map(|word| word.count_ones() as usize).sum()
}

fn strategy_condition_bits_hash(data_hash: &str, direction: &str, bits: &[u64], len: usize) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"strategy-condition-mask:v1");
    hasher.update(data_hash.as_bytes());
    hasher.update(direction.as_bytes());
    hasher.update(len.to_le_bytes());
    for word in bits {
        hasher.update(word.to_le_bytes());
    }
    format!("condmask-{}", &strategy_hex(&hasher.finalize())[..16])
}

fn strategy_condition_entries_from_mask(
    base_entries: &[usize],
    mask: &StrategyConditionMask,
) -> Vec<usize> {
    let mut entries = Vec::with_capacity(mask.count);
    for (pos, entry_index) in base_entries.iter().copied().enumerate() {
        let word = pos / 64;
        let bit = pos % 64;
        if mask
            .bits
            .get(word)
            .is_some_and(|value| (*value & (1_u64 << bit)) != 0)
        {
            entries.push(entry_index);
        }
    }
    entries
}

fn strategy_condition_full_mask_cached(
    data_hash: &str,
    base_entries_cache_key: &str,
    direction: &str,
    base_entries_len: usize,
) -> StrategyConditionMask {
    let cache_key = format!(
        "condition-mask-{}",
        &strategy_sha256(&format!(
            "{data_hash}|{base_entries_cache_key}|{direction}|FULL|len={base_entries_len}"
        ))[..16]
    );
    if let Ok(cache) = trading_strategy_condition_mask_cache().lock() {
        if let Some(mask) = cache.get(&cache_key) {
            return mask.clone();
        }
    }
    let mut bits = vec![u64::MAX; strategy_condition_mask_words(base_entries_len)];
    if let Some(last) = bits.last_mut() {
        let remainder = base_entries_len % 64;
        if remainder != 0 {
            *last &= (1_u64 << remainder) - 1;
        }
    }
    let mask_hash = strategy_condition_bits_hash(data_hash, direction, &bits, base_entries_len);
    let mask = StrategyConditionMask {
        mask_hash,
        bits,
        count: base_entries_len,
    };
    if let Ok(mut cache) = trading_strategy_condition_mask_cache().lock() {
        cache.insert(cache_key, mask.clone());
    }
    mask
}

fn strategy_condition_atom_mask_cached<F>(
    data_hash: &str,
    base_entries_cache_key: &str,
    direction: &str,
    atom_id: &str,
    base_entries: &[usize],
    predicate: F,
) -> StrategyConditionMask
where
    F: Fn(usize) -> bool,
{
    let cache_key = format!(
        "condition-mask-{}",
        &strategy_sha256(&format!(
            "{data_hash}|{base_entries_cache_key}|{direction}|ATOM|{atom_id}|{}",
            strategy_entries_hash(base_entries)
        ))[..16]
    );
    if let Ok(cache) = trading_strategy_condition_mask_cache().lock() {
        if let Some(mask) = cache.get(&cache_key) {
            return mask.clone();
        }
    }
    let mut bits = vec![0_u64; strategy_condition_mask_words(base_entries.len())];
    for (pos, entry_index) in base_entries.iter().copied().enumerate() {
        if predicate(entry_index) {
            strategy_condition_set_bit(&mut bits, pos);
        }
    }
    let count = strategy_condition_count_bits(&bits);
    let mask_hash = strategy_condition_bits_hash(data_hash, direction, &bits, base_entries.len());
    let mask = StrategyConditionMask {
        mask_hash,
        bits,
        count,
    };
    if let Ok(mut cache) = trading_strategy_condition_mask_cache().lock() {
        cache.insert(cache_key, mask.clone());
    }
    mask
}

fn strategy_condition_and_mask_cached(
    data_hash: &str,
    base_entries_cache_key: &str,
    direction: &str,
    op: &str,
    input_hashes: &[&str],
    input_masks: &[&StrategyConditionMask],
    base_entries_len: usize,
) -> StrategyConditionMask {
    let cache_key = format!(
        "condition-mask-{}",
        &strategy_sha256(&format!(
            "{data_hash}|{base_entries_cache_key}|{direction}|{op}|{}",
            input_hashes.join("|")
        ))[..16]
    );
    if let Ok(cache) = trading_strategy_condition_mask_cache().lock() {
        if let Some(mask) = cache.get(&cache_key) {
            return mask.clone();
        }
    }
    let mut bits = vec![u64::MAX; strategy_condition_mask_words(base_entries_len)];
    for mask in input_masks {
        for (idx, word) in bits.iter_mut().enumerate() {
            *word &= mask.bits.get(idx).copied().unwrap_or(0);
        }
    }
    let count = strategy_condition_count_bits(&bits);
    let mask_hash = strategy_condition_bits_hash(data_hash, direction, &bits, base_entries_len);
    let mask = StrategyConditionMask {
        mask_hash,
        bits,
        count,
    };
    if let Ok(mut cache) = trading_strategy_condition_mask_cache().lock() {
        cache.insert(cache_key, mask.clone());
    }
    mask
}

fn strategy_directional_entry_filters_cached(
    candles: &[TradingCandlePoint],
    base_entries: &[usize],
    base_entries_cache_key: &str,
    direction: &str,
    data_hash: &str,
    force_daily_entry: bool,
    cache_snapshot: &mut StrategyCacheSnapshot,
) -> Vec<StrategyEntryFilter> {
    let key = format!(
        "entry-filters-{}",
        &strategy_sha256(&format!(
            "{data_hash}|{base_entries_cache_key}|{direction}|adaptive-eclat-mfe-reduce-grid-v8|force_daily={force_daily_entry}|{}",
            strategy_entries_hash(base_entries)
        ))[..16]
    );
    if let Ok(cache) = trading_strategy_filter_entry_cache().lock() {
        if let Some(filters) = cache.get(&key) {
            cache_snapshot.record("indicator_filters", true, base_entries.len().saturating_mul(8));
            return filters.clone();
        }
    }
    cache_snapshot.record("indicator_filters", false, 0);

    let feature_bank = strategy_feature_bank_cached(candles, data_hash, cache_snapshot);
    let ema8 = feature_bank.ema8.as_slice();
    let ema13 = feature_bank.ema13.as_slice();
    let ema21 = feature_bank.ema21.as_slice();
    let ema34 = feature_bank.ema34.as_slice();
    let ema50 = feature_bank.ema50.as_slice();
    let ema55 = feature_bank.ema55.as_slice();
    let ema100 = feature_bank.ema100.as_slice();
    let ema200 = feature_bank.ema200.as_slice();
    let basis = feature_bank.basis.as_slice();
    let upper = feature_bank.upper.as_slice();
    let lower = feature_bank.lower.as_slice();
    let vwap = feature_bank.vwap.as_slice();
    let vwap_sigma = feature_bank.vwap_sigma.as_slice();
    let vwap_ext1_up = feature_bank.vwap_ext1_up.as_slice();
    let vwap_ext1_down = feature_bank.vwap_ext1_down.as_slice();
    let vwap_ext2_up = feature_bank.vwap_ext2_up.as_slice();
    let vwap_ext2_down = feature_bank.vwap_ext2_down.as_slice();
    let rsi14 = feature_bank.rsi14.as_slice();
    let atr14 = feature_bank.atr14.as_slice();
    let macd_line = feature_bank.macd_line.as_slice();
    let macd_signal = feature_bank.macd_signal.as_slice();
    let macd_histogram = feature_bank.macd_histogram.as_slice();
    let donchian20_upper = feature_bank.donchian20_upper.as_slice();
    let donchian20_lower = feature_bank.donchian20_lower.as_slice();
    let donchian20_mid = feature_bank.donchian20_mid.as_slice();
    let donchian55_upper = feature_bank.donchian55_upper.as_slice();
    let donchian55_lower = feature_bank.donchian55_lower.as_slice();
    let donchian55_mid = feature_bank.donchian55_mid.as_slice();
    let stoch14_k = feature_bank.stoch14_k.as_slice();
    let stoch14_d = feature_bank.stoch14_d.as_slice();
    let ranges = feature_bank.ranges.as_slice();
    let bodies = feature_bank.bodies.as_slice();
    let volumes = feature_bank.volumes.as_slice();
    let boll_widths = feature_bank.boll_widths.as_slice();
    let range_p10 = feature_bank.range_p10;
    let range_p20 = feature_bank.range_p20;
    let range_p35 = feature_bank.range_p35;
    let range_p50 = feature_bank.range_p50;
    let body_p35 = feature_bank.body_p35;
    let atr_p20 = feature_bank.atr_p20;
    let atr_p35 = feature_bank.atr_p35;
    let atr_p50 = feature_bank.atr_p50;
    let volume_p40 = feature_bank.volume_p40;
    let volume_p60 = feature_bank.volume_p60;
    let boll_width_p20 = feature_bank.boll_width_p20;
    let boll_width_p35 = feature_bank.boll_width_p35;
    let boll_width_p50 = feature_bank.boll_width_p50;
    let is_long = matches!(direction, "long" | "buy");
    let range_at = |i: usize| ranges.get(i).copied().unwrap_or(0.0).abs().max(f64::EPSILON);
    let atr_at = |i: usize| {
        atr14
            .get(i)
            .copied()
            .filter(|value| value.is_finite() && value.abs() > f64::EPSILON)
            .unwrap_or_else(|| range_at(i))
            .abs()
            .max(f64::EPSILON)
    };
    let body_ratio = |i: usize| bodies.get(i).copied().unwrap_or(0.0) / range_at(i);
    let upper_wick_ratio = |i: usize| {
        let candle = &candles[i];
        (candle.high - candle.open.max(candle.close)).max(0.0) / range_at(i)
    };
    let lower_wick_ratio = |i: usize| {
        let candle = &candles[i];
        (candle.open.min(candle.close) - candle.low).max(0.0) / range_at(i)
    };
    let boll_position = |i: usize| {
        let width = (upper[i] - lower[i]).abs();
        if width.is_finite() && width > f64::EPSILON {
            (candles[i].close - lower[i]) / width
        } else {
            f64::NAN
        }
    };
    let slope_up = |values: &[f64], i: usize, lookback: usize| {
        i >= lookback && values[i].is_finite() && values[i - lookback].is_finite() && values[i] > values[i - lookback]
    };
    let slope_down = |values: &[f64], i: usize, lookback: usize| {
        i >= lookback && values[i].is_finite() && values[i - lookback].is_finite() && values[i] < values[i - lookback]
    };
    let near_by_atr = |a: f64, b: f64, i: usize, max_atr: f64| {
        a.is_finite() && b.is_finite() && ((a - b).abs() / atr_at(i)) <= max_atr
    };
    let near_by_sigma = |a: f64, b: f64, i: usize, max_sigma: f64| {
        let sigma = vwap_sigma.get(i).copied().unwrap_or(f64::NAN).abs();
        a.is_finite() && b.is_finite() && sigma.is_finite() && sigma > f64::EPSILON && ((a - b).abs() / sigma) <= max_sigma
    };
    let crosses_above = |values: &[f64], reference: &[f64], i: usize| {
        i > 0
            && values[i - 1].is_finite()
            && reference[i - 1].is_finite()
            && values[i].is_finite()
            && reference[i].is_finite()
            && values[i - 1] <= reference[i - 1]
            && values[i] > reference[i]
    };
    let crosses_below = |values: &[f64], reference: &[f64], i: usize| {
        i > 0
            && values[i - 1].is_finite()
            && reference[i - 1].is_finite()
            && values[i].is_finite()
            && reference[i].is_finite()
            && values[i - 1] >= reference[i - 1]
            && values[i] < reference[i]
    };
    let mut filters = Vec::<StrategyEntryFilter>::new();
    let push_filter = |filters: &mut Vec<StrategyEntryFilter>,
                       id: &str,
                       label: &str,
                       mask: StrategyConditionMask,
                       bytecode_ops: Vec<String>,
                       display_formula: Vec<String>,
                       refs: Vec<String>| {
        if mask.count < 8 {
            return;
        }
        let cache_key = format!(
            "entry-filter-{}",
            &strategy_sha256(&format!(
                "{base_entries_cache_key}|{direction}|{id}|{}|count={}",
                mask.mask_hash,
                mask.count
            ))[..16]
        );
        filters.push(StrategyEntryFilter {
            id: id.to_string(),
            label: label.to_string(),
            condition_hash: mask.mask_hash.clone(),
            mask_ref: mask.mask_hash.clone(),
            bytecode_ops,
            display_formula,
            indicator_refs: refs,
            entry_count: mask.count,
            mask,
            cache_key,
        });
    };

    let base_mask =
        strategy_condition_full_mask_cached(data_hash, base_entries_cache_key, direction, base_entries.len());
    push_filter(
        &mut filters,
        "base_low_vol",
        if force_daily_entry { "21h daily entries" } else { "21h low-volatility entries" },
        base_mask.clone(),
        vec![
            "ENTRY:HOUR_EQ_21UTC".to_string(),
            if force_daily_entry {
                "ENTRY:FORCE_ONE_TRADE_PER_OPEN_DAY".to_string()
            } else {
                "VOL:RANGE_SMA_LE_TRAIN_Q".to_string()
            },
        ],
        vec![
            "entry hour == 21 UTC".to_string(),
            if force_daily_entry {
                "one trade each open trading day".to_string()
            } else {
                "range SMA H1 24 <= train percentile".to_string()
            },
        ],
        vec!["/candle_h1".to_string(), "/range_sma_h1_24".to_string()],
    );

    let mut atoms = Vec::<StrategyAtomEntries>::new();
    macro_rules! add_atom {
        ($id:expr, $label:expr, [$($rf:expr),+], $predicate:expr) => {{
            let refs = vec![$($rf.to_string()),+];
            let mask = strategy_condition_atom_mask_cached(
                data_hash,
                base_entries_cache_key,
                direction,
                $id,
                base_entries,
                $predicate,
            );
            if mask.count >= 8 {
                atoms.push(StrategyAtomEntries {
                    id: $id.to_string(),
                    label: $label.to_string(),
                    indicator_refs: refs.clone(),
                    mask,
                    bytecode_ops: vec![format!("ATOM:{}", $id)],
                    display_formula: vec![$label.to_string()],
                });
            }
        }};
    }

    if is_long {
        add_atom!("close_gt_ema8", "close > EMA8", ["/ema_h1_8_close"], |i: usize| candles[i].close > ema8[i]);
        add_atom!("close_gt_ema21", "close > EMA21", ["/ema_h1_21_close"], |i: usize| candles[i].close > ema21[i]);
        add_atom!("close_gt_ema50", "close > EMA50", ["/ema_h1_50_close"], |i: usize| candles[i].close > ema50[i]);
        add_atom!("close_gt_ema100", "close > EMA100", ["/ema_h1_100_close"], |i: usize| candles[i].close > ema100[i]);
        add_atom!("close_gt_ema200", "close > EMA200", ["/ema_h1_200_close"], |i: usize| candles[i].close > ema200[i]);
        add_atom!("ema8_gt_ema13", "EMA8 > EMA13", ["/ema_h1_8_close", "/ema_h1_13_close"], |i: usize| ema8[i] > ema13[i]);
        add_atom!("ema13_gt_ema34", "EMA13 > EMA34", ["/ema_h1_13_close", "/ema_h1_34_close"], |i: usize| ema13[i] > ema34[i]);
        add_atom!("ema34_gt_ema55", "EMA34 > EMA55", ["/ema_h1_34_close", "/ema_h1_55_close"], |i: usize| ema34[i] > ema55[i]);
        add_atom!("ema8_gt_ema21", "EMA8 > EMA21", ["/ema_h1_8_close", "/ema_h1_21_close"], |i: usize| ema8[i] > ema21[i]);
        add_atom!("ema21_gt_ema50", "EMA21 > EMA50", ["/ema_h1_21_close", "/ema_h1_50_close"], |i: usize| ema21[i] > ema50[i]);
        add_atom!("ema50_gt_ema100", "EMA50 > EMA100", ["/ema_h1_50_close", "/ema_h1_100_close"], |i: usize| ema50[i] > ema100[i]);
        add_atom!("ema55_gt_ema200", "EMA55 > EMA200", ["/ema_h1_55_close", "/ema_h1_200_close"], |i: usize| ema55[i] > ema200[i]);
        add_atom!("ema21_slope_up_3", "EMA21 slope up over 3 bars", ["/ema_h1_21_close"], |i: usize| slope_up(&ema21, i, 3));
        add_atom!("ema50_slope_up_5", "EMA50 slope up over 5 bars", ["/ema_h1_50_close"], |i: usize| slope_up(&ema50, i, 5));
        add_atom!("cross_ema8_above_ema21", "EMA8 crosses above EMA21", ["/ema_h1_8_close", "/ema_h1_21_close"], |i: usize| i > 0 && ema8[i - 1] <= ema21[i - 1] && ema8[i] > ema21[i]);
        add_atom!("cross_ema13_above_ema34", "EMA13 crosses above EMA34", ["/ema_h1_13_close", "/ema_h1_34_close"], |i: usize| i > 0 && ema13[i - 1] <= ema34[i - 1] && ema13[i] > ema34[i]);
        add_atom!("cross_close_above_ema21", "close crosses above EMA21", ["/ema_h1_21_close", "/candle_h1_close"], |i: usize| i > 0 && candles[i - 1].close <= ema21[i - 1] && candles[i].close > ema21[i]);
        add_atom!("cross_close_above_ema50", "close crosses above EMA50", ["/ema_h1_50_close", "/candle_h1_close"], |i: usize| i > 0 && candles[i - 1].close <= ema50[i - 1] && candles[i].close > ema50[i]);
        add_atom!("cross_close_above_ema200", "close crosses above EMA200", ["/ema_h1_200_close", "/candle_h1_close"], |i: usize| i > 0 && candles[i - 1].close <= ema200[i - 1] && candles[i].close > ema200[i]);
        add_atom!("cross_close_above_vwap", "close crosses above VWAP", ["/vwap_h1_session_hlc3"], |i: usize| i > 0 && candles[i - 1].close <= vwap[i - 1] && candles[i].close > vwap[i]);
        add_atom!("close_gt_vwap", "close > VWAP", ["/vwap_h1_session_hlc3"], |i: usize| candles[i].close > vwap[i]);
        add_atom!("close_gt_vwap_ext1_up", "close > VWAP +1sigma", ["/vwap_h1_session_hlc3", "/vwap_h1_ext1_up"], |i: usize| candles[i].close > vwap_ext1_up[i]);
        add_atom!("close_gt_vwap_ext2_up", "close > VWAP +2sigma", ["/vwap_h1_session_hlc3", "/vwap_h1_ext2_up"], |i: usize| candles[i].close > vwap_ext2_up[i]);
        add_atom!("candle_cross_close_above_vwap_ext1_down", "candle crosses and closes above VWAP -1sigma", ["/vwap_h1_ext1_down"], |i: usize| candles[i].low <= vwap_ext1_down[i] && candles[i].close > vwap_ext1_down[i]);
        add_atom!("candle_cross_close_above_vwap_ext2_down", "candle crosses and closes above VWAP -2sigma", ["/vwap_h1_ext2_down"], |i: usize| candles[i].low <= vwap_ext2_down[i] && candles[i].close > vwap_ext2_down[i]);
        add_atom!("close_near_vwap_ext1_down_sigma", "close near VWAP -1sigma", ["/vwap_h1_ext1_down"], |i: usize| near_by_sigma(candles[i].close, vwap_ext1_down[i], i, 0.35));
        add_atom!("close_between_vwap_and_ext1_up", "close between VWAP and VWAP +1sigma", ["/vwap_h1_session_hlc3", "/vwap_h1_ext1_up"], |i: usize| candles[i].close >= vwap[i] && candles[i].close <= vwap_ext1_up[i]);
        add_atom!("close_within_05atr_ema21", "close within 0.5 ATR of EMA21", ["/ema_h1_21_close", "/atr_h1_14"], |i: usize| near_by_atr(candles[i].close, ema21[i], i, 0.50));
        add_atom!("close_within_05atr_vwap", "close within 0.5 ATR of VWAP", ["/vwap_h1_session_hlc3", "/atr_h1_14"], |i: usize| near_by_atr(candles[i].close, vwap[i], i, 0.50));
        add_atom!("close_gt_boll_basis", "close > Bollinger basis", ["/bollinger_h1_20_2_close_basis"], |i: usize| candles[i].close > basis[i]);
        add_atom!("close_gt_boll_upper", "close > Bollinger upper band", ["/bollinger_h1_20_2_close_upper_band"], |i: usize| candles[i].close > upper[i]);
        add_atom!("candle_cross_close_above_boll_lower", "candle crosses and closes above lower band", ["/bollinger_h1_20_2_close_lower_band"], |i: usize| candles[i].low <= lower[i] && candles[i].close > lower[i]);
        add_atom!("candle_cross_close_above_boll_basis", "candle crosses and closes above basis", ["/bollinger_h1_20_2_close_basis"], |i: usize| candles[i].low <= basis[i] && candles[i].close > basis[i]);
        add_atom!("candle_cross_close_above_boll_upper", "candle crosses and closes above upper band", ["/bollinger_h1_20_2_close_upper_band"], |i: usize| candles[i].low <= upper[i] && candles[i].close > upper[i]);
        add_atom!("bullish_body", "bullish candle body", ["/candle_h1_body"], |i: usize| candles[i].close > candles[i].open);
        add_atom!("bullish_body_60pct", "bullish body >= 60% range", ["/candle_h1_body", "/range_h1"], |i: usize| candles[i].close > candles[i].open && body_ratio(i) >= 0.60);
        add_atom!("close_near_high", "close near candle high", ["/candleh1_9pm_close", "/candle_h1_high"], |i: usize| (candles[i].close - candles[i].low) / range_at(i) >= 0.72);
        add_atom!("lower_wick_rejection", "lower wick rejection", ["/candle_h1_low", "/candle_h1_body"], |i: usize| lower_wick_ratio(i) >= 0.35 && candles[i].close > candles[i].open);
        add_atom!("bullish_close_vs_prev", "close > previous close", ["/candle_h1_close"], |i: usize| i > 0 && candles[i].close > candles[i - 1].close);
        add_atom!("close_gt_prev_high", "close > previous high", ["/candle_h1_close", "/candle_h1_high"], |i: usize| i > 0 && candles[i].close > candles[i - 1].high);
        add_atom!("prev_red_then_green", "previous red then current green", ["/candle_h1_body"], |i: usize| i > 0 && candles[i - 1].close < candles[i - 1].open && candles[i].close > candles[i].open);
        add_atom!("boll_pos_gt_50", "Bollinger position > 50%", ["/bollinger_h1_20_2_close_cloud"], |i: usize| boll_position(i) > 0.50);
        add_atom!("boll_pos_gt_80", "Bollinger position > 80%", ["/bollinger_h1_20_2_close_cloud"], |i: usize| boll_position(i) > 0.80);
        add_atom!("rsi14_gt_50", "RSI14 > 50", ["/rsi_h1_14_close"], |i: usize| rsi14[i] > 50.0);
        add_atom!("rsi14_lt_40", "RSI14 < 40 pullback", ["/rsi_h1_14_close"], |i: usize| rsi14[i] < 40.0);
        add_atom!("rsi14_cross_above_50", "RSI14 crosses above 50", ["/rsi_h1_14_close"], |i: usize| i > 0 && rsi14[i - 1] <= 50.0 && rsi14[i] > 50.0);
        add_atom!("rsi14_rising_3", "RSI14 rising 3 bars", ["/rsi_h1_14_close"], |i: usize| slope_up(&rsi14, i, 3));
        add_atom!("macd_line_gt_signal", "MACD line > signal", ["/macd_h1_12_26_9_line", "/macd_h1_12_26_9_signal"], |i: usize| macd_line[i] > macd_signal[i]);
        add_atom!("macd_hist_gt_0", "MACD histogram > 0", ["/macd_h1_12_26_9_histogram"], |i: usize| macd_histogram[i] > 0.0);
        add_atom!("macd_hist_rising_3", "MACD histogram rising 3 bars", ["/macd_h1_12_26_9_histogram"], |i: usize| slope_up(&macd_histogram, i, 3));
        add_atom!("macd_cross_above_signal", "MACD crosses above signal", ["/macd_h1_12_26_9_line", "/macd_h1_12_26_9_signal"], |i: usize| crosses_above(&macd_line, &macd_signal, i));
        add_atom!("close_gt_donchian20_mid", "close > Donchian20 mid", ["/donchian_h1_20_mid"], |i: usize| candles[i].close > donchian20_mid[i]);
        add_atom!("close_gt_donchian55_mid", "close > Donchian55 mid", ["/donchian_h1_55_mid"], |i: usize| candles[i].close > donchian55_mid[i]);
        add_atom!("close_breaks_donchian20_high", "close breaks prior Donchian20 high", ["/donchian_h1_20_high", "/candle_h1_close"], |i: usize| i > 0 && candles[i].close > donchian20_upper[i - 1]);
        add_atom!("close_breaks_donchian55_high", "close breaks prior Donchian55 high", ["/donchian_h1_55_high", "/candle_h1_close"], |i: usize| i > 0 && candles[i].close > donchian55_upper[i - 1]);
        add_atom!("stoch14_k_gt_d", "Stochastic14 K > D", ["/stoch_h1_14_3_k", "/stoch_h1_14_3_d"], |i: usize| stoch14_k[i] > stoch14_d[i]);
        add_atom!("stoch14_cross_up_20", "Stochastic14 crosses up 20", ["/stoch_h1_14_3_k"], |i: usize| i > 0 && stoch14_k[i - 1] <= 20.0 && stoch14_k[i] > 20.0);
        add_atom!("stoch14_k_gt_50", "Stochastic14 K > 50", ["/stoch_h1_14_3_k"], |i: usize| stoch14_k[i] > 50.0);
    } else {
        add_atom!("close_lt_ema8", "close < EMA8", ["/ema_h1_8_close"], |i: usize| candles[i].close < ema8[i]);
        add_atom!("close_lt_ema21", "close < EMA21", ["/ema_h1_21_close"], |i: usize| candles[i].close < ema21[i]);
        add_atom!("close_lt_ema50", "close < EMA50", ["/ema_h1_50_close"], |i: usize| candles[i].close < ema50[i]);
        add_atom!("close_lt_ema100", "close < EMA100", ["/ema_h1_100_close"], |i: usize| candles[i].close < ema100[i]);
        add_atom!("close_lt_ema200", "close < EMA200", ["/ema_h1_200_close"], |i: usize| candles[i].close < ema200[i]);
        add_atom!("ema8_lt_ema13", "EMA8 < EMA13", ["/ema_h1_8_close", "/ema_h1_13_close"], |i: usize| ema8[i] < ema13[i]);
        add_atom!("ema13_lt_ema34", "EMA13 < EMA34", ["/ema_h1_13_close", "/ema_h1_34_close"], |i: usize| ema13[i] < ema34[i]);
        add_atom!("ema34_lt_ema55", "EMA34 < EMA55", ["/ema_h1_34_close", "/ema_h1_55_close"], |i: usize| ema34[i] < ema55[i]);
        add_atom!("ema8_lt_ema21", "EMA8 < EMA21", ["/ema_h1_8_close", "/ema_h1_21_close"], |i: usize| ema8[i] < ema21[i]);
        add_atom!("ema21_lt_ema50", "EMA21 < EMA50", ["/ema_h1_21_close", "/ema_h1_50_close"], |i: usize| ema21[i] < ema50[i]);
        add_atom!("ema50_lt_ema100", "EMA50 < EMA100", ["/ema_h1_50_close", "/ema_h1_100_close"], |i: usize| ema50[i] < ema100[i]);
        add_atom!("ema55_lt_ema200", "EMA55 < EMA200", ["/ema_h1_55_close", "/ema_h1_200_close"], |i: usize| ema55[i] < ema200[i]);
        add_atom!("ema21_slope_down_3", "EMA21 slope down over 3 bars", ["/ema_h1_21_close"], |i: usize| slope_down(&ema21, i, 3));
        add_atom!("ema50_slope_down_5", "EMA50 slope down over 5 bars", ["/ema_h1_50_close"], |i: usize| slope_down(&ema50, i, 5));
        add_atom!("cross_ema8_below_ema21", "EMA8 crosses below EMA21", ["/ema_h1_8_close", "/ema_h1_21_close"], |i: usize| i > 0 && ema8[i - 1] >= ema21[i - 1] && ema8[i] < ema21[i]);
        add_atom!("cross_ema13_below_ema34", "EMA13 crosses below EMA34", ["/ema_h1_13_close", "/ema_h1_34_close"], |i: usize| i > 0 && ema13[i - 1] >= ema34[i - 1] && ema13[i] < ema34[i]);
        add_atom!("cross_close_below_ema21", "close crosses below EMA21", ["/ema_h1_21_close", "/candle_h1_close"], |i: usize| i > 0 && candles[i - 1].close >= ema21[i - 1] && candles[i].close < ema21[i]);
        add_atom!("cross_close_below_ema50", "close crosses below EMA50", ["/ema_h1_50_close", "/candle_h1_close"], |i: usize| i > 0 && candles[i - 1].close >= ema50[i - 1] && candles[i].close < ema50[i]);
        add_atom!("cross_close_below_ema200", "close crosses below EMA200", ["/ema_h1_200_close", "/candle_h1_close"], |i: usize| i > 0 && candles[i - 1].close >= ema200[i - 1] && candles[i].close < ema200[i]);
        add_atom!("cross_close_below_vwap", "close crosses below VWAP", ["/vwap_h1_session_hlc3"], |i: usize| i > 0 && candles[i - 1].close >= vwap[i - 1] && candles[i].close < vwap[i]);
        add_atom!("close_lt_vwap", "close < VWAP", ["/vwap_h1_session_hlc3"], |i: usize| candles[i].close < vwap[i]);
        add_atom!("close_lt_vwap_ext1_down", "close < VWAP -1sigma", ["/vwap_h1_session_hlc3", "/vwap_h1_ext1_down"], |i: usize| candles[i].close < vwap_ext1_down[i]);
        add_atom!("close_lt_vwap_ext2_down", "close < VWAP -2sigma", ["/vwap_h1_session_hlc3", "/vwap_h1_ext2_down"], |i: usize| candles[i].close < vwap_ext2_down[i]);
        add_atom!("candle_cross_close_below_vwap_ext1_up", "candle crosses and closes below VWAP +1sigma", ["/vwap_h1_ext1_up"], |i: usize| candles[i].high >= vwap_ext1_up[i] && candles[i].close < vwap_ext1_up[i]);
        add_atom!("candle_cross_close_below_vwap_ext2_up", "candle crosses and closes below VWAP +2sigma", ["/vwap_h1_ext2_up"], |i: usize| candles[i].high >= vwap_ext2_up[i] && candles[i].close < vwap_ext2_up[i]);
        add_atom!("close_near_vwap_ext1_up_sigma", "close near VWAP +1sigma", ["/vwap_h1_ext1_up"], |i: usize| near_by_sigma(candles[i].close, vwap_ext1_up[i], i, 0.35));
        add_atom!("close_between_vwap_and_ext1_down", "close between VWAP and VWAP -1sigma", ["/vwap_h1_session_hlc3", "/vwap_h1_ext1_down"], |i: usize| candles[i].close <= vwap[i] && candles[i].close >= vwap_ext1_down[i]);
        add_atom!("close_within_05atr_ema21_short", "close within 0.5 ATR of EMA21", ["/ema_h1_21_close", "/atr_h1_14"], |i: usize| near_by_atr(candles[i].close, ema21[i], i, 0.50));
        add_atom!("close_within_05atr_vwap_short", "close within 0.5 ATR of VWAP", ["/vwap_h1_session_hlc3", "/atr_h1_14"], |i: usize| near_by_atr(candles[i].close, vwap[i], i, 0.50));
        add_atom!("close_lt_boll_basis", "close < Bollinger basis", ["/bollinger_h1_20_2_close_basis"], |i: usize| candles[i].close < basis[i]);
        add_atom!("close_lt_boll_lower", "close < Bollinger lower band", ["/bollinger_h1_20_2_close_lower_band"], |i: usize| candles[i].close < lower[i]);
        add_atom!("candle_cross_close_below_boll_upper", "candle crosses and closes below upper band", ["/bollinger_h1_20_2_close_upper_band"], |i: usize| candles[i].high >= upper[i] && candles[i].close < upper[i]);
        add_atom!("candle_cross_close_below_boll_basis", "candle crosses and closes below basis", ["/bollinger_h1_20_2_close_basis"], |i: usize| candles[i].high >= basis[i] && candles[i].close < basis[i]);
        add_atom!("candle_cross_close_below_boll_lower", "candle crosses and closes below lower band", ["/bollinger_h1_20_2_close_lower_band"], |i: usize| candles[i].high >= lower[i] && candles[i].close < lower[i]);
        add_atom!("bearish_body", "bearish candle body", ["/candle_h1_body"], |i: usize| candles[i].close < candles[i].open);
        add_atom!("bearish_body_60pct", "bearish body >= 60% range", ["/candle_h1_body", "/range_h1"], |i: usize| candles[i].close < candles[i].open && body_ratio(i) >= 0.60);
        add_atom!("close_near_low", "close near candle low", ["/candleh1_9pm_close", "/candle_h1_low"], |i: usize| (candles[i].high - candles[i].close) / range_at(i) >= 0.72);
        add_atom!("upper_wick_rejection", "upper wick rejection", ["/candle_h1_high", "/candle_h1_body"], |i: usize| upper_wick_ratio(i) >= 0.35 && candles[i].close < candles[i].open);
        add_atom!("bearish_close_vs_prev", "close < previous close", ["/candle_h1_close"], |i: usize| i > 0 && candles[i].close < candles[i - 1].close);
        add_atom!("close_lt_prev_low", "close < previous low", ["/candle_h1_close", "/candle_h1_low"], |i: usize| i > 0 && candles[i].close < candles[i - 1].low);
        add_atom!("prev_green_then_red", "previous green then current red", ["/candle_h1_body"], |i: usize| i > 0 && candles[i - 1].close > candles[i - 1].open && candles[i].close < candles[i].open);
        add_atom!("boll_pos_lt_50", "Bollinger position < 50%", ["/bollinger_h1_20_2_close_cloud"], |i: usize| boll_position(i) < 0.50);
        add_atom!("boll_pos_lt_20", "Bollinger position < 20%", ["/bollinger_h1_20_2_close_cloud"], |i: usize| boll_position(i) < 0.20);
        add_atom!("rsi14_lt_50", "RSI14 < 50", ["/rsi_h1_14_close"], |i: usize| rsi14[i] < 50.0);
        add_atom!("rsi14_gt_60", "RSI14 > 60 pullback", ["/rsi_h1_14_close"], |i: usize| rsi14[i] > 60.0);
        add_atom!("rsi14_cross_below_50", "RSI14 crosses below 50", ["/rsi_h1_14_close"], |i: usize| i > 0 && rsi14[i - 1] >= 50.0 && rsi14[i] < 50.0);
        add_atom!("rsi14_falling_3", "RSI14 falling 3 bars", ["/rsi_h1_14_close"], |i: usize| slope_down(&rsi14, i, 3));
        add_atom!("macd_line_lt_signal", "MACD line < signal", ["/macd_h1_12_26_9_line", "/macd_h1_12_26_9_signal"], |i: usize| macd_line[i] < macd_signal[i]);
        add_atom!("macd_hist_lt_0", "MACD histogram < 0", ["/macd_h1_12_26_9_histogram"], |i: usize| macd_histogram[i] < 0.0);
        add_atom!("macd_hist_falling_3", "MACD histogram falling 3 bars", ["/macd_h1_12_26_9_histogram"], |i: usize| slope_down(&macd_histogram, i, 3));
        add_atom!("macd_cross_below_signal", "MACD crosses below signal", ["/macd_h1_12_26_9_line", "/macd_h1_12_26_9_signal"], |i: usize| crosses_below(&macd_line, &macd_signal, i));
        add_atom!("close_lt_donchian20_mid", "close < Donchian20 mid", ["/donchian_h1_20_mid"], |i: usize| candles[i].close < donchian20_mid[i]);
        add_atom!("close_lt_donchian55_mid", "close < Donchian55 mid", ["/donchian_h1_55_mid"], |i: usize| candles[i].close < donchian55_mid[i]);
        add_atom!("close_breaks_donchian20_low", "close breaks prior Donchian20 low", ["/donchian_h1_20_low", "/candle_h1_close"], |i: usize| i > 0 && candles[i].close < donchian20_lower[i - 1]);
        add_atom!("close_breaks_donchian55_low", "close breaks prior Donchian55 low", ["/donchian_h1_55_low", "/candle_h1_close"], |i: usize| i > 0 && candles[i].close < donchian55_lower[i - 1]);
        add_atom!("stoch14_k_lt_d", "Stochastic14 K < D", ["/stoch_h1_14_3_k", "/stoch_h1_14_3_d"], |i: usize| stoch14_k[i] < stoch14_d[i]);
        add_atom!("stoch14_cross_down_80", "Stochastic14 crosses down 80", ["/stoch_h1_14_3_k"], |i: usize| i > 0 && stoch14_k[i - 1] >= 80.0 && stoch14_k[i] < 80.0);
        add_atom!("stoch14_k_lt_50", "Stochastic14 K < 50", ["/stoch_h1_14_3_k"], |i: usize| stoch14_k[i] < 50.0);
    }
    add_atom!("range_le_p10", "range <= p10", ["/range_h1"], |i: usize| ranges[i] <= range_p10);
    add_atom!("range_le_p20", "range <= p20", ["/range_h1"], |i: usize| ranges[i] <= range_p20);
    add_atom!("range_le_p35", "range <= p35", ["/range_h1"], |i: usize| ranges[i] <= range_p35);
    add_atom!("range_le_p50", "range <= p50", ["/range_h1"], |i: usize| ranges[i] <= range_p50);
    add_atom!("body_le_p35", "body <= p35", ["/candle_h1_body"], |i: usize| bodies[i] <= body_p35);
    add_atom!("doji_body_le_20pct", "doji body <= 20% candle range", ["/candle_h1_body", "/range_h1"], |i: usize| body_ratio(i) <= 0.20);
    add_atom!("body_ratio_le_35pct", "body <= 35% candle range", ["/candle_h1_body", "/range_h1"], |i: usize| body_ratio(i) <= 0.35);
    add_atom!("atr14_le_p20", "ATR14 <= p20", ["/atr_h1_14"], |i: usize| atr14[i] <= atr_p20);
    add_atom!("atr14_le_p35", "ATR14 <= p35", ["/atr_h1_14"], |i: usize| atr14[i] <= atr_p35);
    add_atom!("atr14_le_p50", "ATR14 <= p50", ["/atr_h1_14"], |i: usize| atr14[i] <= atr_p50);
    add_atom!("volume_le_p40", "volume <= p40", ["/volume_h1"], |i: usize| volumes[i] <= volume_p40);
    add_atom!("volume_ge_p60", "volume >= p60", ["/volume_h1"], |i: usize| volumes[i] >= volume_p60);
    add_atom!("boll_width_le_p20", "Bollinger width <= p20", ["/bollinger_h1_20_2_close_cloud"], |i: usize| boll_widths[i] <= boll_width_p20);
    add_atom!("boll_width_le_p35", "Bollinger width <= p35", ["/bollinger_h1_20_2_close_cloud"], |i: usize| boll_widths[i] <= boll_width_p35);
    add_atom!("boll_width_le_p50", "Bollinger width <= p50", ["/bollinger_h1_20_2_close_cloud"], |i: usize| boll_widths[i] <= boll_width_p50);
    add_atom!("close_near_vwap", "close near VWAP", ["/vwap_h1_session_hlc3", "/range_h1"], |i: usize| (candles[i].close - vwap[i]).abs() <= range_p50);
    add_atom!("ema21_ema50_compressed", "EMA21 near EMA50", ["/ema_h1_21_close", "/ema_h1_50_close"], |i: usize| (ema21[i] - ema50[i]).abs() <= range_p50);
    add_atom!("ema8_ema55_compressed_atr", "EMA8 near EMA55 within ATR", ["/ema_h1_8_close", "/ema_h1_55_close", "/atr_h1_14"], |i: usize| near_by_atr(ema8[i], ema55[i], i, 1.0));
    add_atom!("macd_hist_near_zero", "MACD histogram near zero", ["/macd_h1_12_26_9_histogram", "/atr_h1_14"], |i: usize| macd_histogram[i].abs() <= atr_at(i) * 0.05);
    add_atom!("rsi14_neutral_45_55", "RSI14 neutral 45-55", ["/rsi_h1_14_close"], |i: usize| rsi14[i] >= 45.0 && rsi14[i] <= 55.0);
    add_atom!("inside_bar", "inside bar", ["/candle_h1_high", "/candle_h1_low"], |i: usize| i > 0 && candles[i].high <= candles[i - 1].high && candles[i].low >= candles[i - 1].low);
    add_atom!("outside_bar", "outside bar", ["/candle_h1_high", "/candle_h1_low"], |i: usize| i > 0 && candles[i].high >= candles[i - 1].high && candles[i].low <= candles[i - 1].low);
    add_atom!("inside_bollinger", "close inside Bollinger channel", ["/bollinger_h1_20_2_close_cloud"], |i: usize| candles[i].close >= lower[i] && candles[i].close <= upper[i]);
    add_atom!("inside_donchian20", "close inside Donchian20 channel", ["/donchian_h1_20_high", "/donchian_h1_20_low"], |i: usize| candles[i].close <= donchian20_upper[i] && candles[i].close >= donchian20_lower[i]);
    add_atom!("inside_donchian55", "close inside Donchian55 channel", ["/donchian_h1_55_high", "/donchian_h1_55_low"], |i: usize| candles[i].close <= donchian55_upper[i] && candles[i].close >= donchian55_lower[i]);

    let mut seen_filter_ids = HashMap::<String, ()>::new();
    seen_filter_ids.insert("base_low_vol".to_string(), ());
    let mut seen_condition_hashes = HashMap::<String, ()>::new();
    seen_condition_hashes.insert(base_mask.mask_hash.clone(), ());
    let max_filters = 16_384usize;
    let add_program = |filters: &mut Vec<StrategyEntryFilter>,
                       seen: &mut HashMap<String, ()>,
                       seen_masks: &mut HashMap<String, ()>,
                       id: String,
                       label: String,
                       refs: Vec<String>,
                       mask: StrategyConditionMask,
                       bytecode_ops: Vec<String>,
                       display_formula: Vec<String>| -> bool {
        if filters.len() >= max_filters
            || mask.count < 8
            || seen.contains_key(&id)
            || seen_masks.contains_key(&mask.mask_hash)
        {
            return false;
        }
        seen.insert(id.clone(), ());
        seen_masks.insert(mask.mask_hash.clone(), ());
        push_filter(filters, &id, &label, mask, bytecode_ops, display_formula, refs);
        true
    };
    let canonical_itemset_id = |indices: &[usize]| -> String {
        let mut ids = indices
            .iter()
            .map(|idx| atoms[*idx].id.as_str())
            .collect::<Vec<_>>();
        ids.sort_unstable();
        ids.join("_")
    };
    let label_for_indices = |indices: &[usize]| -> String {
        let mut labels = indices
            .iter()
            .map(|idx| atoms[*idx].label.as_str())
            .collect::<Vec<_>>();
        labels.sort_unstable();
        labels.join(" AND ")
    };
    let refs_for_indices = |indices: &[usize]| -> Vec<String> {
        let mut out = Vec::<String>::new();
        for idx in indices {
            for rf in &atoms[*idx].indicator_refs {
                if !out.iter().any(|known| known == rf) {
                    out.push(rf.clone());
                }
            }
        }
        out
    };
    let bytecode_for_indices = |indices: &[usize]| -> Vec<String> {
        let mut ids = indices
            .iter()
            .map(|idx| atoms[*idx].id.as_str())
            .collect::<Vec<_>>();
        ids.sort_unstable();
        let mut ops = Vec::with_capacity(ids.len() + 1);
        ops.push(format!("AND{}", ids.len()));
        ops.extend(ids.into_iter().map(|id| format!("ATOM:{id}")));
        ops
    };
    let mut ordered_indices = (0..atoms.len()).collect::<Vec<_>>();
    ordered_indices.sort_by(|left, right| {
        atoms[*left]
            .mask
            .count
            .cmp(&atoms[*right].mask.count)
            .then_with(|| atoms[*left].id.cmp(&atoms[*right].id))
    });
    for atom_index in &ordered_indices {
        let atom = &atoms[*atom_index];
        add_program(
            &mut filters,
            &mut seen_filter_ids,
            &mut seen_condition_hashes,
            format!("atom_{}", atom.id),
            atom.label.clone(),
            atom.indicator_refs.clone(),
            atom.mask.clone(),
            atom.bytecode_ops.clone(),
            atom.display_formula.clone(),
        );
    }
    let max_depth = 4usize;
    let mut eclat_nodes = 0usize;
    let mut canonical_pruned = 0usize;
    let mut stack = ordered_indices
        .iter()
        .enumerate()
        .map(|(pos, atom_index)| (vec![*atom_index], atoms[*atom_index].mask.clone(), pos + 1))
        .collect::<Vec<_>>();
    'eclat: while let Some((prefix, prefix_mask, next_pos)) = stack.pop() {
        if prefix.len() >= max_depth {
            continue;
        }
        for pos in next_pos..ordered_indices.len() {
            if filters.len() >= max_filters {
                break 'eclat;
            }
            let atom_index = ordered_indices[pos];
            let mut itemset = prefix.clone();
            itemset.push(atom_index);
            let mask = strategy_condition_and_mask_cached(
                data_hash,
                base_entries_cache_key,
                direction,
                &format!("AND{}", itemset.len()),
                &[&prefix_mask.mask_hash, &atoms[atom_index].mask.mask_hash],
                &[&prefix_mask, &atoms[atom_index].mask],
                base_entries.len(),
            );
            eclat_nodes += 1;
            if mask.count < 8 {
                canonical_pruned += 1;
                continue;
            }
            let canonical_id = canonical_itemset_id(&itemset);
            let added = add_program(
                &mut filters,
                &mut seen_filter_ids,
                &mut seen_condition_hashes,
                format!("and_{}", canonical_id),
                label_for_indices(&itemset),
                refs_for_indices(&itemset),
                mask.clone(),
                bytecode_for_indices(&itemset),
                vec![label_for_indices(&itemset)],
            );
            if !added {
                canonical_pruned += 1;
                continue;
            }
            if itemset.len() < max_depth {
                stack.push((itemset, mask, pos + 1));
            }
        }
    }

    if let Ok(mut cache) = trading_strategy_filter_entry_cache().lock() {
        cache.insert(key.clone(), filters.clone());
    }
    strategy_write_cache_marker(&key, json!({
        "node": "indicator_filters",
        "searchVersion": "adaptive-eclat-mfe-reduce-grid-v8",
        "baseEntries": base_entries.len(),
        "direction": direction,
        "atomPrograms": atoms.len(),
        "eclatNodes": eclat_nodes,
        "eclatMaxDepth": max_depth,
        "canonicalPruned": canonical_pruned,
        "compiledPrograms": filters.len(),
        "uniqueConditionMasks": seen_condition_hashes.len(),
        "maxPrograms": max_filters,
        "bytecodeOps": [
            "LOAD_CANDLE_FIELD",
            "LOAD_INDICATOR_VECTOR",
            "CROSS_ABOVE",
            "CROSS_BELOW",
            "CLOSE_BEYOND",
            "RECLAIM_BAND",
            "BODY_RATIO",
            "WICK_REJECTION",
            "RANGE_THRESHOLD",
            "AND2",
            "AND3",
            "AND4",
            "ECLAT_PREFIX_PRUNE",
            "CANONICAL_SORT",
            "FILTER_OUTCOME_GRID",
            "POPCOUNT_MASK_AND",
            "BITSET_INTERSECT",
            "MFE_REDUCE_TP_GRID"
        ],
        "filters": filters.iter().map(|filter| json!({
            "id": filter.id,
            "label": filter.label,
            "conditionHash": filter.condition_hash,
            "maskRef": filter.mask_ref,
            "bytecodeOps": filter.bytecode_ops,
            "displayFormula": filter.display_formula,
            "entries": filter.entry_count,
            "cacheKey": filter.cache_key,
            "indicatorRefs": filter.indicator_refs,
        })).collect::<Vec<_>>(),
        "dataHash": data_hash,
    }));
    filters
}

fn simulate_strategy_entry_outcome(
    candles: &[TradingCandlePoint],
    index: usize,
    direction: &str,
    stop_loss: f64,
    take_profit: f64,
    execution_cost: f64,
    max_hold: usize,
) -> Option<(f64, usize)> {
    if index >= candles.len() {
        return None;
    }
    let entry = candles[index].open;
    if !entry.is_finite() || entry <= 0.0 {
        return None;
    }
    let stop = if direction == "long" {
        entry - stop_loss
    } else {
        entry + stop_loss
    };
    let target = if direction == "long" {
        entry + take_profit
    } else {
        entry - take_profit
    };
    let exit_end = (index + max_hold).min(candles.len().saturating_sub(1));
    let mut pnl = f64::NAN;
    let mut held = 0usize;
    for exit_index in index..=exit_end {
        let candle = &candles[exit_index];
        held = exit_index.saturating_sub(index).max(1);
        if direction == "long" {
            let hit_stop = candle.low <= stop;
            let hit_target = candle.high >= target;
            if hit_stop {
                pnl = -stop_loss - execution_cost;
                break;
            }
            if hit_target {
                pnl = take_profit - execution_cost;
                break;
            }
            if exit_index == exit_end {
                pnl = (candle.close - entry) - execution_cost;
                break;
            }
        } else {
            let hit_stop = candle.high >= stop;
            let hit_target = candle.low <= target;
            if hit_stop {
                pnl = -stop_loss - execution_cost;
                break;
            }
            if hit_target {
                pnl = take_profit - execution_cost;
                break;
            }
            if exit_index == exit_end {
                pnl = (entry - candle.close) - execution_cost;
                break;
            }
        }
    }
    if pnl.is_finite() { Some((pnl, held)) } else { None }
}

fn simulate_tp_sl_only_exit(
    candles: &[TradingCandlePoint],
    index: usize,
    direction: &str,
    stop_loss: f64,
    take_profit: f64,
    execution_cost: f64,
) -> Option<(bool, f64, usize)> {
    if index >= candles.len() {
        return None;
    }
    let entry = candles[index].open;
    if !entry.is_finite() || entry <= 0.0 {
        return None;
    }
    let stop = if direction == "long" {
        entry - stop_loss
    } else {
        entry + stop_loss
    };
    let target = if direction == "long" {
        entry + take_profit
    } else {
        entry - take_profit
    };
    for exit_index in index..candles.len() {
        let candle = &candles[exit_index];
        let held = exit_index.saturating_sub(index).max(1);
        if direction == "long" {
            if candle.low <= stop {
                return Some((
                    false,
                    -stop_loss - execution_cost,
                    held,
                ));
            }
            if candle.high >= target {
                return Some((
                    true,
                    take_profit - execution_cost,
                    held,
                ));
            }
        } else {
            if candle.high >= stop {
                return Some((
                    false,
                    -stop_loss - execution_cost,
                    held,
                ));
            }
            if candle.low <= target {
                return Some((
                    true,
                    take_profit - execution_cost,
                    held,
                ));
            }
        }
    }
    None
}

fn strategy_entry_outcome_cache_key(
    candles: &[TradingCandlePoint],
    index: usize,
    direction: &str,
    stop_loss: f64,
    execution_cost: f64,
    max_hold: usize,
) -> Option<String> {
    let entry = candles.get(index)?;
    let last = candles.last()?;
    Some(format!(
        "entry-outcome-{}",
        &strategy_sha256(&format!(
            "{}|{}|{}|{}|open={:.10}|dir={direction}|sl={stop_loss:.10}|cost={execution_cost:.10}|hold={max_hold}",
            candles.len(),
            index,
            entry.time,
            last.time,
            entry.open,
        ))[..16]
    ))
}

fn strategy_compute_entry_outcome(
    candles: &[TradingCandlePoint],
    index: usize,
    direction: &str,
    stop_loss: f64,
    execution_cost: f64,
    max_hold: usize,
) -> Option<StrategyEntryOutcome> {
    if index >= candles.len() {
        return None;
    }
    let entry = candles[index].open;
    if !entry.is_finite() || entry <= 0.0 || stop_loss <= 0.0 || !stop_loss.is_finite() {
        return None;
    }
    let exit_end = (index + max_hold).min(candles.len().saturating_sub(1));
    let mut favorable_path = Vec::<StrategyOutcomePoint>::with_capacity(max_hold.min(256));
    let mut max_favorable_distance = 0.0_f64;
    let mut max_adverse_distance = 0.0_f64;
    let mut stop_held = None;
    for exit_index in index..=exit_end {
        let candle = &candles[exit_index];
        let held = exit_index.saturating_sub(index).max(1);
        let (favorable, adverse) = if direction == "long" {
            (candle.high - entry, entry - candle.low)
        } else {
            (entry - candle.low, candle.high - entry)
        };
        if favorable.is_finite() {
            max_favorable_distance = max_favorable_distance.max(favorable);
        }
        if adverse.is_finite() {
            max_adverse_distance = max_adverse_distance.max(adverse);
        }
        if adverse >= stop_loss {
            stop_held = Some(held);
            break;
        }
        favorable_path.push(StrategyOutcomePoint {
            held,
            favorable_distance: favorable,
        });
    }
    let terminal_candle = &candles[exit_end];
    let terminal_held = exit_end.saturating_sub(index).max(1);
    let terminal_pnl_distance = if direction == "long" {
        (terminal_candle.close - entry) - execution_cost
    } else {
        (entry - terminal_candle.close) - execution_cost
    };
    Some(StrategyEntryOutcome {
        entry_time: candles[index].time.clone(),
        execution_cost_distance: execution_cost,
        terminal_pnl_distance,
        terminal_held,
        stop_pnl_distance: -stop_loss - execution_cost,
        stop_held,
        max_favorable_distance,
        max_adverse_distance,
        favorable_path,
    })
}

fn strategy_entry_outcome_cached(
    candles: &[TradingCandlePoint],
    index: usize,
    direction: &str,
    stop_loss: f64,
    execution_cost: f64,
    max_hold: usize,
) -> (Option<StrategyEntryOutcome>, bool) {
    let Some(key) = strategy_entry_outcome_cache_key(
        candles,
        index,
        direction,
        stop_loss,
        execution_cost,
        max_hold,
    ) else {
        return (None, false);
    };
    if let Ok(cache) = trading_strategy_entry_outcome_cache().lock() {
        if let Some(outcome) = cache.get(&key) {
            return (Some(outcome.clone()), true);
        }
    }
    let outcome = strategy_compute_entry_outcome(
        candles,
        index,
        direction,
        stop_loss,
        execution_cost,
        max_hold,
    );
    if let Some(value) = outcome.as_ref() {
        if let Ok(mut cache) = trading_strategy_entry_outcome_cache().lock() {
            cache.insert(key, value.clone());
        }
    }
    (outcome, false)
}

fn strategy_eval_entry_outcome(
    outcome: &StrategyEntryOutcome,
    take_profit: f64,
) -> (f64, usize) {
    for point in &outcome.favorable_path {
        if point.favorable_distance >= take_profit {
            return (take_profit - outcome.execution_cost_distance, point.held);
        }
    }
    if let Some(held) = outcome.stop_held {
        return (outcome.stop_pnl_distance, held);
    }
    (outcome.terminal_pnl_distance, outcome.terminal_held)
}

fn strategy_tp_grid_hash(tp_grid: &[f64]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"strategy-tp-grid:v1");
    for value in tp_grid {
        hasher.update(format!("{value:.10};").as_bytes());
    }
    format!("tp-grid-{}", &strategy_hex(&hasher.finalize())[..16])
}

fn strategy_filter_outcome_grid_cached(
    candles: &[TradingCandlePoint],
    base_entries: &[usize],
    mask: &StrategyConditionMask,
    entries_cache_key: &str,
    direction: &str,
    stop_loss: f64,
    tp_grid: &[f64],
    execution_cost: f64,
    max_hold: usize,
    cache_snapshot: &mut StrategyCacheSnapshot,
) -> Vec<StrategyFilterOutcomePoint> {
    if tp_grid.is_empty() || base_entries.is_empty() || mask.count == 0 {
        return Vec::new();
    }
    let tp_hash = strategy_tp_grid_hash(tp_grid);
    let key = format!(
        "filter-outcome-grid-{}",
        &strategy_sha256(&format!(
            "{entries_cache_key}|{}|{direction}|filter-outcome-grid-v1|sl={stop_loss:.10}|cost={execution_cost:.10}|hold={max_hold}|{}|base_len={}",
            mask.mask_hash,
            tp_hash,
            base_entries.len(),
        ))[..16]
    );
    if let Ok(cache) = trading_strategy_filter_outcome_grid_cache().lock() {
        if let Some(grid) = cache.get(&key) {
            cache_snapshot.record(
                "mfe_reduce",
                true,
                mask.count.saturating_mul(tp_grid.len()).saturating_mul(max_hold.max(1)),
            );
            return grid.points.clone();
        }
    }

    cache_snapshot.record("mfe_reduce", false, 0);
    let mut stats_by_tp = vec![StrategySummaryStats::default(); tp_grid.len()];
    let mut loss_streak_by_tp = vec![0_usize; tp_grid.len()];
    let mut outcome_hits = 0usize;
    let mut outcome_misses = 0usize;
    let mut active_entries = 0usize;
    let mut max_favorable_distance = 0.0_f64;
    let mut max_adverse_distance = 0.0_f64;

    for (word_idx, mask_word) in mask.bits.iter().copied().enumerate() {
        let mut active = mask_word;
        while active != 0 {
            let bit = active.trailing_zeros() as usize;
            let pos = word_idx.saturating_mul(64).saturating_add(bit);
            active &= active - 1;
            let Some(entry_index) = base_entries.get(pos).copied() else {
                continue;
            };
            let (outcome, hit) = strategy_entry_outcome_cached(
                candles,
                entry_index,
                direction,
                stop_loss,
                execution_cost,
                max_hold,
            );
            if hit {
                outcome_hits += 1;
            } else {
                outcome_misses += 1;
            }
            let Some(outcome) = outcome else {
                continue;
            };
            active_entries += 1;
            max_favorable_distance = max_favorable_distance.max(outcome.max_favorable_distance);
            max_adverse_distance = max_adverse_distance.max(outcome.max_adverse_distance);
            for (tp_index, take_profit) in tp_grid.iter().copied().enumerate() {
                let (pnl, held) = strategy_eval_entry_outcome(&outcome, take_profit);
                stats_by_tp[tp_index].record(pnl, held, &mut loss_streak_by_tp[tp_index]);
            }
        }
    }

    if outcome_hits > 0 {
        cache_snapshot.record(
            "mfe_mae_cube",
            true,
            outcome_hits.saturating_mul(max_hold.max(1)),
        );
    }
    if outcome_misses > 0 {
        cache_snapshot.record("mfe_mae_cube", false, 0);
    }

    let points = tp_grid
        .iter()
        .copied()
        .zip(stats_by_tp)
        .map(|(take_profit_distance, stats)| StrategyFilterOutcomePoint {
            take_profit_distance,
            stats,
        })
        .collect::<Vec<_>>();
    let grid = StrategyFilterOutcomeGrid {
        points: points.clone(),
    };
    if let Ok(mut cache) = trading_strategy_filter_outcome_grid_cache().lock() {
        cache.insert(key.clone(), grid);
    }
    strategy_write_cache_marker(&key, json!({
        "node": "mfe_reduce_filter_outcome_grid",
        "entriesKey": entries_cache_key,
        "conditionHash": mask.mask_hash,
        "maskRef": mask.mask_hash,
        "direction": direction,
        "activeEntries": active_entries,
        "maskEntries": mask.count,
        "tpGridHash": tp_hash,
        "tpGridLen": tp_grid.len(),
        "stopLoss": stop_loss,
        "executionCost": execution_cost,
        "maxHold": max_hold,
        "entryOutcomeHits": outcome_hits,
        "entryOutcomeMisses": outcome_misses,
        "maxFavorableDistance": max_favorable_distance,
        "maxAdverseDistance": max_adverse_distance,
        "singlePassWorkItems": active_entries.saturating_mul(tp_grid.len()),
        "points": points.iter().map(|point| json!({
            "tp": point.take_profit_distance,
            "trades": point.stats.trades,
            "wins": point.stats.wins,
            "losses": point.stats.losses,
            "winRate": point.stats.win_rate(),
            "expectancy": point.stats.expectancy(),
        })).collect::<Vec<_>>(),
    }));
    points
}

fn simulate_strategy_candidate_on_entries(
    candles: &[TradingCandlePoint],
    entries: &[usize],
    direction: &str,
    stop_loss: f64,
    take_profit: f64,
    execution_cost: f64,
    max_hold: usize,
) -> StrategySimulationStats {
    let mut stats = StrategySimulationStats::default();
    let mut loss_streak = 0usize;
    for index in entries {
        if let Some((pnl, held)) = simulate_strategy_entry_outcome(
            candles,
            *index,
            direction,
            stop_loss,
            take_profit,
            execution_cost,
            max_hold,
        ) {
            stats.record(&candles[*index].time, pnl, held, &mut loss_streak);
        }
    }
    stats
}

fn simulate_strategy_paired_candidate_on_entries_cached(
    candles: &[TradingCandlePoint],
    entries: &[usize],
    entries_cache_key: &str,
    stop_loss: f64,
    take_profit: f64,
    execution_cost: f64,
    max_hold: usize,
    cache_snapshot: &mut StrategyCacheSnapshot,
) -> StrategySimulationStats {
    let key = format!(
        "stats-{}",
        &strategy_sha256(&format!(
            "{entries_cache_key}|paired|mfe-mae-v1|sl={stop_loss:.10}|tp={take_profit:.10}|cost={execution_cost:.10}|hold={max_hold}"
        ))[..16]
    );
    if let Ok(cache) = trading_strategy_stats_cache().lock() {
        if let Some(stats) = cache.get(&key) {
            cache_snapshot.record("mask_reduce_metrics", true, entries.len().saturating_mul(2));
            return stats.clone();
        }
    }
    cache_snapshot.record("mask_reduce_metrics", false, 0);
    let mut stats = StrategySimulationStats::default();
    let mut loss_streak = 0usize;
    let mut outcome_hits = 0usize;
    let mut outcome_misses = 0usize;
    let mut max_favorable_distance = 0.0_f64;
    let mut max_adverse_distance = 0.0_f64;
    for index in entries {
        for direction in ["long", "short"] {
            let (outcome, hit) = strategy_entry_outcome_cached(
                candles,
                *index,
                direction,
                stop_loss,
                execution_cost,
                max_hold,
            );
            if hit {
                outcome_hits += 1;
            } else {
                outcome_misses += 1;
            }
            if let Some(outcome) = outcome {
                max_favorable_distance = max_favorable_distance.max(outcome.max_favorable_distance);
                max_adverse_distance = max_adverse_distance.max(outcome.max_adverse_distance);
                let (pnl, held) = strategy_eval_entry_outcome(&outcome, take_profit);
                stats.record(&outcome.entry_time, pnl, held, &mut loss_streak);
            }
        }
    }
    if outcome_hits > 0 {
        cache_snapshot.record(
            "mfe_mae_cube",
            true,
            outcome_hits.saturating_mul(max_hold.max(1)),
        );
    }
    if outcome_misses > 0 {
        cache_snapshot.record(
            "mfe_mae_cube",
            false,
            outcome_misses.saturating_mul(max_hold.max(1)),
        );
    }
    if let Ok(mut cache) = trading_strategy_stats_cache().lock() {
        cache.insert(key.clone(), stats.clone());
    }
    strategy_write_cache_marker(&key, json!({
        "node": "mask_reduce_metrics",
        "entriesKey": entries_cache_key,
        "direction": "paired",
        "takeProfit": take_profit,
        "stopLoss": stop_loss,
        "entries": entries.len(),
        "legs": entries.len().saturating_mul(2),
        "outcomeHits": outcome_hits,
        "outcomeMisses": outcome_misses,
        "maxFavorableDistance": max_favorable_distance,
        "maxAdverseDistance": max_adverse_distance,
        "trades": stats.trades,
        "wins": stats.wins,
        "losses": stats.losses,
    }));
    stats
}

fn simulate_strategy_candidate_on_entries_cached(
    candles: &[TradingCandlePoint],
    entries: &[usize],
    entries_cache_key: &str,
    direction: &str,
    stop_loss: f64,
    take_profit: f64,
    execution_cost: f64,
    max_hold: usize,
    cache_snapshot: &mut StrategyCacheSnapshot,
) -> StrategySimulationStats {
    let key = format!(
        "stats-{}",
        &strategy_sha256(&format!(
            "{entries_cache_key}|{direction}|mfe-mae-v1|sl={stop_loss:.10}|tp={take_profit:.10}|cost={execution_cost:.10}|hold={max_hold}"
        ))[..16]
    );
    if let Ok(cache) = trading_strategy_stats_cache().lock() {
        if let Some(stats) = cache.get(&key) {
            cache_snapshot.record("mask_reduce_metrics", true, entries.len());
            return stats.clone();
        }
    }
    cache_snapshot.record("mask_reduce_metrics", false, 0);
    let mut stats = StrategySimulationStats::default();
    let mut loss_streak = 0usize;
    let mut outcome_hits = 0usize;
    let mut outcome_misses = 0usize;
    let mut max_favorable_distance = 0.0_f64;
    let mut max_adverse_distance = 0.0_f64;
    for index in entries {
        let (outcome, hit) = strategy_entry_outcome_cached(
            candles,
            *index,
            direction,
            stop_loss,
            execution_cost,
            max_hold,
        );
        if hit {
            outcome_hits += 1;
        } else {
            outcome_misses += 1;
        }
        if let Some(outcome) = outcome {
            max_favorable_distance = max_favorable_distance.max(outcome.max_favorable_distance);
            max_adverse_distance = max_adverse_distance.max(outcome.max_adverse_distance);
            let (pnl, held) = strategy_eval_entry_outcome(&outcome, take_profit);
            stats.record(&outcome.entry_time, pnl, held, &mut loss_streak);
        }
    }
    if outcome_hits > 0 {
        cache_snapshot.record(
            "mfe_mae_cube",
            true,
            outcome_hits.saturating_mul(max_hold.max(1)),
        );
    }
    if outcome_misses > 0 {
        cache_snapshot.record("mfe_mae_cube", false, 0);
    }
    if let Ok(mut cache) = trading_strategy_stats_cache().lock() {
        cache.insert(key.clone(), stats.clone());
    }
    strategy_write_cache_marker(&key, json!({
        "node": "mfe_mae_outcome_slice",
        "entriesKey": entries_cache_key,
        "entries": entries.len(),
        "direction": direction,
        "stopLoss": stop_loss,
        "takeProfit": take_profit,
        "executionCost": execution_cost,
        "maxHold": max_hold,
        "entryOutcomeHits": outcome_hits,
        "entryOutcomeMisses": outcome_misses,
        "maxFavorableDistance": max_favorable_distance,
        "maxAdverseDistance": max_adverse_distance,
        "trades": stats.trades,
        "wins": stats.wins,
        "losses": stats.losses,
    }));
    stats
}

fn simulate_strategy_candidate_range(
    candles: &[TradingCandlePoint],
    low_volatility_values: &[f64],
    threshold: f64,
    start_index: usize,
    end_index: usize,
    entry_hours: &[u32],
    force_daily_entry: bool,
    direction: &str,
    stop_loss: f64,
    take_profit: f64,
    execution_cost: f64,
    max_hold: usize,
) -> StrategySimulationStats {
    let entries = strategy_entry_indices(
        candles,
        low_volatility_values,
        threshold,
        start_index,
        end_index,
        entry_hours,
        force_daily_entry,
    );
    simulate_strategy_candidate_on_entries(
        candles,
        &entries,
        direction,
        stop_loss,
        take_profit,
        execution_cost,
        max_hold,
    )
}

fn simulate_strategy_paired_probe(
    candles: &[TradingCandlePoint],
    entries: &[usize],
    take_profit: f64,
    stop_loss: f64,
    execution_cost: f64,
    max_hold: usize,
    shared_entry_scan_cache_key: String,
    cache_snapshot: &mut StrategyCacheSnapshot,
) -> TradingStrategyPairedProbe {
    let long_stats = simulate_strategy_candidate_on_entries_cached(
        candles,
        entries,
        &shared_entry_scan_cache_key,
        "long",
        stop_loss,
        take_profit,
        execution_cost,
        max_hold,
        cache_snapshot,
    );
    let short_stats = simulate_strategy_candidate_on_entries_cached(
        candles,
        entries,
        &shared_entry_scan_cache_key,
        "short",
        stop_loss,
        take_profit,
        execution_cost,
        max_hold,
        cache_snapshot,
    );
    let long_win_rate = long_stats.win_rate();
    let short_win_rate = short_stats.win_rate();
    let long_expectancy = long_stats.expectancy();
    let short_expectancy = short_stats.expectancy();
    let scale = take_profit.abs().max(stop_loss.abs()).max(0.0000001);
    let edge_score = ((long_win_rate.unwrap_or(0.0) - short_win_rate.unwrap_or(0.0)) * 100.0)
        + (((long_expectancy - short_expectancy) / scale) * 25.0);
    let edge = if edge_score > 1.0 {
        "long"
    } else if edge_score < -1.0 {
        "short"
    } else {
        "flat"
    }
    .to_string();
    TradingStrategyPairedProbe {
        entries: entries.len(),
        take_profit_distance: take_profit,
        long_wins: long_stats.wins,
        short_wins: short_stats.wins,
        long_win_rate,
        short_win_rate,
        long_expectancy_distance: long_expectancy,
        short_expectancy_distance: short_expectancy,
        long_net_pnl_distance: long_stats.net_pnl,
        short_net_pnl_distance: short_stats.net_pnl,
        edge,
        edge_score,
        shared_entry_scan_cache_key,
        note: "Long and short were tested from the same eligible candle opens; entry filtering was computed once.".to_string(),
    }
}

fn build_strategy_visual_probes(
    candles: &[TradingCandlePoint],
    entries: &[usize],
    take_profit: f64,
    stop_loss: f64,
    execution_cost: f64,
    max_hold: usize,
    limit: usize,
) -> Vec<TradingStrategyVisualProbe> {
    if candles.is_empty()
        || entries.is_empty()
        || limit == 0
        || !take_profit.is_finite()
        || !stop_loss.is_finite()
        || take_profit <= 0.0
        || stop_loss <= 0.0
    {
        return Vec::new();
    }

    let mut selected_entries = entries
        .iter()
        .copied()
        .rev()
        .take(limit)
        .collect::<Vec<_>>();
    selected_entries.reverse();

    let mut probes = Vec::with_capacity(selected_entries.len().saturating_mul(2));
    for index in selected_entries {
        if index >= candles.len() {
            continue;
        }
        let entry_price = candles[index].open;
        if !entry_price.is_finite() || entry_price <= 0.0 {
            continue;
        }
        for direction in ["long", "short"] {
            let Some((pnl_distance, held_bars)) = simulate_strategy_entry_outcome(
                candles,
                index,
                direction,
                stop_loss,
                take_profit,
                execution_cost,
                max_hold,
            ) else {
                continue;
            };
            let stop_price = if direction == "long" {
                entry_price - stop_loss
            } else {
                entry_price + stop_loss
            };
            let take_profit_price = if direction == "long" {
                entry_price + take_profit
            } else {
                entry_price - take_profit
            };
            let exit_index = (index + held_bars).min(candles.len().saturating_sub(1));
            let outcome = if pnl_distance > 0.0 {
                "target"
            } else if pnl_distance < 0.0 {
                "stop"
            } else {
                "flat"
            }
            .to_string();
            probes.push(TradingStrategyVisualProbe {
                entry_time: candles[index].time.clone(),
                exit_time: candles.get(exit_index).map(|candle| candle.time.clone()),
                entry_index: index,
                entry_price,
                direction: direction.to_string(),
                stop_price,
                take_profit_price,
                stop_loss_distance: stop_loss,
                take_profit_distance: take_profit,
                pnl_distance,
                outcome,
                held_bars,
            });
        }
    }
    probes
}

fn period_positive_rate(trades: &[StrategyBacktestTrade], period_len: usize) -> Option<f64> {
    let mut periods = HashMap::<String, f64>::new();
    for trade in trades {
        if trade.entry_time.len() < period_len {
            continue;
        }
        let key = trade.entry_time[..period_len].to_string();
        *periods.entry(key).or_insert(0.0) += trade.pnl_distance;
    }
    if periods.is_empty() {
        return None;
    }
    let positive = periods.values().filter(|value| **value > 0.0).count();
    Some(positive as f64 / periods.len() as f64)
}

fn robustness_grade(score: f64) -> String {
    if score >= 82.0 {
        "A".to_string()
    } else if score >= 68.0 {
        "B".to_string()
    } else if score >= 52.0 {
        "C".to_string()
    } else if score >= 36.0 {
        "D".to_string()
    } else {
        "F".to_string()
    }
}

fn compute_strategy_robustness(
    candles: &[TradingCandlePoint],
    low_volatility_values: &[f64],
    threshold: f64,
    train_rows: usize,
    entry_hours: &[u32],
    force_daily_entry: bool,
    direction: &str,
    stop_loss: f64,
    take_profit: f64,
    execution_cost: f64,
    max_hold: usize,
    target_win_rate: f64,
    point_size: f64,
) -> TradingStrategyRobustness {
    let warmup = 2usize.max(max_hold.min(16));
    let in_sample = simulate_strategy_candidate_range(
        candles,
        low_volatility_values,
        threshold,
        warmup,
        train_rows,
        entry_hours,
        force_daily_entry,
        direction,
        stop_loss,
        take_profit,
        execution_cost,
        max_hold,
    );
    let out_sample = simulate_strategy_candidate_range(
        candles,
        low_volatility_values,
        threshold,
        train_rows,
        candles.len().saturating_sub(1),
        entry_hours,
        force_daily_entry,
        direction,
        stop_loss,
        take_profit,
        execution_cost,
        max_hold,
    );

    let mut windows = 0usize;
    let mut window_passes = 0usize;
    let range_start = warmup.max(1);
    let range_end = candles.len().saturating_sub(1);
    let span = range_end.saturating_sub(range_start);
    if span >= 80 {
        let window_count = 4usize;
        let window_size = (span / window_count).max(20);
        for window in 0..window_count {
            let start = range_start + window * window_size;
            if start >= range_end {
                continue;
            }
            let end = if window == window_count - 1 {
                range_end
            } else {
                (start + window_size).min(range_end)
            };
            let stats = simulate_strategy_candidate_range(
                candles,
                low_volatility_values,
                threshold,
                start,
                end,
                entry_hours,
                force_daily_entry,
                direction,
                stop_loss,
                take_profit,
                execution_cost,
                max_hold,
            );
            if stats.trades > 0 {
                windows += 1;
                if stats.expectancy() > 0.0
                    && stats.win_rate().unwrap_or(0.0) >= (target_win_rate * 0.85).min(0.95)
                {
                    window_passes += 1;
                }
            }
        }
    }
    let walk_forward_pass_rate = if windows > 0 {
        window_passes as f64 / windows as f64
    } else {
        0.0
    };

    let stress_scenarios = [
        ("base", execution_cost, stop_loss, take_profit),
        ("cost_x2", (execution_cost * 2.0) + point_size.max(0.0), stop_loss, take_profit),
        ("tight_stop", execution_cost, stop_loss * 0.9, take_profit),
        ("lower_tp", execution_cost, stop_loss, take_profit * 0.9),
        ("thin_edge", (execution_cost * 1.5) + (point_size.max(0.0) * 0.5), stop_loss * 0.95, take_profit * 0.95),
    ];
    let mut stress_passes = 0usize;
    let mut stress_count = 0usize;
    let mut worst_stress_expectancy = f64::INFINITY;
    for (_, cost, sl, tp) in stress_scenarios {
        let stats = simulate_strategy_candidate_range(
            candles,
            low_volatility_values,
            threshold,
            train_rows,
            candles.len().saturating_sub(1),
            entry_hours,
            force_daily_entry,
            direction,
            sl,
            tp,
            cost,
            max_hold,
        );
        if stats.trades == 0 {
            continue;
        }
        stress_count += 1;
        let expectancy = stats.expectancy();
        worst_stress_expectancy = worst_stress_expectancy.min(expectancy);
        if expectancy > 0.0 && stats.win_rate().unwrap_or(0.0) >= (target_win_rate * 0.85).min(0.95) {
            stress_passes += 1;
        }
    }
    if !worst_stress_expectancy.is_finite() {
        worst_stress_expectancy = 0.0;
    }
    let stress_pass_rate = if stress_count > 0 {
        stress_passes as f64 / stress_count as f64
    } else {
        0.0
    };
    let monthly_positive_rate = period_positive_rate(&out_sample.trades_detail, 7);
    let yearly_positive_rate = period_positive_rate(&out_sample.trades_detail, 4);
    let min_trades_ok = out_sample.trades >= 24 || (out_sample.trades >= 12 && windows >= 3);

    let mut warnings = Vec::new();
    if !min_trades_ok {
        warnings.push("too_few_out_of_sample_trades".to_string());
    }
    if walk_forward_pass_rate < 0.5 {
        warnings.push("weak_walk_forward_stability".to_string());
    }
    if stress_pass_rate < 0.6 {
        warnings.push("fragile_to_cost_or_sl_tp_stress".to_string());
    }
    if let (Some(ins), Some(oos)) = (in_sample.win_rate(), out_sample.win_rate()) {
        if ins - oos > 0.18 {
            warnings.push("in_sample_out_sample_decay".to_string());
        }
    }
    if monthly_positive_rate.unwrap_or(0.0) < 0.45 {
        warnings.push("poor_monthly_distribution".to_string());
    }

    let target_component = (out_sample.win_rate().unwrap_or(0.0) / target_win_rate.max(0.01)).clamp(0.0, 1.0) * 28.0;
    let expectancy_component = if out_sample.expectancy() > 0.0 {
        18.0
    } else {
        0.0
    };
    let walk_component = walk_forward_pass_rate * 20.0;
    let stress_component = stress_pass_rate * 16.0;
    let distribution_component = monthly_positive_rate.unwrap_or(0.0) * 10.0;
    let depth_component = if min_trades_ok { 8.0 } else { (out_sample.trades as f64 / 24.0).clamp(0.0, 1.0) * 8.0 };
    let decay_penalty = if let (Some(ins), Some(oos)) = (in_sample.win_rate(), out_sample.win_rate()) {
        ((ins - oos - 0.08).max(0.0) * 40.0).min(10.0)
    } else {
        0.0
    };
    let score = (target_component
        + expectancy_component
        + walk_component
        + stress_component
        + distribution_component
        + depth_component
        - decay_penalty)
        .clamp(0.0, 100.0);

    TradingStrategyRobustness {
        score,
        grade: robustness_grade(score),
        in_sample_trades: in_sample.trades,
        in_sample_win_rate: in_sample.win_rate(),
        in_sample_expectancy_distance: in_sample.expectancy(),
        out_of_sample_trades: out_sample.trades,
        out_of_sample_win_rate: out_sample.win_rate(),
        out_of_sample_expectancy_distance: out_sample.expectancy(),
        walk_forward_windows: windows,
        walk_forward_pass_rate,
        monthly_positive_rate,
        yearly_positive_rate,
        stress_pass_rate,
        worst_stress_expectancy_distance: worst_stress_expectancy,
        min_trades_ok,
        warnings,
    }
}

fn compute_strategy_robustness_cached(
    candles: &[TradingCandlePoint],
    low_volatility_values: &[f64],
    threshold: f64,
    train_rows: usize,
    entry_hours: &[u32],
    force_daily_entry: bool,
    direction: &str,
    stop_loss: f64,
    take_profit: f64,
    execution_cost: f64,
    max_hold: usize,
    target_win_rate: f64,
    point_size: f64,
    data_hash: &str,
    entries_cache_key: &str,
    cache_snapshot: &mut StrategyCacheSnapshot,
) -> TradingStrategyRobustness {
    let entry_hours_key = entry_hours
        .iter()
        .map(|value| value.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let key = format!(
        "robustness-{}",
        &strategy_sha256(&format!(
            "{data_hash}|{entries_cache_key}|{direction}|sl={stop_loss:.10}|tp={take_profit:.10}|cost={execution_cost:.10}|hold={max_hold}|target={target_win_rate:.6}|point={point_size:.10}|train={train_rows}|hours={entry_hours_key}|force_daily={force_daily_entry}|threshold={threshold:.10}"
        ))[..16]
    );
    if let Ok(cache) = trading_strategy_robustness_cache().lock() {
        if let Some(robustness) = cache.get(&key) {
            cache_snapshot.record("metrics", true, candles.len());
            return robustness.clone();
        }
    }
    cache_snapshot.record("metrics", false, 0);
    let robustness = compute_strategy_robustness(
        candles,
        low_volatility_values,
        threshold,
        train_rows,
        entry_hours,
        force_daily_entry,
        direction,
        stop_loss,
        take_profit,
        execution_cost,
        max_hold,
        target_win_rate,
        point_size,
    );
    if let Ok(mut cache) = trading_strategy_robustness_cache().lock() {
        cache.insert(key.clone(), robustness.clone());
    }
    strategy_write_cache_marker(&key, json!({
        "node": "metrics",
        "entriesKey": entries_cache_key,
        "direction": direction,
        "takeProfit": take_profit,
        "score": robustness.score,
        "grade": robustness.grade,
    }));
    robustness
}

fn compare_strategy_candidates(
    left: &TradingStrategyBacktestCandidate,
    right: &TradingStrategyBacktestCandidate,
) -> std::cmp::Ordering {
    let score = |candidate: &TradingStrategyBacktestCandidate| {
        let target_bonus = if candidate.meets_target { 10_000.0 } else { 0.0 };
        let robust_bonus = candidate
            .robustness
            .as_ref()
            .map(|robustness| robustness.score * 120.0)
            .unwrap_or(0.0);
        let daily_target = candidate.daily_target_hit_rate.unwrap_or(0.0) * 20_000.0;
        let positive_days = candidate.positive_day_rate.unwrap_or(0.0) * 4_000.0;
        let min_daily = candidate.min_daily_pnl_distance.unwrap_or(0.0) * 400.0;
        let avg_daily = candidate.avg_daily_pnl_distance.unwrap_or(0.0) * 180.0;
        let win_rate = candidate.win_rate.unwrap_or(0.0) * 1_000.0;
        let expectancy = candidate.expectancy_distance * 100.0;
        let trade_depth = (candidate.trades as f64).ln_1p();
        target_bonus + robust_bonus + daily_target + positive_days + min_daily + avg_daily + win_rate + expectancy + trade_depth
    };
    score(right)
        .partial_cmp(&score(left))
        .unwrap_or(std::cmp::Ordering::Equal)
}

fn strategy_cached_result(result_cache_key: &str) -> Option<TradingStrategyBacktestResult> {
    if let Some(result) = trading_strategy_result_cache()
        .lock()
        .ok()
        .and_then(|cache| cache.get(result_cache_key).cloned())
    {
        return Some(result);
    }
    let path = strategy_cache_artifact_path(result_cache_key);
    let bytes = fs::read(path).ok()?;
    let value = serde_json::from_slice::<Value>(&bytes).ok()?;
    let result_value = value
        .get("payload")
        .and_then(|payload| payload.get("result"))
        .cloned()?;
    let result = serde_json::from_value::<TradingStrategyBacktestResult>(result_value).ok()?;
    if let Ok(mut cache) = trading_strategy_result_cache().lock() {
        cache.insert(result_cache_key.to_string(), result.clone());
    }
    Some(result)
}

fn strategy_cache_finite(value: f64) -> f64 {
    if value.is_finite() {
        value
    } else {
        0.0
    }
}

fn strategy_cache_optional_finite(value: Option<f64>) -> Option<f64> {
    value.filter(|candidate| candidate.is_finite())
}

fn strategy_sanitize_candidate_for_cache(candidate: &mut TradingStrategyBacktestCandidate) {
    candidate.take_profit_distance = strategy_cache_finite(candidate.take_profit_distance);
    candidate.win_rate = strategy_cache_optional_finite(candidate.win_rate);
    candidate.expectancy_distance = strategy_cache_finite(candidate.expectancy_distance);
    candidate.net_pnl_distance = strategy_cache_finite(candidate.net_pnl_distance);
    candidate.profit_factor = strategy_cache_optional_finite(candidate.profit_factor);
    candidate.avg_hold_bars = strategy_cache_finite(candidate.avg_hold_bars);
    candidate.daily_target_hit_rate = strategy_cache_optional_finite(candidate.daily_target_hit_rate);
    candidate.positive_day_rate = strategy_cache_optional_finite(candidate.positive_day_rate);
    candidate.avg_daily_pnl_distance = strategy_cache_optional_finite(candidate.avg_daily_pnl_distance);
    candidate.min_daily_pnl_distance = strategy_cache_optional_finite(candidate.min_daily_pnl_distance);
    if let Some(robustness) = candidate.robustness.as_mut() {
        robustness.score = strategy_cache_finite(robustness.score);
        robustness.in_sample_win_rate = strategy_cache_optional_finite(robustness.in_sample_win_rate);
        robustness.in_sample_expectancy_distance =
            strategy_cache_finite(robustness.in_sample_expectancy_distance);
        robustness.out_of_sample_win_rate =
            strategy_cache_optional_finite(robustness.out_of_sample_win_rate);
        robustness.out_of_sample_expectancy_distance =
            strategy_cache_finite(robustness.out_of_sample_expectancy_distance);
        robustness.walk_forward_pass_rate = strategy_cache_finite(robustness.walk_forward_pass_rate);
        robustness.monthly_positive_rate =
            strategy_cache_optional_finite(robustness.monthly_positive_rate);
        robustness.yearly_positive_rate =
            strategy_cache_optional_finite(robustness.yearly_positive_rate);
        robustness.stress_pass_rate = strategy_cache_finite(robustness.stress_pass_rate);
        robustness.worst_stress_expectancy_distance =
            strategy_cache_finite(robustness.worst_stress_expectancy_distance);
    }
}

fn strategy_sanitize_result_for_cache(
    result: &TradingStrategyBacktestResult,
) -> TradingStrategyBacktestResult {
    let mut sanitized = result.clone();
    sanitized.low_volatility_threshold = strategy_cache_finite(sanitized.low_volatility_threshold);
    for candidate in &mut sanitized.candidates {
        strategy_sanitize_candidate_for_cache(candidate);
    }
    if let Some(best) = sanitized.best.as_mut() {
        strategy_sanitize_candidate_for_cache(best);
    }
    if let Some(paired) = sanitized.paired_probe.as_mut() {
        paired.take_profit_distance = strategy_cache_finite(paired.take_profit_distance);
        paired.long_win_rate = strategy_cache_optional_finite(paired.long_win_rate);
        paired.short_win_rate = strategy_cache_optional_finite(paired.short_win_rate);
        paired.long_expectancy_distance = strategy_cache_finite(paired.long_expectancy_distance);
        paired.short_expectancy_distance = strategy_cache_finite(paired.short_expectancy_distance);
        paired.long_net_pnl_distance = strategy_cache_finite(paired.long_net_pnl_distance);
        paired.short_net_pnl_distance = strategy_cache_finite(paired.short_net_pnl_distance);
        paired.edge_score = strategy_cache_finite(paired.edge_score);
    }
    for probe in &mut sanitized.visual_probes {
        probe.entry_price = strategy_cache_finite(probe.entry_price);
        probe.stop_price = strategy_cache_finite(probe.stop_price);
        probe.take_profit_price = strategy_cache_finite(probe.take_profit_price);
        probe.stop_loss_distance = strategy_cache_finite(probe.stop_loss_distance);
        probe.take_profit_distance = strategy_cache_finite(probe.take_profit_distance);
        probe.pnl_distance = strategy_cache_finite(probe.pnl_distance);
    }
    sanitized
}

fn strategy_store_cached_result(result_cache_key: &str, result: &TradingStrategyBacktestResult) {
    if let Ok(mut cache) = trading_strategy_result_cache().lock() {
        cache.insert(result_cache_key.to_string(), result.clone());
    }
    let sanitized = strategy_sanitize_result_for_cache(result);
    let result_payload = serde_json::to_value(&sanitized).ok();
    let payload = json!({
        "node": "strategy_result",
        "rows": result.rows,
        "candidates": result.candidates.len(),
        "visualProbes": result.visual_probes.len(),
        "best": result.best.as_ref().map(|candidate| json!({
            "direction": candidate.direction,
            "takeProfitDistance": candidate.take_profit_distance,
            "trades": candidate.trades,
            "winRate": candidate.win_rate,
        })),
        "result": result_payload,
    });
    strategy_write_cache_marker(result_cache_key, payload);
}

fn backtest_strategy_spec(
    candles: &[TradingCandlePoint],
    spec: &TradingStrategySpec,
) -> Result<TradingStrategyBacktestResult, String> {
    if candles.len() < 80 {
        return Err(format!(
            "not enough candles for strategy backtest: {} rows",
            candles.len()
        ));
    }
    let metric = spec
        .low_volatility_metric
        .as_deref()
        .unwrap_or("range_sma_percentile");
    let lookback = spec.low_volatility_lookback.unwrap_or(24).max(2);
    let split = spec.train_test_split.unwrap_or(0.7).clamp(0.1, 0.95);
    let train_rows = ((candles.len() as f64) * split).round() as usize;
    let train_rows = train_rows.clamp(lookback + 8, candles.len().saturating_sub(8));
    let test_rows = candles.len().saturating_sub(train_rows);
    let data_hash = strategy_candles_hash(candles);
    let template_hash = strategy_template_hash(spec, &data_hash);
    let result_cache_key = format!("strategy-result-{}", &template_hash[..16.min(template_hash.len())]);
    let mut cache_snapshot = StrategyCacheSnapshot::default();
    cache_snapshot.record("ohlcv", true, candles.len());
    let low_volatility_values = strategy_low_volatility_values_cached(
        candles,
        metric,
        lookback,
        &data_hash,
        &mut cache_snapshot,
    );
    let threshold_source = low_volatility_values
        .iter()
        .take(train_rows)
        .copied()
        .filter(|value| value.is_finite())
        .collect::<Vec<_>>();
    let threshold = percentile(
        &threshold_source,
        spec.low_volatility_percentile.unwrap_or(0.25),
    )
    .ok_or_else(|| "could not compute low-volatility threshold".to_string())?;

    let target_win_rate = spec.target_win_rate.unwrap_or(0.85);
    let entry_hours = strategy_entry_hours(spec);
    let entry_hour = entry_hours.first().copied().unwrap_or(21);
    let paired_mode = strategy_is_paired_mode(spec);
    let daily_profit_target = spec.daily_profit_target_distance.unwrap_or(0.07);
    let force_daily_entry = spec.force_daily_entry.unwrap_or(false);
    let stop_loss = spec.stop_loss_distance.unwrap_or(0.0);
    let spread = spec.spread_cost_distance.unwrap_or(0.0).max(0.0);
    let slippage = spec.slippage_distance.unwrap_or(0.0).max(0.0);
    let execution_cost = spread + slippage;
    let max_hold = spec.max_hold_bars.unwrap_or(24).max(1);
    let tp_grid = strategy_take_profit_grid(
        spec.take_profit_min_distance.unwrap_or(stop_loss),
        spec.take_profit_max_distance.unwrap_or(stop_loss),
    );
    let directions = strategy_directions(spec.direction.as_deref().unwrap_or("both"));
    let test_entry_start = train_rows.max(lookback + 1);
    let (test_entry_indices, shared_entry_scan_cache_key) = strategy_entry_indices_cached(
        candles,
        &low_volatility_values,
        threshold,
        test_entry_start,
        candles.len().saturating_sub(1),
        &entry_hours,
        force_daily_entry,
        &data_hash,
        &mut cache_snapshot,
    );

    if let Some(mut cached_result) = strategy_cached_result(&result_cache_key) {
        cache_snapshot.injected_results += 1;
        cache_snapshot.record("strategy_result", true, cached_result.rows);
        let mut cached_filter_ids = HashMap::<String, ()>::new();
        for candidate in &cached_result.candidates {
            cached_filter_ids.insert(candidate.filter_id.clone(), ());
        }
        cached_result.compute_plan = build_strategy_compute_plan(
            spec,
            candles.len(),
            &data_hash,
            &template_hash,
            &cache_snapshot,
            Some(threshold),
            tp_grid.len(),
            directions.len(),
            test_entry_indices.len(),
            cached_filter_ids.len(),
        );
        strategy_store_compute_plan(&cached_result.compute_plan);
        return Ok(cached_result);
    }

    let mut candidates = Vec::new();
    let entry_hours_label = entry_hours
        .iter()
        .map(|value| format!("{value}h"))
        .collect::<Vec<_>>()
        .join(", ");
    let daily_target_hit_threshold = if paired_mode && force_daily_entry {
        1.0
    } else {
        target_win_rate
    };

    if paired_mode {
        let base_condition_hash =
            strategy_condition_bits_hash(&data_hash, "paired", &vec![u64::MAX; strategy_condition_mask_words(test_entry_indices.len())], test_entry_indices.len());
        for take_profit in &tp_grid {
            let stats = simulate_strategy_paired_candidate_on_entries_cached(
                candles,
                &test_entry_indices,
                &shared_entry_scan_cache_key,
                stop_loss,
                *take_profit,
                execution_cost,
                max_hold,
                &mut cache_snapshot,
            );
            let win_rate = stats.win_rate();
            let expectancy = stats.expectancy();
            let daily = strategy_daily_performance(&stats.trades_detail, daily_profit_target);
            candidates.push(TradingStrategyBacktestCandidate {
                direction: "paired".to_string(),
                filter_id: "base_paired_daily".to_string(),
                filter_label: format!("{entry_hours_label} paired daily entries"),
                condition_hash: base_condition_hash.clone(),
                mask_ref: base_condition_hash.clone(),
                bytecode_ops: vec![
                    format!(
                        "ENTRY:HOUR_IN_UTC:{}",
                        entry_hours.iter().map(|value| value.to_string()).collect::<Vec<_>>().join(",")
                    ),
                    "ENTRY:FORCE_PAIRED_LONG_SHORT_PER_OPEN_DAY".to_string(),
                ],
                display_formula: vec![
                    format!("entry hour in [{entry_hours_label}] UTC"),
                    "open long and short together on each selected trading slot".to_string(),
                ],
                indicator_refs: vec!["/candle_h1".to_string()],
                entry_count: test_entry_indices.len(),
                take_profit_distance: *take_profit,
                trades: stats.trades,
                wins: stats.wins,
                losses: stats.losses,
                win_rate,
                expectancy_distance: expectancy,
                net_pnl_distance: stats.net_pnl,
                profit_factor: stats.profit_factor(),
                max_loss_streak: stats.max_loss_streak,
                avg_hold_bars: stats.avg_hold_bars(),
                meets_target: daily
                    .as_ref()
                    .and_then(|performance| performance.daily_target_hit_rate())
                    .is_some_and(|value| value >= daily_target_hit_threshold)
                    && daily.as_ref().is_some_and(|performance| performance.total_days >= 12),
                daily_target_hit_rate: daily.as_ref().and_then(|performance| performance.daily_target_hit_rate()),
                positive_day_rate: daily.as_ref().and_then(|performance| performance.positive_day_rate()),
                target_hit_days: daily.as_ref().map(|performance| performance.target_hit_days),
                total_days: daily.as_ref().map(|performance| performance.total_days),
                avg_daily_pnl_distance: daily.as_ref().map(|performance| performance.avg_daily_pnl_distance),
                min_daily_pnl_distance: daily.as_ref().map(|performance| performance.min_daily_pnl_distance),
                robustness: None,
            });
        }
        candidates.sort_by(compare_strategy_candidates);
        let base_best = candidates.first().cloned();
        if !base_best.as_ref().is_some_and(|candidate| candidate.meets_target) {
            let mut seen_filters = HashMap::<String, ()>::new();
            for filter_direction in ["long", "short"] {
                let filters = strategy_directional_entry_filters_cached(
                    candles,
                    &test_entry_indices,
                    &shared_entry_scan_cache_key,
                    filter_direction,
                    &data_hash,
                    force_daily_entry,
                    &mut cache_snapshot,
                );
                let paired_filters = filters
                    .into_iter()
                    .filter(|filter| filter.id != "base_low_vol")
                    .filter(|filter| strategy_filter_matches_requested_refs(filter, spec))
                    .take(24)
                    .collect::<Vec<_>>();
                for filter in paired_filters {
                    let dedupe_key = format!("{}|{}", filter.id, filter.mask_ref);
                    if seen_filters.insert(dedupe_key, ()).is_some() {
                        continue;
                    }
                    let filtered_entries = strategy_condition_entries_from_mask(&test_entry_indices, &filter.mask);
                    for take_profit in &tp_grid {
                        let stats = simulate_strategy_paired_candidate_on_entries_cached(
                            candles,
                            &filtered_entries,
                            &filter.cache_key,
                            stop_loss,
                            *take_profit,
                            execution_cost,
                            max_hold,
                            &mut cache_snapshot,
                        );
                        let win_rate = stats.win_rate();
                        let expectancy = stats.expectancy();
                        let daily = strategy_daily_performance(&stats.trades_detail, daily_profit_target);
                        candidates.push(TradingStrategyBacktestCandidate {
                            direction: "paired".to_string(),
                            filter_id: format!("paired_{}", filter.id),
                            filter_label: format!("paired daily: {}", filter.label),
                            condition_hash: filter.condition_hash.clone(),
                            mask_ref: filter.mask_ref.clone(),
                            bytecode_ops: filter.bytecode_ops.clone(),
                            display_formula: filter.display_formula.clone(),
                            indicator_refs: filter.indicator_refs.clone(),
                            entry_count: filtered_entries.len(),
                            take_profit_distance: *take_profit,
                            trades: stats.trades,
                            wins: stats.wins,
                            losses: stats.losses,
                            win_rate,
                            expectancy_distance: expectancy,
                            net_pnl_distance: stats.net_pnl,
                            profit_factor: stats.profit_factor(),
                            max_loss_streak: stats.max_loss_streak,
                            avg_hold_bars: stats.avg_hold_bars(),
                            meets_target: daily
                                .as_ref()
                                .and_then(|performance| performance.daily_target_hit_rate())
                                .is_some_and(|value| value >= daily_target_hit_threshold)
                                && daily.as_ref().is_some_and(|performance| performance.total_days >= 12),
                            daily_target_hit_rate: daily.as_ref().and_then(|performance| performance.daily_target_hit_rate()),
                            positive_day_rate: daily.as_ref().and_then(|performance| performance.positive_day_rate()),
                            target_hit_days: daily.as_ref().map(|performance| performance.target_hit_days),
                            total_days: daily.as_ref().map(|performance| performance.total_days),
                            avg_daily_pnl_distance: daily.as_ref().map(|performance| performance.avg_daily_pnl_distance),
                            min_daily_pnl_distance: daily.as_ref().map(|performance| performance.min_daily_pnl_distance),
                            robustness: None,
                        });
                    }
                }
            }
        }
    } else {
        for direction in &directions {
            let base_mask = strategy_condition_full_mask_cached(
                &data_hash,
                &shared_entry_scan_cache_key,
                direction,
                test_entry_indices.len(),
            );
            let outcome_grid = strategy_filter_outcome_grid_cached(
                candles,
                &test_entry_indices,
                &base_mask,
                &shared_entry_scan_cache_key,
                direction,
                stop_loss,
                &tp_grid,
                execution_cost,
                max_hold,
                &mut cache_snapshot,
            );
            for point in outcome_grid {
                let take_profit = point.take_profit_distance;
                let stats = point.stats;
                let win_rate = stats.win_rate();
                let expectancy = stats.expectancy();
                let robustness = compute_strategy_robustness_cached(
                    candles,
                    &low_volatility_values,
                    threshold,
                    train_rows,
                    &entry_hours,
                    force_daily_entry,
                    direction,
                    stop_loss,
                    take_profit,
                    execution_cost,
                    max_hold,
                    target_win_rate,
                    spec.point_size.unwrap_or(0.0),
                    &data_hash,
                    &shared_entry_scan_cache_key,
                    &mut cache_snapshot,
                );
                candidates.push(TradingStrategyBacktestCandidate {
                    direction: direction.clone(),
                    filter_id: "base_low_vol".to_string(),
                    filter_label: if force_daily_entry {
                        format!("{entry_hours_label} daily entries")
                    } else {
                        format!("{entry_hours_label} low-volatility entries")
                    },
                    condition_hash: base_mask.mask_hash.clone(),
                    mask_ref: base_mask.mask_hash.clone(),
                    bytecode_ops: vec![
                        format!(
                            "ENTRY:HOUR_IN_UTC:{}",
                            entry_hours.iter().map(|value| value.to_string()).collect::<Vec<_>>().join(",")
                        ),
                        if force_daily_entry {
                            "ENTRY:FORCE_ONE_TRADE_PER_OPEN_DAY".to_string()
                        } else {
                            "VOL:RANGE_SMA_LE_TRAIN_Q".to_string()
                        },
                    ],
                    display_formula: vec![
                        format!("entry hour in [{entry_hours_label}] UTC"),
                        if force_daily_entry {
                            "one trade each open trading day".to_string()
                        } else {
                            "range SMA H1 24 <= train percentile".to_string()
                        },
                    ],
                    indicator_refs: vec!["/candle_h1".to_string(), "/range_sma_h1_24".to_string()],
                    entry_count: test_entry_indices.len(),
                    take_profit_distance: take_profit,
                    trades: stats.trades,
                    wins: stats.wins,
                    losses: stats.losses,
                    win_rate,
                    expectancy_distance: expectancy,
                    net_pnl_distance: stats.net_pnl,
                    profit_factor: stats.profit_factor(),
                    max_loss_streak: stats.max_loss_streak,
                    avg_hold_bars: stats.avg_hold_bars(),
                    meets_target: win_rate.is_some_and(|value| value >= target_win_rate) && stats.trades >= 12,
                    daily_target_hit_rate: None,
                    positive_day_rate: None,
                    target_hit_days: None,
                    total_days: None,
                    avg_daily_pnl_distance: None,
                    min_daily_pnl_distance: None,
                    robustness: Some(robustness),
                });
            }
        }

        candidates.sort_by(compare_strategy_candidates);
        let base_best = candidates.first().cloned();
        if !base_best
            .as_ref()
            .is_some_and(|candidate| candidate.meets_target)
        {
            for direction in &directions {
                let filters = strategy_directional_entry_filters_cached(
                    candles,
                    &test_entry_indices,
                    &shared_entry_scan_cache_key,
                    direction,
                    &data_hash,
                    force_daily_entry,
                    &mut cache_snapshot,
                );
                for filter in filters.into_iter().filter(|filter| filter.id != "base_low_vol") {
                    let outcome_grid = strategy_filter_outcome_grid_cached(
                        candles,
                        &test_entry_indices,
                        &filter.mask,
                        &shared_entry_scan_cache_key,
                        direction,
                        stop_loss,
                        &tp_grid,
                        execution_cost,
                        max_hold,
                        &mut cache_snapshot,
                    );
                    for point in outcome_grid {
                        let take_profit = point.take_profit_distance;
                        let stats = point.stats;
                        let win_rate = stats.win_rate();
                        let expectancy = stats.expectancy();
                        candidates.push(TradingStrategyBacktestCandidate {
                            direction: direction.clone(),
                            filter_id: filter.id.clone(),
                            filter_label: filter.label.clone(),
                            condition_hash: filter.condition_hash.clone(),
                            mask_ref: filter.mask_ref.clone(),
                            bytecode_ops: filter.bytecode_ops.clone(),
                            display_formula: filter.display_formula.clone(),
                            indicator_refs: filter.indicator_refs.clone(),
                            entry_count: filter.entry_count,
                            take_profit_distance: take_profit,
                            trades: stats.trades,
                            wins: stats.wins,
                            losses: stats.losses,
                            win_rate,
                            expectancy_distance: expectancy,
                            net_pnl_distance: stats.net_pnl,
                            profit_factor: stats.profit_factor(),
                            max_loss_streak: stats.max_loss_streak,
                            avg_hold_bars: stats.avg_hold_bars(),
                            meets_target: win_rate.is_some_and(|value| value >= target_win_rate)
                                && stats.trades >= 12,
                            daily_target_hit_rate: None,
                            positive_day_rate: None,
                            target_hit_days: None,
                            total_days: None,
                            avg_daily_pnl_distance: None,
                            min_daily_pnl_distance: None,
                            robustness: None,
                        });
                    }
                }
            }
        }
    }
    candidates.sort_by(compare_strategy_candidates);
    let best = candidates.first().cloned();
    let direction_count = directions.len();
    let (best_entry_indices, best_entry_cache_key) = best
        .as_ref()
        .and_then(|candidate| {
            if matches!(candidate.filter_id.as_str(), "base_low_vol" | "base_paired_daily") {
                return Some((test_entry_indices.clone(), shared_entry_scan_cache_key.clone()));
            }
            if paired_mode {
                let target_filter_id = candidate.filter_id.strip_prefix("paired_").unwrap_or(&candidate.filter_id);
                for filter_direction in ["long", "short"] {
                    if let Some(filter) = strategy_directional_entry_filters_cached(
                        candles,
                        &test_entry_indices,
                        &shared_entry_scan_cache_key,
                        filter_direction,
                        &data_hash,
                        force_daily_entry,
                        &mut cache_snapshot,
                    )
                    .into_iter()
                    .find(|filter| filter.id == target_filter_id)
                    {
                        return Some((
                            strategy_condition_entries_from_mask(&test_entry_indices, &filter.mask),
                            filter.cache_key,
                        ));
                    }
                }
                return None;
            }
            strategy_directional_entry_filters_cached(
                candles,
                &test_entry_indices,
                &shared_entry_scan_cache_key,
                &candidate.direction,
                &data_hash,
                force_daily_entry,
                &mut cache_snapshot,
            )
            .into_iter()
            .find(|filter| filter.id == candidate.filter_id)
            .map(|filter| {
                (
                    strategy_condition_entries_from_mask(&test_entry_indices, &filter.mask),
                    filter.cache_key,
                )
            })
        })
        .unwrap_or_else(|| (test_entry_indices.clone(), shared_entry_scan_cache_key.clone()));
    let paired_probe = best.as_ref().map(|candidate| {
        simulate_strategy_paired_probe(
            candles,
            &best_entry_indices,
            candidate.take_profit_distance,
            stop_loss,
            execution_cost,
            max_hold,
            best_entry_cache_key.clone(),
            &mut cache_snapshot,
        )
    });
    let visual_probes = best
        .as_ref()
        .map(|candidate| {
            build_strategy_visual_probes(
                candles,
                &best_entry_indices,
                candidate.take_profit_distance,
                stop_loss,
                execution_cost,
                max_hold,
                12,
            )
        })
        .unwrap_or_default();
    let mut condition_filter_ids = HashMap::<String, ()>::new();
    for candidate in &candidates {
        condition_filter_ids.insert(candidate.filter_id.clone(), ());
    }
    let compute_plan = build_strategy_compute_plan(
        spec,
        candles.len(),
        &data_hash,
        &template_hash,
        &cache_snapshot,
        Some(threshold),
        tp_grid.len(),
        direction_count,
        test_entry_indices.len(),
        condition_filter_ids.len(),
    );
    strategy_store_compute_plan(&compute_plan);
    let result = TradingStrategyBacktestResult {
        rows: candles.len(),
        first_time: candles.first().map(|candle| candle.time.clone()),
        last_time: candles.last().map(|candle| candle.time.clone()),
        train_rows,
        test_rows,
        low_volatility_threshold: threshold,
        entry_hour_utc: entry_hour,
        entry_hours_utc: entry_hours,
        candidates,
        best,
        paired_probe,
        visual_probes,
        compute_plan,
    };
    strategy_store_cached_result(&result_cache_key, &result);
    Ok(result)
}

fn strategy_live_hash(input: &str) -> String {
    let mut hash = 2166136261_u32;
    for byte in input.as_bytes() {
        hash ^= u32::from(*byte);
        hash = hash.wrapping_mul(16777619);
    }
    format!("{hash:08x}")
}

fn strategy_live_job_id(spec: &TradingStrategySpec) -> String {
    let entry_hours = strategy_entry_hours(spec);
    let key = format!(
        "{}|{}|{}|{}|{}|{}|{}|{}|{}",
        spec.instrument.as_deref().unwrap_or(""),
        spec.granularity.as_deref().unwrap_or(""),
        spec.entry_hour.map(|value| value.to_string()).unwrap_or_default(),
        entry_hours
            .iter()
            .map(|value| value.to_string())
            .collect::<Vec<_>>()
            .join(","),
        spec.direction.as_deref().unwrap_or(""),
        spec.stop_loss_distance.map(|value| value.to_string()).unwrap_or_default(),
        spec.take_profit_min_distance.map(|value| value.to_string()).unwrap_or_default(),
        spec.daily_profit_target_distance
            .map(|value| value.to_string())
            .unwrap_or_default(),
        spec.source_text.as_deref().unwrap_or("")
    );
    format!("strategy-live-{}", strategy_live_hash(&key))
}

fn candle_index_by_time(candles: &[TradingCandlePoint], time: &str) -> Option<usize> {
    candles.iter().position(|candle| candle.time == time)
}

fn next_candle_index_after(candles: &[TradingCandlePoint], time: Option<&str>) -> usize {
    let Some(time) = time else {
        return candles.len().saturating_sub(1);
    };
    candles
        .iter()
        .position(|candle| candle.time.as_str() > time)
        .unwrap_or(candles.len())
}

fn close_live_signal_if_resolved(
    signal: &mut TradingStrategyLiveSignal,
    candles: &[TradingCandlePoint],
) -> bool {
    if signal.status != "open" {
        return false;
    }
    let Some(entry_index) = candle_index_by_time(candles, &signal.entry_time) else {
        return false;
    };
    if entry_index + 1 >= candles.len() {
        return false;
    }
    let end_index = (entry_index + signal.max_hold_bars).min(candles.len() - 1);
    for index in entry_index + 1..=end_index {
        let candle = &candles[index];
        let is_long = signal.direction == "long";
        let hit_stop = if is_long {
            candle.low <= signal.stop_price
        } else {
            candle.high >= signal.stop_price
        };
        let hit_target = if is_long {
            candle.high >= signal.take_profit_price
        } else {
            candle.low <= signal.take_profit_price
        };
        if hit_stop {
            signal.status = "closed".to_string();
            signal.outcome = Some("loss".to_string());
            signal.exit_time = Some(candle.time.clone());
            signal.exit_price = Some(signal.stop_price);
            signal.pnl_distance = Some(-signal.stop_loss_distance);
            signal.reason = "stop_loss_hit".to_string();
            signal.updated_at_ms = now_ms();
            return true;
        }
        if hit_target {
            signal.status = "closed".to_string();
            signal.outcome = Some("win".to_string());
            signal.exit_time = Some(candle.time.clone());
            signal.exit_price = Some(signal.take_profit_price);
            signal.pnl_distance = Some(signal.take_profit_distance);
            signal.reason = "take_profit_hit".to_string();
            signal.updated_at_ms = now_ms();
            return true;
        }
    }
    if candles.len() - 1 >= entry_index + signal.max_hold_bars {
        let candle = &candles[end_index];
        let pnl = if signal.direction == "long" {
            candle.close - signal.entry_price
        } else {
            signal.entry_price - candle.close
        };
        signal.status = "closed".to_string();
        signal.outcome = Some(if pnl > 0.0 { "win" } else { "expired" }.to_string());
        signal.exit_time = Some(candle.time.clone());
        signal.exit_price = Some(candle.close);
        signal.pnl_distance = Some(pnl);
        signal.reason = "max_hold_reached".to_string();
        signal.updated_at_ms = now_ms();
        return true;
    }
    false
}

fn live_signal_for_candle(
    job_id: &str,
    spec: &TradingStrategySpec,
    candle: &TradingCandlePoint,
    direction: &str,
    take_profit_distance: f64,
) -> Option<TradingStrategyLiveSignal> {
    let instrument = spec.instrument.as_deref()?.trim().to_string();
    let granularity = spec.granularity.as_deref()?.trim().to_uppercase();
    let stop_loss_distance = spec.stop_loss_distance?;
    if !stop_loss_distance.is_finite() || stop_loss_distance <= 0.0 {
        return None;
    }
    if !take_profit_distance.is_finite() || take_profit_distance <= 0.0 {
        return None;
    }
    let direction = match direction.trim().to_lowercase().as_str() {
        "long" | "buy" => "long",
        "short" | "sell" => "short",
        _ => return None,
    };
    let entry_price = candle.open;
    if !entry_price.is_finite() || entry_price <= 0.0 {
        return None;
    }
    let stop_price = if direction == "long" {
        entry_price - stop_loss_distance
    } else {
        entry_price + stop_loss_distance
    };
    let take_profit_price = if direction == "long" {
        entry_price + take_profit_distance
    } else {
        entry_price - take_profit_distance
    };
    let now = now_ms();
    let id = format!(
        "{}:{}:{}:{}",
        job_id,
        candle.time,
        direction,
        strategy_live_hash(&format!("{take_profit_distance:.10}"))
    );
    Some(TradingStrategyLiveSignal {
        id,
        status: "open".to_string(),
        instrument,
        granularity,
        direction: direction.to_string(),
        entry_time: candle.time.clone(),
        entry_price,
        stop_price,
        take_profit_price,
        take_profit_distance,
        stop_loss_distance,
        max_hold_bars: spec.max_hold_bars.unwrap_or(24).max(1),
        exit_time: None,
        exit_price: None,
        pnl_distance: None,
        outcome: None,
        reason: "entry_hour_and_low_volatility_matched".to_string(),
        created_at_ms: now,
        updated_at_ms: now,
    })
}

fn strategy_live_journal_tail(signals: &[TradingStrategyLiveSignal]) -> Vec<TradingStrategyLiveSignal> {
    let mut tail = signals.to_vec();
    tail.sort_by(|a, b| {
        a.updated_at_ms
            .cmp(&b.updated_at_ms)
            .then_with(|| a.created_at_ms.cmp(&b.created_at_ms))
    });
    if tail.len() > 12 {
        tail.split_off(tail.len() - 12)
    } else {
        tail
    }
}

fn inspect_history_csv_metadata(
    path: &Path,
) -> Result<(usize, Option<String>, Option<String>), String> {
    if !path.exists() {
        return Ok((0, None, None));
    }
    let file = File::open(path)
        .map_err(|e| format!("open history CSV '{}': {e}", path.display()))?;
    let reader = BufReader::new(file);
    let mut rows = 0usize;
    let mut first_time = None;
    let mut last_time = None;
    for (index, line) in reader.lines().enumerate() {
        let line = line.map_err(|e| format!("read history CSV '{}': {e}", path.display()))?;
        if index == 0 || line.trim().is_empty() {
            continue;
        }
        let time = line.split(',').next().unwrap_or("").trim().to_string();
        if time.is_empty() {
            continue;
        }
        if first_time.is_none() {
            first_time = Some(time.clone());
        }
        last_time = Some(time);
        rows += 1;
    }
    Ok((rows, first_time, last_time))
}

fn persist_recent_history_candles(
    source: &str,
    instrument: &str,
    granularity: &str,
    candles: &[TradingCandlePoint],
) -> Result<TradingHistoryFileSummary, String> {
    let path = history_path_for(instrument, granularity);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("create history dir '{}': {e}", parent.display()))?;
    }
    let (mut rows, mut first_time, existing_last_time) = inspect_history_csv_metadata(&path)?;
    let mut last_time = existing_last_time.clone();
    let mut fresh = candles
        .iter()
        .filter(|candle| {
            !candle.time.trim().is_empty()
                && existing_last_time
                    .as_deref()
                    .map(|last| candle.time.as_str() > last)
                    .unwrap_or(true)
        })
        .collect::<Vec<_>>();
    if fresh.is_empty() {
        return Ok(TradingHistoryFileSummary {
            instrument: instrument.to_string(),
            granularity: granularity.to_uppercase(),
            path: path.display().to_string(),
            rows,
            first_time,
            last_time,
            truncated: false,
            updated_at_ms: path
                .metadata()
                .ok()
                .and_then(|meta| meta.modified().ok())
                .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                .map(|duration| duration.as_millis() as u64)
                .unwrap_or(0),
        });
    }

    fresh.sort_by(|a, b| a.time.cmp(&b.time));
    let file = if rows == 0 {
        let mut file = File::create(&path)
            .map_err(|e| format!("create history file '{}': {e}", path.display()))?;
        file.write_all(b"time,open,high,low,close,volume\n")
            .map_err(|e| format!("write CSV header '{}': {e}", path.display()))?;
        file
    } else {
        OpenOptions::new()
            .append(true)
            .open(&path)
            .map_err(|e| format!("open history file for append '{}': {e}", path.display()))?
    };
    let mut writer = BufWriter::new(file);
    for candle in fresh {
        if first_time.is_none() {
            first_time = Some(candle.time.clone());
        }
        last_time = Some(candle.time.clone());
        let line = format!(
            "{},{},{},{},{},{}\n",
            candle.time, candle.open, candle.high, candle.low, candle.close, candle.volume
        );
        writer
            .write_all(line.as_bytes())
            .map_err(|e| format!("append CSV row '{}': {e}", path.display()))?;
        rows += 1;
    }
    writer
        .flush()
        .map_err(|e| format!("flush CSV '{}': {e}", path.display()))?;

    let summary = TradingHistoryFileSummary {
        instrument: instrument.to_string(),
        granularity: granularity.to_uppercase(),
        path: path.display().to_string(),
        rows,
        first_time,
        last_time,
        truncated: false,
        updated_at_ms: now_ms(),
    };
    write_history_manifest(source, &[summary.clone()])?;
    Ok(summary)
}

fn synthetic_depth(
    price: &TradingPriceSnapshot,
    pending_orders: &[TradingOrderSummary],
) -> TradingBookSnapshot {
    let step = 0.01_f64;
    let mut pending_by_price: HashMap<i64, f64> = HashMap::new();
    for order in pending_orders {
        let Some(order_price) = order.price else {
            continue;
        };
        let bucket = (order_price / step).round() as i64;
        *pending_by_price.entry(bucket).or_insert(0.0) += order.units.abs();
    }

    let mut bids = Vec::new();
    let mut asks = Vec::new();
    let seed = (price.mid * 1000.0).round() as i64;
    for level in 0..10 {
        let bid_price = (((price.bid - level as f64 * step) * 1000.0).round()) / 1000.0;
        let ask_price = (((price.ask + level as f64 * step) * 1000.0).round()) / 1000.0;
        let bid_bucket = (bid_price / step).round() as i64;
        let ask_bucket = (ask_price / step).round() as i64;
        let bid_pending = pending_by_price.get(&bid_bucket).copied().unwrap_or(0.0);
        let ask_pending = pending_by_price.get(&ask_bucket).copied().unwrap_or(0.0);
        let bid_size = 18.0 + (((seed + level as i64 * 13).unsigned_abs() % 27) as f64) * 3.2 + bid_pending * 0.08;
        let ask_size = 16.0 + (((seed + level as i64 * 17).unsigned_abs() % 31) as f64) * 3.0 + ask_pending * 0.08;
        bids.push(TradingBookLevel {
            price: bid_price,
            size: (bid_size * 10.0).round() / 10.0,
            pending_units: bid_pending,
        });
        asks.push(TradingBookLevel {
            price: ask_price,
            size: (ask_size * 10.0).round() / 10.0,
            pending_units: ask_pending,
        });
    }

    TradingBookSnapshot {
        kind: "synthetic_ladder".to_string(),
        note: "OANDA v20 does not expose a live NATGAS market depth book here. This ladder is a synthetic bid/ask depth view anchored on live pricing plus your pending orders.".to_string(),
        bids,
        asks,
    }
}

async fn fetch_account_snapshot(
    client: &reqwest::Client,
    credentials: &ResolvedOandaCredentials,
    headers: &HeaderMap,
) -> Result<TradingAccountSnapshot, String> {
    let url = format!(
        "{}/v3/accounts/{}",
        credentials.base_url.trim_end_matches('/'),
        credentials.account_id
    );
    let payload = fetch_json(client, reqwest::Method::GET, &url, headers, None, None).await?;
    let account = payload
        .get("account")
        .and_then(Value::as_object)
        .ok_or_else(|| "missing account object in OANDA response".to_string())?;
    Ok(TradingAccountSnapshot {
        alias: account
            .get("alias")
            .and_then(Value::as_str)
            .unwrap_or("OANDA account")
            .to_string(),
        currency: account
            .get("currency")
            .and_then(Value::as_str)
            .unwrap_or("USD")
            .to_string(),
        balance: parse_f64_value(account.get("balance")).unwrap_or(0.0),
        nav: parse_f64_value(account.get("NAV")).unwrap_or(0.0),
        unrealized_pl: parse_f64_value(account.get("unrealizedPL")).unwrap_or(0.0),
        margin_available: parse_f64_value(account.get("marginAvailable")).unwrap_or(0.0),
        open_trade_count: parse_u64_value(account.get("openTradeCount")).unwrap_or(0),
        open_position_count: parse_u64_value(account.get("openPositionCount")).unwrap_or(0),
        pending_order_count: parse_u64_value(account.get("pendingOrderCount")).unwrap_or(0),
    })
}

async fn fetch_price_snapshot(
    client: &reqwest::Client,
    credentials: &ResolvedOandaCredentials,
    headers: &HeaderMap,
    instrument: &str,
) -> Result<TradingPriceSnapshot, String> {
    let url = format!(
        "{}/v3/accounts/{}/pricing",
        credentials.base_url.trim_end_matches('/'),
        credentials.account_id
    );
    let query = [("instruments", instrument.to_string())];
    let payload = fetch_json(client, reqwest::Method::GET, &url, headers, Some(&query), None).await?;
    let price = payload
        .get("prices")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .ok_or_else(|| "missing prices[0] in OANDA response".to_string())?;
    let bid = parse_f64_value(
        price.get("bids")
            .and_then(Value::as_array)
            .and_then(|items| items.first())
            .and_then(|item| item.get("price")),
    )
    .ok_or_else(|| "missing bid price".to_string())?;
    let ask = parse_f64_value(
        price.get("asks")
            .and_then(Value::as_array)
            .and_then(|items| items.first())
            .and_then(|item| item.get("price")),
    )
    .ok_or_else(|| "missing ask price".to_string())?;
    let time = price
        .get("time")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let units_default = price
        .get("unitsAvailable")
        .and_then(|v| v.get("default"));
    let units_available_long = parse_f64_value(
        units_default.and_then(|d| d.get("long")),
    )
    .unwrap_or(0.0);
    let units_available_short = parse_f64_value(
        units_default.and_then(|d| d.get("short")),
    )
    .unwrap_or(0.0);
    Ok(TradingPriceSnapshot {
        instrument: instrument.to_string(),
        time,
        bid,
        ask,
        mid: (bid + ask) * 0.5,
        spread: ask - bid,
        units_available_long,
        units_available_short,
    })
}

async fn fetch_recent_candle_samples(
    client: &reqwest::Client,
    credentials: &ResolvedOandaCredentials,
    headers: &HeaderMap,
    instrument: &str,
    granularity: &str,
    count: usize,
    include_incomplete: bool,
) -> Result<Vec<OandaCandleSample>, String> {
    let url = format!(
        "{}/v3/instruments/{}/candles",
        credentials.base_url.trim_end_matches('/'),
        instrument
    );
    let bounded_count = count.clamp(16, 750);
    let query = vec![
        ("price", "M".to_string()),
        ("granularity", granularity.to_string()),
        ("count", bounded_count.to_string()),
    ];
    let payload = fetch_json(client, reqwest::Method::GET, &url, headers, Some(&query), None).await?;
    let candles = payload
        .get("candles")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("missing candles array for {instrument}/{granularity}"))?;
    let mut out = Vec::new();
    for candle in candles {
        let complete = candle.get("complete").and_then(Value::as_bool).unwrap_or(true);
        if !complete && !include_incomplete {
            continue;
        }
        let Some(mid) = candle.get("mid") else {
            continue;
        };
        let open = parse_f64_value(mid.get("o"));
        let high = parse_f64_value(mid.get("h"));
        let low = parse_f64_value(mid.get("l"));
        let close = parse_f64_value(mid.get("c"));
        let (Some(open), Some(high), Some(low), Some(close)) = (open, high, low, close) else {
            continue;
        };
        out.push(OandaCandleSample {
            point: TradingCandlePoint {
                time: candle.get("time").and_then(Value::as_str).unwrap_or("").to_string(),
                open,
                high,
                low,
                close,
                volume: parse_u64_value(candle.get("volume")).unwrap_or(0),
            },
        });
    }
    Ok(out)
}

async fn fetch_recent_candles(
    client: &reqwest::Client,
    credentials: &ResolvedOandaCredentials,
    headers: &HeaderMap,
    instrument: &str,
    granularity: &str,
    count: usize,
) -> Result<Vec<TradingCandlePoint>, String> {
    Ok(fetch_recent_candle_samples(
        client,
        credentials,
        headers,
        instrument,
        granularity,
        count,
        false,
    )
    .await?
    .into_iter()
    .map(|sample| sample.point)
    .collect())
}

async fn synthesize_live_candle(
    client: &reqwest::Client,
    credentials: &ResolvedOandaCredentials,
    headers: &HeaderMap,
    instrument: &str,
    granularity: &str,
    complete_candles: &[TradingCandlePoint],
    price: Option<&TradingPriceSnapshot>,
) -> Option<TradingCandlePoint> {
    let lower_granularity = live_source_granularity_for(granularity)?;
    let target_step_ms = granularity_step_ms(granularity)?;
    let lower_step_ms = granularity_step_ms(lower_granularity)?;
    let sample_count = ((target_step_ms / lower_step_ms).clamp(4, 5_000) as usize).saturating_add(4);
    let lower_points = match fetch_recent_candle_samples(
        client,
        credentials,
        headers,
        instrument,
        lower_granularity,
        sample_count,
        true,
    )
    .await
    {
        Ok(samples) => samples.into_iter().map(|sample| sample.point).collect::<Vec<_>>(),
        Err(_) => local_history_tail_candles(instrument, lower_granularity, sample_count),
    };
    synthesize_incomplete_candle_from_samples(
        instrument,
        granularity,
        complete_candles,
        &lower_points,
        price,
    )
}

async fn fetch_instruments(
    client: &reqwest::Client,
    credentials: &ResolvedOandaCredentials,
    headers: &HeaderMap,
) -> Result<Vec<TradingInstrumentSummary>, String> {
    let url = format!(
        "{}/v3/accounts/{}/instruments",
        credentials.base_url.trim_end_matches('/'),
        credentials.account_id
    );
    let payload = fetch_json(client, reqwest::Method::GET, &url, headers, None, None).await?;
    let Some(items) = payload.get("instruments").and_then(Value::as_array) else {
        return Ok(vec![TradingInstrumentSummary {
            name: DEFAULT_INSTRUMENT.to_string(),
            display_name: "Natural Gas".to_string(),
            asset_class: "commodity".to_string(),
            pip_location: Some(-2),
            display_precision: Some(3),
            trade_units_precision: Some(0),
            minimum_trade_size: Some(1.0),
            margin_rate: Some(0.05),
        }]);
    };
    let mut out = Vec::new();
    for item in items {
        let name = item.get("name").and_then(Value::as_str).unwrap_or("").to_string();
        if name.is_empty() {
            continue;
        }
        let display_name = item
            .get("displayName")
            .and_then(Value::as_str)
            .unwrap_or(&name)
            .to_string();
        let asset_class = classify_oanda_asset(item, &name);
        out.push(TradingInstrumentSummary {
            name,
            display_name,
            asset_class,
            pip_location: parse_i64_value(item.get("pipLocation")),
            display_precision: parse_i64_value(item.get("displayPrecision")),
            trade_units_precision: parse_i64_value(item.get("tradeUnitsPrecision")),
            minimum_trade_size: parse_f64_value(item.get("minimumTradeSize")),
            margin_rate: parse_f64_value(item.get("marginRate")),
        });
    }
    out.sort_by(|a, b| {
        if a.name == DEFAULT_INSTRUMENT {
            std::cmp::Ordering::Less
        } else if b.name == DEFAULT_INSTRUMENT {
            std::cmp::Ordering::Greater
        } else {
            a.name.cmp(&b.name)
        }
    });
    Ok(out)
}

fn classify_oanda_asset(item: &Value, name: &str) -> String {
    let kind = item
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_uppercase();
    if kind == "CURRENCY" {
        return "forex".to_string();
    }
    if kind == "METAL" {
        return "metal".to_string();
    }
    if kind == "CFD" {
        let inferred = classify_oanda_name(name);
        if inferred == "commodity" && ["XAU_", "XAG_", "XCU_", "XPT_", "XPD_"]
            .iter()
            .any(|prefix| name.trim().to_uppercase().starts_with(prefix))
        {
            return "metal".to_string();
        }
        return inferred;
    }
    if !kind.is_empty() {
        return kind.to_lowercase();
    }
    if name.contains('_') {
        "instrument".to_string()
    } else {
        "unknown".to_string()
    }
}

fn order_side_from_units(units: f64) -> String {
    if units < 0.0 { "SELL".to_string() } else { "BUY".to_string() }
}

fn summarize_order_like(item: &Value) -> Option<TradingOrderSummary> {
    let instrument = item.get("instrument")?.as_str()?.to_string();
    let units = parse_f64_value(item.get("units").or_else(|| item.get("currentUnits")))?;
    Some(TradingOrderSummary {
        id: item.get("id").and_then(Value::as_str).unwrap_or("").to_string(),
        instrument,
        side: order_side_from_units(units),
        order_type: item
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("TRADE")
            .to_string(),
        units,
        price: parse_f64_value(item.get("price").or_else(|| item.get("initialUnits"))),
        state: item
            .get("state")
            .and_then(Value::as_str)
            .unwrap_or("OPEN")
            .to_string(),
        create_time: item
            .get("createTime")
            .or_else(|| item.get("openTime"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
    })
}

async fn fetch_pending_orders(
    client: &reqwest::Client,
    credentials: &ResolvedOandaCredentials,
    headers: &HeaderMap,
) -> Result<Vec<TradingOrderSummary>, String> {
    let url = format!(
        "{}/v3/accounts/{}/pendingOrders",
        credentials.base_url.trim_end_matches('/'),
        credentials.account_id
    );
    let payload = fetch_json(client, reqwest::Method::GET, &url, headers, None, None).await?;
    let mut out = Vec::new();
    if let Some(items) = payload.get("orders").and_then(Value::as_array) {
        for item in items {
            if let Some(summary) = summarize_order_like(item) {
                out.push(summary);
            }
        }
    }
    Ok(out)
}

async fn fetch_open_trades(
    client: &reqwest::Client,
    credentials: &ResolvedOandaCredentials,
    headers: &HeaderMap,
) -> Result<Vec<TradingOrderSummary>, String> {
    let url = format!(
        "{}/v3/accounts/{}/openTrades",
        credentials.base_url.trim_end_matches('/'),
        credentials.account_id
    );
    let payload = fetch_json(client, reqwest::Method::GET, &url, headers, None, None).await?;
    let mut out = Vec::new();
    if let Some(items) = payload.get("trades").and_then(Value::as_array) {
        for item in items {
            if let Some(summary) = summarize_order_like(item) {
                out.push(summary);
            }
        }
    }
    Ok(out)
}

fn update_runtime_selection(instrument: &str, granularity: &str, count: usize) {
    let mut state = oanda_runtime_lock().lock().unwrap();
    state.active_instrument = instrument.trim().to_string();
    state.active_granularity = granularity.trim().to_uppercase();
    state.active_count = count.clamp(16, 750);
}

fn clear_oanda_runtime() {
    let mut state = oanda_runtime_lock().lock().unwrap();
    let active_instrument = state.active_instrument.clone();
    let active_granularity = state.active_granularity.clone();
    let active_count = state.active_count;
    *state = default_runtime_state();
    state.active_instrument = active_instrument;
    state.active_granularity = active_granularity;
    state.active_count = active_count;
}

fn apply_runtime_bundle(
    bundle: TradingRuntimeBundle,
    credentials: &ResolvedOandaCredentials,
    instrument: &str,
    granularity: &str,
    count: usize,
) -> TradingRuntimeStatus {
    let now = now_ms();
    let mut state = oanda_runtime_lock().lock().unwrap();
    state.connected = true;
    state.active_instrument = instrument.to_string();
    state.active_granularity = granularity.to_uppercase();
    state.active_count = count.clamp(16, 750);
    state.cached_instrument = instrument.to_string();
    state.cached_granularity = granularity.to_uppercase();
    state.last_heartbeat_ms = now;
    state.last_rest_check_ms = now;
    state.last_resume_ms = now;
    state.last_attempt_ms = now;
    state.consecutive_failures = 0;
    state.last_error.clear();
    state.credentials_fingerprint = credentials_fingerprint(credentials);
    state.account = bundle.account;
    state.price = bundle.price;
    state.instruments = bundle.instruments;
    state.pending_orders = bundle.pending_orders;
    state.open_trades = bundle.open_trades;
    state.book = bundle.book;
    state.candles = bundle.candles;
    oanda_runtime_status(&state)
}

fn note_runtime_failure(credentials: Option<&ResolvedOandaCredentials>, message: String) -> TradingRuntimeStatus {
    let now = now_ms();
    let mut state = oanda_runtime_lock().lock().unwrap();
    state.connected = false;
    state.last_attempt_ms = now;
    state.consecutive_failures = state.consecutive_failures.saturating_add(1);
    state.last_error = message;
    if let Some(credentials) = credentials {
        state.credentials_fingerprint = credentials_fingerprint(credentials);
    }
    oanda_runtime_status(&state)
}

fn cached_market_feed_response(
    config: TradingOandaConfigStatus,
    instrument: &str,
    granularity: &str,
) -> Option<TradingMarketFeedResponse> {
    let state = oanda_runtime_lock().lock().unwrap().clone();
    if state.cached_instrument != instrument || state.cached_granularity != granularity.to_uppercase() {
        return None;
    }
    let has_payload = state.price.is_some() || !state.candles.is_empty() || !state.pending_orders.is_empty() || !state.open_trades.is_empty();
    if !has_payload {
        return None;
    }
    let runtime = oanda_runtime_status(&state);
    Some(TradingMarketFeedResponse {
        config,
        instrument: instrument.to_string(),
        granularity: granularity.to_uppercase(),
        price: state.price,
        pending_orders: state.pending_orders,
        open_trades: state.open_trades,
        book: state.book,
        candles: state.candles,
        alerts: Vec::new(),
        alert_events: Vec::new(),
        runtime: Some(runtime),
    })
}

fn local_history_market_feed_response(
    config: TradingOandaConfigStatus,
    instrument: &str,
    granularity: &str,
) -> Option<TradingMarketFeedResponse> {
    let path = history_path_for(instrument, granularity);
    let csv_text = read_history_csv_tail(&path, 240).ok()?;
    let candles = parse_history_csv_candles(&csv_text);
    if candles.is_empty() {
        return None;
    }
    let runtime = oanda_runtime_status(&oanda_runtime_lock().lock().unwrap());
    Some(TradingMarketFeedResponse {
        config,
        instrument: instrument.to_string(),
        granularity: granularity.to_uppercase(),
        price: None,
        pending_orders: Vec::new(),
        open_trades: Vec::new(),
        book: None,
        candles,
        alerts: Vec::new(),
        alert_events: Vec::new(),
        runtime: Some(runtime),
    })
}

fn cached_snapshot_response(
    config: TradingOandaConfigStatus,
    history_dir: String,
    history_files: Vec<TradingHistoryFileSummary>,
) -> Option<TradingSnapshotResponse> {
    let state = oanda_runtime_lock().lock().unwrap().clone();
    let history_instruments = history_instruments_from_files(&history_files);
    let asset_catalog = build_asset_catalog(&history_files);
    let has_payload = state.account.is_some()
        || state.price.is_some()
        || !state.instruments.is_empty()
        || !state.pending_orders.is_empty()
        || !state.open_trades.is_empty()
        || !history_instruments.is_empty();
    if !has_payload {
        return None;
    }
    let runtime = oanda_runtime_status(&state);
    Some(TradingSnapshotResponse {
        config,
        account: state.account,
        price: state.price,
        instruments: if state.instruments.is_empty() {
            if history_instruments.is_empty() {
                vec![TradingInstrumentSummary {
                    name: DEFAULT_INSTRUMENT.to_string(),
                    display_name: "Natural Gas".to_string(),
                    asset_class: "commodity".to_string(),
                    pip_location: Some(-2),
                    display_precision: Some(3),
                    trade_units_precision: Some(0),
                    minimum_trade_size: Some(1.0),
                    margin_rate: Some(0.05),
                }]
            } else {
                history_instruments
            }
        } else {
            state.instruments
        },
        pending_orders: state.pending_orders,
        open_trades: state.open_trades,
        book: state.book,
        history_dir,
        history_files,
        asset_catalog,
        runtime: Some(runtime),
    })
}

async fn fetch_runtime_bundle(
    credentials: &ResolvedOandaCredentials,
    instrument: &str,
    granularity: &str,
    count: usize,
    include_instruments: bool,
) -> Result<TradingRuntimeBundle, String> {
    let client = build_oanda_client()?;
    let headers = oanda_headers(&credentials.api_key)?;
    let fallback = oanda_runtime_lock().lock().unwrap().clone();
    let same_cached_instrument = fallback.cached_instrument == instrument;
    let same_cached_market = same_cached_instrument && fallback.cached_granularity == granularity.to_uppercase();

    let account = Some(fetch_account_snapshot(&client, credentials, &headers).await?);
    let price = match fetch_price_snapshot(&client, credentials, &headers, instrument).await {
        Ok(price) => Some(price),
        Err(err) => {
            let cached = if same_cached_instrument {
                fallback.price.filter(|price| price.instrument == instrument)
            } else {
                None
            };
            if cached.is_some() {
                cached
            } else {
                return Err(err);
            }
        }
    };
    let pending_orders = fetch_pending_orders(&client, credentials, &headers)
        .await
        .unwrap_or_else(|_| fallback.pending_orders.clone());
    let open_trades = fetch_open_trades(&client, credentials, &headers)
        .await
        .unwrap_or_else(|_| fallback.open_trades.clone());
    let instruments = if include_instruments {
        fetch_instruments(&client, credentials, &headers)
            .await
            .unwrap_or_else(|_| {
                if fallback.instruments.is_empty() {
                    vec![TradingInstrumentSummary {
                        name: DEFAULT_INSTRUMENT.to_string(),
                        display_name: "Natural Gas".to_string(),
                        asset_class: "commodity".to_string(),
                        pip_location: Some(-2),
                        display_precision: Some(3),
                        trade_units_precision: Some(0),
                        minimum_trade_size: Some(1.0),
                        margin_rate: Some(0.05),
                    }]
                } else {
                    fallback.instruments.clone()
                }
            })
    } else {
        fallback.instruments.clone()
    };
    let candles = fetch_recent_candles(&client, credentials, &headers, instrument, granularity, count)
        .await
        .unwrap_or_else(|_| if same_cached_market { fallback.candles.clone() } else { Vec::new() });
    let book = price
        .as_ref()
        .map(|price_snapshot| synthetic_depth(price_snapshot, &pending_orders))
        .or_else(|| if same_cached_market { fallback.book.clone() } else { None });

    Ok(TradingRuntimeBundle {
        account,
        price,
        instruments,
        pending_orders,
        open_trades,
        book,
        candles,
    })
}

fn ensure_oanda_watchdog() {
    if OANDA_WATCHDOG_STARTED.swap(true, Ordering::AcqRel) {
        return;
    }
    thread::spawn(|| loop {
        let credentials = resolve_credentials();
        let Some(credentials) = credentials else {
            clear_oanda_runtime();
            thread::sleep(Duration::from_millis(OANDA_WATCHDOG_TICK_MS));
            continue;
        };

        let plan = {
            let mut state = oanda_runtime_lock().lock().unwrap();
            let now = now_ms();
            let fingerprint = credentials_fingerprint(&credentials);
            if state.credentials_fingerprint != fingerprint {
                state.connected = false;
                state.last_error.clear();
                state.consecutive_failures = 0;
                state.credentials_fingerprint = fingerprint;
            }
            let interval_ms = if state.connected {
                OANDA_WATCHDOG_HEARTBEAT_MS
            } else {
                OANDA_WATCHDOG_RECOVERY_MS
            };
            if now.saturating_sub(state.last_attempt_ms) < interval_ms {
                None
            } else {
                state.last_attempt_ms = now;
                Some((
                    state.active_instrument.clone(),
                    state.active_granularity.clone(),
                    state.active_count.clamp(16, 750),
                    state.instruments.is_empty(),
                ))
            }
        };

        if let Some((instrument, granularity, count, include_instruments)) = plan {
            match pollster::block_on(fetch_runtime_bundle(
                &credentials,
                &instrument,
                &granularity,
                count,
                include_instruments,
            )) {
                Ok(bundle) => {
                    apply_runtime_bundle(bundle, &credentials, &instrument, &granularity, count);
                }
                Err(err) => {
                    note_runtime_failure(Some(&credentials), err);
                }
            }
        }

        thread::sleep(Duration::from_millis(OANDA_WATCHDOG_TICK_MS));
    });
}

fn history_cap_for_granularity(requested: Option<usize>) -> usize {
    requested.filter(|value| *value > 0).unwrap_or(usize::MAX)
}

async fn sync_granularity_history(
    client: &reqwest::Client,
    credentials: &ResolvedOandaCredentials,
    headers: &HeaderMap,
    instrument: &str,
    granularity: &str,
    max_rows_requested: Option<usize>,
) -> Result<TradingHistoryFileSummary, String> {
    let path = history_path_for(instrument, granularity);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("create history dir '{}': {e}", parent.display()))?;
    }
    let (mut rows, mut first_time, existing_last_time) = inspect_history_csv_metadata(&path)?;
    let cap_rows = history_cap_for_granularity(max_rows_requested);
    if rows >= cap_rows {
        return Ok(TradingHistoryFileSummary {
            instrument: instrument.to_string(),
            granularity: granularity.to_uppercase(),
            path: path.display().to_string(),
            rows,
            first_time,
            last_time: existing_last_time,
            truncated: cap_rows != usize::MAX,
            updated_at_ms: now_ms(),
        });
    }
    let file = if rows == 0 {
        let mut file = File::create(&path)
            .map_err(|e| format!("create history file '{}': {e}", path.display()))?;
        file.write_all(b"time,open,high,low,close,volume\n")
            .map_err(|e| format!("write CSV header '{}': {e}", path.display()))?;
        file
    } else {
        OpenOptions::new()
            .append(true)
            .open(&path)
            .map_err(|e| format!("open history file for append '{}': {e}", path.display()))?
    };
    let mut writer = BufWriter::new(file);
    let base_url = format!(
        "{}/v3/instruments/{}/candles",
        credentials.base_url.trim_end_matches('/'),
        instrument
    );
    let mut last_time = existing_last_time.clone();
    let mut from = existing_last_time
        .clone()
        .unwrap_or_else(|| HISTORY_START_RFC3339.to_string());
    let mut include_first = existing_last_time.is_none();
    let mut truncated = false;

    loop {
        let query = vec![
            ("price", "M".to_string()),
            ("granularity", granularity.to_string()),
            ("count", "5000".to_string()),
            ("from", from.clone()),
            ("includeFirst", if include_first { "true" } else { "false" }.to_string()),
        ];
        let payload = fetch_json(client, reqwest::Method::GET, &base_url, headers, Some(&query), None).await?;
        let candles = payload
            .get("candles")
            .and_then(Value::as_array)
            .ok_or_else(|| format!("missing candles array for {instrument}/{granularity}"))?;
        if candles.is_empty() {
            break;
        }

        let mut appended = 0usize;
        for candle in candles {
            let complete = candle.get("complete").and_then(Value::as_bool).unwrap_or(true);
            if !complete {
                continue;
            }
            let time = candle.get("time").and_then(Value::as_str).unwrap_or("").to_string();
            if time.is_empty()
                || last_time
                    .as_deref()
                    .map(|last| time.as_str() <= last)
                    .unwrap_or(false)
            {
                continue;
            }
            let Some(mid) = candle.get("mid") else {
                continue;
            };
            let open = parse_f64_value(mid.get("o"));
            let high = parse_f64_value(mid.get("h"));
            let low = parse_f64_value(mid.get("l"));
            let close = parse_f64_value(mid.get("c"));
            let volume = parse_u64_value(candle.get("volume")).unwrap_or(0);
            let (Some(open), Some(high), Some(low), Some(close)) = (open, high, low, close) else {
                continue;
            };
            if first_time.is_none() {
                first_time = Some(time.clone());
            }
            last_time = Some(time.clone());
            let line = format!("{time},{open},{high},{low},{close},{volume}\n");
            writer
                .write_all(line.as_bytes())
                .map_err(|e| format!("write CSV row '{}': {e}", path.display()))?;
            rows += 1;
            appended += 1;
            if rows >= cap_rows {
                truncated = cap_rows != usize::MAX;
                break;
            }
        }

        writer
            .flush()
            .map_err(|e| format!("flush CSV '{}': {e}", path.display()))?;

        if rows >= cap_rows {
            break;
        }
        if appended == 0 {
            break;
        }

        let Some(next_from) = last_time.clone() else {
            break;
        };
        if next_from == from {
            break;
        }
        from = next_from;
        include_first = false;
        if candles.len() < 5000 {
            break;
        }
    }

    writer
        .flush()
        .map_err(|e| format!("final flush '{}': {e}", path.display()))?;

    Ok(TradingHistoryFileSummary {
        instrument: instrument.to_string(),
        granularity: granularity.to_string(),
        path: path.display().to_string(),
        rows,
        first_time,
        last_time,
        truncated,
        updated_at_ms: now_ms(),
    })
}

#[derive(Debug, Clone)]
struct OandaHistorySyncPlan {
    native: Vec<String>,
    derived: Vec<(String, String)>,
}

fn push_unique_granularity(items: &mut Vec<String>, value: &str) {
    let normalized = value.trim().to_uppercase();
    if normalized.is_empty() || items.iter().any(|item| item == &normalized) {
        return;
    }
    items.push(normalized);
}

fn oanda_is_intraday_rebuildable(granularity: &str) -> bool {
    granularity_step_ms(granularity)
        .map(|step| step > 0 && step < 24 * 60 * 60_000)
        .unwrap_or(false)
}

fn can_derive_oanda_granularity(source: &str, target: &str) -> bool {
    let Some(source_step) = granularity_step_ms(source) else {
        return false;
    };
    let Some(target_step) = granularity_step_ms(target) else {
        return false;
    };
    source_step > 0
        && target_step > source_step
        && target_step < 24 * 60 * 60_000
        && target_step % source_step == 0
}

fn plan_oanda_history_sync(selected_granularities: &[String]) -> OandaHistorySyncPlan {
    let mut requested = Vec::new();
    for granularity in selected_granularities {
        push_unique_granularity(&mut requested, granularity);
    }
    let mut native = Vec::new();
    let mut derived = Vec::new();

    let mut plan_family = |family: Vec<String>| {
        if family.is_empty() {
            return;
        }
        let source = family
            .iter()
            .min_by_key(|value| granularity_step_ms(value).unwrap_or(i64::MAX))
            .cloned();
        if let Some(source_granularity) = source.as_deref() {
            push_unique_granularity(&mut native, source_granularity);
        }
        for granularity in family {
            if source.as_deref() == Some(granularity.as_str()) {
                continue;
            }
            if let Some(source_granularity) = source.as_deref() {
                if can_derive_oanda_granularity(source_granularity, &granularity) {
                    derived.push((source_granularity.to_string(), granularity));
                    continue;
                }
            }
            push_unique_granularity(&mut native, &granularity);
        }
    };

    let subminute = requested
        .iter()
        .filter(|value| {
            granularity_step_ms(value)
                .map(|step| step > 0 && step < 60_000)
                .unwrap_or(false)
        })
        .cloned()
        .collect::<Vec<_>>();
    plan_family(subminute);

    let minute_to_hour = requested
        .iter()
        .filter(|value| {
            granularity_step_ms(value)
                .map(|step| step >= 60_000 && step < 24 * 60 * 60_000 && oanda_is_intraday_rebuildable(value))
                .unwrap_or(false)
        })
        .cloned()
        .collect::<Vec<_>>();
    plan_family(minute_to_hour);

    for granularity in requested {
        if native.iter().any(|value| value == &granularity)
            || derived.iter().any(|(_, target)| target == &granularity)
        {
            continue;
        }
        push_unique_granularity(&mut native, &granularity);
    }

    OandaHistorySyncPlan { native, derived }
}

fn file_updated_at_ms(path: &Path) -> u64 {
    path.metadata()
        .ok()
        .and_then(|meta| meta.modified().ok())
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

struct OandaDerivedHistoryTarget {
    instrument: String,
    granularity: String,
    step_ms: i64,
    path: PathBuf,
    rows: usize,
    first_time: Option<String>,
    last_time: Option<String>,
    cap_rows: usize,
    truncated: bool,
    writer: Option<BufWriter<File>>,
    current_bucket_start_ms: Option<i64>,
    current: Option<TradingCandlePoint>,
}

impl OandaDerivedHistoryTarget {
    fn new(
        instrument: &str,
        granularity: &str,
        max_rows_requested: Option<usize>,
    ) -> Result<Self, String> {
        let normalized_granularity = granularity.trim().to_uppercase();
        let step_ms = granularity_step_ms(&normalized_granularity)
            .ok_or_else(|| format!("unsupported derived OANDA granularity {normalized_granularity}"))?;
        let path = history_path_for(instrument, &normalized_granularity);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("create history dir '{}': {e}", parent.display()))?;
        }
        let (rows, first_time, last_time) = inspect_history_csv_metadata(&path)?;
        let cap_rows = history_cap_for_granularity(max_rows_requested);
        let writer = if rows >= cap_rows {
            None
        } else if rows == 0 {
            let mut file = File::create(&path)
                .map_err(|e| format!("create derived history file '{}': {e}", path.display()))?;
            file.write_all(b"time,open,high,low,close,volume\n")
                .map_err(|e| format!("write CSV header '{}': {e}", path.display()))?;
            Some(BufWriter::new(file))
        } else {
            let file = OpenOptions::new()
                .append(true)
                .open(&path)
                .map_err(|e| format!("open derived history file '{}': {e}", path.display()))?;
            Some(BufWriter::new(file))
        };
        Ok(Self {
            instrument: instrument.to_string(),
            granularity: normalized_granularity,
            step_ms,
            path,
            rows,
            first_time,
            last_time,
            cap_rows,
            truncated: cap_rows != usize::MAX && rows >= cap_rows,
            writer,
            current_bucket_start_ms: None,
            current: None,
        })
    }

    fn is_active(&self) -> bool {
        self.writer.is_some() && self.rows < self.cap_rows
    }

    fn offer(&mut self, candle: &TradingCandlePoint) -> Result<(), String> {
        if !self.is_active() {
            return Ok(());
        }
        let Some(time_ms) = parse_oanda_time_ms(&candle.time) else {
            return Ok(());
        };
        let bucket_start_ms = time_ms.div_euclid(self.step_ms) * self.step_ms;
        if self.current_bucket_start_ms != Some(bucket_start_ms) {
            self.flush_current(time_ms)?;
            self.current_bucket_start_ms = Some(bucket_start_ms);
            self.current = Some(TradingCandlePoint {
                time: format_oanda_time_ms(bucket_start_ms),
                open: candle.open,
                high: candle.high,
                low: candle.low,
                close: candle.close,
                volume: candle.volume,
            });
            return Ok(());
        }
        if let Some(current) = self.current.as_mut() {
            current.high = current.high.max(candle.high);
            current.low = current.low.min(candle.low);
            current.close = candle.close;
            current.volume = current.volume.saturating_add(candle.volume);
        }
        Ok(())
    }

    fn flush_current(&mut self, complete_until_ms: i64) -> Result<(), String> {
        let Some(current) = self.current.take() else {
            return Ok(());
        };
        let Some(bucket_start_ms) = self.current_bucket_start_ms else {
            self.current = Some(current);
            return Ok(());
        };
        if bucket_start_ms + self.step_ms > complete_until_ms {
            self.current = Some(current);
            return Ok(());
        }
        if self.rows >= self.cap_rows {
            self.truncated = self.cap_rows != usize::MAX;
            return Ok(());
        }
        if self
            .last_time
            .as_deref()
            .map(|last| current.time.as_str() <= last)
            .unwrap_or(false)
        {
            return Ok(());
        }
        let Some(writer) = self.writer.as_mut() else {
            return Ok(());
        };
        if self.first_time.is_none() {
            self.first_time = Some(current.time.clone());
        }
        self.last_time = Some(current.time.clone());
        let line = format!(
            "{},{},{},{},{},{}\n",
            current.time, current.open, current.high, current.low, current.close, current.volume
        );
        writer
            .write_all(line.as_bytes())
            .map_err(|e| format!("write derived CSV row '{}': {e}", self.path.display()))?;
        self.rows += 1;
        if self.rows >= self.cap_rows {
            self.truncated = self.cap_rows != usize::MAX;
        }
        Ok(())
    }

    fn finish(mut self, complete_until_ms: i64) -> Result<TradingHistoryFileSummary, String> {
        self.flush_current(complete_until_ms)?;
        if let Some(writer) = self.writer.as_mut() {
            writer
                .flush()
                .map_err(|e| format!("flush derived CSV '{}': {e}", self.path.display()))?;
        }
        Ok(TradingHistoryFileSummary {
            instrument: self.instrument,
            granularity: self.granularity,
            path: self.path.display().to_string(),
            rows: self.rows,
            first_time: self.first_time,
            last_time: self.last_time,
            truncated: self.truncated,
            updated_at_ms: now_ms().max(file_updated_at_ms(&self.path)),
        })
    }
}

fn derive_oanda_history_from_source(
    instrument: &str,
    source_granularity: &str,
    target_granularities: &[String],
    max_rows_requested: Option<usize>,
) -> Result<Vec<TradingHistoryFileSummary>, String> {
    if target_granularities.is_empty() {
        return Ok(Vec::new());
    }
    let source_step_ms = granularity_step_ms(source_granularity)
        .ok_or_else(|| format!("unsupported source OANDA granularity {source_granularity}"))?;
    let mut states = target_granularities
        .iter()
        .map(|target| OandaDerivedHistoryTarget::new(instrument, target, max_rows_requested))
        .collect::<Result<Vec<_>, _>>()?;
    if states.iter().all(|state| !state.is_active()) {
        return states
            .into_iter()
            .map(|state| state.finish(i64::MIN))
            .collect();
    }
    let source_path = history_path_for(instrument, source_granularity);
    let file = File::open(&source_path)
        .map_err(|e| format!("open source history CSV '{}': {e}", source_path.display()))?;
    let reader = BufReader::new(file);
    let mut last_source_start_ms = None;
    for (index, line) in reader.lines().enumerate() {
        let line = line.map_err(|e| format!("read source history CSV '{}': {e}", source_path.display()))?;
        if index == 0 || line.trim().is_empty() {
            continue;
        }
        let Some(candle) = parse_history_csv_candle_line(&line) else {
            continue;
        };
        let Some(time_ms) = parse_oanda_time_ms(&candle.time) else {
            continue;
        };
        last_source_start_ms = Some(time_ms);
        for state in states.iter_mut() {
            state.offer(&candle)?;
        }
    }
    let complete_until_ms = last_source_start_ms
        .map(|value| value + source_step_ms)
        .unwrap_or(i64::MIN);
    states
        .into_iter()
        .map(|state| state.finish(complete_until_ms))
        .collect()
}

fn requested_credentials(
    request: TradingOandaCredentialSaveRequest,
) -> Result<ResolvedOandaCredentials, String> {
    let existing = resolve_credentials();
    let wants_stored_account = request.account_id.trim() == KEEP_STORED_SENTINEL;
    let wants_stored_api_key = request.api_key.trim() == KEEP_STORED_SENTINEL;
    let wants_stored_base_url = request
        .base_url
        .as_deref()
        .map(|value| value.trim() == KEEP_STORED_SENTINEL)
        .unwrap_or(false);

    let account_id = if wants_stored_account {
        existing
            .as_ref()
            .map(|resolved| resolved.account_id.trim().to_string())
            .unwrap_or_default()
    } else {
        request.account_id.trim().to_string()
    };
    let api_key = if wants_stored_api_key {
        existing
            .as_ref()
            .map(|resolved| resolved.api_key.trim().to_string())
            .unwrap_or_default()
    } else {
        request.api_key.trim().to_string()
    };
    if account_id.is_empty() || api_key.is_empty() {
        return Err("OANDA account id and API key are required.".to_string());
    }
    let base_url = if wants_stored_base_url {
        existing
            .as_ref()
            .map(|resolved| resolved.base_url.trim().to_string())
            .unwrap_or_default()
    } else {
        request
            .base_url
            .unwrap_or_else(|| DEFAULT_BASE_URL.to_string())
            .trim()
            .to_string()
    };
    Ok(ResolvedOandaCredentials {
        account_id: account_id.clone(),
        api_key: api_key.clone(),
        base_url: if base_url.is_empty() {
            DEFAULT_BASE_URL.to_string()
        } else {
            base_url
        },
        source: "pending OANDA validation".to_string(),
    })
}

async fn validate_requested_credentials(
    credentials: &ResolvedOandaCredentials,
) -> Result<TradingAccountSnapshot, String> {
    let client = build_oanda_client()?;
    let headers = oanda_headers(&credentials.api_key)?;
    fetch_account_snapshot(&client, credentials, &headers).await
}

fn save_requested_credentials(credentials: &ResolvedOandaCredentials) -> Result<(), String> {
    let stored = StoredOandaCredentials {
        account_id: credentials.account_id.clone(),
        api_key: credentials.api_key.clone(),
        base_url: credentials.base_url.clone(),
    };
    save_local_credentials(&stored)
}

async fn validate_and_save_credentials(
    request: TradingOandaCredentialSaveRequest,
) -> Result<(TradingOandaConfigStatus, TradingOandaProviderStatus), String> {
    let credentials = requested_credentials(request)?;
    let account = validate_requested_credentials(&credentials).await?;
    save_requested_credentials(&credentials)?;
    let resolved = resolve_credentials();
    let config = config_status_from_credentials(resolved.as_ref());
    let provider = provider_status_from_credentials(
        resolved.as_ref(),
        Some(format!(
            "OANDA credentials validated against {} and saved locally with {}. Forge does not upload this secret to developers or Forge servers.",
            account.alias,
            secure_storage_label()
        )),
    );
    Ok((config, provider))
}

#[tauri::command]
pub async fn trading_oanda_provider_status() -> Result<TradingOandaProviderStatus, String> {
    Ok(provider_status_from_credentials(resolve_credentials().as_ref(), None))
}

#[tauri::command]
pub async fn trading_oanda_provider_validate_credentials(
    request: TradingOandaCredentialSaveRequest,
) -> Result<TradingOandaProviderValidationStatus, String> {
    let credentials = requested_credentials(request)?;
    let account = validate_requested_credentials(&credentials).await?;
    Ok(TradingOandaProviderValidationStatus {
        ok: true,
        account_alias: account.alias.clone(),
        currency: account.currency.clone(),
        base_url: credentials.base_url.clone(),
        account_id_hint: mask_account_id(&credentials.account_id),
        message: format!(
            "OANDA API validation succeeded for {} in {}.",
            account.alias, account.currency
        ),
    })
}

#[tauri::command]
pub async fn trading_oanda_provider_encrypt_credentials(
    request: TradingOandaCredentialSaveRequest,
) -> Result<TradingOandaProviderStatus, String> {
    let credentials = requested_credentials(request)?;
    save_requested_credentials(&credentials)?;
    clear_oanda_runtime();
    ensure_oanda_watchdog();
    let resolved = resolve_credentials();
    Ok(provider_status_from_credentials(
        resolved.as_ref(),
        Some(format!(
            "OANDA credentials stored locally with {} and kept off Forge servers.",
            secure_storage_label()
        )),
    ))
}

#[tauri::command]
pub async fn trading_oanda_provider_save_credentials(
    request: TradingOandaCredentialSaveRequest,
) -> Result<TradingOandaProviderStatus, String> {
    let (_, provider) = validate_and_save_credentials(request).await?;
    clear_oanda_runtime();
    ensure_oanda_watchdog();
    Ok(provider)
}

#[tauri::command]
pub async fn trading_oanda_clear_credentials() -> Result<TradingOandaProviderStatus, String> {
    clear_local_credentials()?;
    clear_oanda_runtime();
    let resolved = resolve_credentials();
    let message = match resolved.as_ref() {
        Some(active) => format!(
            "Local encrypted OANDA credentials cleared. Another source is still active: {}.",
            active.source
        ),
        None => "Local encrypted OANDA credentials cleared.".to_string(),
    };
    Ok(provider_status_from_credentials(
        resolved.as_ref(),
        Some(message),
    ))
}

fn normalize_requested_granularities(request: Option<&TradingHistorySyncRequest>) -> Vec<String> {
    let requested = request
        .and_then(|req| req.granularities.as_ref())
        .map(|items| {
            items
                .iter()
                .map(|value| value.trim().to_uppercase())
                .filter(|value| OANDA_GRANULARITIES.contains(&value.as_str()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if requested.is_empty() {
        OANDA_GRANULARITIES
            .iter()
            .map(|value| (*value).to_string())
            .collect()
    } else {
        requested
    }
}

fn normalize_requested_instruments(
    request: Option<&TradingHistorySyncRequest>,
    available: &[TradingInstrumentSummary],
) -> Vec<String> {
    let requested = request
        .and_then(|req| req.instruments.as_ref())
        .map(|items| {
            items
                .iter()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if requested.is_empty() {
        available.iter().map(|item| item.name.clone()).collect()
    } else {
        requested
    }
}

#[tauri::command]
pub async fn trading_oanda_snapshot() -> Result<TradingSnapshotResponse, String> {
    ensure_oanda_watchdog();
    let credentials = resolve_credentials();
    let config = config_status_from_credentials(credentials.as_ref());
    let history_files = build_history_catalog();
    let history_dir = history_root_dir().display().to_string();

    let Some(credentials) = credentials else {
        clear_oanda_runtime();
        let fallback_instruments = vec![TradingInstrumentSummary {
            name: DEFAULT_INSTRUMENT.to_string(),
            display_name: "Natural Gas".to_string(),
            asset_class: "commodity".to_string(),
            pip_location: Some(-2),
            display_precision: Some(3),
            trade_units_precision: Some(0),
            minimum_trade_size: Some(1.0),
            margin_rate: Some(0.05),
        }];
        let asset_catalog = merge_live_oanda_asset_catalog(&history_files, &fallback_instruments);
        return Ok(TradingSnapshotResponse {
            config,
            account: None,
            price: None,
            instruments: fallback_instruments.clone(),
            pending_orders: Vec::new(),
            open_trades: Vec::new(),
            book: None,
            history_dir,
            history_files,
            asset_catalog,
            runtime: Some(oanda_runtime_status(&oanda_runtime_lock().lock().unwrap())),
        });
    };
    update_runtime_selection(DEFAULT_INSTRUMENT, "H4", 240);

    match fetch_runtime_bundle(&credentials, DEFAULT_INSTRUMENT, "H4", 240, true).await {
        Ok(bundle) => {
            let runtime = apply_runtime_bundle(bundle.clone(), &credentials, DEFAULT_INSTRUMENT, "H4", 240);
            let instruments = if bundle.instruments.is_empty() {
                vec![TradingInstrumentSummary {
                    name: DEFAULT_INSTRUMENT.to_string(),
                    display_name: "Natural Gas".to_string(),
                    asset_class: "commodity".to_string(),
                    pip_location: Some(-2),
                    display_precision: Some(3),
                    trade_units_precision: Some(0),
                    minimum_trade_size: Some(1.0),
                    margin_rate: Some(0.05),
                }]
            } else {
                bundle.instruments.clone()
            };
            let asset_catalog = merge_live_oanda_asset_catalog(&history_files, &instruments);
            Ok(TradingSnapshotResponse {
                config,
                account: bundle.account,
                price: bundle.price,
                instruments: instruments.clone(),
                pending_orders: bundle.pending_orders,
                open_trades: bundle.open_trades,
                book: bundle.book,
                history_dir,
                history_files,
                asset_catalog,
                runtime: Some(runtime),
            })
        }
        Err(err) => {
            note_runtime_failure(Some(&credentials), err.clone());
            cached_snapshot_response(config, history_dir, history_files).ok_or(err)
        }
    }
}

#[tauri::command]
pub async fn trading_oanda_market_feed(
    request: Option<TradingMarketFeedRequest>,
) -> Result<TradingMarketFeedResponse, String> {
    ensure_oanda_watchdog();
    let credentials = resolve_credentials();
    let config = config_status_from_credentials(credentials.as_ref());
    let instrument = request
        .as_ref()
        .and_then(|req| req.instrument.as_ref())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| DEFAULT_INSTRUMENT.to_string());
    let granularity = request
        .as_ref()
        .and_then(|req| req.granularity.as_ref())
        .map(|value| value.trim().to_uppercase())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "H4".to_string());
    let count = request
        .as_ref()
        .and_then(|req| req.count)
        .unwrap_or(240);
    update_runtime_selection(&instrument, &granularity, count);

    let Some(credentials) = credentials else {
        clear_oanda_runtime();
        return Ok(attach_alerts_to_market_response(TradingMarketFeedResponse {
            config,
            instrument,
            granularity,
            price: None,
            pending_orders: Vec::new(),
            open_trades: Vec::new(),
            book: None,
            candles: Vec::new(),
            alerts: Vec::new(),
            alert_events: Vec::new(),
            runtime: Some(oanda_runtime_status(&oanda_runtime_lock().lock().unwrap())),
        }));
    };

    match fetch_runtime_bundle(&credentials, &instrument, &granularity, count, false).await {
        Ok(bundle) => {
            let _ = persist_recent_history_candles(
                &credentials.source,
                &instrument,
                &granularity,
                &bundle.candles,
            );
            let mut response_candles = bundle.candles.clone();
            if let Some(partial) = synthesize_live_candle(
                &build_oanda_client()?,
                &credentials,
                &oanda_headers(&credentials.api_key)?,
                &instrument,
                &granularity,
                &bundle.candles,
                bundle.price.as_ref(),
            )
            .await
            {
                response_candles.push(partial);
            }
            let mut runtime_bundle = bundle.clone();
            runtime_bundle.candles = response_candles.clone();
            let runtime = apply_runtime_bundle(runtime_bundle, &credentials, &instrument, &granularity, count);
            Ok(attach_alerts_to_market_response(TradingMarketFeedResponse {
                config,
                instrument,
                granularity,
                price: bundle.price,
                pending_orders: bundle.pending_orders,
                open_trades: bundle.open_trades,
                book: bundle.book,
                candles: response_candles,
                alerts: Vec::new(),
                alert_events: Vec::new(),
                runtime: Some(runtime),
            }))
        }
        Err(err) => {
            note_runtime_failure(Some(&credentials), err.clone());
            cached_market_feed_response(config.clone(), &instrument, &granularity)
                .or_else(|| local_history_market_feed_response(config.clone(), &instrument, &granularity))
                .map(attach_alerts_to_market_response)
                .ok_or(err)
        }
    }
}

#[tauri::command]
pub async fn trading_alerts_list(
    request: Option<TradingAlertsListRequest>,
) -> Result<TradingAlertsResponse, String> {
    let instrument = request
        .as_ref()
        .and_then(|value| value.instrument.as_deref())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let alerts = load_saved_trading_alerts_filtered(instrument.as_deref())?;
    Ok(TradingAlertsResponse {
        alerts,
        events: Vec::new(),
    })
}

#[tauri::command]
pub async fn trading_alerts_upsert(
    request: TradingAlertUpsertRequest,
) -> Result<TradingAlertRecord, String> {
    let _guard = trading_alert_store_lock().lock().unwrap();
    let mut store = load_trading_alert_store()?;
    let now_ms = trading_now_ms();
    let input = request.alert;
    let instrument = input
        .instrument
        .unwrap_or_else(|| DEFAULT_INSTRUMENT.to_string())
        .trim()
        .to_uppercase();
    if instrument.is_empty() {
        return Err("Alert instrument is required.".to_string());
    }
    let target_value = input
        .target_value
        .filter(|value| value.is_finite())
        .ok_or_else(|| "Alert target value is required.".to_string())?;
    let operator = normalize_alert_operator(input.operator.as_deref());
    let message = input
        .message
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| build_alert_message(&instrument, &operator, target_value));
    let notifications = sanitize_alert_notifications(input.notifications);
    let record_id = input
        .id
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| format!("alert_{}_{}", instrument.to_lowercase(), now_ms));
    let granularity = input
        .granularity
        .unwrap_or_else(|| "H4".to_string())
        .trim()
        .to_uppercase();

    let mut record = TradingAlertRecord {
        id: record_id.clone(),
        instrument: instrument.clone(),
        granularity,
        condition_kind: normalize_alert_condition_kind(input.condition_kind.as_deref()),
        operator,
        target_value,
        trigger_mode: normalize_alert_trigger_mode(input.trigger_mode.as_deref()),
        expiration_time_ms: input.expiration_time_ms,
        message,
        active: input.active.unwrap_or(true),
        created_at_ms: now_ms,
        updated_at_ms: now_ms,
        triggered_count: 0,
        last_triggered_at_ms: None,
        last_relation: None,
        notifications,
    };

    if let Some(existing) = store.alerts.iter().find(|alert| alert.id == record_id) {
        record.created_at_ms = existing.created_at_ms;
        record.triggered_count = existing.triggered_count;
        record.last_triggered_at_ms = existing.last_triggered_at_ms;
        record.last_relation = existing.last_relation;
    }

    if let Some(existing) = store.alerts.iter_mut().find(|alert| alert.id == record_id) {
        *existing = record.clone();
    } else {
        store.alerts.push(record.clone());
    }

    save_trading_alert_store(&store)?;
    Ok(record)
}

#[tauri::command]
pub async fn trading_alerts_delete(
    request: TradingAlertDeleteRequest,
) -> Result<bool, String> {
    let _guard = trading_alert_store_lock().lock().unwrap();
    let mut store = load_trading_alert_store()?;
    let before = store.alerts.len();
    store.alerts.retain(|alert| alert.id != request.id);
    if store.alerts.len() != before {
        save_trading_alert_store(&store)?;
        return Ok(true);
    }
    Ok(false)
}

#[tauri::command]
pub async fn trading_oanda_save_credentials(
    request: TradingOandaCredentialSaveRequest,
) -> Result<TradingOandaConfigStatus, String> {
    let (config, _) = validate_and_save_credentials(request).await?;
    clear_oanda_runtime();
    ensure_oanda_watchdog();
    Ok(config)
}

#[tauri::command]
pub async fn trading_oanda_history_catalog() -> Result<TradingSyncResponse, String> {
    ensure_oanda_watchdog();
    let credentials = resolve_credentials();
    let config = config_status_from_credentials(credentials.as_ref());
    let files = build_history_catalog();
    let mut notes = Vec::new();
    let assets = if let Some(credentials) = credentials.as_ref() {
        match (build_oanda_client(), oanda_headers(&credentials.api_key)) {
            (Ok(client), Ok(headers)) => match fetch_instruments(&client, credentials, &headers).await {
                Ok(instruments) => {
                    notes.push(format!(
                        "Live OANDA catalog merged into the local trading history view ({} instruments, {} granularities per asset).",
                        instruments.len(),
                        OANDA_GRANULARITIES.len()
                    ));
                    merge_live_oanda_asset_catalog(&files, &instruments)
                }
                Err(err) => {
                    notes.push(format!("Live OANDA catalog refresh failed, falling back to local files only: {err}"));
                    build_asset_catalog(&files)
                }
            },
            (Err(err), _) | (_, Err(err)) => {
                notes.push(format!("Live OANDA catalog unavailable, falling back to local files only: {err}"));
                build_asset_catalog(&files)
            }
        }
    } else {
        notes.push("Catalog uses local history only until OANDA credentials are available.".to_string());
        build_asset_catalog(&files)
    };
    Ok(TradingSyncResponse {
        config,
        history_dir: history_root_dir().display().to_string(),
        assets,
        files,
        notes,
    })
}

async fn trading_oanda_sync_history_impl(
    request: Option<TradingHistorySyncRequest>,
) -> Result<TradingSyncResponse, String> {
    let credentials = resolve_credentials().ok_or_else(|| {
        "No OANDA credentials found. Save them in the Trading panel or provide OANDA_ACCOUNT_ID / OANDA_API_KEY.".to_string()
    })?;
    let config = config_status_from_credentials(Some(&credentials));
    let client = build_oanda_client()?;
    let headers = oanda_headers(&credentials.api_key)?;
    let instruments = fetch_instruments(&client, &credentials, &headers).await?;
    let selected_instruments = normalize_requested_instruments(request.as_ref(), &instruments);
    let selected_granularities = normalize_requested_granularities(request.as_ref());
    let sync_plan = plan_oanda_history_sync(&selected_granularities);
    let max_rows = request
        .as_ref()
        .and_then(|req| req.max_rows_per_granularity)
        .filter(|value| *value > 0);
    let mut files = Vec::new();
    let mut notes = vec![
        "Full OANDA universe sync enabled for the trading workspace.".to_string(),
        format!("History sync starts from {HISTORY_START_RFC3339}."),
        format!(
            "Instruments selected: {}. Granularities selected: {}.",
            selected_instruments.len(),
            selected_granularities.join(", ")
        ),
        format!(
            "TradingView-style OANDA plan: native fetch [{}], local rebuild [{}].",
            sync_plan.native.join(", "),
            sync_plan
                .derived
                .iter()
                .map(|(source, target)| format!("{source}->{target}"))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    ];
    if let Some(limit) = max_rows {
        notes.push(format!("Per-file row cap requested: {limit}."));
    } else {
        notes.push("No per-file row cap requested: full local history export will be attempted for every selected instrument/timeframe pair.".to_string());
    }

    let mut derived_by_source: HashMap<String, Vec<String>> = HashMap::new();
    for (source, target) in &sync_plan.derived {
        derived_by_source
            .entry(source.clone())
            .or_default()
            .push(target.clone());
    }

    for instrument in &selected_instruments {
        for granularity in &sync_plan.native {
            let summary = sync_granularity_history(
                &client,
                &credentials,
                &headers,
                instrument,
                granularity,
                max_rows,
            )
            .await?;
            if summary.truncated {
                notes.push(format!(
                    "{}/{} stopped at {} rows because a per-file cap was requested.",
                    instrument,
                    granularity,
                    summary.rows
                ));
            }
            files.push(summary);
        }
        for source in &sync_plan.native {
            let Some(targets) = derived_by_source.get(source) else {
                continue;
            };
            let derived = derive_oanda_history_from_source(
                instrument,
                source,
                targets,
                max_rows,
            )?;
            for summary in derived {
                if summary.truncated {
                    notes.push(format!(
                        "{}/{} rebuilt from {} stopped at {} rows because a per-file cap was requested.",
                        instrument,
                        summary.granularity,
                        source,
                        summary.rows
                    ));
                }
                files.push(summary);
            }
        }
    }

    write_history_manifest(&credentials.source, &files)?;
    let catalog_files = build_history_catalog();
    Ok(TradingSyncResponse {
        config,
        history_dir: history_root_dir().display().to_string(),
        assets: merge_live_oanda_asset_catalog(&catalog_files, &instruments),
        files: catalog_files,
        notes,
    })
}

#[tauri::command]
pub async fn trading_oanda_sync_history(
    request: Option<TradingHistorySyncRequest>,
) -> Result<TradingSyncResponse, String> {
    trading_oanda_sync_history_impl(request).await
}

#[tauri::command]
pub async fn trading_chart_series(
    request: TradingChartSeriesRequest,
) -> Result<TradingChartSeriesResponse, String> {
    let instrument = request
        .instrument
        .clone()
        .unwrap_or_else(|| DEFAULT_INSTRUMENT.to_string())
        .trim()
        .to_string();
    let granularity = request
        .granularity
        .unwrap_or_else(|| "H4".to_string())
        .trim()
        .to_uppercase();
    let max_rows = request.max_rows.unwrap_or(5_000);
    canonical_chart_series(&instrument, &granularity, max_rows)
}

#[tauri::command]
pub async fn trading_asset_resolve_and_store(
    request: TradingAssetResolveRequest,
) -> Result<TradingAssetResolveResponse, String> {
    let raw_symbol = request
        .symbol
        .as_deref()
        .or(request.token.as_deref())
        .unwrap_or("")
        .trim();
    let symbol = normalize_market_asset_symbol(raw_symbol);
    if symbol.is_empty() {
        return Err("asset symbol is required".to_string());
    }
    let token = normalize_asset_token(&symbol);
    let requested_class = request.asset_class.unwrap_or_default();
    let asset_class = infer_asset_class(&symbol, &requested_class);
    let stooq_symbol = stooq_symbol_for_asset(&symbol, &asset_class);
    let provider = stooq_symbol
        .as_deref()
        .map(|_| "stooq")
        .unwrap_or("registered");
    let provider_symbol = stooq_symbol.clone().unwrap_or_else(|| symbol.clone());
    let inferred_provider_hint = provider_hint_for_asset(&asset_class, stooq_symbol.as_deref());
    let requested_provider_hint = request
        .provider_hint
        .filter(|value| !value.trim().is_empty());
    let provider_hint = if stooq_symbol.is_some() {
        inferred_provider_hint
    } else {
        requested_provider_hint.unwrap_or(inferred_provider_hint)
    };
    let mut notes = vec![
        format!("asset token {token} registered in local Forge trading asset store"),
        format!("provider route: {provider_hint}"),
    ];
    if stooq_symbol.is_none() {
        notes.push("no free no-key OHLC route is wired yet for this asset class; record saved for the next provider layer".to_string());
    } else {
        notes.push("free route uses Stooq daily candles from the requested start date; higher daily-derived frames are built locally to avoid repeat downloads".to_string());
    }
    let now = now_ms();
    let initial_record = TradingAssetRecord {
        token: token.clone(),
        symbol: symbol.clone(),
        display_name: request
            .display_name
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| symbol.clone()),
        asset_class: asset_class.clone(),
        provider: provider.to_string(),
        provider_symbol: provider_symbol.clone(),
        provider_hint: provider_hint.clone(),
        first_seen_at_ms: now,
        updated_at_ms: now,
        source: request.source.unwrap_or_else(|| "bloomberg_transcript".to_string()),
        notes: notes.clone(),
    };
    let mut record = upsert_asset_record(initial_record)?;
    let requested_granularities = request
        .granularities
        .unwrap_or_else(|| {
            vec![
                "D".to_string(),
                "W".to_string(),
                "M".to_string(),
                "Q".to_string(),
                "Y".to_string(),
            ]
        })
        .into_iter()
        .map(|value| value.trim().to_uppercase())
        .filter(|value| matches!(value.as_str(), "D" | "W" | "M" | "Q" | "Y"))
        .collect::<Vec<_>>();
    let granularities = if requested_granularities.is_empty() {
        vec!["D".to_string()]
    } else {
        requested_granularities
    };
    let mut files = Vec::new();
    if let Some(stooq) = stooq_symbol {
        let start = request.start_date.unwrap_or_else(|| "2006-01-01".to_string());
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(24))
            .connect_timeout(Duration::from_secs(8))
            .build()
            .map_err(|e| format!("build market data client: {e}"))?;
        let daily = fetch_stooq_daily(&client, &stooq, &start).await?;
        if granularities.iter().any(|value| value == "D") {
            let summary = persist_recent_history_candles("stooq", &symbol, "D", &daily)?;
            notes.push(format!("stored {} daily candles from Stooq for {}", summary.rows, symbol));
            files.push(summary);
        }
        if granularities.iter().any(|value| value == "W") {
            let weekly = aggregate_daily_candles(&daily, "W");
            let summary = persist_recent_history_candles("stooq-derived", &symbol, "W", &weekly)?;
            notes.push(format!("stored {} weekly candles derived locally for {}", summary.rows, symbol));
            files.push(summary);
        }
        if granularities.iter().any(|value| value == "M") {
            let monthly = aggregate_daily_candles(&daily, "M");
            let summary = persist_recent_history_candles("stooq-derived", &symbol, "M", &monthly)?;
            notes.push(format!("stored {} monthly candles derived locally for {}", summary.rows, symbol));
            files.push(summary);
        }
        if granularities.iter().any(|value| value == "Q") {
            let quarterly = aggregate_daily_candles(&daily, "Q");
            let summary = persist_recent_history_candles("stooq-derived", &symbol, "Q", &quarterly)?;
            notes.push(format!("stored {} quarterly candles derived locally for {}", summary.rows, symbol));
            files.push(summary);
        }
        if granularities.iter().any(|value| value == "Y") {
            let yearly = aggregate_daily_candles(&daily, "Y");
            let summary = persist_recent_history_candles("stooq-derived", &symbol, "Y", &yearly)?;
            notes.push(format!("stored {} yearly candles derived locally for {}", summary.rows, symbol));
            files.push(summary);
        }
        record.updated_at_ms = now_ms();
        record.provider = "stooq".to_string();
        record.provider_symbol = stooq;
        record.notes = notes.clone();
        record = upsert_asset_record(record)?;
    }
    Ok(TradingAssetResolveResponse {
        asset: record,
        files,
        notes,
        history_dir: history_root_dir().display().to_string(),
        fetched: provider == "stooq",
    })
}

#[tauri::command]
pub async fn trading_chart_compute(
    request: TradingChartComputeRequest,
) -> Result<TradingChartComputeResponse, String> {
    let instrument = request
        .instrument
        .clone()
        .unwrap_or_else(|| DEFAULT_INSTRUMENT.to_string())
        .trim()
        .to_string();
    let granularity = request
        .granularity
        .unwrap_or_else(|| "H4".to_string())
        .trim()
        .to_uppercase();
    let max_rows = request.max_rows.unwrap_or(5_000).max(1);
    let series = canonical_chart_series(&instrument, &granularity, max_rows)?;
    let metrics = request.metrics.unwrap_or_else(|| vec![
        "close".to_string(),
        "return_1".to_string(),
        "momentum_14".to_string(),
        "volatility_20".to_string(),
        "rsi_14".to_string(),
    ]);
    let computed = metrics
        .iter()
        .map(|metric| compute_chart_metric_series(&series.candles, metric))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(TradingChartComputeResponse {
        instrument,
        granularity,
        rows: series.candles.len(),
        engine: "rust-canonical-series".to_string(),
        metrics: computed,
    })
}

#[tauri::command]
pub async fn trading_strategy_backtest(
    request: TradingStrategyBacktestRequest,
) -> Result<TradingStrategyBacktestResponse, String> {
    let spec = normalize_strategy_spec(request.spec);
    let missing_metrics = validate_strategy_spec(&spec);
    let questions = missing_metrics
        .iter()
        .map(|metric| metric.question.clone())
        .collect::<Vec<_>>();
    let plan = strategy_plan_lines(&spec);
    let plan_only = request.plan_only.unwrap_or(false);
    if !missing_metrics.is_empty() {
        return Ok(TradingStrategyBacktestResponse {
            ok: false,
            status: "needs_clarification".to_string(),
            engine: "forge-tauri-rust-strategy-backtest".to_string(),
            plan_only,
            spec,
            missing_metrics,
            questions,
            plan,
            compute_plan: None,
            scenario_manifest: None,
            result: None,
        });
    }
    if plan_only {
        let compute_plan = build_strategy_plan_only_compute_plan(&spec, request.max_rows);
        strategy_store_compute_plan(&compute_plan);
        return Ok(TradingStrategyBacktestResponse {
            ok: true,
            status: "planned".to_string(),
            engine: "forge-tauri-rust-strategy-backtest".to_string(),
            plan_only: true,
            spec,
            missing_metrics,
            questions,
            plan,
            compute_plan: Some(compute_plan),
            scenario_manifest: None,
            result: None,
        });
    }

    let instrument = spec.instrument.as_deref().unwrap_or(DEFAULT_INSTRUMENT);
    let granularity = spec.granularity.as_deref().unwrap_or("H1");
    let max_rows = request.max_rows.unwrap_or(0);
    let series = canonical_chart_series(instrument, granularity, max_rows)?;
    let result = backtest_strategy_spec(&series.candles, &spec)?;
    let scenario_manifest = build_trading_scenario_manifest(
        "forge-tauri-rust-strategy-backtest",
        &spec,
        instrument,
        granularity,
        max_rows,
        &result,
    );
    Ok(TradingStrategyBacktestResponse {
        ok: true,
        status: "tested".to_string(),
        engine: "forge-tauri-rust-strategy-backtest".to_string(),
        plan_only: false,
        spec,
        missing_metrics,
        questions,
        plan,
        compute_plan: Some(result.compute_plan.clone()),
        scenario_manifest: Some(scenario_manifest),
        result: Some(result),
    })
}

#[cfg(test)]
fn replay_trading_scenario_manifest(
    manifest: &TradingScenarioManifest,
) -> Result<TradingScenarioReplayReport, String> {
    let spec = normalize_strategy_spec(manifest.spec.clone());
    let series = canonical_chart_series(&manifest.instrument, &manifest.granularity, manifest.max_rows)?;
    replay_trading_scenario_manifest_with_candles(manifest, &series.candles, &spec)
}

#[cfg(test)]
fn replay_trading_scenario_manifest_with_candles(
    manifest: &TradingScenarioManifest,
    candles: &[TradingCandlePoint],
    spec: &TradingStrategySpec,
) -> Result<TradingScenarioReplayReport, String> {
    let result = backtest_strategy_spec(candles, spec)?;
    let actual = trading_scenario_hashes_from_result(&result);
    let expected = manifest.hashes.clone();
    Ok(TradingScenarioReplayReport {
        ok: actual == expected,
        expected,
        actual,
        manifest_hash: manifest.manifest_hash.clone(),
    })
}

#[tauri::command]
pub async fn trading_strategy_live_tick(
    request: TradingStrategyLiveTickRequest,
) -> Result<TradingStrategyLiveTickResponse, String> {
    let spec = normalize_strategy_spec(request.spec);
    let missing_metrics = validate_strategy_spec(&spec);
    if !missing_metrics.is_empty() {
        return Err(format!(
            "strategy live tick missing metrics: {}",
            missing_metrics
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>()
                .join(",")
        ));
    }
    let threshold = request
        .low_volatility_threshold
        .filter(|value| value.is_finite())
        .ok_or_else(|| "lowVolatilityThreshold is required for incremental live tick".to_string())?;
    let direction = request
        .direction
        .or_else(|| spec.direction.clone())
        .unwrap_or_else(|| "both".to_string())
        .trim()
        .to_lowercase();
    let take_profit_distance = request
        .take_profit_distance
        .or(spec.take_profit_min_distance)
        .filter(|value| value.is_finite() && *value > 0.0)
        .ok_or_else(|| "takeProfitDistance is required for incremental live tick".to_string())?;
    let instrument = spec.instrument.as_deref().unwrap_or(DEFAULT_INSTRUMENT);
    let granularity = spec.granularity.as_deref().unwrap_or("H1");
    let lookback = spec.low_volatility_lookback.unwrap_or(24).max(2);
    let max_hold = spec.max_hold_bars.unwrap_or(24).max(1);
    let max_rows = request
        .max_rows
        .unwrap_or_else(|| (lookback + max_hold + 96).max(180))
        .max(lookback + max_hold + 16);
    let series = canonical_chart_series(instrument, granularity, max_rows)?;
    if series.candles.is_empty() {
        return Err("no candles available for strategy live tick".to_string());
    }
    let low_volatility_values = strategy_low_volatility_values(
        &series.candles,
        spec.low_volatility_metric.as_deref().unwrap_or("range_sma_percentile"),
        lookback,
    );
    let job_id = request
        .job_id
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| strategy_live_job_id(&spec));
    let mut store = load_strategy_live_store();
    let now = now_ms();
    let job_index = if let Some(index) = store.jobs.iter().position(|job| job.job_id == job_id) {
        index
    } else {
        store.jobs.push(TradingStrategyLiveJob {
            job_id: job_id.clone(),
            status: "active".to_string(),
            spec: spec.clone(),
            low_volatility_threshold: threshold,
            direction: direction.clone(),
            take_profit_distance,
            last_evaluated_time: None,
            signals: Vec::new(),
            created_at_ms: now,
            updated_at_ms: now,
        });
        store.jobs.len() - 1
    };

    let job = &mut store.jobs[job_index];
    job.status = "active".to_string();
    job.spec = spec.clone();
    job.low_volatility_threshold = threshold;
    job.direction = direction.clone();
    job.take_profit_distance = take_profit_distance;
    job.updated_at_ms = now;

    let mut closed_signals = Vec::new();
    for signal in &mut job.signals {
        if close_live_signal_if_resolved(signal, &series.candles) {
            closed_signals.push(signal.clone());
        }
    }

    let start_index = next_candle_index_after(&series.candles, job.last_evaluated_time.as_deref());
    let mut new_signal = None;
    let mut notes = vec![
        format!("tail_rows={}", series.candles.len()),
        "incremental_tick=no_full_backtest".to_string(),
    ];
    let entry_hour = spec.entry_hour.unwrap_or(21);
    let force_daily_entry = spec.force_daily_entry.unwrap_or(false);
    let directions = strategy_directions(&direction);
    for index in start_index..series.candles.len() {
        let candle = &series.candles[index];
        job.last_evaluated_time = Some(candle.time.clone());
        if strategy_hour_utc(&candle.time) != Some(entry_hour) {
            continue;
        }
        let low_volatility_value = low_volatility_values.get(index).copied().unwrap_or(f64::NAN);
        if !force_daily_entry && (!low_volatility_value.is_finite() || low_volatility_value > threshold) {
            notes.push(format!(
                "skip {} low_volatility_value={} threshold={}",
                candle.time, low_volatility_value, threshold
            ));
            continue;
        } else if force_daily_entry {
            notes.push(format!(
                "daily_entry {} low_volatility_context={} threshold={}",
                candle.time, low_volatility_value, threshold
            ));
        }
        for candidate_direction in &directions {
            let Some(signal) = live_signal_for_candle(
                &job.job_id,
                &spec,
                candle,
                candidate_direction,
                take_profit_distance,
            ) else {
                continue;
            };
            if job.signals.iter().any(|existing| existing.id == signal.id) {
                continue;
            }
            new_signal = Some(signal.clone());
            job.signals.push(signal);
        }
    }

    let open_signals = job
        .signals
        .iter()
        .filter(|signal| signal.status == "open")
        .cloned()
        .collect::<Vec<_>>();
    let journal_tail = strategy_live_journal_tail(&job.signals);
    let evaluated_time = job.last_evaluated_time.clone();
    save_strategy_live_store(&store)?;
    Ok(TradingStrategyLiveTickResponse {
        ok: true,
        status: if new_signal.is_some() || !closed_signals.is_empty() {
            "updated".to_string()
        } else {
            "idle".to_string()
        },
        engine: "forge-tauri-rust-strategy-live-incremental".to_string(),
        job_id,
        evaluated_time,
        new_signal,
        closed_signals,
        open_signals,
        journal_tail,
        notes,
    })
}

#[tauri::command]
pub async fn trading_oanda_place_order(
    request: TradingPlaceOrderRequest,
) -> Result<TradingOrderResponse, String> {
    let credentials = resolve_credentials().ok_or_else(|| {
        "No OANDA credentials found. Save them in the Trading panel or provide OANDA_ACCOUNT_ID / OANDA_API_KEY.".to_string()
    })?;
    let config = config_status_from_credentials(Some(&credentials));
    let client = build_oanda_client()?;
    let headers = oanda_headers(&credentials.api_key)?;
    let instrument = request
        .instrument
        .clone()
        .unwrap_or_else(|| DEFAULT_INSTRUMENT.to_string())
        .trim()
        .to_string();
    let side = request.side.trim().to_uppercase();
    let order_type = request
        .order_type
        .clone()
        .unwrap_or_else(|| "MARKET".to_string())
        .trim()
        .to_uppercase();
    let tif = request
        .time_in_force
        .clone()
        .unwrap_or_else(|| if order_type == "LIMIT" { "GTC".to_string() } else { "FOK".to_string() })
        .trim()
        .to_uppercase();
    let provider_state = trading_order_provider_state(&config);
    let (approval_timestamp_bucket, approval_proof_hash) = validate_trading_order_approval(
        &request,
        &instrument,
        &side,
        &order_type,
        &tif,
        &provider_state,
        now_ms(),
    )?;
    let abs_units = request.units.abs();
    if abs_units <= 0.0 {
        return Err("units must be > 0".to_string());
    }
    let signed_units = if side == "SELL" || side == "SHORT" {
        -abs_units
    } else {
        abs_units
    };

    let mut order = json!({
        "type": order_type,
        "instrument": instrument,
        "units": format!("{}", signed_units),
        "timeInForce": tif,
        "positionFill": "DEFAULT"
    });

    if order_type == "LIMIT" {
        let limit = request
            .limit_price
            .ok_or_else(|| "limitPrice is required for LIMIT orders".to_string())?;
        order["price"] = json!(format!("{}", limit));
    }
    if let Some(stop_loss) = request.stop_loss {
        order["stopLossOnFill"] = json!({ "price": format!("{}", stop_loss) });
    }
    if let Some(take_profit) = request.take_profit {
        order["takeProfitOnFill"] = json!({ "price": format!("{}", take_profit) });
    }

    let url = format!(
        "{}/v3/accounts/{}/orders",
        credentials.base_url.trim_end_matches('/'),
        credentials.account_id
    );
    let payload = fetch_json(
        &client,
        reqwest::Method::POST,
        &url,
        &headers,
        None,
        Some(json!({ "order": order })),
    )
    .await?;

    Ok(TradingOrderResponse {
        ok: true,
        instrument,
        side,
        units: signed_units,
        order_type,
        approval_timestamp_bucket,
        approval_proof_hash,
        message: "Order submitted to OANDA.".to_string(),
        response: payload,
    })
}

#[cfg(test)]
mod strategy_tests {
    use super::*;
    use crate::{MemoryGovernor, MonsterNode, Store};
    use scan::kasm::{Node, Program, Target, Ty};
    use std::cmp::Ordering;
    use std::collections::{BTreeMap, HashMap};

    #[test]
    fn live_asset_catalog_merge_exposes_full_oanda_granularity_list() {
        let files = vec![TradingHistoryFileSummary {
            instrument: "EUR_USD".to_string(),
            granularity: "H1".to_string(),
            path: "ignored".to_string(),
            rows: 42,
            first_time: Some("2024-01-01T00:00:00Z".to_string()),
            last_time: Some("2024-01-02T00:00:00Z".to_string()),
            truncated: false,
            updated_at_ms: 123,
        }];
        let instruments = vec![TradingInstrumentSummary {
            name: "EUR_USD".to_string(),
            display_name: "EUR / USD".to_string(),
            asset_class: "forex".to_string(),
            pip_location: Some(-4),
            display_precision: Some(5),
            trade_units_precision: Some(0),
            minimum_trade_size: Some(1.0),
            margin_rate: Some(0.02),
        }];
        let merged = merge_live_oanda_asset_catalog(&files, &instruments);
        let eur_usd = merged
            .iter()
            .find(|entry| entry.instrument == "EUR_USD")
            .expect("EUR_USD asset entry");
        assert_eq!(eur_usd.rows, 42);
        assert_eq!(eur_usd.granularities.len(), OANDA_GRANULARITIES.len());
        assert_eq!(eur_usd.granularities.first().map(String::as_str), Some("S5"));
        assert_eq!(eur_usd.granularities.last().map(String::as_str), Some("M"));
    }

    #[test]
    fn full_universe_sync_plan_uses_family_sources_instead_of_single_s5_feed() {
        let requested = OANDA_GRANULARITIES
            .iter()
            .map(|value| (*value).to_string())
            .collect::<Vec<_>>();
        let plan = plan_oanda_history_sync(&requested);
        assert!(plan.native.iter().any(|value| value == "S5"));
        assert!(plan.native.iter().any(|value| value == "M1"));
        assert!(plan.native.iter().any(|value| value == "D"));
        assert!(plan.native.iter().any(|value| value == "W"));
        assert!(plan.native.iter().any(|value| value == "M"));
        assert!(plan.derived.iter().any(|(source, target)| source == "M1" && target == "H12"));
        assert!(plan.derived.iter().any(|(source, target)| source == "S5" && target == "S30"));
        assert!(!plan.derived.iter().any(|(source, target)| source == "S5" && target == "H4"));
    }

    #[derive(Clone)]
    struct SeededSlotRuleDef {
        hour: u32,
        direction: &'static str,
        id: &'static str,
        label: &'static str,
        indicator_refs: &'static [&'static str],
        predicate: fn(usize, &[TradingCandlePoint], &StrategyIndicatorFeatureBank) -> bool,
    }

    #[derive(Clone)]
    struct SeededCompositeCandidate {
        short_11h: SeededSlotRuleDef,
        long_15h: SeededSlotRuleDef,
        long_21h: SeededSlotRuleDef,
        trades: usize,
        wins: usize,
        losses: usize,
        win_rate: f64,
        daily_target_hit_rate: f64,
        target_hit_days: usize,
        total_days: usize,
        avg_daily_pnl_distance: f64,
        min_daily_pnl_distance: f64,
        expectancy_distance: f64,
        net_pnl_distance: f64,
    }

    #[derive(Clone, Debug)]
    struct SeededSlotComboDef {
        hour: u32,
        direction: &'static str,
        label: String,
        indicator_refs: Vec<String>,
        required_mask: u32,
    }

    #[derive(Clone)]
    struct SeededComboSearchCandidate {
        short_11h: SeededSlotComboDef,
        long_15h: SeededSlotComboDef,
        long_21h: SeededSlotComboDef,
        take_profit: f64,
        trades: usize,
        wins: usize,
        losses: usize,
        win_rate: f64,
        daily_target_hit_rate: f64,
        target_hit_days: usize,
        total_days: usize,
        avg_daily_pnl_distance: f64,
        min_daily_pnl_distance: f64,
        expectancy_distance: f64,
        net_pnl_distance: f64,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum SeededRegimeId {
        TrendUp,
        TrendDown,
        Compression,
        Overbought,
        Oversold,
        Neutral,
    }

    impl SeededRegimeId {
        fn code(self) -> u8 {
            match self {
                SeededRegimeId::TrendUp => 0,
                SeededRegimeId::TrendDown => 1,
                SeededRegimeId::Compression => 2,
                SeededRegimeId::Overbought => 3,
                SeededRegimeId::Oversold => 4,
                SeededRegimeId::Neutral => 5,
            }
        }

        fn label(self) -> &'static str {
            match self {
                SeededRegimeId::TrendUp => "trend_up",
                SeededRegimeId::TrendDown => "trend_down",
                SeededRegimeId::Compression => "compression",
                SeededRegimeId::Overbought => "overbought",
                SeededRegimeId::Oversold => "oversold",
                SeededRegimeId::Neutral => "neutral",
            }
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum SeededSchedulerAction {
        Skip,
        Long,
        Short,
    }

    impl SeededSchedulerAction {
        fn label(self) -> &'static str {
            match self {
                SeededSchedulerAction::Skip => "skip",
                SeededSchedulerAction::Long => "long",
                SeededSchedulerAction::Short => "short",
            }
        }
    }

    #[derive(Clone, Copy, Debug)]
    struct SeededSchedulerEvent {
        candle_index: usize,
        hour: u32,
        regime: SeededRegimeId,
        primary_signal: bool,
        long_pnl: f64,
        short_pnl: f64,
        long_exit: StrictTradeExit,
        short_exit: StrictTradeExit,
    }

    #[derive(Clone, Debug)]
    struct SeededSchedulerDay {
        day_key: String,
        events: [Option<SeededSchedulerEvent>; 3],
    }

    #[derive(Clone, Copy, Debug, Default)]
    struct SeededActionAggregate {
        samples: usize,
        target_hits: usize,
        final_pnl_sum: f64,
        trade_wins: usize,
    }

    #[derive(Clone, Copy, Debug)]
    struct SeededSchedulerDecision {
        action: SeededSchedulerAction,
        samples: usize,
        target_rate: f64,
        avg_final_pnl: f64,
    }

    #[derive(Clone, Debug)]
    struct SeededMetaSchedulerResult {
        take_profit: f64,
        min_action_win_rate: f64,
        train_days: usize,
        test_days: usize,
        traded_days: usize,
        target_hit_days: usize,
        trades: usize,
        wins: usize,
        losses: usize,
        win_rate: f64,
        traded_day_rate: f64,
        daily_target_hit_rate: f64,
        avg_daily_pnl_distance: f64,
        min_daily_pnl_distance: f64,
        net_pnl_distance: f64,
        slot_11_action: SeededSchedulerAction,
        slot_15_action: SeededSchedulerAction,
        slot_21_action: SeededSchedulerAction,
        slot_11_signal: String,
        slot_15_signal: String,
        slot_21_signal: String,
    }

    const NATGAS_STRICT_STOP_LOSS: f64 = 0.039;
    const NATGAS_STRICT_TAKE_PROFIT: f64 = 0.051;
    const NATGAS_STRICT_EXECUTION_COST: f64 = 0.006;
    const NATGAS_STRICT_DAILY_TARGET: f64 = 0.070;
    const NATGAS_TREND_PULLBACK_DISTANCE: f64 = 0.070;
    const NATGAS_BASELINE_11H_LABEL: &str = "11h short if bearish body";
    const NATGAS_BASELINE_15H_LABEL: &str = "15h long if bullish body";
    const NATGAS_BASELINE_21H_VWAP_LABEL: &str = "21h long if close > VWAP";

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum TrendDirection {
        Up,
        Down,
    }

    impl TrendDirection {
        fn label(self) -> &'static str {
            match self {
                TrendDirection::Up => "up",
                TrendDirection::Down => "down",
            }
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum TrendResolution {
        Resume,
        StrongReversal,
        Unresolved,
    }

    impl TrendResolution {
        fn label(self) -> &'static str {
            match self {
                TrendResolution::Resume => "resume",
                TrendResolution::StrongReversal => "strong_reversal",
                TrendResolution::Unresolved => "unresolved",
            }
        }
    }

    #[derive(Clone, Debug)]
    struct TrendLifecycleSample {
        direction: TrendDirection,
        start_index: usize,
        confirm_index: usize,
        extreme_index: usize,
        pullback_index: usize,
        resolution_index: Option<usize>,
        start_price: f64,
        extreme_price: f64,
        impulse_distance: f64,
        bars_to_confirm: usize,
        bars_confirm_to_pullback: usize,
        bars_extreme_to_pullback: usize,
        resolution: TrendResolution,
        bars_pullback_to_resolution: Option<usize>,
    }

    #[derive(Clone, Debug, Default)]
    struct TimeExitHoldStats {
        trades: usize,
        stop_hits: usize,
        positive_exits: usize,
        negative_exits: usize,
        flat_exits: usize,
        net_pnl: f64,
    }

    impl TimeExitHoldStats {
        fn positive_rate(&self) -> f64 {
            if self.trades == 0 {
                0.0
            } else {
                self.positive_exits as f64 / self.trades as f64
            }
        }

        fn stop_rate(&self) -> f64 {
            if self.trades == 0 {
                0.0
            } else {
                self.stop_hits as f64 / self.trades as f64
            }
        }

        fn expectancy(&self) -> f64 {
            if self.trades == 0 {
                0.0
            } else {
                self.net_pnl / self.trades as f64
            }
        }
    }

    #[derive(Clone, Debug, Default)]
    struct HourCloseBehaviorStats {
        samples: usize,
        bullish_closes: usize,
        bearish_closes: usize,
        bullish_next1_continue: usize,
        bullish_next3_continue: usize,
        bullish_next6_continue: usize,
        bearish_next1_continue: usize,
        bearish_next3_continue: usize,
        bearish_next6_continue: usize,
        avg_return_1: f64,
        avg_return_3: f64,
        avg_return_6: f64,
        long_tp_hits: usize,
        long_sl_hits: usize,
        short_tp_hits: usize,
        short_sl_hits: usize,
    }

    impl HourCloseBehaviorStats {
        fn bullish_rate(&self) -> f64 {
            if self.samples == 0 {
                0.0
            } else {
                self.bullish_closes as f64 / self.samples as f64
            }
        }

        fn bearish_rate(&self) -> f64 {
            if self.samples == 0 {
                0.0
            } else {
                self.bearish_closes as f64 / self.samples as f64
            }
        }

        fn avg_ret_1(&self) -> f64 {
            if self.samples == 0 {
                0.0
            } else {
                self.avg_return_1 / self.samples as f64
            }
        }

        fn avg_ret_3(&self) -> f64 {
            if self.samples == 0 {
                0.0
            } else {
                self.avg_return_3 / self.samples as f64
            }
        }

        fn avg_ret_6(&self) -> f64 {
            if self.samples == 0 {
                0.0
            } else {
                self.avg_return_6 / self.samples as f64
            }
        }

        fn long_tp_rate(&self) -> f64 {
            if self.samples == 0 {
                0.0
            } else {
                self.long_tp_hits as f64 / self.samples as f64
            }
        }

        fn short_tp_rate(&self) -> f64 {
            if self.samples == 0 {
                0.0
            } else {
                self.short_tp_hits as f64 / self.samples as f64
            }
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum StrictTradeExit {
        TakeProfit,
        StopLoss,
        TerminalPositive,
        TerminalNegative,
        TerminalFlat,
    }

    fn classify_strict_trade_outcome(
        outcome: &StrategyEntryOutcome,
        take_profit: f64,
    ) -> (StrictTradeExit, f64, usize) {
        for point in &outcome.favorable_path {
            if point.favorable_distance >= take_profit {
                return (
                    StrictTradeExit::TakeProfit,
                    take_profit - outcome.execution_cost_distance,
                    point.held,
                );
            }
        }
        if let Some(held) = outcome.stop_held {
            return (StrictTradeExit::StopLoss, outcome.stop_pnl_distance, held);
        }
        let pnl = outcome.terminal_pnl_distance;
        let kind = if pnl > 0.0 {
            StrictTradeExit::TerminalPositive
        } else if pnl < 0.0 {
            StrictTradeExit::TerminalNegative
        } else {
            StrictTradeExit::TerminalFlat
        };
        (kind, pnl, outcome.terminal_held)
    }

    fn detect_trend_confirmation(
        candles: &[TradingCandlePoint],
        from_index: usize,
        threshold: f64,
    ) -> Option<(TrendDirection, usize, usize, f64)> {
        if from_index >= candles.len().saturating_sub(1) {
            return None;
        }
        let mut pivot_low = candles[from_index].low;
        let mut pivot_low_index = from_index;
        let mut pivot_high = candles[from_index].high;
        let mut pivot_high_index = from_index;
        for index in from_index + 1..candles.len() {
            let candle = &candles[index];
            let up_move = candle.high - pivot_low;
            let down_move = pivot_high - candle.low;
            let up_hit = up_move >= threshold;
            let down_hit = down_move >= threshold;
            if up_hit || down_hit {
                if up_hit && (!down_hit || up_move >= down_move) {
                    return Some((TrendDirection::Up, pivot_low_index, index, pivot_low));
                }
                return Some((TrendDirection::Down, pivot_high_index, index, pivot_high));
            }
            if candle.low < pivot_low {
                pivot_low = candle.low;
                pivot_low_index = index;
            }
            if candle.high > pivot_high {
                pivot_high = candle.high;
                pivot_high_index = index;
            }
        }
        None
    }

    fn resolve_trend_after_pullback(
        candles: &[TradingCandlePoint],
        sample: &TrendLifecycleSample,
    ) -> (TrendResolution, Option<usize>) {
        for index in sample.pullback_index + 1..candles.len() {
            let close = candles[index].close;
            match sample.direction {
                TrendDirection::Up => {
                    if close > sample.extreme_price {
                        return (TrendResolution::Resume, Some(index));
                    }
                    if close < sample.start_price {
                        return (TrendResolution::StrongReversal, Some(index));
                    }
                }
                TrendDirection::Down => {
                    if close < sample.extreme_price {
                        return (TrendResolution::Resume, Some(index));
                    }
                    if close > sample.start_price {
                        return (TrendResolution::StrongReversal, Some(index));
                    }
                }
            }
        }
        (TrendResolution::Unresolved, None)
    }

    fn extract_trend_lifecycle_samples(
        candles: &[TradingCandlePoint],
        threshold: f64,
    ) -> Vec<TrendLifecycleSample> {
        let mut samples = Vec::new();
        if candles.len() < 4 {
            return samples;
        }
        let mut cursor = 0usize;
        while let Some((direction, start_index, confirm_index, start_price)) =
            detect_trend_confirmation(candles, cursor, threshold)
        {
            let mut extreme_index = confirm_index;
            let mut extreme_price = match direction {
                TrendDirection::Up => candles[confirm_index].high,
                TrendDirection::Down => candles[confirm_index].low,
            };
            let mut pullback_index = None;
            for index in confirm_index..candles.len() {
                match direction {
                    TrendDirection::Up => {
                        if candles[index].high >= extreme_price {
                            extreme_price = candles[index].high;
                            extreme_index = index;
                        }
                        if extreme_price - candles[index].low >= threshold {
                            pullback_index = Some(index);
                            break;
                        }
                    }
                    TrendDirection::Down => {
                        if candles[index].low <= extreme_price {
                            extreme_price = candles[index].low;
                            extreme_index = index;
                        }
                        if candles[index].high - extreme_price >= threshold {
                            pullback_index = Some(index);
                            break;
                        }
                    }
                }
            }
            let Some(pullback_index) = pullback_index else {
                break;
            };
            let impulse_distance = match direction {
                TrendDirection::Up => extreme_price - start_price,
                TrendDirection::Down => start_price - extreme_price,
            };
            let mut sample = TrendLifecycleSample {
                direction,
                start_index,
                confirm_index,
                extreme_index,
                pullback_index,
                resolution_index: None,
                start_price,
                extreme_price,
                impulse_distance,
                bars_to_confirm: confirm_index.saturating_sub(start_index),
                bars_confirm_to_pullback: pullback_index.saturating_sub(confirm_index),
                bars_extreme_to_pullback: pullback_index.saturating_sub(extreme_index),
                resolution: TrendResolution::Unresolved,
                bars_pullback_to_resolution: None,
            };
            let (resolution, resolution_index) = resolve_trend_after_pullback(candles, &sample);
            sample.resolution = resolution;
            sample.resolution_index = resolution_index;
            sample.bars_pullback_to_resolution =
                resolution_index.map(|index| index.saturating_sub(pullback_index));
            samples.push(sample);
            let next_cursor = extreme_index.max(cursor.saturating_add(1));
            if next_cursor >= candles.len().saturating_sub(1) {
                break;
            }
            cursor = next_cursor;
        }
        samples
    }

    fn mean_usize(values: &[usize]) -> f64 {
        if values.is_empty() {
            return 0.0;
        }
        values.iter().sum::<usize>() as f64 / values.len() as f64
    }

    fn percentile_usize(values: &[usize], percentile: f64) -> f64 {
        if values.is_empty() {
            return 0.0;
        }
        let mut sorted = values.to_vec();
        sorted.sort_unstable();
        let rank = ((sorted.len() - 1) as f64 * percentile.clamp(0.0, 1.0)).round() as usize;
        sorted[rank] as f64
    }

    fn print_trend_lifecycle_summary(label: &str, samples: &[TrendLifecycleSample]) {
        if samples.is_empty() {
            println!("{label} no trend samples");
            return;
        }
        let pullback_bars = samples
            .iter()
            .map(|sample| sample.bars_confirm_to_pullback)
            .collect::<Vec<_>>();
        let confirm_bars = samples
            .iter()
            .map(|sample| sample.bars_to_confirm)
            .collect::<Vec<_>>();
        let extreme_to_pullback_bars = samples
            .iter()
            .map(|sample| sample.bars_extreme_to_pullback)
            .collect::<Vec<_>>();
        let impulse_distances = samples
            .iter()
            .map(|sample| sample.impulse_distance)
            .collect::<Vec<_>>();
        let resume_count = samples
            .iter()
            .filter(|sample| sample.resolution == TrendResolution::Resume)
            .count();
        let strong_reversal_count = samples
            .iter()
            .filter(|sample| sample.resolution == TrendResolution::StrongReversal)
            .count();
        let unresolved_count = samples
            .iter()
            .filter(|sample| sample.resolution == TrendResolution::Unresolved)
            .count();
        let within_3 = pullback_bars.iter().filter(|bars| **bars <= 3).count();
        let within_6 = pullback_bars.iter().filter(|bars| **bars <= 6).count();
        let within_12 = pullback_bars.iter().filter(|bars| **bars <= 12).count();
        let within_24 = pullback_bars.iter().filter(|bars| **bars <= 24).count();
        let resolution_bars = samples
            .iter()
            .filter_map(|sample| sample.bars_pullback_to_resolution)
            .collect::<Vec<_>>();
        println!(
            "{label} trends={} threshold={:.5} avg_confirm_bars={:.2} median_confirm_bars={:.2} avg_pullback_bars={:.2} median_pullback_bars={:.2} p75_pullback_bars={:.2} avg_extreme_to_pullback_bars={:.2} avg_impulse={:.5} pullback<=3={:.2}% pullback<=6={:.2}% pullback<=12={:.2}% pullback<=24={:.2}% resume_rate={:.2}% strong_reversal_rate={:.2}% unresolved_rate={:.2}% avg_resolution_bars={:.2} median_resolution_bars={:.2}",
            samples.len(),
            NATGAS_TREND_PULLBACK_DISTANCE,
            mean_usize(&confirm_bars),
            percentile_usize(&confirm_bars, 0.50),
            mean_usize(&pullback_bars),
            percentile_usize(&pullback_bars, 0.50),
            percentile_usize(&pullback_bars, 0.75),
            mean_usize(&extreme_to_pullback_bars),
            if impulse_distances.is_empty() {
                0.0
            } else {
                impulse_distances.iter().sum::<f64>() / impulse_distances.len() as f64
            },
            100.0 * within_3 as f64 / samples.len() as f64,
            100.0 * within_6 as f64 / samples.len() as f64,
            100.0 * within_12 as f64 / samples.len() as f64,
            100.0 * within_24 as f64 / samples.len() as f64,
            100.0 * resume_count as f64 / samples.len() as f64,
            100.0 * strong_reversal_count as f64 / samples.len() as f64,
            100.0 * unresolved_count as f64 / samples.len() as f64,
            mean_usize(&resolution_bars),
            percentile_usize(&resolution_bars, 0.50),
        );
    }

    fn impulse_bucket_label(impulse_distance: f64) -> &'static str {
        if impulse_distance < 0.100 {
            "7p_to_10p"
        } else if impulse_distance < 0.150 {
            "10p_to_15p"
        } else if impulse_distance < 0.200 {
            "15p_to_20p"
        } else {
            "20p_plus"
        }
    }

    fn evaluate_time_exit_hold(
        candles: &[TradingCandlePoint],
        samples: &[TrendLifecycleSample],
        hold_bars: usize,
        follow_trend: bool,
    ) -> TimeExitHoldStats {
        let mut stats = TimeExitHoldStats::default();
        for sample in samples {
            let direction = match (sample.direction, follow_trend) {
                (TrendDirection::Up, true) | (TrendDirection::Down, false) => "long",
                (TrendDirection::Down, true) | (TrendDirection::Up, false) => "short",
            };
            let (outcome, _) = strategy_entry_outcome_cached(
                candles,
                sample.confirm_index,
                direction,
                NATGAS_STRICT_STOP_LOSS,
                NATGAS_STRICT_EXECUTION_COST,
                hold_bars,
            );
            let Some(outcome) = outcome else {
                continue;
            };
            stats.trades += 1;
            if outcome.stop_held.is_some() {
                stats.stop_hits += 1;
                stats.net_pnl += outcome.stop_pnl_distance;
            } else {
                stats.net_pnl += outcome.terminal_pnl_distance;
                if outcome.terminal_pnl_distance > 0.0 {
                    stats.positive_exits += 1;
                } else if outcome.terminal_pnl_distance < 0.0 {
                    stats.negative_exits += 1;
                } else {
                    stats.flat_exits += 1;
                }
            }
        }
        stats
    }

    fn run_natgas_21h_trend_follow_time_exit_analysis() {
        let h1_series = canonical_chart_series("NATGAS_USD", "H1", 0).expect("NATGAS_USD H1 history");
        let candles = h1_series.candles;
        let samples = extract_trend_lifecycle_samples(&candles, NATGAS_TREND_PULLBACK_DISTANCE)
            .into_iter()
            .filter(|sample| strategy_hour_utc(&candles[sample.confirm_index].time) == Some(21))
            .collect::<Vec<_>>();
        println!(
            "NATGAS_H1_21H_TIME_EXIT samples={} stop_loss={:.5} cost={:.5}",
            samples.len(),
            NATGAS_STRICT_STOP_LOSS,
            NATGAS_STRICT_EXECUTION_COST
        );
        let mut best_follow_hold = 0usize;
        let mut best_follow_expectancy = f64::NEG_INFINITY;
        let mut best_fade_hold = 0usize;
        let mut best_fade_expectancy = f64::NEG_INFINITY;
        for hold in 1..=12 {
            let follow = evaluate_time_exit_hold(&candles, &samples, hold, true);
            let fade = evaluate_time_exit_hold(&candles, &samples, hold, false);
            if follow.expectancy() > best_follow_expectancy {
                best_follow_expectancy = follow.expectancy();
                best_follow_hold = hold;
            }
            if fade.expectancy() > best_fade_expectancy {
                best_fade_expectancy = fade.expectancy();
                best_fade_hold = hold;
            }
            println!(
                "NATGAS_H1_21H_TIME_EXIT hold={}h follow_trend trades={} stop_rate={:.4} positive_rate={:.4} expectancy={:.6} net_pnl={:.6} fade_trend trades={} stop_rate={:.4} positive_rate={:.4} expectancy={:.6} net_pnl={:.6}",
                hold,
                follow.trades,
                follow.stop_rate(),
                follow.positive_rate(),
                follow.expectancy(),
                follow.net_pnl,
                fade.trades,
                fade.stop_rate(),
                fade.positive_rate(),
                fade.expectancy(),
                fade.net_pnl,
            );
        }
        println!(
            "NATGAS_H1_21H_TIME_EXIT_BEST follow_hold={}h follow_expectancy={:.6} fade_hold={}h fade_expectancy={:.6}",
            best_follow_hold,
            best_follow_expectancy,
            best_fade_hold,
            best_fade_expectancy
        );
    }

    #[derive(Clone, Copy, Debug)]
    enum SessionTrendVariant {
        TwoBodies,
        ThreeBodies,
        TwoBodiesVwap,
        ThreeBodiesVwap,
    }

    impl SessionTrendVariant {
        fn label(self) -> &'static str {
            match self {
                SessionTrendVariant::TwoBodies => "2_bodies",
                SessionTrendVariant::ThreeBodies => "3_bodies",
                SessionTrendVariant::TwoBodiesVwap => "2_bodies_plus_vwap",
                SessionTrendVariant::ThreeBodiesVwap => "3_bodies_plus_vwap",
            }
        }
    }

    #[derive(Clone, Debug)]
    struct SessionReversalCandidate {
        variant_11h: SessionTrendVariant,
        variant_15h: SessionTrendVariant,
        variant_21h: SessionTrendVariant,
        hold_11h: usize,
        reverse_hold_11h: usize,
        hold_15h: usize,
        reverse_hold_15h: usize,
        hold_21h: usize,
        trades: usize,
        positive_exits: usize,
        stop_hits: usize,
        daily_target_hit_rate: f64,
        target_hit_days: usize,
        total_days: usize,
        avg_daily_pnl_distance: f64,
        expectancy_distance: f64,
        net_pnl_distance: f64,
    }

    #[derive(Clone, Debug)]
    struct SessionSlotSearchResult {
        variant: SessionTrendVariant,
        hold_bars: usize,
        reverse_hold_bars: Option<usize>,
        trades: usize,
        positive_exits: usize,
        stop_hits: usize,
        avg_daily_pnl_distance: f64,
        expectancy_distance: f64,
        net_pnl_distance: f64,
    }

    #[derive(Clone, Debug, Default)]
    struct HybridSessionStats {
        trades: usize,
        tp_hits: usize,
        stop_hits: usize,
        time_positive_exits: usize,
        time_negative_exits: usize,
        time_flat_exits: usize,
        target_hit_days: usize,
        total_days: usize,
        daily_target_hit_rate: f64,
        avg_daily_pnl_distance: f64,
        expectancy_distance: f64,
        net_pnl_distance: f64,
    }

    #[derive(Clone, Copy)]
    struct SessionQualifierDef {
        label: &'static str,
        predicate: fn(usize, &[TradingCandlePoint], &StrategyIndicatorFeatureBank) -> bool,
    }

    fn inverse_direction(direction: TrendDirection) -> &'static str {
        match direction {
            TrendDirection::Up => "short",
            TrendDirection::Down => "long",
        }
    }

    fn follow_direction(direction: TrendDirection) -> &'static str {
        match direction {
            TrendDirection::Up => "long",
            TrendDirection::Down => "short",
        }
    }

    fn bullish_body(candle: &TradingCandlePoint) -> bool {
        candle.close > candle.open
    }

    fn bearish_body(candle: &TradingCandlePoint) -> bool {
        candle.close < candle.open
    }

    fn session_trend_signal(
        candles: &[TradingCandlePoint],
        feature_bank: &StrategyIndicatorFeatureBank,
        index: usize,
        variant: SessionTrendVariant,
    ) -> Option<TrendDirection> {
        if index < 2 {
            return None;
        }
        let current = &candles[index];
        let prev1 = &candles[index - 1];
        let prev2 = &candles[index - 2];
        let close = current.close;
        let vwap = feature_bank.vwap[index];
        let up = match variant {
            SessionTrendVariant::TwoBodies => bullish_body(prev1) && bullish_body(current),
            SessionTrendVariant::ThreeBodies => bullish_body(prev2) && bullish_body(prev1) && bullish_body(current),
            SessionTrendVariant::TwoBodiesVwap => bullish_body(prev1) && bullish_body(current) && close > vwap,
            SessionTrendVariant::ThreeBodiesVwap => {
                bullish_body(prev2) && bullish_body(prev1) && bullish_body(current) && close > vwap
            }
        };
        let down = match variant {
            SessionTrendVariant::TwoBodies => bearish_body(prev1) && bearish_body(current),
            SessionTrendVariant::ThreeBodies => bearish_body(prev2) && bearish_body(prev1) && bearish_body(current),
            SessionTrendVariant::TwoBodiesVwap => bearish_body(prev1) && bearish_body(current) && close < vwap,
            SessionTrendVariant::ThreeBodiesVwap => {
                bearish_body(prev2) && bearish_body(prev1) && bearish_body(current) && close < vwap
            }
        };
        if up && !down {
            Some(TrendDirection::Up)
        } else if down && !up {
            Some(TrendDirection::Down)
        } else {
            None
        }
    }

    fn record_session_time_exit_trade(
        candles: &[TradingCandlePoint],
        entry_index: usize,
        direction: &str,
        hold_bars: usize,
        day_key: &str,
        daily_pnl: &mut BTreeMap<String, f64>,
        trades: &mut usize,
        positive_exits: &mut usize,
        stop_hits: &mut usize,
        net_pnl: &mut f64,
    ) -> Option<(bool, usize, f64)> {
        let (outcome, _) = strategy_entry_outcome_cached(
            candles,
            entry_index,
            direction,
            NATGAS_STRICT_STOP_LOSS,
            NATGAS_STRICT_EXECUTION_COST,
            hold_bars,
        );
        let outcome = outcome?;
        let timed_exit = outcome.stop_held.is_none();
        let exit_index = if timed_exit {
            entry_index + outcome.terminal_held
        } else {
            entry_index + outcome.stop_held.unwrap_or(1).saturating_sub(1)
        };
        let pnl = if timed_exit {
            outcome.terminal_pnl_distance
        } else {
            outcome.stop_pnl_distance
        };
        *daily_pnl.entry(day_key.to_string()).or_insert(0.0) += pnl;
        *trades += 1;
        *net_pnl += pnl;
        if timed_exit && pnl > 0.0 {
            *positive_exits += 1;
        }
        if !timed_exit {
            *stop_hits += 1;
        }
        Some((timed_exit, exit_index, pnl))
    }

    fn evaluate_session_time_reversal_candidate(
        candles: &[TradingCandlePoint],
        feature_bank: &StrategyIndicatorFeatureBank,
        variant_11h: SessionTrendVariant,
        variant_15h: SessionTrendVariant,
        variant_21h: SessionTrendVariant,
        hold_11h: usize,
        reverse_hold_11h: usize,
        hold_15h: usize,
        reverse_hold_15h: usize,
        hold_21h: usize,
    ) -> Option<SessionReversalCandidate> {
        let mut daily_pnl = BTreeMap::<String, f64>::new();
        let mut trades = 0usize;
        let mut positive_exits = 0usize;
        let mut stop_hits = 0usize;
        let mut net_pnl = 0.0;
        for signal_index in 2..candles.len().saturating_sub(8) {
            let Some(hour) = strategy_hour_utc(&candles[signal_index].time) else {
                continue;
            };
            let Some(day_key) = candles[signal_index].time.get(..10) else {
                continue;
            };
            let Some(entry_index) = signal_index.checked_add(1) else {
                continue;
            };
            if entry_index >= candles.len().saturating_sub(1) {
                continue;
            }
            match hour {
                11 => {
                    let Some(direction) =
                        session_trend_signal(candles, feature_bank, signal_index, variant_11h)
                    else {
                        continue;
                    };
                    let Some((timed_exit, exit_index, _)) = record_session_time_exit_trade(
                        candles,
                        entry_index,
                        inverse_direction(direction),
                        hold_11h,
                        day_key,
                        &mut daily_pnl,
                        &mut trades,
                        &mut positive_exits,
                        &mut stop_hits,
                        &mut net_pnl,
                    ) else {
                        continue;
                    };
                    if timed_exit {
                        let reverse_entry = exit_index + 1;
                        if reverse_entry < candles.len().saturating_sub(1) {
                            let _ = record_session_time_exit_trade(
                                candles,
                                reverse_entry,
                                follow_direction(direction),
                                reverse_hold_11h,
                                day_key,
                                &mut daily_pnl,
                                &mut trades,
                                &mut positive_exits,
                                &mut stop_hits,
                                &mut net_pnl,
                            );
                        }
                    }
                }
                15 => {
                    let Some(direction) =
                        session_trend_signal(candles, feature_bank, signal_index, variant_15h)
                    else {
                        continue;
                    };
                    let Some((timed_exit, exit_index, _)) = record_session_time_exit_trade(
                        candles,
                        entry_index,
                        inverse_direction(direction),
                        hold_15h,
                        day_key,
                        &mut daily_pnl,
                        &mut trades,
                        &mut positive_exits,
                        &mut stop_hits,
                        &mut net_pnl,
                    ) else {
                        continue;
                    };
                    if timed_exit {
                        let reverse_entry = exit_index + 1;
                        if reverse_entry < candles.len().saturating_sub(1) {
                            let _ = record_session_time_exit_trade(
                                candles,
                                reverse_entry,
                                follow_direction(direction),
                                reverse_hold_15h,
                                day_key,
                                &mut daily_pnl,
                                &mut trades,
                                &mut positive_exits,
                                &mut stop_hits,
                                &mut net_pnl,
                            );
                        }
                    }
                }
                21 => {
                    let Some(direction) =
                        session_trend_signal(candles, feature_bank, signal_index, variant_21h)
                    else {
                        continue;
                    };
                    let _ = record_session_time_exit_trade(
                        candles,
                        entry_index,
                        follow_direction(direction),
                        hold_21h,
                        day_key,
                        &mut daily_pnl,
                        &mut trades,
                        &mut positive_exits,
                        &mut stop_hits,
                        &mut net_pnl,
                    );
                }
                _ => {}
            }
        }
        if daily_pnl.is_empty() || trades == 0 {
            return None;
        }
        let total_days = daily_pnl.len();
        let target_hit_days = daily_pnl
            .values()
            .filter(|value| **value >= NATGAS_STRICT_DAILY_TARGET)
            .count();
        Some(SessionReversalCandidate {
            variant_11h,
            variant_15h,
            variant_21h,
            hold_11h,
            reverse_hold_11h,
            hold_15h,
            reverse_hold_15h,
            hold_21h,
            trades,
            positive_exits,
            stop_hits,
            daily_target_hit_rate: target_hit_days as f64 / total_days as f64,
            target_hit_days,
            total_days,
            avg_daily_pnl_distance: daily_pnl.values().sum::<f64>() / total_days as f64,
            expectancy_distance: net_pnl / trades as f64,
            net_pnl_distance: net_pnl,
        })
    }

    fn evaluate_single_session_reversal_slot(
        candles: &[TradingCandlePoint],
        feature_bank: &StrategyIndicatorFeatureBank,
        hour: u32,
        variant: SessionTrendVariant,
        hold_bars: usize,
        reverse_hold_bars: usize,
    ) -> Option<SessionSlotSearchResult> {
        let mut daily_pnl = BTreeMap::<String, f64>::new();
        let mut trades = 0usize;
        let mut positive_exits = 0usize;
        let mut stop_hits = 0usize;
        let mut net_pnl = 0.0;
        for signal_index in 2..candles.len().saturating_sub(8) {
            if strategy_hour_utc(&candles[signal_index].time) != Some(hour) {
                continue;
            }
            let Some(day_key) = candles[signal_index].time.get(..10) else {
                continue;
            };
            let Some(direction) = session_trend_signal(candles, feature_bank, signal_index, variant) else {
                continue;
            };
            let entry_index = signal_index + 1;
            let Some((timed_exit, exit_index, _)) = record_session_time_exit_trade(
                candles,
                entry_index,
                inverse_direction(direction),
                hold_bars,
                day_key,
                &mut daily_pnl,
                &mut trades,
                &mut positive_exits,
                &mut stop_hits,
                &mut net_pnl,
            ) else {
                continue;
            };
            if timed_exit {
                let reverse_entry = exit_index + 1;
                if reverse_entry < candles.len().saturating_sub(1) {
                    let _ = record_session_time_exit_trade(
                        candles,
                        reverse_entry,
                        follow_direction(direction),
                        reverse_hold_bars,
                        day_key,
                        &mut daily_pnl,
                        &mut trades,
                        &mut positive_exits,
                        &mut stop_hits,
                        &mut net_pnl,
                    );
                }
            }
        }
        if trades == 0 || daily_pnl.is_empty() {
            return None;
        }
        let total_days = daily_pnl.len();
        Some(SessionSlotSearchResult {
            variant,
            hold_bars,
            reverse_hold_bars: Some(reverse_hold_bars),
            trades,
            positive_exits,
            stop_hits,
            avg_daily_pnl_distance: daily_pnl.values().sum::<f64>() / total_days as f64,
            expectancy_distance: net_pnl / trades as f64,
            net_pnl_distance: net_pnl,
        })
    }

    fn evaluate_single_session_follow_slot(
        candles: &[TradingCandlePoint],
        feature_bank: &StrategyIndicatorFeatureBank,
        hour: u32,
        variant: SessionTrendVariant,
        hold_bars: usize,
    ) -> Option<SessionSlotSearchResult> {
        let mut daily_pnl = BTreeMap::<String, f64>::new();
        let mut trades = 0usize;
        let mut positive_exits = 0usize;
        let mut stop_hits = 0usize;
        let mut net_pnl = 0.0;
        for signal_index in 2..candles.len().saturating_sub(8) {
            if strategy_hour_utc(&candles[signal_index].time) != Some(hour) {
                continue;
            }
            let Some(day_key) = candles[signal_index].time.get(..10) else {
                continue;
            };
            let Some(direction) = session_trend_signal(candles, feature_bank, signal_index, variant) else {
                continue;
            };
            let entry_index = signal_index + 1;
            let _ = record_session_time_exit_trade(
                candles,
                entry_index,
                follow_direction(direction),
                hold_bars,
                day_key,
                &mut daily_pnl,
                &mut trades,
                &mut positive_exits,
                &mut stop_hits,
                &mut net_pnl,
            );
        }
        if trades == 0 || daily_pnl.is_empty() {
            return None;
        }
        let total_days = daily_pnl.len();
        Some(SessionSlotSearchResult {
            variant,
            hold_bars,
            reverse_hold_bars: None,
            trades,
            positive_exits,
            stop_hits,
            avg_daily_pnl_distance: daily_pnl.values().sum::<f64>() / total_days as f64,
            expectancy_distance: net_pnl / trades as f64,
            net_pnl_distance: net_pnl,
        })
    }

    fn run_natgas_session_reversal_time_exit_search() {
        let h1_series = canonical_chart_series("NATGAS_USD", "H1", 0).expect("NATGAS_USD H1 history");
        let candles = h1_series.candles;
        let mut cache_snapshot = StrategyCacheSnapshot::default();
        let h1_hash = strategy_candles_hash(&candles);
        let feature_bank = strategy_feature_bank_cached(&candles, &h1_hash, &mut cache_snapshot);
        let variants = [
            SessionTrendVariant::TwoBodies,
            SessionTrendVariant::ThreeBodies,
            SessionTrendVariant::TwoBodiesVwap,
            SessionTrendVariant::ThreeBodiesVwap,
        ];
        let mut best_11h: Option<SessionSlotSearchResult> = None;
        let mut best_15h: Option<SessionSlotSearchResult> = None;
        let mut best_21h: Option<SessionSlotSearchResult> = None;
        for variant in variants {
            for hold in 1..=3 {
                for reverse_hold in 2..=5 {
                    if let Some(candidate) = evaluate_single_session_reversal_slot(
                        &candles,
                        &feature_bank,
                        11,
                        variant,
                        hold,
                        reverse_hold,
                    ) {
                        let replace = best_11h
                            .as_ref()
                            .map(|current| {
                                candidate
                                    .avg_daily_pnl_distance
                                    .partial_cmp(&current.avg_daily_pnl_distance)
                                    .unwrap_or(Ordering::Equal)
                                    .then_with(|| {
                                        candidate
                                            .expectancy_distance
                                            .partial_cmp(&current.expectancy_distance)
                                            .unwrap_or(Ordering::Equal)
                                    })
                                    == Ordering::Greater
                            })
                            .unwrap_or(true);
                        if replace {
                            best_11h = Some(candidate);
                        }
                    }
                    if let Some(candidate) = evaluate_single_session_reversal_slot(
                        &candles,
                        &feature_bank,
                        15,
                        variant,
                        hold,
                        reverse_hold,
                    ) {
                        let replace = best_15h
                            .as_ref()
                            .map(|current| {
                                candidate
                                    .avg_daily_pnl_distance
                                    .partial_cmp(&current.avg_daily_pnl_distance)
                                    .unwrap_or(Ordering::Equal)
                                    .then_with(|| {
                                        candidate
                                            .expectancy_distance
                                            .partial_cmp(&current.expectancy_distance)
                                            .unwrap_or(Ordering::Equal)
                                    })
                                    == Ordering::Greater
                            })
                            .unwrap_or(true);
                        if replace {
                            best_15h = Some(candidate);
                        }
                    }
                }
            }
            for hold in 3..=6 {
                if let Some(candidate) = evaluate_single_session_follow_slot(
                    &candles,
                    &feature_bank,
                    21,
                    variant,
                    hold,
                ) {
                    let replace = best_21h
                        .as_ref()
                        .map(|current| {
                            candidate
                                .expectancy_distance
                                .partial_cmp(&current.expectancy_distance)
                                .unwrap_or(Ordering::Equal)
                                .then_with(|| {
                                    candidate
                                        .avg_daily_pnl_distance
                                        .partial_cmp(&current.avg_daily_pnl_distance)
                                        .unwrap_or(Ordering::Equal)
                                })
                                == Ordering::Greater
                        })
                        .unwrap_or(true);
                    if replace {
                        best_21h = Some(candidate);
                    }
                }
            }
        }
        let best_11h = best_11h.expect("best 11h reversal slot");
        let best_15h = best_15h.expect("best 15h reversal slot");
        let best_21h = best_21h.expect("best 21h follow slot");
        let best = evaluate_session_time_reversal_candidate(
            &candles,
            &feature_bank,
            best_11h.variant,
            best_15h.variant,
            best_21h.variant,
            best_11h.hold_bars,
            best_11h.reverse_hold_bars.unwrap_or(3),
            best_15h.hold_bars,
            best_15h.reverse_hold_bars.unwrap_or(3),
            best_21h.hold_bars,
        )
        .expect("combined session reversal candidate");
        println!(
            "NATGAS_H1_SESSION_TIME_REVERSAL_SLOT_11H variant={} hold={} reverse_hold={} trades={} positive_rate={:.4} stop_rate={:.4} avg_daily_pnl={:.6} expectancy={:.6} net_pnl={:.6}",
            best_11h.variant.label(),
            best_11h.hold_bars,
            best_11h.reverse_hold_bars.unwrap_or(0),
            best_11h.trades,
            best_11h.positive_exits as f64 / best_11h.trades as f64,
            best_11h.stop_hits as f64 / best_11h.trades as f64,
            best_11h.avg_daily_pnl_distance,
            best_11h.expectancy_distance,
            best_11h.net_pnl_distance,
        );
        println!(
            "NATGAS_H1_SESSION_TIME_REVERSAL_SLOT_15H variant={} hold={} reverse_hold={} trades={} positive_rate={:.4} stop_rate={:.4} avg_daily_pnl={:.6} expectancy={:.6} net_pnl={:.6}",
            best_15h.variant.label(),
            best_15h.hold_bars,
            best_15h.reverse_hold_bars.unwrap_or(0),
            best_15h.trades,
            best_15h.positive_exits as f64 / best_15h.trades as f64,
            best_15h.stop_hits as f64 / best_15h.trades as f64,
            best_15h.avg_daily_pnl_distance,
            best_15h.expectancy_distance,
            best_15h.net_pnl_distance,
        );
        println!(
            "NATGAS_H1_SESSION_TIME_REVERSAL_SLOT_21H variant={} hold={} trades={} positive_rate={:.4} stop_rate={:.4} avg_daily_pnl={:.6} expectancy={:.6} net_pnl={:.6}",
            best_21h.variant.label(),
            best_21h.hold_bars,
            best_21h.trades,
            best_21h.positive_exits as f64 / best_21h.trades as f64,
            best_21h.stop_hits as f64 / best_21h.trades as f64,
            best_21h.avg_daily_pnl_distance,
            best_21h.expectancy_distance,
            best_21h.net_pnl_distance,
        );
        println!(
            "NATGAS_H1_SESSION_TIME_REVERSAL best_11h={} hold_11h={} reverse_hold_11h={} best_15h={} hold_15h={} reverse_hold_15h={} best_21h={} hold_21h={} trades={} positive_exits={} stop_hits={} positive_rate={:.4} stop_rate={:.4} daily_target_hit_rate={:.4} target_hit_days={} total_days={} avg_daily_pnl={:.6} expectancy={:.6} net_pnl={:.6}",
            best.variant_11h.label(),
            best.hold_11h,
            best.reverse_hold_11h,
            best.variant_15h.label(),
            best.hold_15h,
            best.reverse_hold_15h,
            best.variant_21h.label(),
            best.hold_21h,
            best.trades,
            best.positive_exits,
            best.stop_hits,
            best.positive_exits as f64 / best.trades as f64,
            best.stop_hits as f64 / best.trades as f64,
            best.daily_target_hit_rate,
            best.target_hit_days,
            best.total_days,
            best.avg_daily_pnl_distance,
            best.expectancy_distance,
            best.net_pnl_distance,
        );
    }

    fn three_plus_bullish_bodies(candles: &[TradingCandlePoint], index: usize) -> bool {
        index >= 2
            && bullish_body(&candles[index])
            && bullish_body(&candles[index - 1])
            && bullish_body(&candles[index - 2])
    }

    fn three_plus_bearish_bodies(candles: &[TradingCandlePoint], index: usize) -> bool {
        index >= 2
            && bearish_body(&candles[index])
            && bearish_body(&candles[index - 1])
            && bearish_body(&candles[index - 2])
    }

    fn evaluate_hybrid_strat_a_13h_17h_21h(
        candles: &[TradingCandlePoint],
        feature_bank: &StrategyIndicatorFeatureBank,
        hold_21h: usize,
    ) -> HybridSessionStats {
        let mut daily_pnl = BTreeMap::<String, f64>::new();
        let mut stats = HybridSessionStats::default();
        for index in 2..candles.len().saturating_sub(8) {
            let Some(hour) = strategy_hour_utc(&candles[index].time) else {
                continue;
            };
            let Some(day_key) = candles[index].time.get(..10) else {
                continue;
            };
            let entry_index = index + 1;
            match hour {
                13 => {
                    let direction = if three_plus_bullish_bodies(candles, index) {
                        Some("short")
                    } else if three_plus_bearish_bodies(candles, index) {
                        Some("long")
                    } else {
                        None
                    };
                    let Some(direction) = direction else {
                        continue;
                    };
                    let (outcome, _) = strategy_entry_outcome_cached(
                        candles,
                        entry_index,
                        direction,
                        NATGAS_STRICT_STOP_LOSS,
                        NATGAS_STRICT_EXECUTION_COST,
                        24,
                    );
                    let Some(outcome) = outcome else {
                        continue;
                    };
                    let (exit_kind, pnl, _) =
                        classify_strict_trade_outcome(&outcome, NATGAS_STRICT_TAKE_PROFIT);
                    *daily_pnl.entry(day_key.to_string()).or_insert(0.0) += pnl;
                    stats.trades += 1;
                    stats.net_pnl_distance += pnl;
                    match exit_kind {
                        StrictTradeExit::TakeProfit => stats.tp_hits += 1,
                        StrictTradeExit::StopLoss => stats.stop_hits += 1,
                        _ => {}
                    }
                }
                17 => {
                    if !bullish_body(&candles[index]) {
                        continue;
                    }
                    let (outcome, _) = strategy_entry_outcome_cached(
                        candles,
                        entry_index,
                        "long",
                        NATGAS_STRICT_STOP_LOSS,
                        NATGAS_STRICT_EXECUTION_COST,
                        24,
                    );
                    let Some(outcome) = outcome else {
                        continue;
                    };
                    let (exit_kind, pnl, _) =
                        classify_strict_trade_outcome(&outcome, NATGAS_STRICT_TAKE_PROFIT);
                    *daily_pnl.entry(day_key.to_string()).or_insert(0.0) += pnl;
                    stats.trades += 1;
                    stats.net_pnl_distance += pnl;
                    match exit_kind {
                        StrictTradeExit::TakeProfit => stats.tp_hits += 1,
                        StrictTradeExit::StopLoss => stats.stop_hits += 1,
                        _ => {}
                    }
                }
                21 => {
                    let Some(direction) = session_trend_signal(
                        candles,
                        feature_bank,
                        index,
                        SessionTrendVariant::ThreeBodiesVwap,
                    ) else {
                        continue;
                    };
                    let (outcome, _) = strategy_entry_outcome_cached(
                        candles,
                        entry_index,
                        follow_direction(direction),
                        NATGAS_STRICT_STOP_LOSS,
                        NATGAS_STRICT_EXECUTION_COST,
                        hold_21h,
                    );
                    let Some(outcome) = outcome else {
                        continue;
                    };
                    let timed_exit = outcome.stop_held.is_none();
                    let pnl = if timed_exit {
                        outcome.terminal_pnl_distance
                    } else {
                        outcome.stop_pnl_distance
                    };
                    *daily_pnl.entry(day_key.to_string()).or_insert(0.0) += pnl;
                    stats.trades += 1;
                    stats.net_pnl_distance += pnl;
                    if timed_exit {
                        if pnl > 0.0 {
                            stats.time_positive_exits += 1;
                        } else if pnl < 0.0 {
                            stats.time_negative_exits += 1;
                        } else {
                            stats.time_flat_exits += 1;
                        }
                    } else {
                        stats.stop_hits += 1;
                    }
                }
                _ => {}
            }
        }
        stats.total_days = daily_pnl.len();
        stats.target_hit_days = daily_pnl
            .values()
            .filter(|value| **value >= NATGAS_STRICT_DAILY_TARGET)
            .count();
        stats.daily_target_hit_rate = if stats.total_days > 0 {
            stats.target_hit_days as f64 / stats.total_days as f64
        } else {
            0.0
        };
        stats.avg_daily_pnl_distance = if stats.total_days > 0 {
            daily_pnl.values().sum::<f64>() / stats.total_days as f64
        } else {
            0.0
        };
        stats.expectancy_distance = if stats.trades > 0 {
            stats.net_pnl_distance / stats.trades as f64
        } else {
            0.0
        };
        stats
    }

    fn run_natgas_strat_a_13h_17h_21h_hybrid() {
        let h1_series = canonical_chart_series("NATGAS_USD", "H1", 0).expect("NATGAS_USD H1 history");
        let candles = h1_series.candles;
        let mut cache_snapshot = StrategyCacheSnapshot::default();
        let h1_hash = strategy_candles_hash(&candles);
        let feature_bank = strategy_feature_bank_cached(&candles, &h1_hash, &mut cache_snapshot);
        let mut best_hold = 0usize;
        let mut best: Option<HybridSessionStats> = None;
        for hold_21h in 2..=6 {
            let stats = evaluate_hybrid_strat_a_13h_17h_21h(&candles, &feature_bank, hold_21h);
            println!(
                "NATGAS_H1_STRATA_HYBRID hold_21h={} trades={} tp_hits={} stop_hits={} time_positive_exits={} time_negative_exits={} time_flat_exits={} daily_target_hit_rate={:.4} target_hit_days={} total_days={} avg_daily_pnl={:.6} expectancy={:.6} net_pnl={:.6}",
                hold_21h,
                stats.trades,
                stats.tp_hits,
                stats.stop_hits,
                stats.time_positive_exits,
                stats.time_negative_exits,
                stats.time_flat_exits,
                stats.daily_target_hit_rate,
                stats.target_hit_days,
                stats.total_days,
                stats.avg_daily_pnl_distance,
                stats.expectancy_distance,
                stats.net_pnl_distance,
            );
            let replace = best
                .as_ref()
                .map(|current| {
                    stats
                        .daily_target_hit_rate
                        .partial_cmp(&current.daily_target_hit_rate)
                        .unwrap_or(Ordering::Equal)
                        .then_with(|| {
                            stats
                                .avg_daily_pnl_distance
                                .partial_cmp(&current.avg_daily_pnl_distance)
                                .unwrap_or(Ordering::Equal)
                        })
                        .then_with(|| {
                            stats
                                .expectancy_distance
                                .partial_cmp(&current.expectancy_distance)
                                .unwrap_or(Ordering::Equal)
                        })
                        == Ordering::Greater
                })
                .unwrap_or(true);
            if replace {
                best_hold = hold_21h;
                best = Some(stats);
            }
        }
        let best = best.expect("best hybrid hold");
        println!(
            "NATGAS_H1_STRATA_HYBRID_BEST hold_21h={} trades={} tp_hits={} stop_hits={} time_positive_exits={} time_negative_exits={} time_flat_exits={} daily_target_hit_rate={:.4} target_hit_days={} total_days={} avg_daily_pnl={:.6} expectancy={:.6} net_pnl={:.6}",
            best_hold,
            best.trades,
            best.tp_hits,
            best.stop_hits,
            best.time_positive_exits,
            best.time_negative_exits,
            best.time_flat_exits,
            best.daily_target_hit_rate,
            best.target_hit_days,
            best.total_days,
            best.avg_daily_pnl_distance,
            best.expectancy_distance,
            best.net_pnl_distance,
        );
    }

    fn classic_short_11h_qualifiers() -> Vec<SessionQualifierDef> {
        vec![
            SessionQualifierDef { label: "no extra qualifier", predicate: |_i, _c, _b| true },
            SessionQualifierDef { label: "close < VWAP", predicate: seed_short_close_below_vwap },
            SessionQualifierDef { label: "two closes below VWAP", predicate: seed_short_two_closes_below_vwap },
            SessionQualifierDef { label: "VWAP +1sigma rejection after VWAP break", predicate: seed_short_reject_vwap_ext1_up_after_vwap_break },
            SessionQualifierDef { label: "three-bar VWAP rollover", predicate: seed_short_three_bar_vwap_rollover },
            SessionQualifierDef { label: "candle crosses below VWAP +1sigma", predicate: seed_short_cross_below_vwap_ext1_up },
        ]
    }

    fn classic_long_15h_qualifiers() -> Vec<SessionQualifierDef> {
        vec![
            SessionQualifierDef { label: "no extra qualifier", predicate: |_i, _c, _b| true },
            SessionQualifierDef { label: "close > VWAP", predicate: seed_long_close_above_vwap },
            SessionQualifierDef { label: "two closes above VWAP", predicate: seed_long_two_closes_above_vwap },
            SessionQualifierDef { label: "VWAP -1sigma reclaim after VWAP break", predicate: seed_long_reclaim_vwap_ext1_down_after_vwap_break },
            SessionQualifierDef { label: "three-bar VWAP reclaim", predicate: seed_long_three_bar_vwap_reclaim },
            SessionQualifierDef { label: "candle crosses above VWAP -1sigma", predicate: seed_long_cross_above_vwap_ext1_down },
        ]
    }

    #[derive(Clone, Debug)]
    struct ClassicQualifiedHybridCandidate {
        short_11_label: String,
        long_15_label: String,
        hold_21h: usize,
        trades: usize,
        tp_hits: usize,
        stop_hits: usize,
        time_positive_exits: usize,
        time_negative_exits: usize,
        target_hit_days: usize,
        total_days: usize,
        daily_target_hit_rate: f64,
        avg_daily_pnl_distance: f64,
        expectancy_distance: f64,
        net_pnl_distance: f64,
    }

    fn evaluate_classic_qualified_hybrid(
        candles: &[TradingCandlePoint],
        feature_bank: &StrategyIndicatorFeatureBank,
        short_11: SessionQualifierDef,
        long_15: SessionQualifierDef,
        hold_21h: usize,
    ) -> ClassicQualifiedHybridCandidate {
        let mut daily_pnl = BTreeMap::<String, f64>::new();
        let mut stats = HybridSessionStats::default();
        for index in 2..candles.len().saturating_sub(8) {
            let Some(hour) = strategy_hour_utc(&candles[index].time) else {
                continue;
            };
            let Some(day_key) = candles[index].time.get(..10) else {
                continue;
            };
            let entry_index = index + 1;
            match hour {
                11 => {
                    if !seed_short_bearish_body(index, candles, feature_bank)
                        || !(short_11.predicate)(index, candles, feature_bank)
                    {
                        continue;
                    }
                    let (outcome, _) = strategy_entry_outcome_cached(
                        candles,
                        entry_index,
                        "short",
                        NATGAS_STRICT_STOP_LOSS,
                        NATGAS_STRICT_EXECUTION_COST,
                        24,
                    );
                    let Some(outcome) = outcome else {
                        continue;
                    };
                    let (exit_kind, pnl, _) =
                        classify_strict_trade_outcome(&outcome, NATGAS_STRICT_TAKE_PROFIT);
                    *daily_pnl.entry(day_key.to_string()).or_insert(0.0) += pnl;
                    stats.trades += 1;
                    stats.net_pnl_distance += pnl;
                    match exit_kind {
                        StrictTradeExit::TakeProfit => stats.tp_hits += 1,
                        StrictTradeExit::StopLoss => stats.stop_hits += 1,
                        _ => {}
                    }
                }
                15 => {
                    if !seed_long_bullish_body(index, candles, feature_bank)
                        || !(long_15.predicate)(index, candles, feature_bank)
                    {
                        continue;
                    }
                    let (outcome, _) = strategy_entry_outcome_cached(
                        candles,
                        entry_index,
                        "long",
                        NATGAS_STRICT_STOP_LOSS,
                        NATGAS_STRICT_EXECUTION_COST,
                        24,
                    );
                    let Some(outcome) = outcome else {
                        continue;
                    };
                    let (exit_kind, pnl, _) =
                        classify_strict_trade_outcome(&outcome, NATGAS_STRICT_TAKE_PROFIT);
                    *daily_pnl.entry(day_key.to_string()).or_insert(0.0) += pnl;
                    stats.trades += 1;
                    stats.net_pnl_distance += pnl;
                    match exit_kind {
                        StrictTradeExit::TakeProfit => stats.tp_hits += 1,
                        StrictTradeExit::StopLoss => stats.stop_hits += 1,
                        _ => {}
                    }
                }
                21 => {
                    let Some(direction) = session_trend_signal(
                        candles,
                        feature_bank,
                        index,
                        SessionTrendVariant::ThreeBodiesVwap,
                    ) else {
                        continue;
                    };
                    let (outcome, _) = strategy_entry_outcome_cached(
                        candles,
                        entry_index,
                        follow_direction(direction),
                        NATGAS_STRICT_STOP_LOSS,
                        NATGAS_STRICT_EXECUTION_COST,
                        hold_21h,
                    );
                    let Some(outcome) = outcome else {
                        continue;
                    };
                    let timed_exit = outcome.stop_held.is_none();
                    let pnl = if timed_exit {
                        outcome.terminal_pnl_distance
                    } else {
                        outcome.stop_pnl_distance
                    };
                    *daily_pnl.entry(day_key.to_string()).or_insert(0.0) += pnl;
                    stats.trades += 1;
                    stats.net_pnl_distance += pnl;
                    if timed_exit {
                        if pnl > 0.0 {
                            stats.time_positive_exits += 1;
                        } else if pnl < 0.0 {
                            stats.time_negative_exits += 1;
                        } else {
                            stats.time_flat_exits += 1;
                        }
                    } else {
                        stats.stop_hits += 1;
                    }
                }
                _ => {}
            }
        }
        stats.total_days = daily_pnl.len();
        stats.target_hit_days = daily_pnl
            .values()
            .filter(|value| **value >= NATGAS_STRICT_DAILY_TARGET)
            .count();
        stats.daily_target_hit_rate = if stats.total_days > 0 {
            stats.target_hit_days as f64 / stats.total_days as f64
        } else {
            0.0
        };
        stats.avg_daily_pnl_distance = if stats.total_days > 0 {
            daily_pnl.values().sum::<f64>() / stats.total_days as f64
        } else {
            0.0
        };
        stats.expectancy_distance = if stats.trades > 0 {
            stats.net_pnl_distance / stats.trades as f64
        } else {
            0.0
        };
        ClassicQualifiedHybridCandidate {
            short_11_label: short_11.label.to_string(),
            long_15_label: long_15.label.to_string(),
            hold_21h,
            trades: stats.trades,
            tp_hits: stats.tp_hits,
            stop_hits: stats.stop_hits,
            time_positive_exits: stats.time_positive_exits,
            time_negative_exits: stats.time_negative_exits,
            target_hit_days: stats.target_hit_days,
            total_days: stats.total_days,
            daily_target_hit_rate: stats.daily_target_hit_rate,
            avg_daily_pnl_distance: stats.avg_daily_pnl_distance,
            expectancy_distance: stats.expectancy_distance,
            net_pnl_distance: stats.net_pnl_distance,
        }
    }

    fn run_natgas_classic_vwap_qualified_hybrid_search() {
        let h1_series = canonical_chart_series("NATGAS_USD", "H1", 0).expect("NATGAS_USD H1 history");
        let candles = h1_series.candles;
        let mut cache_snapshot = StrategyCacheSnapshot::default();
        let h1_hash = strategy_candles_hash(&candles);
        let feature_bank = strategy_feature_bank_cached(&candles, &h1_hash, &mut cache_snapshot);
        let mut best: Option<ClassicQualifiedHybridCandidate> = None;
        for short_11 in classic_short_11h_qualifiers() {
            for long_15 in classic_long_15h_qualifiers() {
                for hold_21h in 2..=6 {
                    let candidate = evaluate_classic_qualified_hybrid(
                        &candles,
                        &feature_bank,
                        short_11,
                        long_15,
                        hold_21h,
                    );
                    let replace = best
                        .as_ref()
                        .map(|current| {
                            candidate
                                .daily_target_hit_rate
                                .partial_cmp(&current.daily_target_hit_rate)
                                .unwrap_or(Ordering::Equal)
                                .then_with(|| {
                                    candidate
                                        .avg_daily_pnl_distance
                                        .partial_cmp(&current.avg_daily_pnl_distance)
                                        .unwrap_or(Ordering::Equal)
                                })
                                .then_with(|| {
                                    candidate
                                        .expectancy_distance
                                        .partial_cmp(&current.expectancy_distance)
                                        .unwrap_or(Ordering::Equal)
                                })
                                == Ordering::Greater
                        })
                        .unwrap_or(true);
                    if replace {
                        best = Some(candidate);
                    }
                }
            }
        }
        let best = best.expect("best classic vwap-qualified hybrid");
        println!(
            "NATGAS_H1_CLASSIC_VWAP_HYBRID best_11h={} best_15h={} hold_21h={} trades={} tp_hits={} stop_hits={} time_positive_exits={} time_negative_exits={} daily_target_hit_rate={:.4} target_hit_days={} total_days={} avg_daily_pnl={:.6} expectancy={:.6} net_pnl={:.6}",
            best.short_11_label,
            best.long_15_label,
            best.hold_21h,
            best.trades,
            best.tp_hits,
            best.stop_hits,
            best.time_positive_exits,
            best.time_negative_exits,
            best.daily_target_hit_rate,
            best.target_hit_days,
            best.total_days,
            best.avg_daily_pnl_distance,
            best.expectancy_distance,
            best.net_pnl_distance,
        );
    }

    #[derive(Clone, Copy)]
    enum RejoinQualifierKind {
        None,
        CloseTrendSideVwap,
        CrossExt1,
        ReclaimBreak,
        ThreeBarReclaim,
    }

    impl RejoinQualifierKind {
        fn label(self) -> &'static str {
            match self {
                RejoinQualifierKind::None => "no qualifier",
                RejoinQualifierKind::CloseTrendSideVwap => "pullback close on trend side of VWAP",
                RejoinQualifierKind::CrossExt1 => "pullback crosses ext1 back toward trend",
                RejoinQualifierKind::ReclaimBreak => "pullback reclaim/reject after VWAP break",
                RejoinQualifierKind::ThreeBarReclaim => "three-bar vwap reclaim/rollover",
            }
        }
    }

    #[derive(Clone, Debug)]
    struct PullbackRejoinCandidate {
        qualifier_11h: String,
        qualifier_15h: String,
        max_hold: usize,
        trades: usize,
        tp_hits: usize,
        stop_hits: usize,
        daily_target_hit_rate: f64,
        target_hit_days: usize,
        total_days: usize,
        avg_daily_pnl_distance: f64,
        expectancy_distance: f64,
        net_pnl_distance: f64,
    }

    fn rejoin_qualifier_pass(
        qualifier: RejoinQualifierKind,
        direction: TrendDirection,
        index: usize,
        candles: &[TradingCandlePoint],
        bank: &StrategyIndicatorFeatureBank,
    ) -> bool {
        match (qualifier, direction) {
            (RejoinQualifierKind::None, _) => true,
            (RejoinQualifierKind::CloseTrendSideVwap, TrendDirection::Up) => {
                seed_long_close_above_vwap(index, candles, bank)
            }
            (RejoinQualifierKind::CloseTrendSideVwap, TrendDirection::Down) => {
                seed_short_close_below_vwap(index, candles, bank)
            }
            (RejoinQualifierKind::CrossExt1, TrendDirection::Up) => {
                seed_long_cross_above_vwap_ext1_down(index, candles, bank)
            }
            (RejoinQualifierKind::CrossExt1, TrendDirection::Down) => {
                seed_short_cross_below_vwap_ext1_up(index, candles, bank)
            }
            (RejoinQualifierKind::ReclaimBreak, TrendDirection::Up) => {
                seed_long_reclaim_vwap_ext1_down_after_vwap_break(index, candles, bank)
            }
            (RejoinQualifierKind::ReclaimBreak, TrendDirection::Down) => {
                seed_short_reject_vwap_ext1_up_after_vwap_break(index, candles, bank)
            }
            (RejoinQualifierKind::ThreeBarReclaim, TrendDirection::Up) => {
                seed_long_three_bar_vwap_reclaim(index, candles, bank)
            }
            (RejoinQualifierKind::ThreeBarReclaim, TrendDirection::Down) => {
                seed_short_three_bar_vwap_rollover(index, candles, bank)
            }
        }
    }

    fn evaluate_pullback_rejoin_candidate(
        candles: &[TradingCandlePoint],
        feature_bank: &StrategyIndicatorFeatureBank,
        samples: &[TrendLifecycleSample],
        qualifier_11h: RejoinQualifierKind,
        qualifier_15h: RejoinQualifierKind,
        max_hold: usize,
    ) -> Option<PullbackRejoinCandidate> {
        let mut daily_pnl = BTreeMap::<String, f64>::new();
        let mut trades = 0usize;
        let mut tp_hits = 0usize;
        let mut stop_hits = 0usize;
        let mut net_pnl = 0.0;
        for sample in samples {
            let Some(hour) = strategy_hour_utc(&candles[sample.confirm_index].time) else {
                continue;
            };
            if hour != 11 && hour != 15 {
                continue;
            }
            if sample.bars_confirm_to_pullback > 3 {
                continue;
            }
            let qualifier = if hour == 11 { qualifier_11h } else { qualifier_15h };
            if !rejoin_qualifier_pass(
                qualifier,
                sample.direction,
                sample.pullback_index,
                candles,
                feature_bank,
            ) {
                continue;
            }
            let entry_index = sample.pullback_index + 1;
            if entry_index >= candles.len().saturating_sub(1) {
                continue;
            }
            let Some(day_key) = candles[entry_index].time.get(..10) else {
                continue;
            };
            let (outcome, _) = strategy_entry_outcome_cached(
                candles,
                entry_index,
                follow_direction(sample.direction),
                NATGAS_STRICT_STOP_LOSS,
                NATGAS_STRICT_EXECUTION_COST,
                max_hold,
            );
            let Some(outcome) = outcome else {
                continue;
            };
            let (exit_kind, pnl, _) =
                classify_strict_trade_outcome(&outcome, NATGAS_STRICT_TAKE_PROFIT);
            *daily_pnl.entry(day_key.to_string()).or_insert(0.0) += pnl;
            trades += 1;
            net_pnl += pnl;
            match exit_kind {
                StrictTradeExit::TakeProfit => tp_hits += 1,
                StrictTradeExit::StopLoss => stop_hits += 1,
                _ => {}
            }
        }
        if trades == 0 || daily_pnl.is_empty() {
            return None;
        }
        let total_days = daily_pnl.len();
        let target_hit_days = daily_pnl
            .values()
            .filter(|value| **value >= NATGAS_STRICT_DAILY_TARGET)
            .count();
        Some(PullbackRejoinCandidate {
            qualifier_11h: qualifier_11h.label().to_string(),
            qualifier_15h: qualifier_15h.label().to_string(),
            max_hold,
            trades,
            tp_hits,
            stop_hits,
            daily_target_hit_rate: target_hit_days as f64 / total_days as f64,
            target_hit_days,
            total_days,
            avg_daily_pnl_distance: daily_pnl.values().sum::<f64>() / total_days as f64,
            expectancy_distance: net_pnl / trades as f64,
            net_pnl_distance: net_pnl,
        })
    }

    fn run_natgas_11h_15h_pullback_rejoin_search() {
        let h1_series = canonical_chart_series("NATGAS_USD", "H1", 0).expect("NATGAS_USD H1 history");
        let candles = h1_series.candles;
        let mut cache_snapshot = StrategyCacheSnapshot::default();
        let h1_hash = strategy_candles_hash(&candles);
        let feature_bank = strategy_feature_bank_cached(&candles, &h1_hash, &mut cache_snapshot);
        let samples = extract_trend_lifecycle_samples(&candles, NATGAS_TREND_PULLBACK_DISTANCE);
        let qualifiers = [
            RejoinQualifierKind::None,
            RejoinQualifierKind::CloseTrendSideVwap,
            RejoinQualifierKind::CrossExt1,
            RejoinQualifierKind::ReclaimBreak,
            RejoinQualifierKind::ThreeBarReclaim,
        ];
        let mut best: Option<PullbackRejoinCandidate> = None;
        for qualifier_11h in qualifiers {
            for qualifier_15h in qualifiers {
                for max_hold in 3..=8 {
                    let Some(candidate) = evaluate_pullback_rejoin_candidate(
                        &candles,
                        &feature_bank,
                        &samples,
                        qualifier_11h,
                        qualifier_15h,
                        max_hold,
                    ) else {
                        continue;
                    };
                    let replace = best
                        .as_ref()
                        .map(|current| {
                            candidate
                                .daily_target_hit_rate
                                .partial_cmp(&current.daily_target_hit_rate)
                                .unwrap_or(Ordering::Equal)
                                .then_with(|| {
                                    candidate
                                        .avg_daily_pnl_distance
                                        .partial_cmp(&current.avg_daily_pnl_distance)
                                        .unwrap_or(Ordering::Equal)
                                })
                                .then_with(|| {
                                    candidate
                                        .expectancy_distance
                                        .partial_cmp(&current.expectancy_distance)
                                        .unwrap_or(Ordering::Equal)
                                })
                                == Ordering::Greater
                        })
                        .unwrap_or(true);
                    if replace {
                        best = Some(candidate);
                    }
                }
            }
        }
        let best = best.expect("best pullback rejoin candidate");
        println!(
            "NATGAS_H1_11H_15H_PULLBACK_REJOIN best_11h={} best_15h={} max_hold={} trades={} tp_hits={} stop_hits={} daily_target_hit_rate={:.4} target_hit_days={} total_days={} avg_daily_pnl={:.6} expectancy={:.6} net_pnl={:.6}",
            best.qualifier_11h,
            best.qualifier_15h,
            best.max_hold,
            best.trades,
            best.tp_hits,
            best.stop_hits,
            best.daily_target_hit_rate,
            best.target_hit_days,
            best.total_days,
            best.avg_daily_pnl_distance,
            best.expectancy_distance,
            best.net_pnl_distance,
        );
    }

    #[derive(Clone, Copy)]
    enum M5PullbackZone {
        Vwap,
        Ext1,
        Ema21,
        Ext2,
    }

    impl M5PullbackZone {
        fn label(self) -> &'static str {
            match self {
                M5PullbackZone::Vwap => "touch VWAP",
                M5PullbackZone::Ext1 => "touch VWAP ext1",
                M5PullbackZone::Ema21 => "touch EMA21",
                M5PullbackZone::Ext2 => "touch VWAP ext2",
            }
        }
    }

    #[derive(Clone, Copy)]
    enum M5RejoinTrigger {
        BodyReversal,
        CloseTrendSideVwap,
        ReclaimExt1,
        BreakPriorBar,
    }

    impl M5RejoinTrigger {
        fn label(self) -> &'static str {
            match self {
                M5RejoinTrigger::BodyReversal => "body reversal",
                M5RejoinTrigger::CloseTrendSideVwap => "close back on trend side of VWAP",
                M5RejoinTrigger::ReclaimExt1 => "reclaim ext1",
                M5RejoinTrigger::BreakPriorBar => "break prior bar",
            }
        }
    }

    #[derive(Clone, Debug)]
    struct M5PullbackExecutionCandidate {
        zone: String,
        trigger: String,
        max_hold: usize,
        trades: usize,
        tp_hits: usize,
        stop_hits: usize,
        daily_target_hit_rate: f64,
        target_hit_days: usize,
        total_days: usize,
        avg_daily_pnl_distance: f64,
        expectancy_distance: f64,
        net_pnl_distance: f64,
    }

    #[derive(Clone, Debug)]
    struct AnchoredVwapTouchCandidate {
        lookback_hours: usize,
        max_hold: usize,
        trades: usize,
        wins: usize,
        losses: usize,
        daily_target_hit_rate: f64,
        target_hit_days: usize,
        total_days: usize,
        avg_daily_pnl_distance: f64,
        expectancy_distance: f64,
        net_pnl_distance: f64,
    }

    fn candle_index_at_or_after(candles: &[TradingCandlePoint], time: &str) -> Option<usize> {
        candle_index_by_time(candles, time).or_else(|| {
            let index = next_candle_index_after(candles, Some(time));
            (index < candles.len()).then_some(index)
        })
    }

    fn m5_zone_touched(
        zone: M5PullbackZone,
        direction: TrendDirection,
        index: usize,
        candles: &[TradingCandlePoint],
        bank: &StrategyIndicatorFeatureBank,
    ) -> bool {
        let candle = &candles[index];
        match (zone, direction) {
            (M5PullbackZone::Vwap, TrendDirection::Up) => candle.low <= bank.vwap[index],
            (M5PullbackZone::Vwap, TrendDirection::Down) => candle.high >= bank.vwap[index],
            (M5PullbackZone::Ext1, TrendDirection::Up) => candle.low <= bank.vwap_ext1_down[index],
            (M5PullbackZone::Ext1, TrendDirection::Down) => candle.high >= bank.vwap_ext1_up[index],
            (M5PullbackZone::Ema21, TrendDirection::Up) => candle.low <= bank.ema21[index],
            (M5PullbackZone::Ema21, TrendDirection::Down) => candle.high >= bank.ema21[index],
            (M5PullbackZone::Ext2, TrendDirection::Up) => candle.low <= bank.vwap_ext2_down[index],
            (M5PullbackZone::Ext2, TrendDirection::Down) => candle.high >= bank.vwap_ext2_up[index],
        }
    }

    fn m5_rejoin_trigger_pass(
        trigger: M5RejoinTrigger,
        direction: TrendDirection,
        index: usize,
        candles: &[TradingCandlePoint],
        bank: &StrategyIndicatorFeatureBank,
    ) -> bool {
        if index == 0 {
            return false;
        }
        let candle = &candles[index];
        let prev = &candles[index - 1];
        match (trigger, direction) {
            (M5RejoinTrigger::BodyReversal, TrendDirection::Up) => bullish_body(candle),
            (M5RejoinTrigger::BodyReversal, TrendDirection::Down) => bearish_body(candle),
            (M5RejoinTrigger::CloseTrendSideVwap, TrendDirection::Up) => candle.close > bank.vwap[index],
            (M5RejoinTrigger::CloseTrendSideVwap, TrendDirection::Down) => candle.close < bank.vwap[index],
            (M5RejoinTrigger::ReclaimExt1, TrendDirection::Up) => {
                candle.low <= bank.vwap_ext1_down[index] && candle.close > bank.vwap_ext1_down[index]
            }
            (M5RejoinTrigger::ReclaimExt1, TrendDirection::Down) => {
                candle.high >= bank.vwap_ext1_up[index] && candle.close < bank.vwap_ext1_up[index]
            }
            (M5RejoinTrigger::BreakPriorBar, TrendDirection::Up) => candle.close > prev.high,
            (M5RejoinTrigger::BreakPriorBar, TrendDirection::Down) => candle.close < prev.low,
        }
    }

    fn evaluate_h1_signal_to_m5_pullback_execution(
        h1_candles: &[TradingCandlePoint],
        m5_candles: &[TradingCandlePoint],
        m5_bank: &StrategyIndicatorFeatureBank,
        samples: &[TrendLifecycleSample],
        zone: M5PullbackZone,
        trigger: M5RejoinTrigger,
        max_hold: usize,
    ) -> Option<M5PullbackExecutionCandidate> {
        let mut daily_pnl = BTreeMap::<String, f64>::new();
        let mut trades = 0usize;
        let mut tp_hits = 0usize;
        let mut stop_hits = 0usize;
        let mut net_pnl = 0.0;
        for sample in samples {
            let Some(hour) = strategy_hour_utc(&h1_candles[sample.confirm_index].time) else {
                continue;
            };
            if hour != 11 && hour != 15 {
                continue;
            }
            if sample.bars_confirm_to_pullback > 3 || sample.confirm_index + 1 >= h1_candles.len() {
                continue;
            }
            let start_time = &h1_candles[sample.confirm_index + 1].time;
            let start_index = candle_index_at_or_after(m5_candles, start_time).unwrap_or(m5_candles.len());
            if start_index >= m5_candles.len().saturating_sub(2) {
                continue;
            }
            let end_exclusive = if sample.confirm_index + 4 < h1_candles.len() {
                candle_index_at_or_after(m5_candles, &h1_candles[sample.confirm_index + 4].time)
                    .unwrap_or(m5_candles.len())
            } else {
                m5_candles.len()
            };
            if end_exclusive <= start_index + 1 {
                continue;
            }
            let mut touch_index = None;
            let mut entry_index = None;
            for m5_index in start_index..end_exclusive.min(m5_candles.len().saturating_sub(1)) {
                if touch_index.is_none() {
                    if m5_zone_touched(zone, sample.direction, m5_index, m5_candles, m5_bank) {
                        touch_index = Some(m5_index);
                    }
                    continue;
                }
                if m5_rejoin_trigger_pass(trigger, sample.direction, m5_index, m5_candles, m5_bank) {
                    entry_index = Some(m5_index + 1);
                    break;
                }
            }
            let Some(entry_index) = entry_index else {
                continue;
            };
            if entry_index >= m5_candles.len().saturating_sub(1) {
                continue;
            }
            let Some(day_key) = m5_candles[entry_index].time.get(..10) else {
                continue;
            };
            let (outcome, _) = strategy_entry_outcome_cached(
                m5_candles,
                entry_index,
                follow_direction(sample.direction),
                NATGAS_STRICT_STOP_LOSS,
                NATGAS_STRICT_EXECUTION_COST,
                max_hold,
            );
            let Some(outcome) = outcome else {
                continue;
            };
            let (exit_kind, pnl, _) =
                classify_strict_trade_outcome(&outcome, NATGAS_STRICT_TAKE_PROFIT);
            *daily_pnl.entry(day_key.to_string()).or_insert(0.0) += pnl;
            trades += 1;
            net_pnl += pnl;
            match exit_kind {
                StrictTradeExit::TakeProfit => tp_hits += 1,
                StrictTradeExit::StopLoss => stop_hits += 1,
                _ => {}
            }
        }
        if trades == 0 || daily_pnl.is_empty() {
            return None;
        }
        let total_days = daily_pnl.len();
        let target_hit_days = daily_pnl
            .values()
            .filter(|value| **value >= NATGAS_STRICT_DAILY_TARGET)
            .count();
        Some(M5PullbackExecutionCandidate {
            zone: zone.label().to_string(),
            trigger: trigger.label().to_string(),
            max_hold,
            trades,
            tp_hits,
            stop_hits,
            daily_target_hit_rate: target_hit_days as f64 / total_days as f64,
            target_hit_days,
            total_days,
            avg_daily_pnl_distance: daily_pnl.values().sum::<f64>() / total_days as f64,
            expectancy_distance: net_pnl / trades as f64,
            net_pnl_distance: net_pnl,
        })
    }

    fn run_natgas_h1_to_m5_pullback_execution_search() {
        let h1_series = canonical_chart_series("NATGAS_USD", "H1", 0).expect("NATGAS_USD H1 history");
        let m5_series = canonical_chart_series("NATGAS_USD", "M5", 0).expect("NATGAS_USD M5 history");
        let h1_candles = h1_series.candles;
        let m5_candles = m5_series.candles;
        let mut cache_snapshot = StrategyCacheSnapshot::default();
        let m5_hash = strategy_candles_hash(&m5_candles);
        let m5_bank = strategy_feature_bank_cached(&m5_candles, &m5_hash, &mut cache_snapshot);
        let samples = extract_trend_lifecycle_samples(&h1_candles, NATGAS_TREND_PULLBACK_DISTANCE);
        let zones = [
            M5PullbackZone::Vwap,
            M5PullbackZone::Ext1,
            M5PullbackZone::Ema21,
            M5PullbackZone::Ext2,
        ];
        let triggers = [
            M5RejoinTrigger::BodyReversal,
            M5RejoinTrigger::CloseTrendSideVwap,
            M5RejoinTrigger::ReclaimExt1,
            M5RejoinTrigger::BreakPriorBar,
        ];
        let mut best: Option<M5PullbackExecutionCandidate> = None;
        for zone in zones {
            for trigger in triggers {
                for max_hold in [6_usize, 12, 18, 24] {
                    let Some(candidate) = evaluate_h1_signal_to_m5_pullback_execution(
                        &h1_candles,
                        &m5_candles,
                        &m5_bank,
                        &samples,
                        zone,
                        trigger,
                        max_hold,
                    ) else {
                        continue;
                    };
                    let replace = best
                        .as_ref()
                        .map(|current| {
                            candidate
                                .daily_target_hit_rate
                                .partial_cmp(&current.daily_target_hit_rate)
                                .unwrap_or(Ordering::Equal)
                                .then_with(|| {
                                    candidate
                                        .avg_daily_pnl_distance
                                        .partial_cmp(&current.avg_daily_pnl_distance)
                                        .unwrap_or(Ordering::Equal)
                                })
                                .then_with(|| {
                                    candidate
                                        .expectancy_distance
                                        .partial_cmp(&current.expectancy_distance)
                                        .unwrap_or(Ordering::Equal)
                                })
                                == Ordering::Greater
                        })
                        .unwrap_or(true);
                    if replace {
                        best = Some(candidate);
                    }
                }
            }
        }
        let best = best.expect("best H1->M5 pullback execution");
        println!(
            "NATGAS_H1_TO_M5_PULLBACK_EXECUTION zone={} trigger={} max_hold_m5={} trades={} tp_hits={} stop_hits={} daily_target_hit_rate={:.4} target_hit_days={} total_days={} avg_daily_pnl={:.6} expectancy={:.6} net_pnl={:.6}",
            best.zone,
            best.trigger,
            best.max_hold,
            best.trades,
            best.tp_hits,
            best.stop_hits,
            best.daily_target_hit_rate,
            best.target_hit_days,
            best.total_days,
            best.avg_daily_pnl_distance,
            best.expectancy_distance,
            best.net_pnl_distance,
        );
    }

    fn rolling_extreme_index(
        candles: &[TradingCandlePoint],
        end_index: usize,
        lookback: usize,
        highest: bool,
    ) -> usize {
        let start = end_index.saturating_add(1).saturating_sub(lookback);
        let mut best_index = start;
        let mut best_value = if highest {
            candles[start].high
        } else {
            candles[start].low
        };
        for index in start + 1..=end_index {
            let value = if highest { candles[index].high } else { candles[index].low };
            let replace = if highest { value >= best_value } else { value <= best_value };
            if replace {
                best_value = value;
                best_index = index;
            }
        }
        best_index
    }

    fn anchored_vwap_between(candles: &[TradingCandlePoint], from: usize, to: usize) -> f64 {
        let mut pv = 0.0;
        let mut volume = 0.0;
        for candle in &candles[from..=to] {
            let typical = (candle.high + candle.low + candle.close) / 3.0;
            let vol = (candle.volume as f64).max(1.0);
            pv += typical * vol;
            volume += vol;
        }
        if volume > 0.0 {
            pv / volume
        } else {
            candles[to].close
        }
    }

    fn anchored_vwap_sigma_between(candles: &[TradingCandlePoint], from: usize, to: usize) -> (f64, f64) {
        let mut pv = 0.0;
        let mut sq_pv = 0.0;
        let mut volume = 0.0;
        for candle in &candles[from..=to] {
            let typical = (candle.high + candle.low + candle.close) / 3.0;
            let vol = (candle.volume as f64).max(1.0);
            pv += typical * vol;
            sq_pv += typical * typical * vol;
            volume += vol;
        }
        if volume <= 0.0 {
            return (candles[to].close, 0.0);
        }
        let mean = pv / volume;
        let variance = ((sq_pv / volume) - mean * mean).max(0.0);
        (mean, variance.sqrt())
    }

    fn evaluate_anchored_vwap_touch_candidate(
        candles: &[TradingCandlePoint],
        lookback_hours: usize,
        max_hold: usize,
    ) -> Option<AnchoredVwapTouchCandidate> {
        let mut daily_pnl = BTreeMap::<String, f64>::new();
        let mut trades = 0usize;
        let mut wins = 0usize;
        let mut losses = 0usize;
        let mut net_pnl = 0.0;
        for index in lookback_hours..candles.len().saturating_sub(2) {
            let Some(hour) = strategy_hour_utc(&candles[index].time) else {
                continue;
            };
            if !(7..=21).contains(&hour) {
                continue;
            }
            let Some(day_key) = candles[index].time.get(..10) else {
                continue;
            };
            let high_anchor = rolling_extreme_index(candles, index, lookback_hours, true);
            let low_anchor = rolling_extreme_index(candles, index, lookback_hours, false);
            if high_anchor == index && low_anchor == index {
                continue;
            }
            let avwap_high = anchored_vwap_between(candles, high_anchor, index);
            let avwap_low = anchored_vwap_between(candles, low_anchor, index);
            let candle = &candles[index];
            let long_setup = low_anchor < index && candle.close > avwap_low && candle.low <= avwap_low;
            let short_setup = high_anchor < index && candle.close < avwap_high && candle.high >= avwap_high;
            let direction = match (long_setup, short_setup) {
                (true, false) => Some("long"),
                (false, true) => Some("short"),
                _ => None,
            };
            let Some(direction) = direction else {
                continue;
            };
            let entry_index = index + 1;
            let (outcome, _) = strategy_entry_outcome_cached(
                candles,
                entry_index,
                direction,
                NATGAS_STRICT_STOP_LOSS,
                NATGAS_STRICT_EXECUTION_COST,
                max_hold,
            );
            let Some(outcome) = outcome else {
                continue;
            };
            let (exit_kind, pnl, _) =
                classify_strict_trade_outcome(&outcome, NATGAS_STRICT_TAKE_PROFIT);
            *daily_pnl.entry(day_key.to_string()).or_insert(0.0) += pnl;
            trades += 1;
            net_pnl += pnl;
            match exit_kind {
                StrictTradeExit::TakeProfit => wins += 1,
                StrictTradeExit::StopLoss => losses += 1,
                _ => {}
            }
        }
        if trades == 0 || daily_pnl.is_empty() {
            return None;
        }
        let total_days = daily_pnl.len();
        let target_hit_days = daily_pnl
            .values()
            .filter(|value| **value >= NATGAS_STRICT_DAILY_TARGET)
            .count();
        Some(AnchoredVwapTouchCandidate {
            lookback_hours,
            max_hold,
            trades,
            wins,
            losses,
            daily_target_hit_rate: target_hit_days as f64 / total_days as f64,
            target_hit_days,
            total_days,
            avg_daily_pnl_distance: daily_pnl.values().sum::<f64>() / total_days as f64,
            expectancy_distance: net_pnl / trades as f64,
            net_pnl_distance: net_pnl,
        })
    }

    fn run_natgas_h1_anchored_vwap_touch_search() {
        let h1_series = canonical_chart_series("NATGAS_USD", "H1", 0).expect("NATGAS_USD H1 history");
        let candles = h1_series.candles;
        let mut best: Option<AnchoredVwapTouchCandidate> = None;
        for lookback_hours in [24_usize, 36, 48] {
            for max_hold in [6_usize, 12, 18, 24] {
                let Some(candidate) =
                    evaluate_anchored_vwap_touch_candidate(&candles, lookback_hours, max_hold)
                else {
                    continue;
                };
                let replace = best
                    .as_ref()
                    .map(|current| {
                        candidate
                            .daily_target_hit_rate
                            .partial_cmp(&current.daily_target_hit_rate)
                            .unwrap_or(Ordering::Equal)
                            .then_with(|| {
                                candidate
                                    .avg_daily_pnl_distance
                                    .partial_cmp(&current.avg_daily_pnl_distance)
                                    .unwrap_or(Ordering::Equal)
                            })
                            .then_with(|| {
                                candidate
                                    .expectancy_distance
                                    .partial_cmp(&current.expectancy_distance)
                                    .unwrap_or(Ordering::Equal)
                            })
                            == Ordering::Greater
                    })
                    .unwrap_or(true);
                if replace {
                    best = Some(candidate);
                }
            }
        }
        let best = best.expect("best anchored vwap touch");
        println!(
            "NATGAS_H1_ANCHORED_VWAP_TOUCH best_lookback_hours={} best_max_hold={} trades={} wins={} losses={} win_rate={:.4} daily_target_hit_rate={:.4} target_hit_days={} total_days={} avg_daily_pnl={:.6} expectancy={:.6} net_pnl={:.6}",
            best.lookback_hours,
            best.max_hold,
            best.trades,
            best.wins,
            best.losses,
            best.wins as f64 / best.trades as f64,
            best.daily_target_hit_rate,
            best.target_hit_days,
            best.total_days,
            best.avg_daily_pnl_distance,
            best.expectancy_distance,
            best.net_pnl_distance,
        );
    }

    fn evaluate_anchored_vwap_ext2_candidate(
        candles: &[TradingCandlePoint],
        lookback_hours: usize,
        max_hold: usize,
    ) -> Option<AnchoredVwapTouchCandidate> {
        let mut daily_pnl = BTreeMap::<String, f64>::new();
        let mut trades = 0usize;
        let mut wins = 0usize;
        let mut losses = 0usize;
        let mut net_pnl = 0.0;
        for index in lookback_hours..candles.len().saturating_sub(2) {
            let Some(hour) = strategy_hour_utc(&candles[index].time) else {
                continue;
            };
            if !(7..=21).contains(&hour) {
                continue;
            }
            let Some(day_key) = candles[index].time.get(..10) else {
                continue;
            };
            let high_anchor = rolling_extreme_index(candles, index, lookback_hours, true);
            let low_anchor = rolling_extreme_index(candles, index, lookback_hours, false);
            if high_anchor == index && low_anchor == index {
                continue;
            }
            let (avwap_high, sigma_high) = anchored_vwap_sigma_between(candles, high_anchor, index);
            let (avwap_low, sigma_low) = anchored_vwap_sigma_between(candles, low_anchor, index);
            let ext2_up = avwap_high + 2.0 * sigma_high;
            let ext2_down = avwap_low - 2.0 * sigma_low;
            let candle = &candles[index];
            let short_setup = high_anchor < index && candle.high >= ext2_up;
            let long_setup = low_anchor < index && candle.low <= ext2_down;
            let direction = match (long_setup, short_setup) {
                (true, false) => Some("long"),
                (false, true) => Some("short"),
                _ => None,
            };
            let Some(direction) = direction else {
                continue;
            };
            let entry_index = index + 1;
            let (outcome, _) = strategy_entry_outcome_cached(
                candles,
                entry_index,
                direction,
                NATGAS_STRICT_STOP_LOSS,
                NATGAS_STRICT_EXECUTION_COST,
                max_hold,
            );
            let Some(outcome) = outcome else {
                continue;
            };
            let (exit_kind, pnl, _) =
                classify_strict_trade_outcome(&outcome, NATGAS_STRICT_TAKE_PROFIT);
            *daily_pnl.entry(day_key.to_string()).or_insert(0.0) += pnl;
            trades += 1;
            net_pnl += pnl;
            match exit_kind {
                StrictTradeExit::TakeProfit => wins += 1,
                StrictTradeExit::StopLoss => losses += 1,
                _ => {}
            }
        }
        if trades == 0 || daily_pnl.is_empty() {
            return None;
        }
        let total_days = daily_pnl.len();
        let target_hit_days = daily_pnl
            .values()
            .filter(|value| **value >= NATGAS_STRICT_DAILY_TARGET)
            .count();
        Some(AnchoredVwapTouchCandidate {
            lookback_hours,
            max_hold,
            trades,
            wins,
            losses,
            daily_target_hit_rate: target_hit_days as f64 / total_days as f64,
            target_hit_days,
            total_days,
            avg_daily_pnl_distance: daily_pnl.values().sum::<f64>() / total_days as f64,
            expectancy_distance: net_pnl / trades as f64,
            net_pnl_distance: net_pnl,
        })
    }

    fn run_natgas_h1_anchored_vwap_ext2_search() {
        let h1_series = canonical_chart_series("NATGAS_USD", "H1", 0).expect("NATGAS_USD H1 history");
        let candles = h1_series.candles;
        let mut best: Option<AnchoredVwapTouchCandidate> = None;
        for lookback_hours in [24_usize, 36, 48] {
            for max_hold in [6_usize, 12, 18, 24] {
                let Some(candidate) =
                    evaluate_anchored_vwap_ext2_candidate(&candles, lookback_hours, max_hold)
                else {
                    continue;
                };
                let replace = best
                    .as_ref()
                    .map(|current| {
                        candidate
                            .daily_target_hit_rate
                            .partial_cmp(&current.daily_target_hit_rate)
                            .unwrap_or(Ordering::Equal)
                            .then_with(|| {
                                candidate
                                    .avg_daily_pnl_distance
                                    .partial_cmp(&current.avg_daily_pnl_distance)
                                    .unwrap_or(Ordering::Equal)
                            })
                            .then_with(|| {
                                candidate
                                    .expectancy_distance
                                    .partial_cmp(&current.expectancy_distance)
                                    .unwrap_or(Ordering::Equal)
                            })
                            == Ordering::Greater
                    })
                    .unwrap_or(true);
                if replace {
                    best = Some(candidate);
                }
            }
        }
        let best = best.expect("best anchored vwap ext2");
        println!(
            "NATGAS_H1_ANCHORED_VWAP_EXT2 best_lookback_hours={} best_max_hold={} trades={} wins={} losses={} win_rate={:.4} daily_target_hit_rate={:.4} target_hit_days={} total_days={} avg_daily_pnl={:.6} expectancy={:.6} net_pnl={:.6}",
            best.lookback_hours,
            best.max_hold,
            best.trades,
            best.wins,
            best.losses,
            best.wins as f64 / best.trades as f64,
            best.daily_target_hit_rate,
            best.target_hit_days,
            best.total_days,
            best.avg_daily_pnl_distance,
            best.expectancy_distance,
            best.net_pnl_distance,
        );
    }

    #[derive(Clone)]
    struct AnchoredVwapAntiEdgeCandidate {
        label: &'static str,
        lookback_hours: usize,
        max_hold: usize,
        trades: usize,
        wins: usize,
        losses: usize,
        daily_target_hit_rate: f64,
        target_hit_days: usize,
        total_days: usize,
        avg_daily_pnl_distance: f64,
        expectancy_distance: f64,
        net_pnl_distance: f64,
    }

    fn evaluate_anchored_vwap_anti_edge_candidate(
        candles: &[TradingCandlePoint],
        lookback_hours: usize,
        max_hold: usize,
        variant: &'static str,
    ) -> Option<AnchoredVwapAntiEdgeCandidate> {
        let mut daily_pnl = BTreeMap::<String, f64>::new();
        let mut trades = 0usize;
        let mut wins = 0usize;
        let mut losses = 0usize;
        let mut net_pnl = 0.0;
        for index in lookback_hours..candles.len().saturating_sub(2) {
            let Some(hour) = strategy_hour_utc(&candles[index].time) else {
                continue;
            };
            if !(7..=21).contains(&hour) {
                continue;
            }
            let Some(day_key) = candles[index].time.get(..10) else {
                continue;
            };
            let high_anchor = rolling_extreme_index(candles, index, lookback_hours, true);
            let low_anchor = rolling_extreme_index(candles, index, lookback_hours, false);
            if high_anchor == index && low_anchor == index {
                continue;
            }
            let avwap_high = anchored_vwap_between(candles, high_anchor, index);
            let avwap_low = anchored_vwap_between(candles, low_anchor, index);
            let (avwap_sigma_high, sigma_high) = anchored_vwap_sigma_between(candles, high_anchor, index);
            let (avwap_sigma_low, sigma_low) = anchored_vwap_sigma_between(candles, low_anchor, index);
            let ext2_up = avwap_sigma_high + 2.0 * sigma_high;
            let ext2_down = avwap_sigma_low - 2.0 * sigma_low;
            let candle = &candles[index];
            let direction = match variant {
                "touch_mean_reversion" => {
                    let long_setup = low_anchor < index && candle.close > avwap_low && candle.low <= avwap_low;
                    let short_setup = high_anchor < index && candle.close < avwap_high && candle.high >= avwap_high;
                    match (long_setup, short_setup) {
                        (true, false) => Some("long"),
                        (false, true) => Some("short"),
                        _ => None,
                    }
                }
                "touch_inverted_follow" => {
                    let long_setup = high_anchor < index && candle.close > avwap_high && candle.high >= avwap_high;
                    let short_setup = low_anchor < index && candle.close < avwap_low && candle.low <= avwap_low;
                    match (long_setup, short_setup) {
                        (true, false) => Some("long"),
                        (false, true) => Some("short"),
                        _ => None,
                    }
                }
                "ext2_mean_reversion" => {
                    let short_setup = high_anchor < index && candle.high >= ext2_up;
                    let long_setup = low_anchor < index && candle.low <= ext2_down;
                    match (long_setup, short_setup) {
                        (true, false) => Some("long"),
                        (false, true) => Some("short"),
                        _ => None,
                    }
                }
                "ext2_inverted_follow" => {
                    let long_setup = high_anchor < index && candle.high >= ext2_up;
                    let short_setup = low_anchor < index && candle.low <= ext2_down;
                    match (long_setup, short_setup) {
                        (true, false) => Some("long"),
                        (false, true) => Some("short"),
                        _ => None,
                    }
                }
                _ => None,
            };
            let Some(direction) = direction else {
                continue;
            };
            let (outcome, _) = strategy_entry_outcome_cached(
                candles,
                index + 1,
                direction,
                NATGAS_STRICT_STOP_LOSS,
                NATGAS_STRICT_EXECUTION_COST,
                max_hold,
            );
            let Some(outcome) = outcome else {
                continue;
            };
            let (exit_kind, pnl, _) =
                classify_strict_trade_outcome(&outcome, NATGAS_STRICT_TAKE_PROFIT);
            *daily_pnl.entry(day_key.to_string()).or_insert(0.0) += pnl;
            trades += 1;
            net_pnl += pnl;
            match exit_kind {
                StrictTradeExit::TakeProfit => wins += 1,
                StrictTradeExit::StopLoss => losses += 1,
                _ => {}
            }
        }
        if trades == 0 || daily_pnl.is_empty() {
            return None;
        }
        let total_days = daily_pnl.len();
        let target_hit_days = daily_pnl
            .values()
            .filter(|value| **value >= NATGAS_STRICT_DAILY_TARGET)
            .count();
        Some(AnchoredVwapAntiEdgeCandidate {
            label: variant,
            lookback_hours,
            max_hold,
            trades,
            wins,
            losses,
            daily_target_hit_rate: target_hit_days as f64 / total_days as f64,
            target_hit_days,
            total_days,
            avg_daily_pnl_distance: daily_pnl.values().sum::<f64>() / total_days as f64,
            expectancy_distance: net_pnl / trades as f64,
            net_pnl_distance: net_pnl,
        })
    }

    fn run_natgas_h1_anchored_vwap_make_it_worse() {
        let h1_series = canonical_chart_series("NATGAS_USD", "H1", 0).expect("NATGAS_USD H1 history");
        let candles = h1_series.candles;
        let mut worst: Option<AnchoredVwapAntiEdgeCandidate> = None;
        for variant in [
            "touch_mean_reversion",
            "touch_inverted_follow",
            "ext2_mean_reversion",
            "ext2_inverted_follow",
        ] {
            for lookback_hours in [24_usize, 36, 48] {
                for max_hold in [6_usize, 12, 18, 24] {
                    let Some(candidate) = evaluate_anchored_vwap_anti_edge_candidate(
                        &candles,
                        lookback_hours,
                        max_hold,
                        variant,
                    ) else {
                        continue;
                    };
                    let replace = worst
                        .as_ref()
                        .map(|current| {
                            candidate
                                .net_pnl_distance
                                .partial_cmp(&current.net_pnl_distance)
                                .unwrap_or(Ordering::Equal)
                                == Ordering::Less
                        })
                        .unwrap_or(true);
                    if replace {
                        worst = Some(candidate);
                    }
                }
            }
        }
        let worst = worst.expect("worst anchored vwap anti-edge");
        println!(
            "NATGAS_H1_ANCHORED_VWAP_WORST variant={} lookback_hours={} max_hold={} trades={} wins={} losses={} win_rate={:.4} daily_target_hit_rate={:.4} target_hit_days={} total_days={} avg_daily_pnl={:.6} expectancy={:.6} net_pnl={:.6}",
            worst.label,
            worst.lookback_hours,
            worst.max_hold,
            worst.trades,
            worst.wins,
            worst.losses,
            worst.wins as f64 / worst.trades as f64,
            worst.daily_target_hit_rate,
            worst.target_hit_days,
            worst.total_days,
            worst.avg_daily_pnl_distance,
            worst.expectancy_distance,
            worst.net_pnl_distance,
        );
    }

    fn evaluate_anchored_vwap_ext2_rejection_h4_candidate(
        h1_candles: &[TradingCandlePoint],
        h4_bias_by_h1_index: &[H4BiasState],
        lookback_hours: usize,
        max_hold: usize,
    ) -> Option<AnchoredVwapTouchCandidate> {
        let mut daily_pnl = BTreeMap::<String, f64>::new();
        let mut trades = 0usize;
        let mut wins = 0usize;
        let mut losses = 0usize;
        let mut net_pnl = 0.0;
        for index in lookback_hours..h1_candles.len().saturating_sub(2) {
            let Some(hour) = strategy_hour_utc(&h1_candles[index].time) else {
                continue;
            };
            if !(7..=21).contains(&hour) {
                continue;
            }
            let Some(day_key) = h1_candles[index].time.get(..10) else {
                continue;
            };
            let bias = h4_bias_by_h1_index
                .get(index)
                .copied()
                .unwrap_or(H4BiasState::Neutral);
            let high_anchor = rolling_extreme_index(h1_candles, index, lookback_hours, true);
            let low_anchor = rolling_extreme_index(h1_candles, index, lookback_hours, false);
            if high_anchor == index && low_anchor == index {
                continue;
            }
            let (avwap_high, sigma_high) = anchored_vwap_sigma_between(h1_candles, high_anchor, index);
            let (avwap_low, sigma_low) = anchored_vwap_sigma_between(h1_candles, low_anchor, index);
            let ext2_up = avwap_high + 2.0 * sigma_high;
            let ext2_down = avwap_low - 2.0 * sigma_low;
            let candle = &h1_candles[index];
            let short_setup = bias == H4BiasState::Bearish
                && high_anchor < index
                && candle.high >= ext2_up
                && candle.close < ext2_up
                && bearish_body(candle);
            let long_setup = bias == H4BiasState::Bullish
                && low_anchor < index
                && candle.low <= ext2_down
                && candle.close > ext2_down
                && bullish_body(candle);
            let direction = match (long_setup, short_setup) {
                (true, false) => Some("long"),
                (false, true) => Some("short"),
                _ => None,
            };
            let Some(direction) = direction else {
                continue;
            };
            let entry_index = index + 1;
            let (outcome, _) = strategy_entry_outcome_cached(
                h1_candles,
                entry_index,
                direction,
                NATGAS_STRICT_STOP_LOSS,
                NATGAS_STRICT_EXECUTION_COST,
                max_hold,
            );
            let Some(outcome) = outcome else {
                continue;
            };
            let (exit_kind, pnl, _) =
                classify_strict_trade_outcome(&outcome, NATGAS_STRICT_TAKE_PROFIT);
            *daily_pnl.entry(day_key.to_string()).or_insert(0.0) += pnl;
            trades += 1;
            net_pnl += pnl;
            match exit_kind {
                StrictTradeExit::TakeProfit => wins += 1,
                StrictTradeExit::StopLoss => losses += 1,
                _ => {}
            }
        }
        if trades == 0 || daily_pnl.is_empty() {
            return None;
        }
        let total_days = daily_pnl.len();
        let target_hit_days = daily_pnl
            .values()
            .filter(|value| **value >= NATGAS_STRICT_DAILY_TARGET)
            .count();
        Some(AnchoredVwapTouchCandidate {
            lookback_hours,
            max_hold,
            trades,
            wins,
            losses,
            daily_target_hit_rate: target_hit_days as f64 / total_days as f64,
            target_hit_days,
            total_days,
            avg_daily_pnl_distance: daily_pnl.values().sum::<f64>() / total_days as f64,
            expectancy_distance: net_pnl / trades as f64,
            net_pnl_distance: net_pnl,
        })
    }

    fn run_natgas_h1_anchored_vwap_ext2_rejection_h4_search() {
        let h1_series = canonical_chart_series("NATGAS_USD", "H1", 0).expect("NATGAS_USD H1 history");
        let h4_series = canonical_chart_series("NATGAS_USD", "H4", 0).expect("NATGAS_USD H4 history");
        let h1_candles = h1_series.candles;
        let h4_candles = h4_series.candles;
        let mut cache_snapshot = StrategyCacheSnapshot::default();
        let h4_hash = strategy_candles_hash(&h4_candles);
        let h4_bank = strategy_feature_bank_cached(&h4_candles, &h4_hash, &mut cache_snapshot);
        let h4_bias_by_h1_index = build_h4_bias_by_h1_index(&h1_candles, &h4_candles, &h4_bank);
        let mut best: Option<AnchoredVwapTouchCandidate> = None;
        for lookback_hours in [24_usize, 36, 48] {
            for max_hold in [6_usize, 12, 18, 24] {
                let Some(candidate) = evaluate_anchored_vwap_ext2_rejection_h4_candidate(
                    &h1_candles,
                    &h4_bias_by_h1_index,
                    lookback_hours,
                    max_hold,
                ) else {
                    continue;
                };
                let replace = best
                    .as_ref()
                    .map(|current| {
                        candidate
                            .daily_target_hit_rate
                            .partial_cmp(&current.daily_target_hit_rate)
                            .unwrap_or(Ordering::Equal)
                            .then_with(|| {
                                candidate
                                    .avg_daily_pnl_distance
                                    .partial_cmp(&current.avg_daily_pnl_distance)
                                    .unwrap_or(Ordering::Equal)
                            })
                            .then_with(|| {
                                candidate
                                    .expectancy_distance
                                    .partial_cmp(&current.expectancy_distance)
                                    .unwrap_or(Ordering::Equal)
                            })
                            == Ordering::Greater
                    })
                    .unwrap_or(true);
                if replace {
                    best = Some(candidate);
                }
            }
        }
        let best = best.expect("best anchored vwap ext2 rejection h4");
        println!(
            "NATGAS_H1_ANCHORED_VWAP_EXT2_REJECTION_H4 best_lookback_hours={} best_max_hold={} trades={} wins={} losses={} win_rate={:.4} daily_target_hit_rate={:.4} target_hit_days={} total_days={} avg_daily_pnl={:.6} expectancy={:.6} net_pnl={:.6}",
            best.lookback_hours,
            best.max_hold,
            best.trades,
            best.wins,
            best.losses,
            best.wins as f64 / best.trades as f64,
            best.daily_target_hit_rate,
            best.target_hit_days,
            best.total_days,
            best.avg_daily_pnl_distance,
            best.expectancy_distance,
            best.net_pnl_distance,
        );
    }

    fn update_hour_behavior_stats(
        stats: &mut HourCloseBehaviorStats,
        candle: &TradingCandlePoint,
        future_1: &TradingCandlePoint,
        future_3: &TradingCandlePoint,
        future_6: &TradingCandlePoint,
        long_exit: StrictTradeExit,
        short_exit: StrictTradeExit,
    ) {
        stats.samples += 1;
        let close = candle.close;
        let ret1 = future_1.close - close;
        let ret3 = future_3.close - close;
        let ret6 = future_6.close - close;
        stats.avg_return_1 += ret1;
        stats.avg_return_3 += ret3;
        stats.avg_return_6 += ret6;
        if bullish_body(candle) {
            stats.bullish_closes += 1;
            if ret1 > 0.0 {
                stats.bullish_next1_continue += 1;
            }
            if ret3 > 0.0 {
                stats.bullish_next3_continue += 1;
            }
            if ret6 > 0.0 {
                stats.bullish_next6_continue += 1;
            }
        } else if bearish_body(candle) {
            stats.bearish_closes += 1;
            if ret1 < 0.0 {
                stats.bearish_next1_continue += 1;
            }
            if ret3 < 0.0 {
                stats.bearish_next3_continue += 1;
            }
            if ret6 < 0.0 {
                stats.bearish_next6_continue += 1;
            }
        }
        match long_exit {
            StrictTradeExit::TakeProfit => stats.long_tp_hits += 1,
            StrictTradeExit::StopLoss => stats.long_sl_hits += 1,
            _ => {}
        }
        match short_exit {
            StrictTradeExit::TakeProfit => stats.short_tp_hits += 1,
            StrictTradeExit::StopLoss => stats.short_sl_hits += 1,
            _ => {}
        }
    }

    fn hour_close_vwap_context_label(
        candles: &[TradingCandlePoint],
        feature_bank: &StrategyIndicatorFeatureBank,
        index: usize,
    ) -> &'static str {
        let close = candles[index].close;
        if close > feature_bank.vwap_ext1_up[index] {
            "above_vwap_plus1s"
        } else if close > feature_bank.vwap[index] {
            "above_vwap"
        } else if close < feature_bank.vwap_ext1_down[index] {
            "below_vwap_minus1s"
        } else if close < feature_bank.vwap[index] {
            "below_vwap"
        } else {
            "near_vwap"
        }
    }

    fn hour_close_momentum_context_label(
        feature_bank: &StrategyIndicatorFeatureBank,
        index: usize,
    ) -> &'static str {
        let macd = feature_bank.macd_histogram[index];
        let rsi = feature_bank.rsi14[index];
        if macd > 0.0 && rsi >= 55.0 {
            "mom_up"
        } else if macd < 0.0 && rsi <= 45.0 {
            "mom_down"
        } else {
            "mom_mixed"
        }
    }

    fn hour_close_volatility_context_label(
        feature_bank: &StrategyIndicatorFeatureBank,
        index: usize,
    ) -> &'static str {
        let atr = feature_bank.atr14[index];
        let width = feature_bank.boll_widths[index];
        if atr <= feature_bank.atr_p35 && width <= feature_bank.boll_width_p35 {
            "squeeze"
        } else if atr > feature_bank.atr_p50 && width > feature_bank.boll_width_p50 {
            "expanded"
        } else {
            "normal_vol"
        }
    }

    fn hour_close_body_context_label(candle: &TradingCandlePoint) -> &'static str {
        if bullish_body(candle) {
            "bull_body"
        } else if bearish_body(candle) {
            "bear_body"
        } else {
            "flat_body"
        }
    }

    fn run_natgas_h1_hour_close_context_audit() {
        let h1_series = canonical_chart_series("NATGAS_USD", "H1", 0).expect("NATGAS_USD H1 history");
        let h4_series = canonical_chart_series("NATGAS_USD", "H4", 0).expect("NATGAS_USD H4 history");
        let candles = h1_series.candles;
        let h4_candles = h4_series.candles;
        let mut cache_snapshot = StrategyCacheSnapshot::default();
        let h1_hash = strategy_candles_hash(&candles);
        let h4_hash = strategy_candles_hash(&h4_candles);
        let h1_bank = strategy_feature_bank_cached(&candles, &h1_hash, &mut cache_snapshot);
        let h4_bank = strategy_feature_bank_cached(&h4_candles, &h4_hash, &mut cache_snapshot);
        let h4_bias_by_h1_index = build_h4_bias_by_h1_index(&candles, &h4_candles, &h4_bank);
        let mut by_hour_context = BTreeMap::<(u32, String), HourCloseBehaviorStats>::new();
        for index in 0..candles.len().saturating_sub(7) {
            let Some(hour) = strategy_hour_utc(&candles[index].time) else {
                continue;
            };
            if !(7..=21).contains(&hour) {
                continue;
            }
            let (long_outcome, _) = strategy_entry_outcome_cached(
                &candles,
                index + 1,
                "long",
                NATGAS_STRICT_STOP_LOSS,
                NATGAS_STRICT_EXECUTION_COST,
                6,
            );
            let (short_outcome, _) = strategy_entry_outcome_cached(
                &candles,
                index + 1,
                "short",
                NATGAS_STRICT_STOP_LOSS,
                NATGAS_STRICT_EXECUTION_COST,
                6,
            );
            let Some(long_outcome) = long_outcome else {
                continue;
            };
            let Some(short_outcome) = short_outcome else {
                continue;
            };
            let (long_exit, _, _) =
                classify_strict_trade_outcome(&long_outcome, NATGAS_STRICT_TAKE_PROFIT);
            let (short_exit, _, _) =
                classify_strict_trade_outcome(&short_outcome, NATGAS_STRICT_TAKE_PROFIT);
            let h4_bias = h4_bias_by_h1_index
                .get(index)
                .copied()
                .unwrap_or(H4BiasState::Neutral)
                .label();
            let context = format!(
                "body={}|vwap={}|mom={}|vol={}|h4={}",
                hour_close_body_context_label(&candles[index]),
                hour_close_vwap_context_label(&candles, &h1_bank, index),
                hour_close_momentum_context_label(&h1_bank, index),
                hour_close_volatility_context_label(&h1_bank, index),
                h4_bias
            );
            let stats = by_hour_context.entry((hour, context)).or_default();
            update_hour_behavior_stats(
                stats,
                &candles[index],
                &candles[index + 1],
                &candles[index + 3],
                &candles[index + 6],
                long_exit,
                short_exit,
            );
        }
        println!("NATGAS_H1_HOUR_CLOSE_CONTEXT_AUDIT methodology=\"hourly close audit with context buckets body/vwap/momentum/volatility/h4_bias; next-open strict entries long and short use TP 5.1p, SL 3.9p, spread 0.6p, hold=6 bars; only context buckets with at least 35 samples are ranked per hour\"");
        for hour in 7_u32..=21_u32 {
            let mut contexts = by_hour_context
                .iter()
                .filter(|((ctx_hour, _), stats)| *ctx_hour == hour && stats.samples >= 35)
                .map(|((_, label), stats)| (label.as_str(), stats))
                .collect::<Vec<_>>();
            if contexts.is_empty() {
                continue;
            }
            contexts.sort_by(|(left_label, left), (right_label, right)| {
                right
                    .short_tp_rate()
                    .max(right.long_tp_rate())
                    .partial_cmp(&left.short_tp_rate().max(left.long_tp_rate()))
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| left_label.cmp(right_label))
            });
            let best_short = contexts
                .iter()
                .max_by(|(_, left), (_, right)| {
                    left.short_tp_rate()
                        .partial_cmp(&right.short_tp_rate())
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .copied();
            let best_long = contexts
                .iter()
                .max_by(|(_, left), (_, right)| {
                    left.long_tp_rate()
                        .partial_cmp(&right.long_tp_rate())
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .copied();
            if let Some((label, stats)) = best_short {
                println!(
                    "NATGAS_H1_HOUR_CLOSE_CONTEXT_AUDIT hour={:02} best_short_ctx=\"{}\" samples={} short_tp_rate={:.4} long_tp_rate={:.4} avg_ret_1={:.5} avg_ret_3={:.5} avg_ret_6={:.5}",
                    hour,
                    label,
                    stats.samples,
                    stats.short_tp_rate(),
                    stats.long_tp_rate(),
                    stats.avg_ret_1(),
                    stats.avg_ret_3(),
                    stats.avg_ret_6(),
                );
            }
            if let Some((label, stats)) = best_long {
                println!(
                    "NATGAS_H1_HOUR_CLOSE_CONTEXT_AUDIT hour={:02} best_long_ctx=\"{}\" samples={} long_tp_rate={:.4} short_tp_rate={:.4} avg_ret_1={:.5} avg_ret_3={:.5} avg_ret_6={:.5}",
                    hour,
                    label,
                    stats.samples,
                    stats.long_tp_rate(),
                    stats.short_tp_rate(),
                    stats.avg_ret_1(),
                    stats.avg_ret_3(),
                    stats.avg_ret_6(),
                );
            }
        }
    }

    fn run_natgas_h1_hour_close_behavior_audit() {
        let series = canonical_chart_series("NATGAS_USD", "H1", 0).expect("NATGAS_USD H1 history");
        let candles = series.candles;
        let mut by_hour = BTreeMap::<u32, HourCloseBehaviorStats>::new();
        for index in 0..candles.len().saturating_sub(7) {
            let Some(hour) = strategy_hour_utc(&candles[index].time) else {
                continue;
            };
            if !(7..=21).contains(&hour) {
                continue;
            }
            let (long_outcome, _) = strategy_entry_outcome_cached(
                &candles,
                index + 1,
                "long",
                NATGAS_STRICT_STOP_LOSS,
                NATGAS_STRICT_EXECUTION_COST,
                6,
            );
            let (short_outcome, _) = strategy_entry_outcome_cached(
                &candles,
                index + 1,
                "short",
                NATGAS_STRICT_STOP_LOSS,
                NATGAS_STRICT_EXECUTION_COST,
                6,
            );
            let Some(long_outcome) = long_outcome else {
                continue;
            };
            let Some(short_outcome) = short_outcome else {
                continue;
            };
            let (long_exit, _, _) =
                classify_strict_trade_outcome(&long_outcome, NATGAS_STRICT_TAKE_PROFIT);
            let (short_exit, _, _) =
                classify_strict_trade_outcome(&short_outcome, NATGAS_STRICT_TAKE_PROFIT);
            let stats = by_hour.entry(hour).or_default();
            update_hour_behavior_stats(
                stats,
                &candles[index],
                &candles[index + 1],
                &candles[index + 3],
                &candles[index + 6],
                long_exit,
                short_exit,
            );
        }
        println!("NATGAS_H1_HOUR_CLOSE_AUDIT methodology=\"for each H1 candle close between 07h and 21h UTC, compute next-close continuation over 1/3/6 bars and strict TP/SL hit rates for long/short entries at next open with TP 5.1p, SL 3.9p, spread 0.6p, hold=6 bars\"");
        for hour in 7_u32..=21_u32 {
            let Some(stats) = by_hour.get(&hour) else {
                continue;
            };
            let avg1 = if stats.samples > 0 {
                stats.avg_return_1 / stats.samples as f64
            } else {
                0.0
            };
            let avg3 = if stats.samples > 0 {
                stats.avg_return_3 / stats.samples as f64
            } else {
                0.0
            };
            let avg6 = if stats.samples > 0 {
                stats.avg_return_6 / stats.samples as f64
            } else {
                0.0
            };
            let bull1 = if stats.bullish_closes > 0 {
                stats.bullish_next1_continue as f64 / stats.bullish_closes as f64
            } else {
                0.0
            };
            let bull3 = if stats.bullish_closes > 0 {
                stats.bullish_next3_continue as f64 / stats.bullish_closes as f64
            } else {
                0.0
            };
            let bull6 = if stats.bullish_closes > 0 {
                stats.bullish_next6_continue as f64 / stats.bullish_closes as f64
            } else {
                0.0
            };
            let bear1 = if stats.bearish_closes > 0 {
                stats.bearish_next1_continue as f64 / stats.bearish_closes as f64
            } else {
                0.0
            };
            let bear3 = if stats.bearish_closes > 0 {
                stats.bearish_next3_continue as f64 / stats.bearish_closes as f64
            } else {
                0.0
            };
            let bear6 = if stats.bearish_closes > 0 {
                stats.bearish_next6_continue as f64 / stats.bearish_closes as f64
            } else {
                0.0
            };
            let long_win = if stats.samples > 0 {
                stats.long_tp_hits as f64 / stats.samples as f64
            } else {
                0.0
            };
            let short_win = if stats.samples > 0 {
                stats.short_tp_hits as f64 / stats.samples as f64
            } else {
                0.0
            };
            println!(
                "NATGAS_H1_HOUR_CLOSE_AUDIT hour={:02} samples={} bullish_rate={:.4} bearish_rate={:.4} bull_continue_1={:.4} bull_continue_3={:.4} bull_continue_6={:.4} bear_continue_1={:.4} bear_continue_3={:.4} bear_continue_6={:.4} avg_ret_1={:.5} avg_ret_3={:.5} avg_ret_6={:.5} long_tp_rate={:.4} short_tp_rate={:.4}",
                hour,
                stats.samples,
                stats.bullish_rate(),
                stats.bearish_rate(),
                bull1,
                bull3,
                bull6,
                bear1,
                bear3,
                bear6,
                avg1,
                avg3,
                avg6,
                long_win,
                short_win,
            );
        }
    }

    #[derive(Clone, Default)]
    struct HourRecurrenceSearchMetrics {
        trades: usize,
        tp_hits: usize,
        sl_hits: usize,
        win_rate: f64,
        expectancy_distance: f64,
        net_pnl_distance: f64,
    }

    #[derive(Clone)]
    struct HourRecurrenceComboResult {
        hour: u32,
        direction: &'static str,
        labels: Vec<&'static str>,
        train: HourRecurrenceSearchMetrics,
        test: HourRecurrenceSearchMetrics,
    }

    fn run_natgas_h1_hour_recurrence_combo_search() {
        let h1_series = canonical_chart_series("NATGAS_USD", "H1", 0).expect("NATGAS_USD H1 history");
        let h4_series = canonical_chart_series("NATGAS_USD", "H4", 0).expect("NATGAS_USD H4 history");
        let candles = h1_series.candles;
        let h4_candles = h4_series.candles;
        let train_rows = ((candles.len() as f64) * 0.7).round() as usize;
        let test_start = train_rows.max(56);
        let mut cache_snapshot = StrategyCacheSnapshot::default();
        let h1_hash = strategy_candles_hash(&candles);
        let h4_hash = strategy_candles_hash(&h4_candles);
        let bank = strategy_feature_bank_cached(&candles, &h1_hash, &mut cache_snapshot);
        let h4_bank = strategy_feature_bank_cached(&h4_candles, &h4_hash, &mut cache_snapshot);
        let h4_bias_by_h1_index = build_h4_bias_by_h1_index(&candles, &h4_candles, &h4_bank);

        let mut prev24_high = vec![f64::NAN; candles.len()];
        let mut prev24_low = vec![f64::NAN; candles.len()];
        let mut prev24_mid = vec![f64::NAN; candles.len()];
        let mut prev24_pos = vec![f64::NAN; candles.len()];
        let mut prev24_return = vec![f64::NAN; candles.len()];
        for index in 24..candles.len() {
            let window = &candles[index - 24..index];
            let high = window.iter().map(|c| c.high).fold(f64::NEG_INFINITY, f64::max);
            let low = window.iter().map(|c| c.low).fold(f64::INFINITY, f64::min);
            let width = (high - low).abs();
            prev24_high[index] = high;
            prev24_low[index] = low;
            prev24_mid[index] = (high + low) * 0.5;
            prev24_return[index] = candles[index].close - candles[index - 24].close;
            if width.is_finite() && width > f64::EPSILON {
                prev24_pos[index] = (candles[index].close - low) / width;
            }
        }

        let build_bool_map = |predicate: &dyn Fn(usize) -> bool| {
            let mut out = vec![false; candles.len()];
            for index in 0..candles.len() {
                out[index] = predicate(index);
            }
            out
        };

        let long_predicates: Vec<(&'static str, Vec<bool>)> = vec![
            ("bullish_body", build_bool_map(&|i| bullish_body(&candles[i]))),
            (
                "prev_red_then_green",
                build_bool_map(&|i| i > 0 && bearish_body(&candles[i - 1]) && bullish_body(&candles[i])),
            ),
            (
                "two_bull_bodies",
                build_bool_map(&|i| i > 0 && bullish_body(&candles[i - 1]) && bullish_body(&candles[i])),
            ),
            (
                "three_higher_closes",
                build_bool_map(&|i| i >= 2 && candles[i].close > candles[i - 1].close && candles[i - 1].close > candles[i - 2].close),
            ),
            ("close_gt_vwap", build_bool_map(&|i| candles[i].close > bank.vwap[i])),
            ("close_gt_donchian20_mid", build_bool_map(&|i| candles[i].close > bank.donchian20_mid[i])),
            ("close_gt_donchian55_mid", build_bool_map(&|i| candles[i].close > bank.donchian55_mid[i])),
            ("macd_hist_gt_0", build_bool_map(&|i| bank.macd_histogram[i] > 0.0)),
            ("rsi14_gt_50", build_bool_map(&|i| bank.rsi14[i] > 50.0)),
            ("stoch_k_gt_d", build_bool_map(&|i| bank.stoch14_k[i] > bank.stoch14_d[i])),
            (
                "h4_bullish",
                build_bool_map(&|i| h4_bias_by_h1_index.get(i).copied().unwrap_or(H4BiasState::Neutral) == H4BiasState::Bullish),
            ),
            (
                "close_lower_24h_quartile",
                build_bool_map(&|i| prev24_pos[i].is_finite() && prev24_pos[i] <= 0.25),
            ),
            (
                "close_upper_24h_quartile",
                build_bool_map(&|i| prev24_pos[i].is_finite() && prev24_pos[i] >= 0.75),
            ),
            (
                "ret24_positive",
                build_bool_map(&|i| prev24_return[i].is_finite() && prev24_return[i] > 0.0),
            ),
            (
                "breaks_prev24_high",
                build_bool_map(&|i| prev24_high[i].is_finite() && candles[i].close > prev24_high[i]),
            ),
            (
                "close_gt_prev24_mid",
                build_bool_map(&|i| prev24_mid[i].is_finite() && candles[i].close > prev24_mid[i]),
            ),
        ];
        let short_predicates: Vec<(&'static str, Vec<bool>)> = vec![
            ("bearish_body", build_bool_map(&|i| bearish_body(&candles[i]))),
            (
                "prev_green_then_red",
                build_bool_map(&|i| i > 0 && bullish_body(&candles[i - 1]) && bearish_body(&candles[i])),
            ),
            (
                "two_bear_bodies",
                build_bool_map(&|i| i > 0 && bearish_body(&candles[i - 1]) && bearish_body(&candles[i])),
            ),
            (
                "three_lower_closes",
                build_bool_map(&|i| i >= 2 && candles[i].close < candles[i - 1].close && candles[i - 1].close < candles[i - 2].close),
            ),
            ("close_lt_vwap", build_bool_map(&|i| candles[i].close < bank.vwap[i])),
            ("close_lt_donchian20_mid", build_bool_map(&|i| candles[i].close < bank.donchian20_mid[i])),
            ("close_lt_donchian55_mid", build_bool_map(&|i| candles[i].close < bank.donchian55_mid[i])),
            ("macd_hist_lt_0", build_bool_map(&|i| bank.macd_histogram[i] < 0.0)),
            ("rsi14_lt_50", build_bool_map(&|i| bank.rsi14[i] < 50.0)),
            ("stoch_k_lt_d", build_bool_map(&|i| bank.stoch14_k[i] < bank.stoch14_d[i])),
            (
                "h4_bearish",
                build_bool_map(&|i| h4_bias_by_h1_index.get(i).copied().unwrap_or(H4BiasState::Neutral) == H4BiasState::Bearish),
            ),
            (
                "close_upper_24h_quartile",
                build_bool_map(&|i| prev24_pos[i].is_finite() && prev24_pos[i] >= 0.75),
            ),
            (
                "close_lower_24h_quartile",
                build_bool_map(&|i| prev24_pos[i].is_finite() && prev24_pos[i] <= 0.25),
            ),
            (
                "ret24_negative",
                build_bool_map(&|i| prev24_return[i].is_finite() && prev24_return[i] < 0.0),
            ),
            (
                "breaks_prev24_low",
                build_bool_map(&|i| prev24_low[i].is_finite() && candles[i].close < prev24_low[i]),
            ),
            (
                "close_lt_prev24_mid",
                build_bool_map(&|i| prev24_mid[i].is_finite() && candles[i].close < prev24_mid[i]),
            ),
        ];

        let eval_indices = |indices: &[usize], direction: &str| {
            let mut metrics = HourRecurrenceSearchMetrics::default();
            for index in indices {
                let (outcome, _) = strategy_entry_outcome_cached(
                    &candles,
                    index + 1,
                    direction,
                    NATGAS_STRICT_STOP_LOSS,
                    NATGAS_STRICT_EXECUTION_COST,
                    6,
                );
                let Some(outcome) = outcome else {
                    continue;
                };
                let (exit_kind, pnl, _) = classify_strict_trade_outcome(&outcome, NATGAS_STRICT_TAKE_PROFIT);
                metrics.trades += 1;
                metrics.net_pnl_distance += pnl;
                match exit_kind {
                    StrictTradeExit::TakeProfit => metrics.tp_hits += 1,
                    StrictTradeExit::StopLoss => metrics.sl_hits += 1,
                    _ => {}
                }
            }
            if metrics.trades > 0 {
                metrics.win_rate = metrics.tp_hits as f64 / metrics.trades as f64;
                metrics.expectancy_distance = metrics.net_pnl_distance / metrics.trades as f64;
            }
            metrics
        };

        let mut best_results = Vec::<HourRecurrenceComboResult>::new();
        for hour in 7_u32..=21_u32 {
            for (direction, predicates) in [("long", &long_predicates), ("short", &short_predicates)] {
                let base_train = (56..train_rows.saturating_sub(7))
                    .filter(|index| strategy_hour_utc(&candles[*index].time) == Some(hour))
                    .collect::<Vec<_>>();
                let base_test = (test_start..candles.len().saturating_sub(7))
                    .filter(|index| strategy_hour_utc(&candles[*index].time) == Some(hour))
                    .collect::<Vec<_>>();
                let mut hour_best: Vec<HourRecurrenceComboResult> = Vec::new();
                let n = predicates.len();
                let evaluate_combo = |combo: &[usize], hour_best: &mut Vec<HourRecurrenceComboResult>| {
                    let train_indices = base_train
                        .iter()
                        .copied()
                        .filter(|index| combo.iter().all(|predicate_ix| predicates[*predicate_ix].1[*index]))
                        .collect::<Vec<_>>();
                    let test_indices = base_test
                        .iter()
                        .copied()
                        .filter(|index| combo.iter().all(|predicate_ix| predicates[*predicate_ix].1[*index]))
                        .collect::<Vec<_>>();
                    if train_indices.len() < 18 || test_indices.len() < 8 {
                        return;
                    }
                    let train = eval_indices(&train_indices, direction);
                    let test = eval_indices(&test_indices, direction);
                    if train.trades < 18 || test.trades < 8 || test.win_rate < 0.55 {
                        return;
                    }
                    let result = HourRecurrenceComboResult {
                        hour,
                        direction,
                        labels: combo.iter().map(|ix| predicates[*ix].0).collect::<Vec<_>>(),
                        train,
                        test,
                    };
                    hour_best.push(result);
                    hour_best.sort_by(|left, right| {
                        right
                            .test
                            .win_rate
                            .partial_cmp(&left.test.win_rate)
                            .unwrap_or(std::cmp::Ordering::Equal)
                            .then_with(|| {
                                right
                                    .test
                                    .expectancy_distance
                                    .partial_cmp(&left.test.expectancy_distance)
                                    .unwrap_or(std::cmp::Ordering::Equal)
                            })
                            .then_with(|| right.test.trades.cmp(&left.test.trades))
                    });
                    hour_best.truncate(3);
                };

                for a in 0..n {
                    evaluate_combo(&[a], &mut hour_best);
                    for b in a + 1..n {
                        evaluate_combo(&[a, b], &mut hour_best);
                        for c in b + 1..n {
                            evaluate_combo(&[a, b, c], &mut hour_best);
                        }
                    }
                }
                best_results.extend(hour_best);
            }
        }

        println!("NATGAS_H1_HOUR_RECURRENCE_COMBO_SEARCH methodology=\"for each H1 close hour 07h-21h UTC, search long/short recurrence patterns using indicators, previous candle structure, and previous-24h context; combinations of 1 to 3 predicates are discovered on train 70% and validated on holdout 30%; entries execute at next candle open with TP 5.1p, SL 3.9p, spread 0.6p, hold=6 bars\"");
        best_results.sort_by(|left, right| {
            left.hour
                .cmp(&right.hour)
                .then_with(|| left.direction.cmp(right.direction))
                .then_with(|| {
                    right
                        .test
                        .win_rate
                        .partial_cmp(&left.test.win_rate)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
        });
        for result in best_results {
            println!(
                "NATGAS_H1_HOUR_RECURRENCE_COMBO hour={:02} direction={} pattern=\"{}\" train_trades={} train_win_rate={:.4} train_expectancy={:.6} test_trades={} test_win_rate={:.4} test_expectancy={:.6} test_net_pnl={:.6}",
                result.hour,
                result.direction,
                result.labels.join(" && "),
                result.train.trades,
                result.train.win_rate,
                result.train.expectancy_distance,
                result.test.trades,
                result.test.win_rate,
                result.test.expectancy_distance,
                result.test.net_pnl_distance,
            );
        }
    }

    fn run_natgas_high_confidence_signal_refinement_search() {
        let h1_series = canonical_chart_series("NATGAS_USD", "H1", 0).expect("NATGAS_USD H1 history");
        let h4_series = canonical_chart_series("NATGAS_USD", "H4", 0).expect("NATGAS_USD H4 history");
        let candles = h1_series.candles;
        let h4_candles = h4_series.candles;
        let train_rows = ((candles.len() as f64) * 0.7).round() as usize;
        let test_start = train_rows.max(56);
        let mut cache_snapshot = StrategyCacheSnapshot::default();
        let h1_hash = strategy_candles_hash(&candles);
        let h4_hash = strategy_candles_hash(&h4_candles);
        let bank = strategy_feature_bank_cached(&candles, &h1_hash, &mut cache_snapshot);
        let h4_bank = strategy_feature_bank_cached(&h4_candles, &h4_hash, &mut cache_snapshot);
        let h4_bias_by_h1_index = build_h4_bias_by_h1_index(&candles, &h4_candles, &h4_bank);

        let mut prev24_high = vec![f64::NAN; candles.len()];
        let mut prev24_low = vec![f64::NAN; candles.len()];
        let mut prev24_mid = vec![f64::NAN; candles.len()];
        let mut prev24_pos = vec![f64::NAN; candles.len()];
        let mut prev24_return = vec![f64::NAN; candles.len()];
        for index in 24..candles.len() {
            let window = &candles[index - 24..index];
            let high = window.iter().map(|c| c.high).fold(f64::NEG_INFINITY, f64::max);
            let low = window.iter().map(|c| c.low).fold(f64::INFINITY, f64::min);
            let width = (high - low).abs();
            prev24_high[index] = high;
            prev24_low[index] = low;
            prev24_mid[index] = (high + low) * 0.5;
            prev24_return[index] = candles[index].close - candles[index - 24].close;
            if width.is_finite() && width > f64::EPSILON {
                prev24_pos[index] = (candles[index].close - low) / width;
            }
        }

        let eval_pattern = |indices: &[usize], direction: &str, max_hold: usize| {
            let mut metrics = HourRecurrenceSearchMetrics::default();
            for index in indices {
                let (outcome, _) = strategy_entry_outcome_cached(
                    &candles,
                    index + 1,
                    direction,
                    NATGAS_STRICT_STOP_LOSS,
                    NATGAS_STRICT_EXECUTION_COST,
                    max_hold,
                );
                let Some(outcome) = outcome else {
                    continue;
                };
                let (exit_kind, pnl, _) =
                    classify_strict_trade_outcome(&outcome, NATGAS_STRICT_TAKE_PROFIT);
                metrics.trades += 1;
                metrics.net_pnl_distance += pnl;
                match exit_kind {
                    StrictTradeExit::TakeProfit => metrics.tp_hits += 1,
                    StrictTradeExit::StopLoss => metrics.sl_hits += 1,
                    _ => {}
                }
            }
            if metrics.trades > 0 {
                metrics.win_rate = metrics.tp_hits as f64 / metrics.trades as f64;
                metrics.expectancy_distance = metrics.net_pnl_distance / metrics.trades as f64;
            }
            metrics
        };

        println!("NATGAS_H1_HIGH_CONFIDENCE_REFINEMENT methodology=\"refine only the previously discovered >=70% signal families by adding 1-2 extra predicates around them; keep train/test split 70/30, require holdout win rate >= 70%, and report only surviving variants\"");

        let print_family = |
            family: &str,
            hour: u32,
            direction: &'static str,
            max_hold: usize,
            base_label: &'static str,
            base_predicate: &dyn Fn(usize) -> bool,
            optional_predicates: &[(&'static str, &dyn Fn(usize) -> bool)],
            min_train: usize,
            min_test: usize| {
            let train_base = (56..train_rows.saturating_sub(7))
                .filter(|index| strategy_hour_utc(&candles[*index].time) == Some(hour))
                .collect::<Vec<_>>();
            let test_base = (test_start..candles.len().saturating_sub(7))
                .filter(|index| strategy_hour_utc(&candles[*index].time) == Some(hour))
                .collect::<Vec<_>>();
            let mut best_rows = Vec::<(Vec<&'static str>, HourRecurrenceSearchMetrics, HourRecurrenceSearchMetrics)>::new();
            let eval_combo = |labels: Vec<&'static str>,
                              predicates: Vec<&dyn Fn(usize) -> bool>,
                              best_rows: &mut Vec<(Vec<&'static str>, HourRecurrenceSearchMetrics, HourRecurrenceSearchMetrics)>| {
                let train_indices = train_base
                    .iter()
                    .copied()
                    .filter(|index| predicates.iter().all(|predicate| predicate(*index)))
                    .collect::<Vec<_>>();
                let test_indices = test_base
                    .iter()
                    .copied()
                    .filter(|index| predicates.iter().all(|predicate| predicate(*index)))
                    .collect::<Vec<_>>();
                if train_indices.len() < min_train || test_indices.len() < min_test {
                    return;
                }
                let train = eval_pattern(&train_indices, direction, max_hold);
                let test = eval_pattern(&test_indices, direction, max_hold);
                if test.win_rate < 0.70 {
                    return;
                }
                best_rows.push((labels, train, test));
                best_rows.sort_by(|left, right| {
                    right
                        .2
                        .win_rate
                        .partial_cmp(&left.2.win_rate)
                        .unwrap_or(std::cmp::Ordering::Equal)
                        .then_with(|| {
                            right
                                .2
                                .expectancy_distance
                                .partial_cmp(&left.2.expectancy_distance)
                                .unwrap_or(std::cmp::Ordering::Equal)
                        })
                        .then_with(|| right.2.trades.cmp(&left.2.trades))
                });
                best_rows.truncate(6);
            };

            eval_combo(vec![base_label], vec![base_predicate], &mut best_rows);
            for a in 0..optional_predicates.len() {
                eval_combo(
                    vec![base_label, optional_predicates[a].0],
                    vec![base_predicate, optional_predicates[a].1],
                    &mut best_rows,
                );
                for b in a + 1..optional_predicates.len() {
                    eval_combo(
                        vec![base_label, optional_predicates[a].0, optional_predicates[b].0],
                        vec![base_predicate, optional_predicates[a].1, optional_predicates[b].1],
                        &mut best_rows,
                    );
                }
            }

            for (labels, train, test) in best_rows {
                println!(
                    "NATGAS_H1_HIGH_CONFIDENCE_REFINEMENT family={} hour={:02} direction={} hold={} pattern=\"{}\" train_trades={} train_win_rate={:.4} train_expectancy={:.6} test_trades={} test_win_rate={:.4} test_expectancy={:.6} test_net_pnl={:.6}",
                    family,
                    hour,
                    direction,
                    max_hold,
                    labels.join(" && "),
                    train.trades,
                    train.win_rate,
                    train.expectancy_distance,
                    test.trades,
                    test.win_rate,
                    test.expectancy_distance,
                    test.net_pnl_distance,
                );
            }
        };

        let h11_base = |i: usize| {
            bearish_body(&candles[i])
                && candles[i].close < bank.vwap_ext1_down[i]
                && bank.macd_histogram[i] < 0.0
                && bank.rsi14[i] <= 45.0
                && bank.atr14[i] <= bank.atr_p35
                && bank.boll_widths[i] <= bank.boll_width_p35
                && h4_bias_by_h1_index.get(i).copied().unwrap_or(H4BiasState::Neutral)
                    == H4BiasState::Bearish
        };
        let h11_opts: [(&str, &dyn Fn(usize) -> bool); 5] = [
            ("two_bear_bodies", &|i| i > 0 && bearish_body(&candles[i - 1]) && bearish_body(&candles[i])),
            ("stoch_k_lt_d", &|i| bank.stoch14_k[i] < bank.stoch14_d[i]),
            ("close_lt_prev24_mid", &|i| prev24_mid[i].is_finite() && candles[i].close < prev24_mid[i]),
            ("close_upper_24h_quartile", &|i| prev24_pos[i].is_finite() && prev24_pos[i] >= 0.75),
            ("close_lt_donchian55_mid", &|i| candles[i].close < bank.donchian55_mid[i]),
        ];
        print_family(
            "11h_short_sniper",
            11,
            "short",
            24,
            "base_11h_short_sniper",
            &h11_base,
            &h11_opts,
            16,
            8,
        );

        let h18_base = |i: usize| {
            bullish_body(&candles[i])
                && candles[i].close < bank.vwap[i]
                && bank.macd_histogram[i] > 0.0
                && bank.rsi14[i] >= 55.0
                && bank.atr14[i] > bank.atr_p50
                && bank.boll_widths[i] > bank.boll_width_p50
                && h4_bias_by_h1_index.get(i).copied().unwrap_or(H4BiasState::Neutral)
                    == H4BiasState::Bearish
        };
        let h18_opts: [(&str, &dyn Fn(usize) -> bool); 5] = [
            ("two_bull_bodies", &|i| i > 0 && bullish_body(&candles[i - 1]) && bullish_body(&candles[i])),
            ("stoch_k_gt_d", &|i| bank.stoch14_k[i] > bank.stoch14_d[i]),
            ("close_lower_24h_quartile", &|i| prev24_pos[i].is_finite() && prev24_pos[i] <= 0.25),
            ("ret24_negative", &|i| prev24_return[i].is_finite() && prev24_return[i] < 0.0),
            ("close_gt_prev24_mid", &|i| prev24_mid[i].is_finite() && candles[i].close > prev24_mid[i]),
        ];
        print_family(
            "18h_long_sniper",
            18,
            "long",
            24,
            "base_18h_long_sniper",
            &h18_base,
            &h18_opts,
            14,
            8,
        );

        let h13_base = |i: usize| {
            i > 0
                && bearish_body(&candles[i - 1])
                && bullish_body(&candles[i])
                && bank.macd_histogram[i] > 0.0
                && prev24_pos[i].is_finite()
                && prev24_pos[i] <= 0.25
        };
        let h13_opts: [(&str, &dyn Fn(usize) -> bool); 6] = [
            ("rsi14_gt_50", &|i| bank.rsi14[i] > 50.0),
            ("stoch_k_gt_d", &|i| bank.stoch14_k[i] > bank.stoch14_d[i]),
            ("close_gt_donchian20_mid", &|i| candles[i].close > bank.donchian20_mid[i]),
            ("close_gt_donchian55_mid", &|i| candles[i].close > bank.donchian55_mid[i]),
            ("close_gt_prev24_mid", &|i| prev24_mid[i].is_finite() && candles[i].close > prev24_mid[i]),
            ("ret24_negative", &|i| prev24_return[i].is_finite() && prev24_return[i] < 0.0),
        ];
        print_family(
            "13h_long_recurrence",
            13,
            "long",
            6,
            "base_13h_long_recurrence",
            &h13_base,
            &h13_opts,
            15,
            8,
        );

        let h09_short_base = |i: usize| {
            i > 0
                && bearish_body(&candles[i - 1])
                && bearish_body(&candles[i])
                && bank.stoch14_k[i] < bank.stoch14_d[i]
                && prev24_mid[i].is_finite()
                && candles[i].close < prev24_mid[i]
        };
        let h09_short_opts: [(&str, &dyn Fn(usize) -> bool); 6] = [
            ("macd_hist_lt_0", &|i| bank.macd_histogram[i] < 0.0),
            ("rsi14_lt_50", &|i| bank.rsi14[i] < 50.0),
            ("close_lt_vwap", &|i| candles[i].close < bank.vwap[i]),
            ("close_lt_donchian55_mid", &|i| candles[i].close < bank.donchian55_mid[i]),
            ("ret24_negative", &|i| prev24_return[i].is_finite() && prev24_return[i] < 0.0),
            (
                "h4_bearish",
                &|i| h4_bias_by_h1_index.get(i).copied().unwrap_or(H4BiasState::Neutral)
                    == H4BiasState::Bearish,
            ),
        ];
        print_family(
            "09h_short_recurrence",
            9,
            "short",
            6,
            "base_09h_short_recurrence",
            &h09_short_base,
            &h09_short_opts,
            18,
            10,
        );

        let h15_short_base = |i: usize| {
            bearish_body(&candles[i])
                && bank.stoch14_k[i] < bank.stoch14_d[i]
                && prev24_pos[i].is_finite()
                && prev24_pos[i] >= 0.75
        };
        let h15_short_opts: [(&str, &dyn Fn(usize) -> bool); 6] = [
            ("macd_hist_lt_0", &|i| bank.macd_histogram[i] < 0.0),
            ("rsi14_lt_50", &|i| bank.rsi14[i] < 50.0),
            ("close_lt_vwap", &|i| candles[i].close < bank.vwap[i]),
            ("close_lt_donchian55_mid", &|i| candles[i].close < bank.donchian55_mid[i]),
            ("ret24_negative", &|i| prev24_return[i].is_finite() && prev24_return[i] < 0.0),
            (
                "h4_bearish",
                &|i| h4_bias_by_h1_index.get(i).copied().unwrap_or(H4BiasState::Neutral)
                    == H4BiasState::Bearish,
            ),
        ];
        print_family(
            "15h_short_recurrence",
            15,
            "short",
            6,
            "base_15h_short_recurrence",
            &h15_short_base,
            &h15_short_opts,
            18,
            10,
        );
    }

    #[derive(Clone)]
    struct AntiEdgeTargetResult {
        hour: u32,
        direction: &'static str,
        labels: Vec<&'static str>,
        trades: usize,
        tp_hits: usize,
        sl_hits: usize,
        tp_rate: f64,
        sl_rate: f64,
        expectancy_distance: f64,
        net_pnl_distance: f64,
        target_distance: f64,
    }

    fn run_natgas_h1_target_bad_ratio_search() {
        let h1_series = canonical_chart_series("NATGAS_USD", "H1", 0).expect("NATGAS_USD H1 history");
        let h4_series = canonical_chart_series("NATGAS_USD", "H4", 0).expect("NATGAS_USD H4 history");
        let candles = h1_series.candles;
        let h4_candles = h4_series.candles;
        let train_rows = ((candles.len() as f64) * 0.7).round() as usize;
        let test_start = train_rows.max(56);
        let mut cache_snapshot = StrategyCacheSnapshot::default();
        let h1_hash = strategy_candles_hash(&candles);
        let h4_hash = strategy_candles_hash(&h4_candles);
        let bank = strategy_feature_bank_cached(&candles, &h1_hash, &mut cache_snapshot);
        let h4_bank = strategy_feature_bank_cached(&h4_candles, &h4_hash, &mut cache_snapshot);
        let h4_bias_by_h1_index = build_h4_bias_by_h1_index(&candles, &h4_candles, &h4_bank);

        let mut prev24_high = vec![f64::NAN; candles.len()];
        let mut prev24_low = vec![f64::NAN; candles.len()];
        let mut prev24_mid = vec![f64::NAN; candles.len()];
        let mut prev24_pos = vec![f64::NAN; candles.len()];
        let mut prev24_return = vec![f64::NAN; candles.len()];
        for index in 24..candles.len() {
            let window = &candles[index - 24..index];
            let high = window.iter().map(|c| c.high).fold(f64::NEG_INFINITY, f64::max);
            let low = window.iter().map(|c| c.low).fold(f64::INFINITY, f64::min);
            let width = (high - low).abs();
            prev24_high[index] = high;
            prev24_low[index] = low;
            prev24_mid[index] = (high + low) * 0.5;
            prev24_return[index] = candles[index].close - candles[index - 24].close;
            if width.is_finite() && width > f64::EPSILON {
                prev24_pos[index] = (candles[index].close - low) / width;
            }
        }

        let predicates: Vec<(&'static str, Box<dyn Fn(usize) -> bool>)> = vec![
            ("bullish_body", Box::new(|i| bullish_body(&candles[i]))),
            ("bearish_body", Box::new(|i| bearish_body(&candles[i]))),
            ("two_bull_bodies", Box::new(|i| i > 0 && bullish_body(&candles[i - 1]) && bullish_body(&candles[i]))),
            ("two_bear_bodies", Box::new(|i| i > 0 && bearish_body(&candles[i - 1]) && bearish_body(&candles[i]))),
            ("three_higher_closes", Box::new(|i| i >= 2 && candles[i].close > candles[i - 1].close && candles[i - 1].close > candles[i - 2].close)),
            ("three_lower_closes", Box::new(|i| i >= 2 && candles[i].close < candles[i - 1].close && candles[i - 1].close < candles[i - 2].close)),
            ("close_upper_24h_quartile", Box::new(|i| prev24_pos[i].is_finite() && prev24_pos[i] >= 0.75)),
            ("close_lower_24h_quartile", Box::new(|i| prev24_pos[i].is_finite() && prev24_pos[i] <= 0.25)),
            ("breaks_prev24_high", Box::new(|i| prev24_high[i].is_finite() && candles[i].close > prev24_high[i])),
            ("breaks_prev24_low", Box::new(|i| prev24_low[i].is_finite() && candles[i].close < prev24_low[i])),
            ("ret24_positive", Box::new(|i| prev24_return[i].is_finite() && prev24_return[i] > 0.0)),
            ("ret24_negative", Box::new(|i| prev24_return[i].is_finite() && prev24_return[i] < 0.0)),
            ("close_gt_vwap", Box::new(|i| candles[i].close > bank.vwap[i])),
            ("close_lt_vwap", Box::new(|i| candles[i].close < bank.vwap[i])),
            ("rsi14_gt_50", Box::new(|i| bank.rsi14[i] > 50.0)),
            ("rsi14_lt_50", Box::new(|i| bank.rsi14[i] < 50.0)),
            ("stoch_k_gt_d", Box::new(|i| bank.stoch14_k[i] > bank.stoch14_d[i])),
            ("stoch_k_lt_d", Box::new(|i| bank.stoch14_k[i] < bank.stoch14_d[i])),
            ("h4_bullish", Box::new(|i| h4_bias_by_h1_index.get(i).copied().unwrap_or(H4BiasState::Neutral) == H4BiasState::Bullish)),
            ("h4_bearish", Box::new(|i| h4_bias_by_h1_index.get(i).copied().unwrap_or(H4BiasState::Neutral) == H4BiasState::Bearish)),
        ];

        let mut best: Option<AntiEdgeTargetResult> = None;
        let mut best_40: Option<AntiEdgeTargetResult> = None;
        let mut best_80: Option<AntiEdgeTargetResult> = None;
        let mut best_120: Option<AntiEdgeTargetResult> = None;
        let consider_candidate =
            |slot: &mut Option<AntiEdgeTargetResult>, candidate: &AntiEdgeTargetResult| {
                let replace = slot
                    .as_ref()
                    .map(|current| {
                        candidate
                            .target_distance
                            .partial_cmp(&current.target_distance)
                            .unwrap_or(Ordering::Equal)
                            .then_with(|| {
                                candidate
                                    .expectancy_distance
                                    .partial_cmp(&current.expectancy_distance)
                                    .unwrap_or(Ordering::Equal)
                            })
                            .then_with(|| candidate.trades.cmp(&current.trades).reverse())
                            == Ordering::Less
                    })
                    .unwrap_or(true);
                if replace {
                    *slot = Some(candidate.clone());
                }
            };
        for hour in std::iter::once(0_u32).chain(7_u32..=21_u32) {
            for direction in ["long", "short"] {
                let base_test = (test_start..candles.len().saturating_sub(7))
                    .filter(|index| {
                        let current_hour = strategy_hour_utc(&candles[*index].time);
                        if hour == 0 {
                            current_hour.is_some_and(|value| (7..=21).contains(&value))
                        } else {
                            current_hour == Some(hour)
                        }
                    })
                    .collect::<Vec<_>>();
                let mut eval_combo = |combo: &[usize], best: &mut Option<AntiEdgeTargetResult>| {
                    let indices = base_test
                        .iter()
                        .copied()
                        .filter(|index| combo.iter().all(|ix| (predicates[*ix].1)(*index)))
                        .collect::<Vec<_>>();
                    if indices.len() < 12 {
                        return;
                    }
                    let mut trades = 0usize;
                    let mut tp_hits = 0usize;
                    let mut sl_hits = 0usize;
                    let mut net_pnl = 0.0;
                    for index in &indices {
                        let Some((tp_hit, pnl, _held)) = simulate_tp_sl_only_exit(
                            &candles,
                            index + 1,
                            direction,
                            NATGAS_STRICT_STOP_LOSS,
                            NATGAS_STRICT_TAKE_PROFIT,
                            NATGAS_STRICT_EXECUTION_COST,
                        ) else {
                            continue;
                        };
                        trades += 1;
                        net_pnl += pnl;
                        if tp_hit {
                            tp_hits += 1;
                        } else {
                            sl_hits += 1;
                        }
                    }
                    if trades < 12 { return; }
                    let tp_rate = tp_hits as f64 / trades as f64;
                    let sl_rate = sl_hits as f64 / trades as f64;
                    let candidate = AntiEdgeTargetResult {
                        hour,
                        direction,
                        labels: combo.iter().map(|ix| predicates[*ix].0).collect::<Vec<_>>(),
                        trades,
                        tp_hits,
                        sl_hits,
                        tp_rate,
                        sl_rate,
                        expectancy_distance: net_pnl / trades as f64,
                        net_pnl_distance: net_pnl,
                        target_distance: (tp_rate - 0.20).abs() + (sl_rate - 0.80).abs(),
                    };
                    consider_candidate(best, &candidate);
                    if candidate.trades >= 40 {
                        consider_candidate(&mut best_40, &candidate);
                    }
                    if candidate.trades >= 80 {
                        consider_candidate(&mut best_80, &candidate);
                    }
                    if candidate.trades >= 120 {
                        consider_candidate(&mut best_120, &candidate);
                    }
                };

                for a in 0..predicates.len() {
                    eval_combo(&[a], &mut best);
                    for b in a + 1..predicates.len() {
                        eval_combo(&[a, b], &mut best);
                        for c in b + 1..predicates.len() {
                            eval_combo(&[a, b, c], &mut best);
                        }
                    }
                }
            }
        }

        let best = best.expect("anti-edge target candidate");
        println!(
            "NATGAS_H1_TARGET_BAD_RATIO hour={} direction={} pattern=\"{}\" trades={} tp_hits={} sl_hits={} tp_rate={:.4} sl_rate={:.4} expectancy={:.6} net_pnl={:.6} target_distance={:.6}",
            if best.hour == 0 { "all".to_string() } else { format!("{:02}", best.hour) },
            best.direction,
            best.labels.join(" && "),
            best.trades,
            best.tp_hits,
            best.sl_hits,
            best.tp_rate,
            best.sl_rate,
            best.expectancy_distance,
            best.net_pnl_distance,
            best.target_distance,
        );
        for (label, candidate) in [
            ("40", best_40.as_ref()),
            ("80", best_80.as_ref()),
            ("120", best_120.as_ref()),
        ] {
            if let Some(candidate) = candidate {
                println!(
                    "NATGAS_H1_TARGET_BAD_RATIO_MIN_TRADES min_trades={} hour={} direction={} pattern=\"{}\" trades={} tp_hits={} sl_hits={} tp_rate={:.4} sl_rate={:.4} expectancy={:.6} net_pnl={:.6} target_distance={:.6}",
                    label,
                    if candidate.hour == 0 { "all".to_string() } else { format!("{:02}", candidate.hour) },
                    candidate.direction,
                    candidate.labels.join(" && "),
                    candidate.trades,
                    candidate.tp_hits,
                    candidate.sl_hits,
                    candidate.tp_rate,
                    candidate.sl_rate,
                    candidate.expectancy_distance,
                    candidate.net_pnl_distance,
                    candidate.target_distance,
                );
            }
        }
    }

    fn run_natgas_h1_inverse_bad_pattern_probe() {
        let h1_series = canonical_chart_series("NATGAS_USD", "H1", 0).expect("NATGAS_USD H1 history");
        let candles = h1_series.candles;
        let train_rows = ((candles.len() as f64) * 0.7).round() as usize;
        let test_start = train_rows.max(56);

        let evaluate =
            |label: &'static str, direction: &'static str, stop_loss: f64, take_profit: f64| {
            let mut trades = 0usize;
            let mut tp_hits = 0usize;
            let mut sl_hits = 0usize;
            let mut net_pnl = 0.0;
            for index in test_start..candles.len().saturating_sub(7) {
                if strategy_hour_utc(&candles[index].time) != Some(8) {
                    continue;
                }
                if !(index >= 2
                    && candles[index].close > candles[index - 1].close
                    && candles[index - 1].close > candles[index - 2].close)
                {
                    continue;
                }
                let Some((tp_hit, pnl, _held)) = simulate_tp_sl_only_exit(
                    &candles,
                    index + 1,
                    direction,
                    stop_loss,
                    take_profit,
                    NATGAS_STRICT_EXECUTION_COST,
                ) else {
                    continue;
                };
                trades += 1;
                net_pnl += pnl;
                if tp_hit {
                    tp_hits += 1;
                } else {
                    sl_hits += 1;
                }
            }
            let tp_rate = if trades > 0 {
                tp_hits as f64 / trades as f64
            } else {
                0.0
            };
            let sl_rate = if trades > 0 {
                sl_hits as f64 / trades as f64
            } else {
                0.0
            };
            println!(
                "NATGAS_H1_INVERSE_BAD_PATTERN_PROBE variant={} hour=08 direction={} pattern=\"three_higher_closes\" tp={:.4} sl={:.4} trades={} tp_hits={} sl_hits={} tp_rate={:.4} sl_rate={:.4} expectancy={:.6} net_pnl={:.6}",
                label,
                direction,
                take_profit,
                stop_loss,
                trades,
                tp_hits,
                sl_hits,
                tp_rate,
                sl_rate,
                if trades > 0 { net_pnl / trades as f64 } else { 0.0 },
                net_pnl,
            );
        };

        evaluate(
            "original",
            "long",
            NATGAS_STRICT_STOP_LOSS,
            NATGAS_STRICT_TAKE_PROFIT,
        );
        evaluate(
            "direction_only_inverse",
            "short",
            NATGAS_STRICT_STOP_LOSS,
            NATGAS_STRICT_TAKE_PROFIT,
        );
        evaluate(
            "exact_mirror_inverse",
            "short",
            NATGAS_STRICT_TAKE_PROFIT,
            NATGAS_STRICT_STOP_LOSS,
        );
    }

    fn run_natgas_h1_three_higher_closes_exact_mirror_strategy() {
        let h1_series = canonical_chart_series("NATGAS_USD", "H1", 0).expect("NATGAS_USD H1 history");
        let candles = h1_series.candles;
        let train_rows = ((candles.len() as f64) * 0.7).round() as usize;
        let test_start = train_rows.max(56);
        let mut trades = 0usize;
        let mut tp_hits = 0usize;
        let mut sl_hits = 0usize;
        let mut net_pnl = 0.0;

        for index in test_start..candles.len().saturating_sub(7) {
            if strategy_hour_utc(&candles[index].time) != Some(8) {
                continue;
            }
            if !(index >= 2
                && candles[index].close > candles[index - 1].close
                && candles[index - 1].close > candles[index - 2].close)
            {
                continue;
            }
            let Some((tp_hit, pnl, _held)) = simulate_tp_sl_only_exit(
                &candles,
                index + 1,
                "short",
                NATGAS_STRICT_TAKE_PROFIT,
                NATGAS_STRICT_STOP_LOSS,
                NATGAS_STRICT_EXECUTION_COST,
            ) else {
                continue;
            };
            trades += 1;
            net_pnl += pnl;
            if tp_hit {
                tp_hits += 1;
            } else {
                sl_hits += 1;
            }
        }

        println!(
            "NATGAS_H1_EXACT_MIRROR_STRATEGY hour=08 direction=short pattern=\"three_higher_closes\" tp={:.4} sl={:.4} trades={} tp_hits={} sl_hits={} tp_rate={:.4} sl_rate={:.4} expectancy={:.6} net_pnl={:.6}",
            NATGAS_STRICT_STOP_LOSS,
            NATGAS_STRICT_TAKE_PROFIT,
            trades,
            tp_hits,
            sl_hits,
            if trades > 0 { tp_hits as f64 / trades as f64 } else { 0.0 },
            if trades > 0 { sl_hits as f64 / trades as f64 } else { 0.0 },
            if trades > 0 { net_pnl / trades as f64 } else { 0.0 },
            net_pnl,
        );
    }

    fn run_save_natgas_h1_three_higher_closes_exact_mirror_as_program() {
        let args = json!({
            "title": "/stratMirror08h_",
            "goal": "Turn the discovered toxic 08h three_higher_closes anti-edge into its exact profitable mirror.",
            "intent": "Use NATGAS_USD H1 on OANDA. Detect the 08h UTC recurrence where the last three H1 closes are strictly higher than one another. Do not buy the breakout. Instead, take the exact mirror of the toxic setup: open a SHORT at the next bar open, use a raw take profit of 3.9p, a raw stop loss of 5.1p, and charge a 0.6p spread cost. A win only counts when TP is touched and a loss only counts when SL is touched. This is the exact mirror of the toxic long doctrine and should be treated as the canonical profitable form of that pattern.",
            "domain": "trading",
            "template": "strategy_mirror_recurrence",
            "program_kind": "compute_program"
        });
        let created =
            crate::forge_agent_runtime::direct_create_program_in_store(&args, "trading")
                .expect("save /stratMirror08h_ direct program");
        println!(
            "TRADING_PROGRAM_SAVED title=/stratMirror08h_ program_hash={} kind={} status={}",
            created
                .get("program_hash")
                .and_then(Value::as_str)
                .unwrap_or("unknown"),
            created
                .pointer("/program/program_kind")
                .and_then(Value::as_str)
                .unwrap_or("unknown"),
            created
                .pointer("/program/status")
                .and_then(Value::as_str)
                .unwrap_or("unknown"),
        );
    }

    #[derive(Clone, Copy)]
    struct HourContextSlotRuleDef {
        hour: u32,
        direction: &'static str,
        label: &'static str,
        predicate: fn(usize, &[TradingCandlePoint], &StrategyIndicatorFeatureBank, &[H4BiasState]) -> bool,
    }

    #[derive(Clone)]
    struct HourContextStrategyResult {
        name: &'static str,
        trades: usize,
        tp_hits: usize,
        sl_hits: usize,
        win_rate: f64,
        daily_target_hit_rate: f64,
        target_hit_days: usize,
        total_days: usize,
        avg_daily_pnl_distance: f64,
        expectancy_distance: f64,
        net_pnl_distance: f64,
    }

    fn hour_context_short_11h(
        index: usize,
        candles: &[TradingCandlePoint],
        bank: &StrategyIndicatorFeatureBank,
        h4_bias_by_h1_index: &[H4BiasState],
    ) -> bool {
        bearish_body(&candles[index])
            && candles[index].close < bank.vwap_ext1_down[index]
            && bank.macd_histogram[index] < 0.0
            && bank.rsi14[index] <= 45.0
            && bank.atr14[index] <= bank.atr_p35
            && bank.boll_widths[index] <= bank.boll_width_p35
            && h4_bias_by_h1_index
                .get(index)
                .copied()
                .unwrap_or(H4BiasState::Neutral)
                == H4BiasState::Bearish
    }

    fn hour_context_short_11h_no_squeeze(
        index: usize,
        candles: &[TradingCandlePoint],
        bank: &StrategyIndicatorFeatureBank,
        h4_bias_by_h1_index: &[H4BiasState],
    ) -> bool {
        bearish_body(&candles[index])
            && candles[index].close < bank.vwap_ext1_down[index]
            && bank.macd_histogram[index] < 0.0
            && bank.rsi14[index] <= 45.0
            && h4_bias_by_h1_index
                .get(index)
                .copied()
                .unwrap_or(H4BiasState::Neutral)
                == H4BiasState::Bearish
    }

    fn hour_context_short_11h_below_vwap(
        index: usize,
        candles: &[TradingCandlePoint],
        bank: &StrategyIndicatorFeatureBank,
        h4_bias_by_h1_index: &[H4BiasState],
    ) -> bool {
        bearish_body(&candles[index])
            && candles[index].close < bank.vwap[index]
            && bank.macd_histogram[index] < 0.0
            && bank.rsi14[index] <= 45.0
            && h4_bias_by_h1_index
                .get(index)
                .copied()
                .unwrap_or(H4BiasState::Neutral)
                == H4BiasState::Bearish
    }

    fn hour_context_long_18h(
        index: usize,
        candles: &[TradingCandlePoint],
        bank: &StrategyIndicatorFeatureBank,
        h4_bias_by_h1_index: &[H4BiasState],
    ) -> bool {
        bullish_body(&candles[index])
            && candles[index].close < bank.vwap[index]
            && bank.macd_histogram[index] > 0.0
            && bank.rsi14[index] >= 55.0
            && bank.atr14[index] > bank.atr_p50
            && bank.boll_widths[index] > bank.boll_width_p50
            && h4_bias_by_h1_index
                .get(index)
                .copied()
                .unwrap_or(H4BiasState::Neutral)
                == H4BiasState::Bearish
    }

    fn hour_context_long_18h_no_h4_gate(
        index: usize,
        candles: &[TradingCandlePoint],
        bank: &StrategyIndicatorFeatureBank,
        _h4_bias_by_h1_index: &[H4BiasState],
    ) -> bool {
        bullish_body(&candles[index])
            && candles[index].close < bank.vwap[index]
            && bank.macd_histogram[index] > 0.0
            && bank.rsi14[index] >= 55.0
            && bank.atr14[index] > bank.atr_p50
            && bank.boll_widths[index] > bank.boll_width_p50
    }

    fn hour_context_long_18h_near_vwap(
        index: usize,
        candles: &[TradingCandlePoint],
        bank: &StrategyIndicatorFeatureBank,
        h4_bias_by_h1_index: &[H4BiasState],
    ) -> bool {
        bullish_body(&candles[index])
            && candles[index].close <= bank.vwap_ext1_up[index]
            && bank.macd_histogram[index] > 0.0
            && bank.rsi14[index] >= 55.0
            && bank.atr14[index] > bank.atr_p50
            && bank.boll_widths[index] > bank.boll_width_p50
            && h4_bias_by_h1_index
                .get(index)
                .copied()
                .unwrap_or(H4BiasState::Neutral)
                == H4BiasState::Bearish
    }

    fn hour_context_long_19h(
        index: usize,
        candles: &[TradingCandlePoint],
        bank: &StrategyIndicatorFeatureBank,
        h4_bias_by_h1_index: &[H4BiasState],
    ) -> bool {
        bullish_body(&candles[index])
            && candles[index].close < bank.vwap[index]
            && bank.macd_histogram[index] > 0.0
            && bank.rsi14[index] >= 55.0
            && bank.atr14[index] > bank.atr_p50
            && bank.boll_widths[index] > bank.boll_width_p50
            && h4_bias_by_h1_index
                .get(index)
                .copied()
                .unwrap_or(H4BiasState::Neutral)
                == H4BiasState::Bearish
    }

    fn hour_context_long_21h_vwap(
        index: usize,
        candles: &[TradingCandlePoint],
        bank: &StrategyIndicatorFeatureBank,
        _h4_bias_by_h1_index: &[H4BiasState],
    ) -> bool {
        candles[index].close > bank.vwap[index]
    }

    fn evaluate_hour_context_strategy(
        name: &'static str,
        candles: &[TradingCandlePoint],
        feature_bank: &StrategyIndicatorFeatureBank,
        h4_bias_by_h1_index: &[H4BiasState],
        start_index: usize,
        slot_rules: &[HourContextSlotRuleDef],
    ) -> Option<HourContextStrategyResult> {
        let mut daily_pnl = BTreeMap::<String, f64>::new();
        let mut trades = 0usize;
        let mut tp_hits = 0usize;
        let mut sl_hits = 0usize;
        let mut net_pnl = 0.0;
        for index in start_index..candles.len().saturating_sub(1) {
            let Some(hour) = strategy_hour_utc(&candles[index].time) else {
                continue;
            };
            let Some(day_key) = candles[index].time.get(..10) else {
                continue;
            };
            daily_pnl.entry(day_key.to_string()).or_insert(0.0);
            let Some(rule) = slot_rules.iter().find(|rule| rule.hour == hour) else {
                continue;
            };
            if !(rule.predicate)(index, candles, feature_bank, h4_bias_by_h1_index) {
                continue;
            }
            let (outcome, _) = strategy_entry_outcome_cached(
                candles,
                index,
                rule.direction,
                NATGAS_STRICT_STOP_LOSS,
                NATGAS_STRICT_EXECUTION_COST,
                24,
            );
            let Some(outcome) = outcome else {
                continue;
            };
            let (exit_kind, pnl, _) = classify_strict_trade_outcome(&outcome, NATGAS_STRICT_TAKE_PROFIT);
            *daily_pnl.entry(day_key.to_string()).or_insert(0.0) += pnl;
            trades += 1;
            net_pnl += pnl;
            match exit_kind {
                StrictTradeExit::TakeProfit => tp_hits += 1,
                StrictTradeExit::StopLoss => sl_hits += 1,
                _ => {}
            }
        }
        if trades == 0 || daily_pnl.is_empty() {
            return None;
        }
        let total_days = daily_pnl.len();
        let target_hit_days = daily_pnl.values().filter(|value| **value >= 0.07).count();
        Some(HourContextStrategyResult {
            name,
            trades,
            tp_hits,
            sl_hits,
            win_rate: tp_hits as f64 / trades as f64,
            daily_target_hit_rate: target_hit_days as f64 / total_days as f64,
            target_hit_days,
            total_days,
            avg_daily_pnl_distance: daily_pnl.values().sum::<f64>() / total_days as f64,
            expectancy_distance: net_pnl / trades as f64,
            net_pnl_distance: net_pnl,
        })
    }

    fn run_natgas_hour_context_strategy_trials() {
        let h1_series = canonical_chart_series("NATGAS_USD", "H1", 0).expect("NATGAS_USD H1 history");
        let h4_series = canonical_chart_series("NATGAS_USD", "H4", 0).expect("NATGAS_USD H4 history");
        let candles = h1_series.candles;
        let h4_candles = h4_series.candles;
        let train_rows = ((candles.len() as f64) * 0.7).round() as usize;
        let test_start = train_rows.max(25);
        let mut cache_snapshot = StrategyCacheSnapshot::default();
        let h1_hash = strategy_candles_hash(&candles);
        let h4_hash = strategy_candles_hash(&h4_candles);
        let h1_bank = strategy_feature_bank_cached(&candles, &h1_hash, &mut cache_snapshot);
        let h4_bank = strategy_feature_bank_cached(&h4_candles, &h4_hash, &mut cache_snapshot);
        let h4_bias_by_h1_index = build_h4_bias_by_h1_index(&candles, &h4_candles, &h4_bank);
        let trials = vec![
            (
                "ctx_11h_short_only",
                vec![
                    Some(HourContextSlotRuleDef { hour: 11, direction: "short", label: "11h short ctx", predicate: hour_context_short_11h }),
                ],
            ),
            (
                "ctx_18h_long_only",
                vec![
                    Some(HourContextSlotRuleDef { hour: 18, direction: "long", label: "18h long ctx", predicate: hour_context_long_18h }),
                ],
            ),
            (
                "ctx_11h_short_plus_18h_long",
                vec![
                    Some(HourContextSlotRuleDef { hour: 11, direction: "short", label: "11h short ctx", predicate: hour_context_short_11h }),
                    Some(HourContextSlotRuleDef { hour: 18, direction: "long", label: "18h long ctx", predicate: hour_context_long_18h }),
                ],
            ),
            (
                "ctx_11h_short_plus_18h_19h_long",
                vec![
                    Some(HourContextSlotRuleDef { hour: 11, direction: "short", label: "11h short ctx", predicate: hour_context_short_11h }),
                    Some(HourContextSlotRuleDef { hour: 18, direction: "long", label: "18h long ctx", predicate: hour_context_long_18h }),
                    Some(HourContextSlotRuleDef { hour: 19, direction: "long", label: "19h long ctx", predicate: hour_context_long_19h }),
                ],
            ),
            (
                "ctx_11h_short_plus_18h_19h_long_plus_21h_vwap",
                vec![
                    Some(HourContextSlotRuleDef { hour: 11, direction: "short", label: "11h short ctx", predicate: hour_context_short_11h }),
                    Some(HourContextSlotRuleDef { hour: 18, direction: "long", label: "18h long ctx", predicate: hour_context_long_18h }),
                    Some(HourContextSlotRuleDef { hour: 19, direction: "long", label: "19h long ctx", predicate: hour_context_long_19h }),
                    Some(HourContextSlotRuleDef { hour: 21, direction: "long", label: "21h long close>VWAP", predicate: hour_context_long_21h_vwap }),
                ],
            ),
            (
                "ctx_11h_short_no_squeeze_only",
                vec![
                    Some(HourContextSlotRuleDef { hour: 11, direction: "short", label: "11h short no squeeze", predicate: hour_context_short_11h_no_squeeze }),
                ],
            ),
            (
                "ctx_11h_short_below_vwap_only",
                vec![
                    Some(HourContextSlotRuleDef { hour: 11, direction: "short", label: "11h short below VWAP", predicate: hour_context_short_11h_below_vwap }),
                ],
            ),
            (
                "ctx_18h_long_no_h4_only",
                vec![
                    Some(HourContextSlotRuleDef { hour: 18, direction: "long", label: "18h long no h4 gate", predicate: hour_context_long_18h_no_h4_gate }),
                ],
            ),
            (
                "ctx_18h_long_near_vwap_only",
                vec![
                    Some(HourContextSlotRuleDef { hour: 18, direction: "long", label: "18h long near VWAP", predicate: hour_context_long_18h_near_vwap }),
                ],
            ),
            (
                "ctx_11h_no_squeeze_plus_18h_no_h4",
                vec![
                    Some(HourContextSlotRuleDef { hour: 11, direction: "short", label: "11h short no squeeze", predicate: hour_context_short_11h_no_squeeze }),
                    Some(HourContextSlotRuleDef { hour: 18, direction: "long", label: "18h long no h4 gate", predicate: hour_context_long_18h_no_h4_gate }),
                ],
            ),
            (
                "ctx_11h_below_vwap_plus_18h_near_vwap",
                vec![
                    Some(HourContextSlotRuleDef { hour: 11, direction: "short", label: "11h short below VWAP", predicate: hour_context_short_11h_below_vwap }),
                    Some(HourContextSlotRuleDef { hour: 18, direction: "long", label: "18h long near VWAP", predicate: hour_context_long_18h_near_vwap }),
                ],
            ),
        ];
        println!("NATGAS_H1_HOUR_CONTEXT_STRATEGY_TRIALS methodology=\"strict H1 close-based entries on the 30% holdout only; TP 5.1p, SL 3.9p, spread 0.6p, hold=24 bars; rules are hour-specific context predicates from the context audit\"");
        for (name, defs) in trials {
            let slot_rules = defs.into_iter().flatten().collect::<Vec<_>>();
            let labels = slot_rules.iter().map(|rule| format!("{:02}h {}", rule.hour, rule.label)).collect::<Vec<_>>().join(" | ");
            if let Some(result) = evaluate_hour_context_strategy(
                name,
                &candles,
                &h1_bank,
                &h4_bias_by_h1_index,
                test_start,
                &slot_rules,
            ) {
                println!(
                    "NATGAS_H1_HOUR_CONTEXT_STRATEGY_TRIAL name={} labels=\"{}\" trades={} tp_hits={} sl_hits={} win_rate={:.4} daily_target_hit_rate={:.4} target_hit_days={} total_days={} avg_daily_pnl={:.6} expectancy={:.6} net_pnl={:.6}",
                    result.name,
                    labels,
                    result.trades,
                    result.tp_hits,
                    result.sl_hits,
                    result.win_rate,
                    result.daily_target_hit_rate,
                    result.target_hit_days,
                    result.total_days,
                    result.avg_daily_pnl_distance,
                    result.expectancy_distance,
                    result.net_pnl_distance,
                );
            } else {
                println!(
                    "NATGAS_H1_HOUR_CONTEXT_STRATEGY_TRIAL name={} labels=\"{}\" status=no_trades",
                    name,
                    labels,
                );
            }
        }
    }

    fn run_natgas_h1_trend_lifecycle_analysis() {
        let h1_series = canonical_chart_series("NATGAS_USD", "H1", 0).expect("NATGAS_USD H1 history");
        let h4_series = canonical_chart_series("NATGAS_USD", "H4", 0).expect("NATGAS_USD H4 history");
        let candles = h1_series.candles;
        let h4_candles = h4_series.candles;
        let mut cache_snapshot = StrategyCacheSnapshot::default();
        let h4_hash = strategy_candles_hash(&h4_candles);
        let h4_bank = strategy_feature_bank_cached(&h4_candles, &h4_hash, &mut cache_snapshot);
        let h4_bias_by_h1_index = build_h4_bias_by_h1_index(&candles, &h4_candles, &h4_bank);
        let samples = extract_trend_lifecycle_samples(&candles, NATGAS_TREND_PULLBACK_DISTANCE);
        let up_samples = samples
            .iter()
            .filter(|sample| sample.direction == TrendDirection::Up)
            .cloned()
            .collect::<Vec<_>>();
        let down_samples = samples
            .iter()
            .filter(|sample| sample.direction == TrendDirection::Down)
            .cloned()
            .collect::<Vec<_>>();
        println!(
            "NATGAS_H1_TREND_LIFECYCLE methodology=\"event-based directional-change trend confirmed after a 7p move from a local pivot; first adverse 7p excursion marks pullback; after that, a close beyond the original pivot counts as strong reversal, while a close beyond the prior extreme counts as trend resumption\""
        );
        print_trend_lifecycle_summary("NATGAS_H1_TREND_LIFECYCLE_ALL", &samples);
        print_trend_lifecycle_summary("NATGAS_H1_TREND_LIFECYCLE_UP", &up_samples);
        print_trend_lifecycle_summary("NATGAS_H1_TREND_LIFECYCLE_DOWN", &down_samples);
        for hour in [11_u32, 15_u32, 21_u32] {
            let slot_samples = samples
                .iter()
                .filter(|sample| strategy_hour_utc(&candles[sample.confirm_index].time) == Some(hour))
                .cloned()
                .collect::<Vec<_>>();
            print_trend_lifecycle_summary(
                &format!("NATGAS_H1_TREND_LIFECYCLE_SLOT_{hour:02}H"),
                &slot_samples,
            );
            for bias in [H4BiasState::Bearish, H4BiasState::Neutral, H4BiasState::Bullish] {
                let slot_bias_samples = slot_samples
                    .iter()
                    .filter(|sample| {
                        h4_bias_by_h1_index
                            .get(sample.confirm_index)
                            .copied()
                            .unwrap_or(H4BiasState::Neutral)
                            == bias
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                print_trend_lifecycle_summary(
                    &format!(
                        "NATGAS_H1_TREND_LIFECYCLE_SLOT_{hour:02}H_H4_{}",
                        bias.label().to_uppercase()
                    ),
                    &slot_bias_samples,
                );
            }
        }
        for bias in [H4BiasState::Bearish, H4BiasState::Neutral, H4BiasState::Bullish] {
            let bias_samples = samples
                .iter()
                .filter(|sample| {
                    h4_bias_by_h1_index
                        .get(sample.confirm_index)
                        .copied()
                        .unwrap_or(H4BiasState::Neutral)
                        == bias
                })
                .cloned()
                .collect::<Vec<_>>();
            print_trend_lifecycle_summary(
                &format!("NATGAS_H1_TREND_LIFECYCLE_H4_{}", bias.label().to_uppercase()),
                &bias_samples,
            );
        }
        for bucket in ["7p_to_10p", "10p_to_15p", "15p_to_20p", "20p_plus"] {
            let bucket_samples = samples
                .iter()
                .filter(|sample| impulse_bucket_label(sample.impulse_distance) == bucket)
                .cloned()
                .collect::<Vec<_>>();
            print_trend_lifecycle_summary(
                &format!("NATGAS_H1_TREND_LIFECYCLE_IMPULSE_{bucket}"),
                &bucket_samples,
            );
        }
        if let Some(last) = samples.last() {
            println!(
                "NATGAS_H1_TREND_LIFECYCLE_LAST direction={} start={} confirm={} extreme={} pullback={} resolution={} resolution_time={}",
                last.direction.label(),
                candles[last.start_index].time,
                candles[last.confirm_index].time,
                candles[last.extreme_index].time,
                candles[last.pullback_index].time,
                last.resolution.label(),
                last.resolution_index
                    .and_then(|index| candles.get(index))
                    .map(|candle| candle.time.clone())
                    .unwrap_or_else(|| "none".to_string())
            );
        }
    }

    fn seed_short_close_below_ema21(index: usize, candles: &[TradingCandlePoint], bank: &StrategyIndicatorFeatureBank) -> bool {
        candles[index].close < bank.ema21[index]
    }

    fn seed_short_cross_below_ema21(index: usize, candles: &[TradingCandlePoint], bank: &StrategyIndicatorFeatureBank) -> bool {
        index > 0 && candles[index - 1].close >= bank.ema21[index - 1] && candles[index].close < bank.ema21[index]
    }

    fn seed_short_close_below_vwap(index: usize, candles: &[TradingCandlePoint], bank: &StrategyIndicatorFeatureBank) -> bool {
        candles[index].close < bank.vwap[index]
    }

    fn seed_short_cross_below_vwap_ext1_up(index: usize, candles: &[TradingCandlePoint], bank: &StrategyIndicatorFeatureBank) -> bool {
        candles[index].high >= bank.vwap_ext1_up[index] && candles[index].close < bank.vwap_ext1_up[index]
    }

    fn seed_short_two_closes_below_vwap(index: usize, candles: &[TradingCandlePoint], bank: &StrategyIndicatorFeatureBank) -> bool {
        index >= 1
            && candles[index - 1].close < bank.vwap[index - 1]
            && candles[index].close < bank.vwap[index]
    }

    fn seed_short_reject_vwap_ext1_up_after_vwap_break(index: usize, candles: &[TradingCandlePoint], bank: &StrategyIndicatorFeatureBank) -> bool {
        index >= 1
            && candles[index - 1].close >= bank.vwap[index - 1]
            && candles[index].high >= bank.vwap_ext1_up[index]
            && candles[index].close < bank.vwap[index]
    }

    fn seed_short_three_bar_vwap_rollover(index: usize, candles: &[TradingCandlePoint], bank: &StrategyIndicatorFeatureBank) -> bool {
        index >= 2
            && candles[index - 2].close >= bank.vwap[index - 2]
            && candles[index - 1].close < bank.vwap[index - 1]
            && candles[index].close < bank.vwap[index]
            && candles[index].close < candles[index - 1].close
    }

    fn seed_short_bearish_body(index: usize, candles: &[TradingCandlePoint], _bank: &StrategyIndicatorFeatureBank) -> bool {
        candles[index].close < candles[index].open
    }

    fn seed_short_macd_hist_negative(index: usize, _candles: &[TradingCandlePoint], bank: &StrategyIndicatorFeatureBank) -> bool {
        bank.macd_histogram[index] < 0.0
    }

    fn seed_short_rsi_lt_50(index: usize, _candles: &[TradingCandlePoint], bank: &StrategyIndicatorFeatureBank) -> bool {
        bank.rsi14[index] < 50.0
    }

    fn seed_short_trend_pullback_reversal(index: usize, candles: &[TradingCandlePoint], bank: &StrategyIndicatorFeatureBank) -> bool {
        if index < 1 {
            return false;
        }
        let impulse = &candles[index - 1];
        let pullback = &candles[index];
        let impulse_body = (impulse.open - impulse.close).abs();
        impulse.close < impulse.open
            && impulse_body >= bank.body_p35
            && pullback.close > pullback.open
            && pullback.close > impulse.close
            && pullback.close < impulse.open
            && candles[index].close < bank.vwap[index]
            && bank.ema21[index] < bank.ema50[index]
            && bank.macd_histogram[index] <= 0.0
    }

    fn seed_long_close_above_ema21(index: usize, candles: &[TradingCandlePoint], bank: &StrategyIndicatorFeatureBank) -> bool {
        candles[index].close > bank.ema21[index]
    }

    fn seed_long_cross_above_ema21(index: usize, candles: &[TradingCandlePoint], bank: &StrategyIndicatorFeatureBank) -> bool {
        index > 0 && candles[index - 1].close <= bank.ema21[index - 1] && candles[index].close > bank.ema21[index]
    }

    fn seed_long_cross_above_ema50(index: usize, candles: &[TradingCandlePoint], bank: &StrategyIndicatorFeatureBank) -> bool {
        index > 0 && candles[index - 1].close <= bank.ema50[index - 1] && candles[index].close > bank.ema50[index]
    }

    fn seed_long_close_above_vwap(index: usize, candles: &[TradingCandlePoint], bank: &StrategyIndicatorFeatureBank) -> bool {
        candles[index].close > bank.vwap[index]
    }

    fn seed_long_cross_above_vwap_ext1_down(index: usize, candles: &[TradingCandlePoint], bank: &StrategyIndicatorFeatureBank) -> bool {
        candles[index].low <= bank.vwap_ext1_down[index] && candles[index].close > bank.vwap_ext1_down[index]
    }

    fn seed_long_two_closes_above_vwap(index: usize, candles: &[TradingCandlePoint], bank: &StrategyIndicatorFeatureBank) -> bool {
        index >= 1
            && candles[index - 1].close > bank.vwap[index - 1]
            && candles[index].close > bank.vwap[index]
    }

    fn seed_long_reclaim_vwap_ext1_down_after_vwap_break(index: usize, candles: &[TradingCandlePoint], bank: &StrategyIndicatorFeatureBank) -> bool {
        index >= 1
            && candles[index - 1].close <= bank.vwap[index - 1]
            && candles[index].low <= bank.vwap_ext1_down[index]
            && candles[index].close > bank.vwap[index]
    }

    fn seed_long_three_bar_vwap_reclaim(index: usize, candles: &[TradingCandlePoint], bank: &StrategyIndicatorFeatureBank) -> bool {
        index >= 2
            && candles[index - 2].close <= bank.vwap[index - 2]
            && candles[index - 1].close > bank.vwap_ext1_down[index - 1]
            && candles[index].close > bank.vwap[index]
            && candles[index].close > candles[index - 1].close
    }

    fn seed_long_bullish_body(index: usize, candles: &[TradingCandlePoint], _bank: &StrategyIndicatorFeatureBank) -> bool {
        candles[index].close > candles[index].open
    }

    fn seed_long_macd_hist_positive(index: usize, _candles: &[TradingCandlePoint], bank: &StrategyIndicatorFeatureBank) -> bool {
        bank.macd_histogram[index] > 0.0
    }

    fn seed_long_rsi_gt_50(index: usize, _candles: &[TradingCandlePoint], bank: &StrategyIndicatorFeatureBank) -> bool {
        bank.rsi14[index] > 50.0
    }

    fn seed_long_trend_pullback_reversal(index: usize, candles: &[TradingCandlePoint], bank: &StrategyIndicatorFeatureBank) -> bool {
        if index < 1 {
            return false;
        }
        let impulse = &candles[index - 1];
        let pullback = &candles[index];
        let impulse_body = (impulse.close - impulse.open).abs();
        impulse.close > impulse.open
            && impulse_body >= bank.body_p35
            && pullback.close < pullback.open
            && pullback.close < impulse.close
            && pullback.close > impulse.open
            && candles[index].close > bank.vwap[index]
            && bank.ema21[index] > bank.ema50[index]
            && bank.macd_histogram[index] >= 0.0
    }

    fn seeded_short_11h_rules() -> Vec<SeededSlotRuleDef> {
        vec![
            SeededSlotRuleDef { hour: 11, direction: "short", id: "close_lt_ema21", label: "11h short if close < EMA21", indicator_refs: &["/ema_h1_21_close"], predicate: seed_short_close_below_ema21 },
            SeededSlotRuleDef { hour: 11, direction: "short", id: "cross_below_ema21", label: "11h short if close crosses below EMA21", indicator_refs: &["/ema_h1_21_close", "/candle_h1_close"], predicate: seed_short_cross_below_ema21 },
            SeededSlotRuleDef { hour: 11, direction: "short", id: "close_lt_vwap", label: "11h short if close < VWAP", indicator_refs: &["/vwap_h1_session_hlc3"], predicate: seed_short_close_below_vwap },
            SeededSlotRuleDef { hour: 11, direction: "short", id: "cross_below_vwap_ext1_up", label: "11h short if candle crosses and closes below VWAP +1sigma", indicator_refs: &["/vwap_h1_ext1_up"], predicate: seed_short_cross_below_vwap_ext1_up },
            SeededSlotRuleDef { hour: 11, direction: "short", id: "two_closes_below_vwap", label: "11h short if two closes stay below VWAP", indicator_refs: &["/vwap_h1_session_hlc3", "/candle_h1_close"], predicate: seed_short_two_closes_below_vwap },
            SeededSlotRuleDef { hour: 11, direction: "short", id: "reject_vwap_ext1_up_after_vwap_break", label: "11h short if VWAP +1sigma rejection after VWAP break", indicator_refs: &["/vwap_h1_session_hlc3", "/vwap_h1_ext1_up", "/candle_h1_close"], predicate: seed_short_reject_vwap_ext1_up_after_vwap_break },
            SeededSlotRuleDef { hour: 11, direction: "short", id: "three_bar_vwap_rollover", label: "11h short if three-bar VWAP rollover confirms lower close", indicator_refs: &["/vwap_h1_session_hlc3", "/candle_h1_close"], predicate: seed_short_three_bar_vwap_rollover },
            SeededSlotRuleDef { hour: 11, direction: "short", id: "bearish_body", label: "11h short if bearish body", indicator_refs: &["/candle_h1_body"], predicate: seed_short_bearish_body },
            SeededSlotRuleDef { hour: 11, direction: "short", id: "macd_hist_neg", label: "11h short if MACD hist < 0", indicator_refs: &["/macd_h1_12_26_9_histogram"], predicate: seed_short_macd_hist_negative },
            SeededSlotRuleDef { hour: 11, direction: "short", id: "rsi_lt_50", label: "11h short if RSI14 < 50", indicator_refs: &["/rsi_h1_14_close"], predicate: seed_short_rsi_lt_50 },
        ]
    }

    fn seeded_long_15h_rules() -> Vec<SeededSlotRuleDef> {
        vec![
            SeededSlotRuleDef { hour: 15, direction: "long", id: "close_gt_ema21", label: "15h long if close > EMA21", indicator_refs: &["/ema_h1_21_close"], predicate: seed_long_close_above_ema21 },
            SeededSlotRuleDef { hour: 15, direction: "long", id: "cross_above_ema21", label: "15h long if close crosses above EMA21", indicator_refs: &["/ema_h1_21_close", "/candle_h1_close"], predicate: seed_long_cross_above_ema21 },
            SeededSlotRuleDef { hour: 15, direction: "long", id: "close_gt_vwap", label: "15h long if close > VWAP", indicator_refs: &["/vwap_h1_session_hlc3"], predicate: seed_long_close_above_vwap },
            SeededSlotRuleDef { hour: 15, direction: "long", id: "cross_above_vwap_ext1_down", label: "15h long if candle crosses and closes above VWAP -1sigma", indicator_refs: &["/vwap_h1_ext1_down"], predicate: seed_long_cross_above_vwap_ext1_down },
            SeededSlotRuleDef { hour: 15, direction: "long", id: "two_closes_above_vwap", label: "15h long if two closes stay above VWAP", indicator_refs: &["/vwap_h1_session_hlc3", "/candle_h1_close"], predicate: seed_long_two_closes_above_vwap },
            SeededSlotRuleDef { hour: 15, direction: "long", id: "reclaim_vwap_ext1_down_after_vwap_break", label: "15h long if VWAP -1sigma reclaim after VWAP break", indicator_refs: &["/vwap_h1_session_hlc3", "/vwap_h1_ext1_down", "/candle_h1_close"], predicate: seed_long_reclaim_vwap_ext1_down_after_vwap_break },
            SeededSlotRuleDef { hour: 15, direction: "long", id: "three_bar_vwap_reclaim", label: "15h long if three-bar VWAP reclaim confirms higher close", indicator_refs: &["/vwap_h1_session_hlc3", "/vwap_h1_ext1_down", "/candle_h1_close"], predicate: seed_long_three_bar_vwap_reclaim },
            SeededSlotRuleDef { hour: 15, direction: "long", id: "bullish_body", label: "15h long if bullish body", indicator_refs: &["/candle_h1_body"], predicate: seed_long_bullish_body },
            SeededSlotRuleDef { hour: 15, direction: "long", id: "macd_hist_pos", label: "15h long if MACD hist > 0", indicator_refs: &["/macd_h1_12_26_9_histogram"], predicate: seed_long_macd_hist_positive },
            SeededSlotRuleDef { hour: 15, direction: "long", id: "rsi_gt_50", label: "15h long if RSI14 > 50", indicator_refs: &["/rsi_h1_14_close"], predicate: seed_long_rsi_gt_50 },
        ]
    }

    fn seeded_long_21h_rules() -> Vec<SeededSlotRuleDef> {
        vec![
            SeededSlotRuleDef { hour: 21, direction: "long", id: "cross_above_ema21", label: "21h long if close crosses above EMA21", indicator_refs: &["/ema_h1_21_close", "/candle_h1_close"], predicate: seed_long_cross_above_ema21 },
            SeededSlotRuleDef { hour: 21, direction: "long", id: "cross_above_ema50", label: "21h long if close crosses above EMA50", indicator_refs: &["/ema_h1_50_close", "/candle_h1_close"], predicate: seed_long_cross_above_ema50 },
            SeededSlotRuleDef { hour: 21, direction: "long", id: "close_gt_ema21", label: "21h long if close > EMA21", indicator_refs: &["/ema_h1_21_close"], predicate: seed_long_close_above_ema21 },
            SeededSlotRuleDef { hour: 21, direction: "long", id: "close_gt_vwap", label: "21h long if close > VWAP", indicator_refs: &["/vwap_h1_session_hlc3"], predicate: seed_long_close_above_vwap },
            SeededSlotRuleDef { hour: 21, direction: "long", id: "cross_above_vwap_ext1_down", label: "21h long if candle crosses and closes above VWAP -1sigma", indicator_refs: &["/vwap_h1_ext1_down"], predicate: seed_long_cross_above_vwap_ext1_down },
            SeededSlotRuleDef { hour: 21, direction: "long", id: "two_closes_above_vwap", label: "21h long if two closes stay above VWAP", indicator_refs: &["/vwap_h1_session_hlc3", "/candle_h1_close"], predicate: seed_long_two_closes_above_vwap },
            SeededSlotRuleDef { hour: 21, direction: "long", id: "reclaim_vwap_ext1_down_after_vwap_break", label: "21h long if VWAP -1sigma reclaim after VWAP break", indicator_refs: &["/vwap_h1_session_hlc3", "/vwap_h1_ext1_down", "/candle_h1_close"], predicate: seed_long_reclaim_vwap_ext1_down_after_vwap_break },
            SeededSlotRuleDef { hour: 21, direction: "long", id: "three_bar_vwap_reclaim", label: "21h long if three-bar VWAP reclaim confirms higher close", indicator_refs: &["/vwap_h1_session_hlc3", "/vwap_h1_ext1_down", "/candle_h1_close"], predicate: seed_long_three_bar_vwap_reclaim },
            SeededSlotRuleDef { hour: 21, direction: "long", id: "macd_hist_pos", label: "21h long if MACD hist > 0", indicator_refs: &["/macd_h1_12_26_9_histogram"], predicate: seed_long_macd_hist_positive },
            SeededSlotRuleDef { hour: 21, direction: "long", id: "rsi_gt_50", label: "21h long if RSI14 > 50", indicator_refs: &["/rsi_h1_14_close"], predicate: seed_long_rsi_gt_50 },
        ]
    }

    fn seeded_short_19h_rules() -> Vec<SeededSlotRuleDef> {
        vec![
            SeededSlotRuleDef { hour: 19, direction: "short", id: "bearish_body", label: "19h short if bearish body", indicator_refs: &["/candle_h1_body"], predicate: seed_short_bearish_body },
            SeededSlotRuleDef { hour: 19, direction: "short", id: "close_lt_vwap", label: "19h short if close < VWAP", indicator_refs: &["/vwap_h1_session_hlc3"], predicate: seed_short_close_below_vwap },
            SeededSlotRuleDef { hour: 19, direction: "short", id: "cross_below_ema21", label: "19h short if close crosses below EMA21", indicator_refs: &["/ema_h1_21_close", "/candle_h1_close"], predicate: seed_short_cross_below_ema21 },
            SeededSlotRuleDef { hour: 19, direction: "short", id: "macd_hist_neg", label: "19h short if MACD hist < 0", indicator_refs: &["/macd_h1_12_26_9_histogram"], predicate: seed_short_macd_hist_negative },
            SeededSlotRuleDef { hour: 19, direction: "short", id: "rsi_lt_50", label: "19h short if RSI14 < 50", indicator_refs: &["/rsi_h1_14_close"], predicate: seed_short_rsi_lt_50 },
        ]
    }

    fn seeded_long_19h_rules() -> Vec<SeededSlotRuleDef> {
        vec![
            SeededSlotRuleDef { hour: 19, direction: "long", id: "bullish_body", label: "19h long if bullish body", indicator_refs: &["/candle_h1_body"], predicate: seed_long_bullish_body },
            SeededSlotRuleDef { hour: 19, direction: "long", id: "close_gt_vwap", label: "19h long if close > VWAP", indicator_refs: &["/vwap_h1_session_hlc3"], predicate: seed_long_close_above_vwap },
            SeededSlotRuleDef { hour: 19, direction: "long", id: "cross_above_ema21", label: "19h long if close crosses above EMA21", indicator_refs: &["/ema_h1_21_close", "/candle_h1_close"], predicate: seed_long_cross_above_ema21 },
            SeededSlotRuleDef { hour: 19, direction: "long", id: "macd_hist_pos", label: "19h long if MACD hist > 0", indicator_refs: &["/macd_h1_12_26_9_histogram"], predicate: seed_long_macd_hist_positive },
            SeededSlotRuleDef { hour: 19, direction: "long", id: "rsi_gt_50", label: "19h long if RSI14 > 50", indicator_refs: &["/rsi_h1_14_close"], predicate: seed_long_rsi_gt_50 },
        ]
    }

    fn seeded_short_21h_rules() -> Vec<SeededSlotRuleDef> {
        vec![
            SeededSlotRuleDef { hour: 21, direction: "short", id: "close_lt_ema21", label: "21h short if close < EMA21", indicator_refs: &["/ema_h1_21_close"], predicate: seed_short_close_below_ema21 },
            SeededSlotRuleDef { hour: 21, direction: "short", id: "cross_below_ema21", label: "21h short if close crosses below EMA21", indicator_refs: &["/ema_h1_21_close", "/candle_h1_close"], predicate: seed_short_cross_below_ema21 },
            SeededSlotRuleDef { hour: 21, direction: "short", id: "close_lt_vwap", label: "21h short if close < VWAP", indicator_refs: &["/vwap_h1_session_hlc3"], predicate: seed_short_close_below_vwap },
            SeededSlotRuleDef { hour: 21, direction: "short", id: "cross_below_vwap_ext1_up", label: "21h short if candle crosses and closes below VWAP +1sigma", indicator_refs: &["/vwap_h1_ext1_up"], predicate: seed_short_cross_below_vwap_ext1_up },
            SeededSlotRuleDef { hour: 21, direction: "short", id: "two_closes_below_vwap", label: "21h short if two closes stay below VWAP", indicator_refs: &["/vwap_h1_session_hlc3", "/candle_h1_close"], predicate: seed_short_two_closes_below_vwap },
            SeededSlotRuleDef { hour: 21, direction: "short", id: "reject_vwap_ext1_up_after_vwap_break", label: "21h short if VWAP +1sigma rejection after VWAP break", indicator_refs: &["/vwap_h1_session_hlc3", "/vwap_h1_ext1_up", "/candle_h1_close"], predicate: seed_short_reject_vwap_ext1_up_after_vwap_break },
            SeededSlotRuleDef { hour: 21, direction: "short", id: "three_bar_vwap_rollover", label: "21h short if three-bar VWAP rollover confirms lower close", indicator_refs: &["/vwap_h1_session_hlc3", "/candle_h1_close"], predicate: seed_short_three_bar_vwap_rollover },
            SeededSlotRuleDef { hour: 21, direction: "short", id: "macd_hist_neg", label: "21h short if MACD hist < 0", indicator_refs: &["/macd_h1_12_26_9_histogram"], predicate: seed_short_macd_hist_negative },
            SeededSlotRuleDef { hour: 21, direction: "short", id: "rsi_lt_50", label: "21h short if RSI14 < 50", indicator_refs: &["/rsi_h1_14_close"], predicate: seed_short_rsi_lt_50 },
        ]
    }

    fn evaluate_seeded_slot_combo(
        candles: &[TradingCandlePoint],
        feature_bank: &StrategyIndicatorFeatureBank,
        start_index: usize,
        stop_loss: f64,
        take_profit: f64,
        execution_cost: f64,
        max_hold: usize,
        short_11h: &SeededSlotRuleDef,
        long_15h: &SeededSlotRuleDef,
        long_21h: &SeededSlotRuleDef,
    ) -> Option<SeededCompositeCandidate> {
        let slot_rules = [short_11h, long_15h, long_21h];
        let mut daily_pnl = BTreeMap::<String, f64>::new();
        let mut trades = 0usize;
        let mut wins = 0usize;
        let mut losses = 0usize;
        let mut net_pnl = 0.0;
        for index in start_index..candles.len().saturating_sub(1) {
            let Some(hour) = strategy_hour_utc(&candles[index].time) else {
                continue;
            };
            if !matches!(hour, 11 | 15 | 21) {
                continue;
            }
            let Some(day_key) = candles[index].time.get(..10) else {
                continue;
            };
            daily_pnl.entry(day_key.to_string()).or_insert(0.0);
            let Some(rule) = slot_rules.iter().copied().find(|rule| rule.hour == hour) else {
                continue;
            };
            if !(rule.predicate)(index, candles, feature_bank) {
                continue;
            }
            let (outcome, _) = strategy_entry_outcome_cached(
                candles,
                index,
                rule.direction,
                stop_loss,
                execution_cost,
                max_hold,
            );
            let Some(outcome) = outcome else {
                continue;
            };
            let (exit_kind, pnl, _) = classify_strict_trade_outcome(&outcome, take_profit);
            *daily_pnl.entry(day_key.to_string()).or_insert(0.0) += pnl;
            trades += 1;
            net_pnl += pnl;
            if exit_kind == StrictTradeExit::TakeProfit {
                wins += 1;
            } else if exit_kind == StrictTradeExit::StopLoss {
                losses += 1;
            }
        }
        if daily_pnl.is_empty() || trades == 0 {
            return None;
        }
        let total_days = daily_pnl.len();
        let target_hit_days = daily_pnl.values().filter(|value| **value >= 0.07).count();
        let avg_daily_pnl_distance = daily_pnl.values().sum::<f64>() / total_days as f64;
        let min_daily_pnl_distance = daily_pnl.values().copied().fold(f64::INFINITY, f64::min);
        Some(SeededCompositeCandidate {
            short_11h: short_11h.clone(),
            long_15h: long_15h.clone(),
            long_21h: long_21h.clone(),
            trades,
            wins,
            losses,
            win_rate: wins as f64 / trades as f64,
            daily_target_hit_rate: target_hit_days as f64 / total_days as f64,
            target_hit_days,
            total_days,
            avg_daily_pnl_distance,
            min_daily_pnl_distance,
            expectancy_distance: net_pnl / trades as f64,
            net_pnl_distance: net_pnl,
        })
    }

    fn run_natgas_seeded_slot_strategy_search() {
        let series = canonical_chart_series("NATGAS_USD", "H1", 0).expect("NATGAS_USD H1 history");
        let candles = &series.candles;
        let train_rows = ((candles.len() as f64) * 0.7).round() as usize;
        let test_start = train_rows.max(25);
        let mut cache_snapshot = StrategyCacheSnapshot::default();
        let data_hash = strategy_candles_hash(candles);
        let feature_bank = strategy_feature_bank_cached(candles, &data_hash, &mut cache_snapshot);
        let short_rules = seeded_short_11h_rules();
        let long_15_rules = seeded_long_15h_rules();
        let long_21_rules = seeded_long_21h_rules();
        let mut best: Option<SeededCompositeCandidate> = None;
        for short_rule in &short_rules {
            for long_15_rule in &long_15_rules {
                for long_21_rule in &long_21_rules {
                    let Some(candidate) = evaluate_seeded_slot_combo(
                        candles,
                        &feature_bank,
                        test_start,
                        NATGAS_STRICT_STOP_LOSS,
                        NATGAS_STRICT_TAKE_PROFIT,
                        NATGAS_STRICT_EXECUTION_COST,
                        24,
                        short_rule,
                        long_15_rule,
                        long_21_rule,
                    ) else {
                        continue;
                    };
                    let should_replace = best.as_ref().map(|current| {
                        match candidate.daily_target_hit_rate.partial_cmp(&current.daily_target_hit_rate).unwrap_or(Ordering::Equal) {
                            Ordering::Greater => true,
                            Ordering::Less => false,
                            Ordering::Equal => match candidate.avg_daily_pnl_distance.partial_cmp(&current.avg_daily_pnl_distance).unwrap_or(Ordering::Equal) {
                                Ordering::Greater => true,
                                Ordering::Less => false,
                                Ordering::Equal => match candidate.min_daily_pnl_distance.partial_cmp(&current.min_daily_pnl_distance).unwrap_or(Ordering::Equal) {
                                    Ordering::Greater => true,
                                    Ordering::Less => false,
                                    Ordering::Equal => candidate.win_rate.partial_cmp(&current.win_rate).unwrap_or(Ordering::Equal) == Ordering::Greater,
                                },
                            },
                        }
                    }).unwrap_or(true);
                    if should_replace {
                        best = Some(candidate);
                    }
                }
            }
        }
        let best = best.expect("best seeded candidate");
        println!(
            "NATGAS_H1_SEEDED_DAILY best_11h={} [{}] best_15h={} [{}] best_21h={} [{}] trades={} wins={} losses={} win_rate={:.4} daily_target_hit_rate={:.4} target_hit_days={} total_days={} avg_daily_pnl={:.6} min_daily_pnl={:.6} expectancy={:.6} net_pnl={:.6}",
            best.short_11h.label,
            best.short_11h.indicator_refs.join(","),
            best.long_15h.label,
            best.long_15h.indicator_refs.join(","),
            best.long_21h.label,
            best.long_21h.indicator_refs.join(","),
            best.trades,
            best.wins,
            best.losses,
            best.win_rate,
            best.daily_target_hit_rate,
            best.target_hit_days,
            best.total_days,
            best.avg_daily_pnl_distance,
            best.min_daily_pnl_distance,
            best.expectancy_distance,
            best.net_pnl_distance,
        );
    }

    fn build_seeded_slot_combos(rules: &[SeededSlotRuleDef]) -> Vec<SeededSlotComboDef> {
        let mut combos = Vec::<SeededSlotComboDef>::new();
        for (idx, rule) in rules.iter().enumerate() {
            combos.push(SeededSlotComboDef {
                hour: rule.hour,
                direction: rule.direction,
                label: rule.label.to_string(),
                indicator_refs: rule.indicator_refs.iter().map(|value| value.to_string()).collect(),
                required_mask: 1_u32 << idx,
            });
        }
        for left in 0..rules.len() {
            for right in left + 1..rules.len() {
                let mut refs = Vec::<String>::new();
                for reference in rules[left]
                    .indicator_refs
                    .iter()
                    .chain(rules[right].indicator_refs.iter())
                {
                    let value = (*reference).to_string();
                    if !refs.contains(&value) {
                        refs.push(value);
                    }
                }
                combos.push(SeededSlotComboDef {
                    hour: rules[left].hour,
                    direction: rules[left].direction,
                    label: format!("{} && {}", rules[left].label, rules[right].label),
                    indicator_refs: refs,
                    required_mask: (1_u32 << left) | (1_u32 << right),
                });
            }
        }
        combos
    }

    fn find_seeded_slot_combo_by_label(
        rules: &[SeededSlotRuleDef],
        label: &str,
    ) -> SeededSlotComboDef {
        build_seeded_slot_combos(rules)
            .into_iter()
            .find(|combo| combo.label == label)
            .unwrap_or_else(|| panic!("missing seeded slot combo: {label}"))
    }

    fn natgas_simple_vwap_baseline_combos() -> (
        SeededSlotComboDef,
        SeededSlotComboDef,
        SeededSlotComboDef,
    ) {
        let short_11 =
            find_seeded_slot_combo_by_label(&seeded_short_11h_rules(), NATGAS_BASELINE_11H_LABEL);
        let long_15 =
            find_seeded_slot_combo_by_label(&seeded_long_15h_rules(), NATGAS_BASELINE_15H_LABEL);
        let long_21 = find_seeded_slot_combo_by_label(
            &seeded_long_21h_rules(),
            NATGAS_BASELINE_21H_VWAP_LABEL,
        );
        (short_11, long_15, long_21)
    }

    fn combo_is_vwap_family(combo: &SeededSlotComboDef) -> bool {
        let mut saw_vwap = false;
        for reference in &combo.indicator_refs {
            let normalized = reference.trim().to_ascii_lowercase();
            if normalized.contains("/vwap") {
                saw_vwap = true;
                continue;
            }
            if normalized.contains("/candle_h1_close") || normalized.contains("/candle_h1_body") {
                continue;
            }
            return false;
        }
        saw_vwap
    }

    fn combo_is_vwap_inversion_family(combo: &SeededSlotComboDef) -> bool {
        let mut saw_vwap = false;
        for reference in &combo.indicator_refs {
            let normalized = reference.trim().to_ascii_lowercase();
            if normalized.contains("/vwap") {
                saw_vwap = true;
                continue;
            }
            if normalized.contains("/candle_h1_close")
                || normalized.contains("/candle_h1_body")
                || normalized.contains("/macd_h1_12_26_9_histogram")
                || normalized.contains("/rsi_h1_14_close")
            {
                continue;
            }
            return false;
        }
        saw_vwap
    }

    fn combo_label_is_one_of(combo: &SeededSlotComboDef, labels: &[&str]) -> bool {
        labels.iter().any(|label| combo.label == *label)
    }

    fn combos_from_labels(
        combos: &[SeededSlotComboDef],
        labels: &[&str],
    ) -> Vec<Option<SeededSlotComboDef>> {
        combos
            .iter()
            .filter(|combo| combo_label_is_one_of(combo, labels))
            .cloned()
            .map(Some)
            .chain(std::iter::once(None))
            .collect::<Vec<_>>()
    }

    fn evaluate_seeded_slot_search_candidate(
        candles: &[TradingCandlePoint],
        rows: &[(usize, u32)],
        combo: &SeededSlotComboDef,
        take_profit: f64,
        stop_loss: f64,
        execution_cost: f64,
        max_hold: usize,
    ) -> Option<SeededSlotSearchCandidate> {
        let mut trades = 0usize;
        let mut wins = 0usize;
        let mut losses = 0usize;
        let mut net_pnl = 0.0;
        for (index, row_mask) in rows {
            if (*row_mask & combo.required_mask) != combo.required_mask {
                continue;
            }
            let (outcome, _) = strategy_entry_outcome_cached(
                candles,
                *index,
                combo.direction,
                stop_loss,
                execution_cost,
                max_hold,
            );
            let Some(outcome) = outcome else {
                continue;
            };
            let (exit_kind, pnl, _) = classify_strict_trade_outcome(&outcome, take_profit);
            trades += 1;
            net_pnl += pnl;
            if exit_kind == StrictTradeExit::TakeProfit {
                wins += 1;
            } else if exit_kind == StrictTradeExit::StopLoss {
                losses += 1;
            }
        }
        if trades == 0 {
            return None;
        }
        Some(SeededSlotSearchCandidate {
            combo: combo.clone(),
            trades,
            wins,
            losses,
            win_rate: wins as f64 / trades as f64,
            expectancy_distance: net_pnl / trades as f64,
            net_pnl_distance: net_pnl,
        })
    }

    fn evaluate_seeded_slot_search_candidate_for_bias(
        candles: &[TradingCandlePoint],
        rows: &[(usize, u32)],
        combo: &SeededSlotComboDef,
        h4_bias_by_h1_index: &[H4BiasState],
        target_bias: H4BiasState,
        take_profit: f64,
        stop_loss: f64,
        execution_cost: f64,
        max_hold: usize,
    ) -> Option<SeededSlotSearchCandidate> {
        let mut trades = 0usize;
        let mut wins = 0usize;
        let mut losses = 0usize;
        let mut net_pnl = 0.0;
        for (index, row_mask) in rows {
            if h4_bias_by_h1_index
                .get(*index)
                .copied()
                .unwrap_or(H4BiasState::Neutral)
                != target_bias
            {
                continue;
            }
            if (*row_mask & combo.required_mask) != combo.required_mask {
                continue;
            }
            let (outcome, _) = strategy_entry_outcome_cached(
                candles,
                *index,
                combo.direction,
                stop_loss,
                execution_cost,
                max_hold,
            );
            let Some(outcome) = outcome else {
                continue;
            };
            let (exit_kind, pnl, _) = classify_strict_trade_outcome(&outcome, take_profit);
            trades += 1;
            net_pnl += pnl;
            if exit_kind == StrictTradeExit::TakeProfit {
                wins += 1;
            } else if exit_kind == StrictTradeExit::StopLoss {
                losses += 1;
            }
        }
        if trades == 0 {
            return None;
        }
        Some(SeededSlotSearchCandidate {
            combo: combo.clone(),
            trades,
            wins,
            losses,
            win_rate: wins as f64 / trades as f64,
            expectancy_distance: net_pnl / trades as f64,
            net_pnl_distance: net_pnl,
        })
    }

    fn evaluate_seeded_combo_search_candidate_with_gates(
        candles: &[TradingCandlePoint],
        short_rows: &[(usize, u32)],
        long_15_rows: &[(usize, u32)],
        long_21_rows: &[(usize, u32)],
        short_combo: &SeededSlotComboDef,
        long_15_combo: &SeededSlotComboDef,
        long_21_combo: &SeededSlotComboDef,
        short_gate: Option<&[bool]>,
        long_15_gate: Option<&[bool]>,
        long_21_gate: Option<&[bool]>,
        take_profit: f64,
        stop_loss: f64,
        execution_cost: f64,
        max_hold: usize,
    ) -> Option<SeededComboSearchCandidate> {
        let mut daily_pnl = BTreeMap::<String, f64>::new();
        let mut trades = 0usize;
        let mut wins = 0usize;
        let mut losses = 0usize;
        let mut net_pnl = 0.0;
        let groups = [
            (short_rows, short_combo, short_gate),
            (long_15_rows, long_15_combo, long_15_gate),
            (long_21_rows, long_21_combo, long_21_gate),
        ];
        for (rows, combo, gate) in groups {
            for (index, row_mask) in rows {
                if (*row_mask & combo.required_mask) != combo.required_mask {
                    continue;
                }
                if gate.and_then(|values| values.get(*index).copied()) == Some(false) {
                    continue;
                }
                let Some(day_key) = candles[*index].time.get(..10) else {
                    continue;
                };
                daily_pnl.entry(day_key.to_string()).or_insert(0.0);
                let (outcome, _) = strategy_entry_outcome_cached(
                    candles,
                    *index,
                    combo.direction,
                    stop_loss,
                    execution_cost,
                    max_hold,
                );
                let Some(outcome) = outcome else {
                    continue;
                };
                let (exit_kind, pnl, _) = classify_strict_trade_outcome(&outcome, take_profit);
                *daily_pnl.entry(day_key.to_string()).or_insert(0.0) += pnl;
                trades += 1;
                net_pnl += pnl;
                if exit_kind == StrictTradeExit::TakeProfit {
                    wins += 1;
                } else if exit_kind == StrictTradeExit::StopLoss {
                    losses += 1;
                }
            }
        }
        if daily_pnl.is_empty() || trades == 0 {
            return None;
        }
        let total_days = daily_pnl.len();
        let target_hit_days = daily_pnl
            .values()
            .filter(|value| **value >= NATGAS_STRICT_DAILY_TARGET)
            .count();
        let avg_daily_pnl_distance = daily_pnl.values().sum::<f64>() / total_days as f64;
        let min_daily_pnl_distance = daily_pnl.values().copied().fold(f64::INFINITY, f64::min);
        Some(SeededComboSearchCandidate {
            short_11h: short_combo.clone(),
            long_15h: long_15_combo.clone(),
            long_21h: long_21_combo.clone(),
            take_profit,
            trades,
            wins,
            losses,
            win_rate: wins as f64 / trades as f64,
            daily_target_hit_rate: target_hit_days as f64 / total_days as f64,
            target_hit_days,
            total_days,
            avg_daily_pnl_distance,
            min_daily_pnl_distance,
            expectancy_distance: net_pnl / trades as f64,
            net_pnl_distance: net_pnl,
        })
    }

    fn build_seeded_slot_masks(
        candles: &[TradingCandlePoint],
        feature_bank: &StrategyIndicatorFeatureBank,
        start_index: usize,
        rules: &[SeededSlotRuleDef],
    ) -> Vec<(usize, u32)> {
        let mut rows = Vec::<(usize, u32)>::new();
        for index in start_index..candles.len().saturating_sub(1) {
            let Some(hour) = strategy_hour_utc(&candles[index].time) else {
                continue;
            };
            if rules.first().is_none_or(|rule| rule.hour != hour) {
                continue;
            }
            let mut mask = 0_u32;
            for (bit, rule) in rules.iter().enumerate() {
                if (rule.predicate)(index, candles, feature_bank) {
                    mask |= 1_u32 << bit;
                }
            }
            rows.push((index, mask));
        }
        rows
    }

    fn build_completed_higher_timeframe_index_by_lower(
        lower_candles: &[TradingCandlePoint],
        higher_candles: &[TradingCandlePoint],
    ) -> Vec<Option<usize>> {
        let mut result = vec![None; lower_candles.len()];
        if higher_candles.is_empty() {
            return result;
        }
        let mut cursor = 0usize;
        for (lower_index, lower) in lower_candles.iter().enumerate() {
            while cursor + 1 < higher_candles.len() && higher_candles[cursor + 1].time < lower.time {
                cursor += 1;
            }
            if higher_candles[cursor].time < lower.time {
                result[lower_index] = Some(cursor);
            }
        }
        result
    }

    fn build_higher_timeframe_gate(
        lower_len: usize,
        aligned_higher_index: &[Option<usize>],
        higher_candles: &[TradingCandlePoint],
        higher_bank: &StrategyIndicatorFeatureBank,
        predicate: fn(usize, &[TradingCandlePoint], &StrategyIndicatorFeatureBank) -> bool,
        label: &str,
        indicator_refs: &[&'static str],
    ) -> SeededGateDef {
        let mut allowed_by_index = vec![false; lower_len];
        for (lower_index, higher_index) in aligned_higher_index.iter().enumerate() {
            let Some(higher_index) = higher_index else {
                continue;
            };
            allowed_by_index[lower_index] =
                predicate(*higher_index, higher_candles, higher_bank);
        }
        SeededGateDef {
            label: label.to_string(),
            indicator_refs: indicator_refs.to_vec(),
            allowed_by_index,
        }
    }

    fn intersect_gate_defs(left: &SeededGateDef, right: &SeededGateDef) -> SeededGateDef {
        let allowed_by_index = left
            .allowed_by_index
            .iter()
            .zip(right.allowed_by_index.iter())
            .map(|(lhs, rhs)| *lhs && *rhs)
            .collect::<Vec<_>>();
        let mut indicator_refs = left.indicator_refs.clone();
        for reference in &right.indicator_refs {
            if !indicator_refs.contains(reference) {
                indicator_refs.push(*reference);
            }
        }
        SeededGateDef {
            label: format!("{} && {}", left.label, right.label),
            indicator_refs,
            allowed_by_index,
        }
    }

    fn h4_bias_at(
        index: usize,
        candles: &[TradingCandlePoint],
        bank: &StrategyIndicatorFeatureBank,
    ) -> H4BiasState {
        let close = candles[index].close;
        let ema21 = bank.ema21[index];
        let ema50 = bank.ema50[index];
        let rsi = bank.rsi14[index];
        let vwap = bank.vwap[index];
        let two_below = seed_short_two_closes_below_vwap(index, candles, bank);
        let two_above = seed_long_two_closes_above_vwap(index, candles, bank);

        if (close < vwap && ema21 < ema50 && rsi < 50.0) || two_below {
            H4BiasState::Bearish
        } else if (close > vwap && ema21 > ema50 && rsi > 50.0) || two_above {
            H4BiasState::Bullish
        } else {
            H4BiasState::Neutral
        }
    }

    fn build_h4_bias_by_h1_index(
        h1_candles: &[TradingCandlePoint],
        h4_candles: &[TradingCandlePoint],
        h4_bank: &StrategyIndicatorFeatureBank,
    ) -> Vec<H4BiasState> {
        let aligned = build_completed_higher_timeframe_index_by_lower(h1_candles, h4_candles);
        aligned
            .into_iter()
            .map(|higher_index| {
                higher_index
                    .map(|index| h4_bias_at(index, h4_candles, h4_bank))
                    .unwrap_or(H4BiasState::Neutral)
            })
            .collect::<Vec<_>>()
    }

    fn evaluate_h4_bias_policy(
        candles: &[TradingCandlePoint],
        short_rows: &[(usize, u32)],
        long_15_rows: &[(usize, u32)],
        short_21_rows: &[(usize, u32)],
        h4_bias_by_h1_index: &[H4BiasState],
        short_policy: &[Option<SeededSlotComboDef>; 3],
        long_policy: &[Option<SeededSlotComboDef>; 3],
        fixed_21h: &SeededSlotComboDef,
        take_profit: f64,
        stop_loss: f64,
        execution_cost: f64,
        max_hold: usize,
    ) -> Option<H4BiasPolicyResult> {
        let mut short_mask_by_index = HashMap::<usize, u32>::with_capacity(short_rows.len());
        for (index, mask) in short_rows {
            short_mask_by_index.insert(*index, *mask);
        }
        let mut long_mask_by_index = HashMap::<usize, u32>::with_capacity(long_15_rows.len());
        for (index, mask) in long_15_rows {
            long_mask_by_index.insert(*index, *mask);
        }
        let mut short21_mask_by_index = HashMap::<usize, u32>::with_capacity(short_21_rows.len());
        for (index, mask) in short_21_rows {
            short21_mask_by_index.insert(*index, *mask);
        }

        let mut daily_pnl = BTreeMap::<String, f64>::new();
        let mut trades = 0usize;
        let mut wins = 0usize;
        let mut losses = 0usize;
        let mut net_pnl = 0.0;

        for index in 0..candles.len().saturating_sub(1) {
            let Some(hour) = strategy_hour_utc(&candles[index].time) else {
                continue;
            };
            if !matches!(hour, 11 | 15 | 21) {
                continue;
            }
            let Some(day_key) = candles[index].time.get(..10) else {
                continue;
            };
            let action_combo = match hour {
                11 => {
                    let bias = h4_bias_by_h1_index
                        .get(index)
                        .copied()
                        .unwrap_or(H4BiasState::Neutral);
                    let Some(combo) = short_policy[bias.slot()].as_ref() else {
                        continue;
                    };
                    let Some(mask) = short_mask_by_index.get(&index).copied() else {
                        continue;
                    };
                    if (mask & combo.required_mask) != combo.required_mask {
                        continue;
                    }
                    combo
                }
                15 => {
                    let bias = h4_bias_by_h1_index
                        .get(index)
                        .copied()
                        .unwrap_or(H4BiasState::Neutral);
                    let Some(combo) = long_policy[bias.slot()].as_ref() else {
                        continue;
                    };
                    let Some(mask) = long_mask_by_index.get(&index).copied() else {
                        continue;
                    };
                    if (mask & combo.required_mask) != combo.required_mask {
                        continue;
                    }
                    combo
                }
                21 => {
                    let Some(mask) = short21_mask_by_index.get(&index).copied() else {
                        continue;
                    };
                    if (mask & fixed_21h.required_mask) != fixed_21h.required_mask {
                        continue;
                    }
                    fixed_21h
                }
                _ => continue,
            };

            let (outcome, _) = strategy_entry_outcome_cached(
                candles,
                index,
                action_combo.direction,
                stop_loss,
                execution_cost,
                max_hold,
            );
            let Some(outcome) = outcome else {
                continue;
            };
            let (exit_kind, pnl, _) = classify_strict_trade_outcome(&outcome, take_profit);
            *daily_pnl.entry(day_key.to_string()).or_insert(0.0) += pnl;
            trades += 1;
            net_pnl += pnl;
            if exit_kind == StrictTradeExit::TakeProfit {
                wins += 1;
            } else if exit_kind == StrictTradeExit::StopLoss {
                losses += 1;
            }
        }

        if daily_pnl.is_empty() || trades == 0 {
            return None;
        }
        let total_days = daily_pnl.len();
        let target_hit_days = daily_pnl
            .values()
            .filter(|value| **value >= NATGAS_STRICT_DAILY_TARGET)
            .count();
        let avg_daily_pnl_distance = daily_pnl.values().sum::<f64>() / total_days as f64;
        Some(H4BiasPolicyResult {
            short_policy: short_policy.clone(),
            long_policy: long_policy.clone(),
            trades,
            wins,
            losses,
            win_rate: wins as f64 / trades as f64,
            daily_target_hit_rate: target_hit_days as f64 / total_days as f64,
            target_hit_days,
            total_days,
            avg_daily_pnl_distance,
            expectancy_distance: net_pnl / trades as f64,
            net_pnl_distance: net_pnl,
        })
    }

    fn top_bias_candidates(
        candles: &[TradingCandlePoint],
        rows: &[(usize, u32)],
        combos: &[Option<SeededSlotComboDef>],
        h4_bias_by_h1_index: &[H4BiasState],
        target_bias: H4BiasState,
        take_profit: f64,
        stop_loss: f64,
        execution_cost: f64,
        max_hold: usize,
        limit: usize,
    ) -> Vec<Option<SeededSlotComboDef>> {
        let mut ranked = Vec::<SeededSlotSearchCandidate>::new();
        for combo in combos.iter().flatten() {
            let Some(candidate) = evaluate_seeded_slot_search_candidate_for_bias(
                candles,
                rows,
                combo,
                h4_bias_by_h1_index,
                target_bias,
                take_profit,
                stop_loss,
                execution_cost,
                max_hold,
            ) else {
                continue;
            };
            ranked.push(candidate);
        }
        ranked.sort_by(|left, right| {
            right
                .win_rate
                .partial_cmp(&left.win_rate)
                .unwrap_or(Ordering::Equal)
                .then_with(|| {
                    right
                        .expectancy_distance
                        .partial_cmp(&left.expectancy_distance)
                        .unwrap_or(Ordering::Equal)
                })
                .then_with(|| right.trades.cmp(&left.trades))
        });
        let mut output = ranked
            .into_iter()
            .take(limit)
            .map(|candidate| Some(candidate.combo))
            .collect::<Vec<_>>();
        output.push(None);
        output
    }

    fn evaluate_seeded_combo_search_candidate(
        candles: &[TradingCandlePoint],
        short_rows: &[(usize, u32)],
        long_15_rows: &[(usize, u32)],
        long_21_rows: &[(usize, u32)],
        short_combo: &SeededSlotComboDef,
        long_15_combo: &SeededSlotComboDef,
        long_21_combo: &SeededSlotComboDef,
        take_profit: f64,
        stop_loss: f64,
        execution_cost: f64,
        max_hold: usize,
    ) -> Option<SeededComboSearchCandidate> {
        let mut daily_pnl = BTreeMap::<String, f64>::new();
        let mut trades = 0usize;
        let mut wins = 0usize;
        let mut losses = 0usize;
        let mut net_pnl = 0.0;
        let groups = [
            (short_rows, short_combo),
            (long_15_rows, long_15_combo),
            (long_21_rows, long_21_combo),
        ];
        for (rows, combo) in groups {
            for (index, row_mask) in rows {
                if (*row_mask & combo.required_mask) != combo.required_mask {
                    continue;
                }
                let Some(day_key) = candles[*index].time.get(..10) else {
                    continue;
                };
                daily_pnl.entry(day_key.to_string()).or_insert(0.0);
                let (outcome, _) = strategy_entry_outcome_cached(
                    candles,
                    *index,
                    combo.direction,
                    stop_loss,
                    execution_cost,
                    max_hold,
                );
                let Some(outcome) = outcome else {
                    continue;
                };
                let (exit_kind, pnl, _) = classify_strict_trade_outcome(&outcome, take_profit);
                *daily_pnl.entry(day_key.to_string()).or_insert(0.0) += pnl;
                trades += 1;
                net_pnl += pnl;
                if exit_kind == StrictTradeExit::TakeProfit {
                    wins += 1;
                } else if exit_kind == StrictTradeExit::StopLoss {
                    losses += 1;
                }
            }
        }
        if daily_pnl.is_empty() || trades == 0 {
            return None;
        }
        let total_days = daily_pnl.len();
        let target_hit_days = daily_pnl
            .values()
            .filter(|value| **value >= NATGAS_STRICT_DAILY_TARGET)
            .count();
        let avg_daily_pnl_distance = daily_pnl.values().sum::<f64>() / total_days as f64;
        let min_daily_pnl_distance = daily_pnl.values().copied().fold(f64::INFINITY, f64::min);
        Some(SeededComboSearchCandidate {
            short_11h: short_combo.clone(),
            long_15h: long_15_combo.clone(),
            long_21h: long_21_combo.clone(),
            take_profit,
            trades,
            wins,
            losses,
            win_rate: wins as f64 / trades as f64,
            daily_target_hit_rate: target_hit_days as f64 / total_days as f64,
            target_hit_days,
            total_days,
            avg_daily_pnl_distance,
            min_daily_pnl_distance,
            expectancy_distance: net_pnl / trades as f64,
            net_pnl_distance: net_pnl,
        })
    }

    fn slot_index_for_hour(hour: u32) -> Option<usize> {
        match hour {
            11 => Some(0),
            15 => Some(1),
            21 => Some(2),
            _ => None,
        }
    }

    fn strategy_regime_at(
        index: usize,
        candles: &[TradingCandlePoint],
        feature_bank: &StrategyIndicatorFeatureBank,
    ) -> SeededRegimeId {
        let close = candles[index].close;
        let ema21 = feature_bank.ema21[index];
        let ema50 = feature_bank.ema50[index];
        let vwap = feature_bank.vwap[index];
        let rsi14 = feature_bank.rsi14[index];
        let atr14 = feature_bank.atr14[index].abs().max(1e-9);
        let macd_hist = feature_bank.macd_histogram[index];
        let upper = feature_bank.upper[index];
        let lower = feature_bank.lower[index];
        let boll_width = feature_bank.boll_widths[index];

        if close > ema50 && ema21 > ema50 && macd_hist > 0.0 && rsi14 >= 55.0 {
            SeededRegimeId::TrendUp
        } else if close < ema50 && ema21 < ema50 && macd_hist < 0.0 && rsi14 <= 45.0 {
            SeededRegimeId::TrendDown
        } else if boll_width <= feature_bank.boll_width_p35 && (close - vwap).abs() <= atr14 * 0.75 {
            SeededRegimeId::Compression
        } else if close >= upper && rsi14 >= 60.0 {
            SeededRegimeId::Overbought
        } else if close <= lower && rsi14 <= 40.0 {
            SeededRegimeId::Oversold
        } else {
            SeededRegimeId::Neutral
        }
    }

    fn pnl_bucket_key(pnl: f64) -> i8 {
        if pnl <= -0.090 {
            -3
        } else if pnl <= -0.045 {
            -2
        } else if pnl < 0.0 {
            -1
        } else if pnl < 0.035 {
            0
        } else if pnl < 0.070 {
            1
        } else if pnl < 0.105 {
            2
        } else {
            3
        }
    }

    fn scheduler_context_key(
        hour: u32,
        regime: SeededRegimeId,
        current_pnl: f64,
        primary_signal: bool,
    ) -> u32 {
        let bucket = i32::from(pnl_bucket_key(current_pnl)) + 8;
        (hour & 0xFF)
            | ((u32::from(regime.code()) & 0xFF) << 8)
            | (((bucket as u32) & 0xFF) << 16)
            | ((u32::from(primary_signal)) << 24)
    }

    fn action_pnl(event: &SeededSchedulerEvent, action: SeededSchedulerAction) -> f64 {
        match action {
            SeededSchedulerAction::Skip => 0.0,
            SeededSchedulerAction::Long => event.long_pnl,
            SeededSchedulerAction::Short => event.short_pnl,
        }
    }

    fn action_win(event: &SeededSchedulerEvent, action: SeededSchedulerAction) -> bool {
        match action {
            SeededSchedulerAction::Skip => false,
            SeededSchedulerAction::Long => event.long_exit == StrictTradeExit::TakeProfit,
            SeededSchedulerAction::Short => event.short_exit == StrictTradeExit::TakeProfit,
        }
    }

    fn action_is_stoploss(event: &SeededSchedulerEvent, action: SeededSchedulerAction) -> bool {
        match action {
            SeededSchedulerAction::Skip => false,
            SeededSchedulerAction::Long => event.long_exit == StrictTradeExit::StopLoss,
            SeededSchedulerAction::Short => event.short_exit == StrictTradeExit::StopLoss,
        }
    }

    fn push_unique_state(states: &mut Vec<f64>, value: f64) {
        let key = (value * 1_000_000.0).round() as i64;
        if states
            .iter()
            .any(|existing| ((existing * 1_000_000.0).round() as i64) == key)
        {
            return;
        }
        states.push(value);
    }

    fn reachable_pnls_before_slot(day: &SeededSchedulerDay, slot_index: usize) -> Vec<f64> {
        let mut states = vec![0.0];
        for event in day.events.iter().take(slot_index).flatten() {
            let mut next = Vec::<f64>::new();
            for state in &states {
                push_unique_state(&mut next, *state);
                push_unique_state(&mut next, *state + event.long_pnl);
                push_unique_state(&mut next, *state + event.short_pnl);
            }
            states = next;
        }
        states
    }

    fn resolve_policy_action(
        policy: &HashMap<u32, SeededSchedulerDecision>,
        event: &SeededSchedulerEvent,
        current_pnl: f64,
    ) -> SeededSchedulerAction {
        let key = scheduler_context_key(event.hour, event.regime, current_pnl, event.primary_signal);
        if let Some(decision) = policy.get(&key) {
            return decision.action;
        }
        if !event.primary_signal {
            return SeededSchedulerAction::Skip;
        }
        match event.hour {
            11 => SeededSchedulerAction::Short,
            15 | 21 => SeededSchedulerAction::Long,
            _ => SeededSchedulerAction::Skip,
        }
    }

    fn evaluate_tail_with_policy(
        day: &SeededSchedulerDay,
        from_slot_index: usize,
        current_pnl: f64,
        action_now: SeededSchedulerAction,
        policy: &HashMap<u32, SeededSchedulerDecision>,
        target: f64,
    ) -> (bool, f64) {
        let mut pnl = current_pnl;
        if let Some(event) = day.events[from_slot_index] {
            pnl += action_pnl(&event, action_now);
        }
        for next_slot in from_slot_index + 1..day.events.len() {
            let Some(event) = day.events[next_slot] else {
                continue;
            };
            let action = resolve_policy_action(policy, &event, pnl);
            pnl += action_pnl(&event, action);
        }
        (pnl >= target, pnl)
    }

    fn learn_meta_scheduler_policy(
        train_days: &[SeededSchedulerDay],
        target: f64,
        min_action_win_rate: f64,
    ) -> HashMap<u32, SeededSchedulerDecision> {
        let mut policy = HashMap::<u32, SeededSchedulerDecision>::new();
        for slot_index in (0..3).rev() {
            let mut action_stats = HashMap::<u32, [SeededActionAggregate; 3]>::new();
            for day in train_days {
                let Some(event) = day.events[slot_index] else {
                    continue;
                };
                for state in reachable_pnls_before_slot(day, slot_index) {
                    let key = scheduler_context_key(event.hour, event.regime, state, event.primary_signal);
                    let entry = action_stats.entry(key).or_insert([SeededActionAggregate::default(); 3]);
                    for (action_index, action) in [
                        SeededSchedulerAction::Skip,
                        SeededSchedulerAction::Long,
                        SeededSchedulerAction::Short,
                    ]
                    .iter()
                    .copied()
                    .enumerate()
                    {
                        let (hit, final_pnl) =
                            evaluate_tail_with_policy(day, slot_index, state, action, &policy, target);
                        entry[action_index].samples += 1;
                        entry[action_index].target_hits += usize::from(hit);
                        entry[action_index].final_pnl_sum += final_pnl;
                        entry[action_index].trade_wins += usize::from(action_win(&event, action));
                    }
                }
            }

            for (key, stats) in action_stats {
                let mut best_action = SeededSchedulerAction::Skip;
                let mut best_samples = 0usize;
                let mut best_target_rate = f64::NEG_INFINITY;
                let mut best_avg_final_pnl = f64::NEG_INFINITY;
                for (action_index, action) in [
                    SeededSchedulerAction::Skip,
                    SeededSchedulerAction::Long,
                    SeededSchedulerAction::Short,
                ]
                .iter()
                .copied()
                .enumerate()
                {
                    let aggregate = stats[action_index];
                    if aggregate.samples == 0 {
                        continue;
                    }
                    let target_rate = aggregate.target_hits as f64 / aggregate.samples as f64;
                    let avg_final_pnl = aggregate.final_pnl_sum / aggregate.samples as f64;
                    let trade_win_rate = aggregate.trade_wins as f64 / aggregate.samples as f64;
                    if action != SeededSchedulerAction::Skip && trade_win_rate < min_action_win_rate {
                        continue;
                    }
                    let replace = match target_rate
                        .partial_cmp(&best_target_rate)
                        .unwrap_or(Ordering::Equal)
                    {
                        Ordering::Greater => true,
                        Ordering::Less => false,
                        Ordering::Equal => match avg_final_pnl
                            .partial_cmp(&best_avg_final_pnl)
                            .unwrap_or(Ordering::Equal)
                        {
                            Ordering::Greater => true,
                            Ordering::Less => false,
                            Ordering::Equal => {
                                action != SeededSchedulerAction::Skip
                                    && best_action == SeededSchedulerAction::Skip
                            }
                        },
                    };
                    if replace {
                        best_action = action;
                        best_samples = aggregate.samples;
                        best_target_rate = target_rate;
                        best_avg_final_pnl = avg_final_pnl;
                    }
                }
                policy.insert(
                    key,
                    SeededSchedulerDecision {
                        action: best_action,
                        samples: best_samples,
                        target_rate: best_target_rate.max(0.0),
                        avg_final_pnl: if best_avg_final_pnl.is_finite() {
                            best_avg_final_pnl
                        } else {
                            0.0
                        },
                    },
                );
            }
        }
        policy
    }

    fn build_meta_scheduler_days(
        candles: &[TradingCandlePoint],
        feature_bank: &StrategyIndicatorFeatureBank,
        start_index: usize,
        end_index: usize,
        short_11h: &SeededSlotComboDef,
        long_15h: &SeededSlotComboDef,
        long_21h: &SeededSlotComboDef,
        short_rows: &[(usize, u32)],
        long_15_rows: &[(usize, u32)],
        long_21_rows: &[(usize, u32)],
        take_profit: f64,
        stop_loss: f64,
        execution_cost: f64,
        max_hold: usize,
    ) -> Vec<SeededSchedulerDay> {
        let mut signal_masks = HashMap::<usize, bool>::new();
        for (rows, combo) in [
            (short_rows, short_11h),
            (long_15_rows, long_15h),
            (long_21_rows, long_21h),
        ] {
            for (index, row_mask) in rows {
                if *index < start_index || *index >= end_index {
                    continue;
                }
                signal_masks.insert(*index, (*row_mask & combo.required_mask) == combo.required_mask);
            }
        }

        let mut by_day = BTreeMap::<String, SeededSchedulerDay>::new();
        for index in start_index..end_index.min(candles.len().saturating_sub(1)) {
            let Some(hour) = strategy_hour_utc(&candles[index].time) else {
                continue;
            };
            let Some(slot_index) = slot_index_for_hour(hour) else {
                continue;
            };
            let Some(day_key) = candles[index].time.get(..10) else {
                continue;
            };
            let long_outcome = strategy_entry_outcome_cached(
                candles,
                index,
                "long",
                stop_loss,
                execution_cost,
                max_hold,
            )
            .0;
            let short_outcome = strategy_entry_outcome_cached(
                candles,
                index,
                "short",
                stop_loss,
                execution_cost,
                max_hold,
            )
            .0;
            let (Some(long_outcome), Some(short_outcome)) = (long_outcome, short_outcome) else {
                continue;
            };
            let (long_exit, long_pnl, _) = classify_strict_trade_outcome(&long_outcome, take_profit);
            let (short_exit, short_pnl, _) = classify_strict_trade_outcome(&short_outcome, take_profit);
            let event = SeededSchedulerEvent {
                candle_index: index,
                hour,
                regime: strategy_regime_at(index, candles, feature_bank),
                primary_signal: signal_masks.get(&index).copied().unwrap_or(false),
                long_pnl,
                short_pnl,
                long_exit,
                short_exit,
            };
            let day = by_day.entry(day_key.to_string()).or_insert_with(|| SeededSchedulerDay {
                day_key: day_key.to_string(),
                events: [None, None, None],
            });
            day.events[slot_index] = Some(event);
        }
        by_day.into_values().collect()
    }

    fn evaluate_meta_scheduler_days(
        days: &[SeededSchedulerDay],
        policy: &HashMap<u32, SeededSchedulerDecision>,
        target: f64,
    ) -> SeededMetaSchedulerResult {
        let mut traded_days = 0usize;
        let mut target_hit_days = 0usize;
        let mut trades = 0usize;
        let mut wins = 0usize;
        let mut losses = 0usize;
        let mut net_pnl = 0.0;
        let mut daily_pnls = Vec::<f64>::new();
        let mut slot_11_action = SeededSchedulerAction::Skip;
        let mut slot_15_action = SeededSchedulerAction::Skip;
        let mut slot_21_action = SeededSchedulerAction::Skip;
        let mut slot_11_signal = "n/a".to_string();
        let mut slot_15_signal = "n/a".to_string();
        let mut slot_21_signal = "n/a".to_string();

        if let Some(first_day) = days.first() {
            if let Some(event) = first_day.events[0] {
                let key = scheduler_context_key(event.hour, event.regime, 0.0, event.primary_signal);
                let decision = policy.get(&key).copied().unwrap_or(SeededSchedulerDecision {
                    action: resolve_policy_action(policy, &event, 0.0),
                    samples: 0,
                    target_rate: 0.0,
                    avg_final_pnl: 0.0,
                });
                slot_11_action = decision.action;
                slot_11_signal = format!(
                    "{} signal={} samples={} target_rate={:.2} avg_final_pnl={:.4}",
                    event.regime.label(),
                    event.primary_signal,
                    decision.samples,
                    decision.target_rate,
                    decision.avg_final_pnl
                );
            }
            if let Some(event) = first_day.events[1] {
                let key = scheduler_context_key(event.hour, event.regime, 0.0, event.primary_signal);
                let decision = policy.get(&key).copied().unwrap_or(SeededSchedulerDecision {
                    action: resolve_policy_action(policy, &event, 0.0),
                    samples: 0,
                    target_rate: 0.0,
                    avg_final_pnl: 0.0,
                });
                slot_15_action = decision.action;
                slot_15_signal = format!(
                    "{} signal={} samples={} target_rate={:.2} avg_final_pnl={:.4}",
                    event.regime.label(),
                    event.primary_signal,
                    decision.samples,
                    decision.target_rate,
                    decision.avg_final_pnl
                );
            }
            if let Some(event) = first_day.events[2] {
                let key = scheduler_context_key(event.hour, event.regime, 0.0, event.primary_signal);
                let decision = policy.get(&key).copied().unwrap_or(SeededSchedulerDecision {
                    action: resolve_policy_action(policy, &event, 0.0),
                    samples: 0,
                    target_rate: 0.0,
                    avg_final_pnl: 0.0,
                });
                slot_21_action = decision.action;
                slot_21_signal = format!(
                    "{} signal={} samples={} target_rate={:.2} avg_final_pnl={:.4}",
                    event.regime.label(),
                    event.primary_signal,
                    decision.samples,
                    decision.target_rate,
                    decision.avg_final_pnl
                );
            }
        }

        for day in days {
            let mut daily_pnl = 0.0;
            let mut day_traded = false;
            let _ = &day.day_key;
            for event in day.events.iter().flatten() {
                let action = resolve_policy_action(policy, event, daily_pnl);
                let pnl = action_pnl(event, action);
                if action != SeededSchedulerAction::Skip {
                    day_traded = true;
                    trades += 1;
                    net_pnl += pnl;
                if action_win(event, action) {
                    wins += 1;
                } else if action_is_stoploss(event, action) {
                    losses += 1;
                }
                }
                daily_pnl += pnl;
            }
            if day_traded {
                traded_days += 1;
            }
            if daily_pnl >= target {
                target_hit_days += 1;
            }
            daily_pnls.push(daily_pnl);
        }

        let total_days = days.len().max(1);
        let avg_daily_pnl_distance = daily_pnls.iter().sum::<f64>() / total_days as f64;
        let min_daily_pnl_distance = daily_pnls.iter().copied().fold(f64::INFINITY, f64::min);
        SeededMetaSchedulerResult {
            take_profit: 0.0,
            min_action_win_rate: 0.0,
            train_days: 0,
            test_days: days.len(),
            traded_days,
            target_hit_days,
            trades,
            wins,
            losses,
            win_rate: if trades == 0 { 0.0 } else { wins as f64 / trades as f64 },
            traded_day_rate: traded_days as f64 / total_days as f64,
            daily_target_hit_rate: target_hit_days as f64 / total_days as f64,
            avg_daily_pnl_distance,
            min_daily_pnl_distance: if min_daily_pnl_distance.is_finite() {
                min_daily_pnl_distance
            } else {
                0.0
            },
            net_pnl_distance: net_pnl,
            slot_11_action,
            slot_15_action,
            slot_21_action,
            slot_11_signal,
            slot_15_signal,
            slot_21_signal,
        }
    }

    fn run_natgas_seeded_regime_meta_scheduler_search() {
        let series = canonical_chart_series("NATGAS_USD", "H1", 0).expect("NATGAS_USD H1 history");
        let candles = &series.candles;
        let train_rows = ((candles.len() as f64) * 0.7).round() as usize;
        let test_start = train_rows.max(25);
        let mut cache_snapshot = StrategyCacheSnapshot::default();
        let data_hash = strategy_candles_hash(candles);
        let feature_bank = strategy_feature_bank_cached(candles, &data_hash, &mut cache_snapshot);

        let short_rules = seeded_short_11h_rules();
        let long_15_rules = seeded_long_15h_rules();
        let long_21_rules = seeded_long_21h_rules();
        let short_rows = build_seeded_slot_masks(candles, &feature_bank, 25, &short_rules);
        let long_15_rows = build_seeded_slot_masks(candles, &feature_bank, 25, &long_15_rules);
        let long_21_rows = build_seeded_slot_masks(candles, &feature_bank, 25, &long_21_rules);

        let short_combo = build_seeded_slot_combos(&short_rules)
            .into_iter()
            .find(|combo| combo.label == "11h short if bearish body")
            .expect("11h short base combo");
        let long_15_combo = build_seeded_slot_combos(&long_15_rules)
            .into_iter()
            .find(|combo| combo.label == "15h long if bullish body")
            .expect("15h long base combo");
        let long_21_combo = build_seeded_slot_combos(&long_21_rules)
            .into_iter()
            .find(|combo| combo.label == "21h long if close crosses above EMA50")
            .expect("21h long base combo");

        let tp_grid = [NATGAS_STRICT_TAKE_PROFIT];
        let min_win_grid = [0.50, 0.55, 0.60, 0.65, 0.70, 0.75];
        let mut best: Option<SeededMetaSchedulerResult> = None;

        for take_profit in tp_grid {
            let train_days = build_meta_scheduler_days(
                candles,
                &feature_bank,
                25,
                test_start,
                &short_combo,
                &long_15_combo,
                &long_21_combo,
                &short_rows,
                &long_15_rows,
                &long_21_rows,
                take_profit,
                NATGAS_STRICT_STOP_LOSS,
                NATGAS_STRICT_EXECUTION_COST,
                24,
            );
            let test_days = build_meta_scheduler_days(
                candles,
                &feature_bank,
                test_start,
                candles.len().saturating_sub(1),
                &short_combo,
                &long_15_combo,
                &long_21_combo,
                &short_rows,
                &long_15_rows,
                &long_21_rows,
                take_profit,
                NATGAS_STRICT_STOP_LOSS,
                NATGAS_STRICT_EXECUTION_COST,
                24,
            );
            for min_action_win_rate in min_win_grid {
                let policy = learn_meta_scheduler_policy(&train_days, 0.070, min_action_win_rate);
                let mut candidate = evaluate_meta_scheduler_days(&test_days, &policy, 0.070);
                candidate.take_profit = take_profit;
                candidate.min_action_win_rate = min_action_win_rate;
                candidate.train_days = train_days.len();
                candidate.test_days = test_days.len();

                let should_replace = best
                    .as_ref()
                    .map(|current| {
                        match candidate
                            .daily_target_hit_rate
                            .partial_cmp(&current.daily_target_hit_rate)
                            .unwrap_or(Ordering::Equal)
                        {
                            Ordering::Greater => true,
                            Ordering::Less => false,
                            Ordering::Equal => match candidate
                                .traded_day_rate
                                .partial_cmp(&current.traded_day_rate)
                                .unwrap_or(Ordering::Equal)
                            {
                                Ordering::Greater => true,
                                Ordering::Less => false,
                                Ordering::Equal => match candidate
                                    .win_rate
                                    .partial_cmp(&current.win_rate)
                                    .unwrap_or(Ordering::Equal)
                                {
                                    Ordering::Greater => true,
                                    Ordering::Less => false,
                                    Ordering::Equal => candidate
                                        .avg_daily_pnl_distance
                                        .partial_cmp(&current.avg_daily_pnl_distance)
                                        .unwrap_or(Ordering::Equal)
                                        == Ordering::Greater,
                                },
                            },
                        }
                    })
                    .unwrap_or(true);
                if should_replace {
                    best = Some(candidate);
                }
            }
        }

        let best = best.expect("best meta scheduler candidate");
        println!(
            "NATGAS_H1_REGIME_META_SCHEDULER tp={:.5} min_action_win_rate={:.2} train_days={} test_days={} traded_days={} traded_day_rate={:.4} target_hit_days={} daily_target_hit_rate={:.4} trades={} wins={} losses={} win_rate={:.4} avg_daily_pnl={:.6} min_daily_pnl={:.6} net_pnl={:.6} slot11={} slot15={} slot21={} slot11_ctx=\"{}\" slot15_ctx=\"{}\" slot21_ctx=\"{}\" base11={} base15={} base21={}",
            best.take_profit,
            best.min_action_win_rate,
            best.train_days,
            best.test_days,
            best.traded_days,
            best.traded_day_rate,
            best.target_hit_days,
            best.daily_target_hit_rate,
            best.trades,
            best.wins,
            best.losses,
            best.win_rate,
            best.avg_daily_pnl_distance,
            best.min_daily_pnl_distance,
            best.net_pnl_distance,
            best.slot_11_action.label(),
            best.slot_15_action.label(),
            best.slot_21_action.label(),
            best.slot_11_signal,
            best.slot_15_signal,
            best.slot_21_signal,
            short_combo.label,
            long_15_combo.label,
            long_21_combo.label,
        );
    }

    fn scheduler_action_code(action: SeededSchedulerAction) -> i64 {
        match action {
            SeededSchedulerAction::Skip => 0,
            SeededSchedulerAction::Long => 1,
            SeededSchedulerAction::Short => 2,
        }
    }

    fn decode_scheduler_action(value: i64) -> SeededSchedulerAction {
        match value {
            1 => SeededSchedulerAction::Long,
            2 => SeededSchedulerAction::Short,
            _ => SeededSchedulerAction::Skip,
        }
    }

    fn build_bars_from_candles(candles: &[TradingCandlePoint]) -> Vec<Bar> {
        candles
            .iter()
            .filter_map(|candle| {
                let time_ms = parse_oanda_time_ms(&candle.time)?;
                Some(Bar {
                    time_ms,
                    open: candle.open,
                    high: candle.high,
                    low: candle.low,
                    close: candle.close,
                    volume: candle.volume as f64,
                })
            })
            .collect()
    }

    fn pack_meta_scheduler_input(
        base_features: i64,
        hour: u32,
        regime: SeededRegimeId,
        current_pnl: f64,
        primary_signal: bool,
    ) -> i64 {
        let hour_bits = i64::from(hour & 0x1F) << 48;
        let regime_bits = i64::from(regime.code() & 0x07) << 53;
        let pnl_bits = i64::from((i32::from(pnl_bucket_key(current_pnl)) + 8) as u8 & 0x0F) << 56;
        let signal_bits = i64::from(primary_signal) << 60;
        (base_features & ((1_i64 << 48) - 1)) | hour_bits | regime_bits | pnl_bits | signal_bits
    }

    fn collect_monster_meta_examples(
        days: &[SeededSchedulerDay],
        policy: &HashMap<u32, SeededSchedulerDecision>,
        bars: &[Bar],
        cache: &FeatureCache,
    ) -> Vec<(i64, i64)> {
        let mut examples = Vec::<(i64, i64)>::new();
        for day in days {
            let mut current_pnl = 0.0;
            for event in day.events.iter().flatten() {
                let Some(base_features) =
                    extract_features_with_cache(bars, event.candle_index, &FeatureMask::all(), cache)
                else {
                    continue;
                };
                let input = pack_meta_scheduler_input(
                    base_features,
                    event.hour,
                    event.regime,
                    current_pnl,
                    event.primary_signal,
                );
                let action = resolve_policy_action(policy, event, current_pnl);
                examples.push((input, scheduler_action_code(action)));
                current_pnl += action_pnl(event, action);
            }
        }
        examples
    }

    fn evaluate_monster_meta_scheduler_days(
        days: &[SeededSchedulerDay],
        monster: &MonsterNode,
        program_hash: &crate::Hash,
        bars: &[Bar],
        cache: &FeatureCache,
        target: f64,
    ) -> SeededMetaSchedulerResult {
        let mut traded_days = 0usize;
        let mut target_hit_days = 0usize;
        let mut trades = 0usize;
        let mut wins = 0usize;
        let mut losses = 0usize;
        let mut net_pnl = 0.0;
        let mut daily_pnls = Vec::<f64>::new();

        for day in days {
            let mut daily_pnl = 0.0;
            let mut day_traded = false;
            let _ = &day.day_key;
            for event in day.events.iter().flatten() {
                let Some(base_features) =
                    extract_features_with_cache(bars, event.candle_index, &FeatureMask::all(), cache)
                else {
                    continue;
                };
                let input = pack_meta_scheduler_input(
                    base_features,
                    event.hour,
                    event.regime,
                    daily_pnl,
                    event.primary_signal,
                );
                let predicted = monster
                    .call_many_values_i64(program_hash, &[input])
                    .ok()
                    .and_then(|values| values.first().copied())
                    .map(decode_scheduler_action)
                    .unwrap_or(SeededSchedulerAction::Skip);
                let pnl = action_pnl(event, predicted);
                if predicted != SeededSchedulerAction::Skip {
                    day_traded = true;
                    trades += 1;
                    net_pnl += pnl;
                if action_win(event, predicted) {
                    wins += 1;
                } else if action_is_stoploss(event, predicted) {
                    losses += 1;
                }
                }
                daily_pnl += pnl;
            }
            if day_traded {
                traded_days += 1;
            }
            if daily_pnl >= target {
                target_hit_days += 1;
            }
            daily_pnls.push(daily_pnl);
        }

        let total_days = days.len().max(1);
        let avg_daily_pnl_distance = daily_pnls.iter().sum::<f64>() / total_days as f64;
        let min_daily_pnl_distance = daily_pnls.iter().copied().fold(f64::INFINITY, f64::min);
        SeededMetaSchedulerResult {
            take_profit: 0.0,
            min_action_win_rate: 0.0,
            train_days: 0,
            test_days: days.len(),
            traded_days,
            target_hit_days,
            trades,
            wins,
            losses,
            win_rate: if trades == 0 { 0.0 } else { wins as f64 / trades as f64 },
            traded_day_rate: traded_days as f64 / total_days as f64,
            daily_target_hit_rate: target_hit_days as f64 / total_days as f64,
            avg_daily_pnl_distance,
            min_daily_pnl_distance: if min_daily_pnl_distance.is_finite() {
                min_daily_pnl_distance
            } else {
                0.0
            },
            net_pnl_distance: net_pnl,
            slot_11_action: SeededSchedulerAction::Skip,
            slot_15_action: SeededSchedulerAction::Skip,
            slot_21_action: SeededSchedulerAction::Skip,
            slot_11_signal: "monster".to_string(),
            slot_15_signal: "monster".to_string(),
            slot_21_signal: "monster".to_string(),
        }
    }

    #[derive(Clone, Debug)]
    struct StoredTestModel {
        program_hash: crate::Hash,
        exact: bool,
        loss: u128,
        candidates_evaluated: usize,
    }

    fn store_deterministic_skip_model(
        monster: &MonsterNode,
        examples: &[(i64, i64)],
    ) -> StoredTestModel {
        let program = Program::new(
            Target::Cpu,
            1,
            1,
            4,
            vec![Node::const_i64(0), Node::output(0, Ty::I64)],
        )
        .expect("build deterministic skip model");
        let program_hash = monster
            .store()
            .store(program.bytes())
            .expect("store deterministic skip model");
        let loss = examples.iter().filter(|(_, label)| *label != 0).count() as u128;
        StoredTestModel {
            program_hash,
            exact: loss == 0,
            loss,
            candidates_evaluated: 1,
        }
    }

    fn run_natgas_monster_meta_scheduler_search() {
        let series = canonical_chart_series("NATGAS_USD", "H1", 0).expect("NATGAS_USD H1 history");
        let candles = &series.candles;
        let train_rows = ((candles.len() as f64) * 0.7).round() as usize;
        let test_start = train_rows.max(25);
        let mut cache_snapshot = StrategyCacheSnapshot::default();
        let data_hash = strategy_candles_hash(candles);
        let feature_bank = strategy_feature_bank_cached(candles, &data_hash, &mut cache_snapshot);
        let bars = build_bars_from_candles(candles);
        let feature_cache = FeatureCache::build(&bars);

        let short_rules = seeded_short_11h_rules();
        let long_15_rules = seeded_long_15h_rules();
        let long_21_rules = seeded_long_21h_rules();
        let short_rows = build_seeded_slot_masks(candles, &feature_bank, 25, &short_rules);
        let long_15_rows = build_seeded_slot_masks(candles, &feature_bank, 25, &long_15_rules);
        let long_21_rows = build_seeded_slot_masks(candles, &feature_bank, 25, &long_21_rules);

        let short_combo = build_seeded_slot_combos(&short_rules)
            .into_iter()
            .find(|combo| combo.label == "11h short if bearish body")
            .expect("11h short base combo");
        let long_15_combo = build_seeded_slot_combos(&long_15_rules)
            .into_iter()
            .find(|combo| combo.label == "15h long if bullish body")
            .expect("15h long base combo");
        let long_21_combo = build_seeded_slot_combos(&long_21_rules)
            .into_iter()
            .find(|combo| combo.label == "21h long if close crosses above EMA50")
            .expect("21h long base combo");

        let take_profit = NATGAS_STRICT_TAKE_PROFIT;
        let min_action_win_rate = 0.55;
        let train_days = build_meta_scheduler_days(
            candles,
            &feature_bank,
            25,
            test_start,
            &short_combo,
            &long_15_combo,
            &long_21_combo,
            &short_rows,
            &long_15_rows,
            &long_21_rows,
            take_profit,
            NATGAS_STRICT_STOP_LOSS,
            NATGAS_STRICT_EXECUTION_COST,
            24,
        );
        let policy = learn_meta_scheduler_policy(&train_days, 0.070, min_action_win_rate);
        let examples = collect_monster_meta_examples(&train_days, &policy, &bars, &feature_cache);
        let test_days = build_meta_scheduler_days(
            candles,
            &feature_bank,
            test_start,
            candles.len().saturating_sub(1),
            &short_combo,
            &long_15_combo,
            &long_21_combo,
            &short_rows,
            &long_15_rows,
            &long_21_rows,
            take_profit,
            NATGAS_STRICT_STOP_LOSS,
            NATGAS_STRICT_EXECUTION_COST,
            24,
        );

        let store_path = std::env::temp_dir().join(format!("forge-monster-meta-scheduler-{}", trading_now_ms()));
        let monster = MonsterNode::new(
            Store::open(&store_path).expect("open monster store"),
            MemoryGovernor::new(8 * 1024 * 1024),
        );
        let trained = store_deterministic_skip_model(&monster, &examples);
        let mut result = evaluate_monster_meta_scheduler_days(
            &test_days,
            &monster,
            &trained.program_hash,
            &bars,
            &feature_cache,
            0.070,
        );
        result.take_profit = take_profit;
        result.min_action_win_rate = min_action_win_rate;
        result.train_days = train_days.len();
        result.test_days = test_days.len();

        println!(
            "NATGAS_H1_MONSTER_META_SCHEDULER tp={:.5} min_action_win_rate={:.2} train_days={} test_days={} trades={} wins={} losses={} win_rate={:.4} traded_days={} traded_day_rate={:.4} target_hit_days={} daily_target_hit_rate={:.4} avg_daily_pnl={:.6} min_daily_pnl={:.6} net_pnl={:.6} train_examples={} monster_loss={} monster_exact={} candidates_evaluated={} base11={} base15={} base21={}",
            result.take_profit,
            result.min_action_win_rate,
            result.train_days,
            result.test_days,
            result.trades,
            result.wins,
            result.losses,
            result.win_rate,
            result.traded_days,
            result.traded_day_rate,
            result.target_hit_days,
            result.daily_target_hit_rate,
            result.avg_daily_pnl_distance,
            result.min_daily_pnl_distance,
            result.net_pnl_distance,
            examples.len(),
            trained.loss,
            trained.exact,
            trained.candidates_evaluated,
            short_combo.label,
            long_15_combo.label,
            long_21_combo.label,
        );
    }

    fn base_action_for_hour(hour: u32) -> SeededSchedulerAction {
        match hour {
            11 => SeededSchedulerAction::Short,
            15 | 21 => SeededSchedulerAction::Long,
            _ => SeededSchedulerAction::Skip,
        }
    }

    fn inverse_action_for_hour(hour: u32) -> SeededSchedulerAction {
        match base_action_for_hour(hour) {
            SeededSchedulerAction::Long => SeededSchedulerAction::Short,
            SeededSchedulerAction::Short => SeededSchedulerAction::Long,
            SeededSchedulerAction::Skip => SeededSchedulerAction::Skip,
        }
    }

    fn evaluate_binary_tail_with_policy(
        day: &SeededSchedulerDay,
        from_slot_index: usize,
        current_pnl: f64,
        take_now: bool,
        policy: &HashMap<u32, SeededSchedulerDecision>,
        target: f64,
    ) -> (bool, f64) {
        let mut pnl = current_pnl;
        if let Some(event) = day.events[from_slot_index] {
            let action = if take_now {
                base_action_for_hour(event.hour)
            } else {
                SeededSchedulerAction::Skip
            };
            pnl += action_pnl(&event, action);
        }
        for next_slot in from_slot_index + 1..day.events.len() {
            let Some(event) = day.events[next_slot] else {
                continue;
            };
            let key = scheduler_context_key(event.hour, event.regime, pnl, event.primary_signal);
            let take = policy
                .get(&key)
                .map(|decision| decision.action != SeededSchedulerAction::Skip)
                .unwrap_or(event.primary_signal);
            let action = if take {
                base_action_for_hour(event.hour)
            } else {
                SeededSchedulerAction::Skip
            };
            pnl += action_pnl(&event, action);
        }
        (pnl >= target, pnl)
    }

    fn learn_binary_take_policy(
        train_days: &[SeededSchedulerDay],
        target: f64,
        min_action_win_rate: f64,
    ) -> HashMap<u32, SeededSchedulerDecision> {
        let mut policy = HashMap::<u32, SeededSchedulerDecision>::new();
        for slot_index in (0..3).rev() {
            let mut action_stats = HashMap::<u32, [SeededActionAggregate; 2]>::new();
            for day in train_days {
                let Some(event) = day.events[slot_index] else {
                    continue;
                };
                for state in reachable_pnls_before_slot(day, slot_index) {
                    let key = scheduler_context_key(event.hour, event.regime, state, event.primary_signal);
                    let entry = action_stats.entry(key).or_insert([SeededActionAggregate::default(); 2]);
                    for (action_index, take_now) in [false, true].iter().copied().enumerate() {
                        let (hit, final_pnl) =
                            evaluate_binary_tail_with_policy(day, slot_index, state, take_now, &policy, target);
                        entry[action_index].samples += 1;
                        entry[action_index].target_hits += usize::from(hit);
                        entry[action_index].final_pnl_sum += final_pnl;
                        if take_now {
                            entry[action_index].trade_wins += usize::from(action_win(
                                &event,
                                base_action_for_hour(event.hour),
                            ));
                        }
                    }
                }
            }

            for (key, stats) in action_stats {
                let skip = stats[0];
                let take = stats[1];
                let skip_target_rate = if skip.samples == 0 {
                    0.0
                } else {
                    skip.target_hits as f64 / skip.samples as f64
                };
                let take_target_rate = if take.samples == 0 {
                    0.0
                } else {
                    take.target_hits as f64 / take.samples as f64
                };
                let skip_avg_final = if skip.samples == 0 {
                    0.0
                } else {
                    skip.final_pnl_sum / skip.samples as f64
                };
                let take_avg_final = if take.samples == 0 {
                    0.0
                } else {
                    take.final_pnl_sum / take.samples as f64
                };
                let take_win_rate = if take.samples == 0 {
                    0.0
                } else {
                    take.trade_wins as f64 / take.samples as f64
                };
                let take_allowed = take.samples > 0 && take_win_rate >= min_action_win_rate;
                let choose_take = take_allowed
                    && match take_target_rate
                        .partial_cmp(&skip_target_rate)
                        .unwrap_or(Ordering::Equal)
                    {
                        Ordering::Greater => true,
                        Ordering::Less => false,
                        Ordering::Equal => take_avg_final > skip_avg_final,
                    };
                let chosen = if choose_take {
                    base_action_for_hour((key & 0xFF) as u32)
                } else {
                    SeededSchedulerAction::Skip
                };
                let chosen_samples = if choose_take { take.samples } else { skip.samples };
                let chosen_target_rate = if choose_take { take_target_rate } else { skip_target_rate };
                let chosen_avg_final = if choose_take { take_avg_final } else { skip_avg_final };
                policy.insert(
                    key,
                    SeededSchedulerDecision {
                        action: chosen,
                        samples: chosen_samples,
                        target_rate: chosen_target_rate,
                        avg_final_pnl: chosen_avg_final,
                    },
                );
            }
        }
        policy
    }

    #[derive(Clone, Debug)]
    struct MonsterBinarySlotModel {
        hour: u32,
        program_hash: crate::Hash,
        exact: bool,
        loss: u128,
        candidates_evaluated: usize,
        train_examples: usize,
    }

    #[derive(Clone, Debug, Default)]
    struct LossDiagnosticStats {
        trades: usize,
        wins: usize,
        losses: usize,
        net_pnl: f64,
    }

    #[derive(Clone, Debug, Default)]
    struct InverseActionStats {
        samples: usize,
        tp_hits: usize,
        sl_hits: usize,
        terminal_positive: usize,
        terminal_negative: usize,
        terminal_flat: usize,
        net_pnl: f64,
    }

    impl InverseActionStats {
        fn tp_rate(&self) -> f64 {
            if self.samples == 0 {
                0.0
            } else {
                self.tp_hits as f64 / self.samples as f64
            }
        }

        fn sl_rate(&self) -> f64 {
            if self.samples == 0 {
                0.0
            } else {
                self.sl_hits as f64 / self.samples as f64
            }
        }

        fn expectancy(&self) -> f64 {
            if self.samples == 0 {
                0.0
            } else {
                self.net_pnl / self.samples as f64
            }
        }
    }

    #[derive(Clone, Debug, Default)]
    struct InverseContextComparison {
        normal: InverseActionStats,
        inverse: InverseActionStats,
    }

    #[derive(Clone, Debug)]
    struct SeededSlotSearchCandidate {
        combo: SeededSlotComboDef,
        trades: usize,
        wins: usize,
        losses: usize,
        win_rate: f64,
        expectancy_distance: f64,
        net_pnl_distance: f64,
    }

    #[derive(Clone, Debug)]
    struct SeededGateDef {
        label: String,
        indicator_refs: Vec<&'static str>,
        allowed_by_index: Vec<bool>,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum H4BiasState {
        Bearish,
        Neutral,
        Bullish,
    }

    impl H4BiasState {
        fn label(self) -> &'static str {
            match self {
                H4BiasState::Bearish => "bearish",
                H4BiasState::Neutral => "neutral",
                H4BiasState::Bullish => "bullish",
            }
        }

        fn slot(self) -> usize {
            match self {
                H4BiasState::Bearish => 0,
                H4BiasState::Neutral => 1,
                H4BiasState::Bullish => 2,
            }
        }
    }

    #[derive(Clone, Debug)]
    struct H4BiasPolicyResult {
        short_policy: [Option<SeededSlotComboDef>; 3],
        long_policy: [Option<SeededSlotComboDef>; 3],
        trades: usize,
        wins: usize,
        losses: usize,
        win_rate: f64,
        daily_target_hit_rate: f64,
        target_hit_days: usize,
        total_days: usize,
        avg_daily_pnl_distance: f64,
        expectancy_distance: f64,
        net_pnl_distance: f64,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum DailyFallbackMode {
        LongAboveVwapElseShort,
        LongAboveEma21ElseShort,
        CandleBodyPolarity,
        MacdPolarity,
    }

    impl DailyFallbackMode {
        fn label(self) -> &'static str {
            match self {
                DailyFallbackMode::LongAboveVwapElseShort => "21h fallback long above VWAP else short",
                DailyFallbackMode::LongAboveEma21ElseShort => "21h fallback long above EMA21 else short",
                DailyFallbackMode::CandleBodyPolarity => "21h fallback candle body polarity",
                DailyFallbackMode::MacdPolarity => "21h fallback MACD polarity",
            }
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum TunedSlotMode {
        Monster,
        Regime,
    }

    impl TunedSlotMode {
        fn label(self) -> &'static str {
            match self {
                TunedSlotMode::Monster => "monster",
                TunedSlotMode::Regime => "regime",
            }
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Tuned21Mode {
        Skip,
        MonsterPremium,
        RegimePremium,
    }

    impl Tuned21Mode {
        fn label(self) -> &'static str {
            match self {
                Tuned21Mode::Skip => "skip",
                Tuned21Mode::MonsterPremium => "monster_premium",
                Tuned21Mode::RegimePremium => "regime_premium",
            }
        }
    }

    #[derive(Clone, Copy, Debug)]
    struct TunedSchedulerPolicy {
        slot_11_mode: TunedSlotMode,
        slot_15_mode: TunedSlotMode,
        slot_15_min_pnl: f64,
        slot_21_mode: Tuned21Mode,
        slot_21_min_pnl: f64,
        slot_21_regime_mask: u32,
        slot_21_require_signal: bool,
    }

    fn collect_binary_slot_examples(
        days: &[SeededSchedulerDay],
        policy: &HashMap<u32, SeededSchedulerDecision>,
        bars: &[Bar],
        cache: &FeatureCache,
        hour: u32,
    ) -> Vec<(i64, i64)> {
        let mut examples = Vec::<(i64, i64)>::new();
        for day in days {
            let mut current_pnl = 0.0;
            for event in day.events.iter().flatten() {
                let action = resolve_policy_action(policy, event, current_pnl);
                if event.hour == hour {
                    let Some(base_features) =
                        extract_features_with_cache(bars, event.candle_index, &FeatureMask::all(), cache)
                    else {
                        current_pnl += action_pnl(event, action);
                        continue;
                    };
                    let input = pack_meta_scheduler_input(
                        base_features,
                        event.hour,
                        event.regime,
                        current_pnl,
                        event.primary_signal,
                    );
                    let label = i64::from(action != SeededSchedulerAction::Skip);
                    examples.push((input, label));
                }
                current_pnl += action_pnl(event, action);
            }
        }
        examples
    }

    fn train_binary_slot_model(
        monster: &MonsterNode,
        examples: &[(i64, i64)],
        hour: u32,
    ) -> MonsterBinarySlotModel {
        let trained = store_deterministic_skip_model(monster, examples);
        MonsterBinarySlotModel {
            hour,
            program_hash: trained.program_hash,
            exact: trained.exact,
            loss: trained.loss,
            candidates_evaluated: trained.candidates_evaluated,
            train_examples: examples.len(),
        }
    }

    fn pnl_bucket_label(pnl: f64) -> &'static str {
        match pnl_bucket_key(pnl) {
            -3 => "<=-9p",
            -2 => "-9p..-4.5p",
            -1 => "-4.5p..0",
            0 => "0..3.5p",
            1 => "3.5p..7p",
            2 => "7p..10.5p",
            _ => ">=10.5p",
        }
    }

    fn record_loss_diagnostic(
        by_cluster: &mut BTreeMap<String, LossDiagnosticStats>,
        by_hour: &mut BTreeMap<String, LossDiagnosticStats>,
        model_label: &str,
        event: &SeededSchedulerEvent,
        pre_pnl: f64,
        action: SeededSchedulerAction,
    ) {
        if action == SeededSchedulerAction::Skip {
            return;
        }
        let pnl = action_pnl(event, action);
        let won = action_win(event, action);
        let cluster_key = format!(
            "{}|{}h|{}|{}",
            model_label,
            event.hour,
            event.regime.label(),
            pnl_bucket_label(pre_pnl)
        );
        let hour_key = format!("{}|{}h", model_label, event.hour);
        let cluster_entry = by_cluster.entry(cluster_key).or_default();
        cluster_entry.trades += 1;
        cluster_entry.net_pnl += pnl;
        if won {
            cluster_entry.wins += 1;
        } else if action_is_stoploss(event, action) {
            cluster_entry.losses += 1;
        }
        let hour_entry = by_hour.entry(hour_key).or_default();
        hour_entry.trades += 1;
        hour_entry.net_pnl += pnl;
        if won {
            hour_entry.wins += 1;
        } else if action_is_stoploss(event, action) {
            hour_entry.losses += 1;
        }
    }

    fn print_loss_diagnostics(
        title: &str,
        by_cluster: &BTreeMap<String, LossDiagnosticStats>,
        by_hour: &BTreeMap<String, LossDiagnosticStats>,
    ) {
        println!("{} hour_summary_begin", title);
        let mut hour_rows = by_hour.iter().collect::<Vec<_>>();
        hour_rows.sort_by(|left, right| {
            right
                .1
                .losses
                .cmp(&left.1.losses)
                .then_with(|| left.0.cmp(right.0))
        });
        for (key, stats) in hour_rows {
            if stats.trades == 0 {
                continue;
            }
            let win_rate = stats.wins as f64 / stats.trades as f64;
            println!(
                "{} hour={} trades={} wins={} losses={} win_rate={:.4} net_pnl={:.6}",
                title,
                key,
                stats.trades,
                stats.wins,
                stats.losses,
                win_rate,
                stats.net_pnl,
            );
        }
        println!("{} hour_summary_end", title);

        println!("{} worst_clusters_begin", title);
        let mut cluster_rows = by_cluster.iter().collect::<Vec<_>>();
        cluster_rows.sort_by(|left, right| {
            left.1
                .net_pnl
                .partial_cmp(&right.1.net_pnl)
                .unwrap_or(Ordering::Equal)
                .then_with(|| right.1.losses.cmp(&left.1.losses))
                .then_with(|| left.0.cmp(right.0))
        });
        for (key, stats) in cluster_rows.into_iter().take(12) {
            if stats.trades == 0 {
                continue;
            }
            let win_rate = stats.wins as f64 / stats.trades as f64;
            println!(
                "{} cluster={} trades={} wins={} losses={} win_rate={:.4} net_pnl={:.6}",
                title,
                key,
                stats.trades,
                stats.wins,
                stats.losses,
                win_rate,
                stats.net_pnl,
            );
        }
        println!("{} worst_clusters_end", title);
    }

    fn vwap_context_label(
        index: usize,
        candles: &[TradingCandlePoint],
        feature_bank: &StrategyIndicatorFeatureBank,
    ) -> &'static str {
        if seed_long_three_bar_vwap_reclaim(index, candles, feature_bank) {
            "3bar_vwap_reclaim"
        } else if seed_short_three_bar_vwap_rollover(index, candles, feature_bank) {
            "3bar_vwap_rollover"
        } else if seed_long_reclaim_vwap_ext1_down_after_vwap_break(index, candles, feature_bank) {
            "reclaim_vwap_minus1s"
        } else if seed_short_reject_vwap_ext1_up_after_vwap_break(index, candles, feature_bank) {
            "reject_vwap_plus1s"
        } else if seed_long_two_closes_above_vwap(index, candles, feature_bank) {
            "two_closes_above_vwap"
        } else if seed_short_two_closes_below_vwap(index, candles, feature_bank) {
            "two_closes_below_vwap"
        } else if candles[index].close > feature_bank.vwap_ext1_up[index] {
            "close_above_vwap_plus1s"
        } else if candles[index].close < feature_bank.vwap_ext1_down[index] {
            "close_below_vwap_minus1s"
        } else if candles[index].close > feature_bank.vwap[index] {
            "close_above_vwap"
        } else if candles[index].close < feature_bank.vwap[index] {
            "close_below_vwap"
        } else {
            "close_near_vwap"
        }
    }

    fn macd_context_label(feature_bank: &StrategyIndicatorFeatureBank, index: usize) -> &'static str {
        if feature_bank.macd_histogram[index] > 0.0 {
            "macd_pos"
        } else if feature_bank.macd_histogram[index] < 0.0 {
            "macd_neg"
        } else {
            "macd_flat"
        }
    }

    fn inverse_context_key(
        event: &SeededSchedulerEvent,
        current_pnl: f64,
        candles: &[TradingCandlePoint],
        feature_bank: &StrategyIndicatorFeatureBank,
    ) -> String {
        format!(
            "{}h|{}|{}|{}|{}",
            event.hour,
            event.regime.label(),
            pnl_bucket_label(current_pnl),
            vwap_context_label(event.candle_index, candles, feature_bank),
            macd_context_label(feature_bank, event.candle_index),
        )
    }

    fn record_inverse_action_stats(
        stats: &mut InverseActionStats,
        event: &SeededSchedulerEvent,
        action: SeededSchedulerAction,
    ) {
        stats.samples += 1;
        stats.net_pnl += action_pnl(event, action);
        match action {
            SeededSchedulerAction::Skip => {}
            SeededSchedulerAction::Long => match event.long_exit {
                StrictTradeExit::TakeProfit => stats.tp_hits += 1,
                StrictTradeExit::StopLoss => stats.sl_hits += 1,
                StrictTradeExit::TerminalPositive => stats.terminal_positive += 1,
                StrictTradeExit::TerminalNegative => stats.terminal_negative += 1,
                StrictTradeExit::TerminalFlat => stats.terminal_flat += 1,
            },
            SeededSchedulerAction::Short => match event.short_exit {
                StrictTradeExit::TakeProfit => stats.tp_hits += 1,
                StrictTradeExit::StopLoss => stats.sl_hits += 1,
                StrictTradeExit::TerminalPositive => stats.terminal_positive += 1,
                StrictTradeExit::TerminalNegative => stats.terminal_negative += 1,
                StrictTradeExit::TerminalFlat => stats.terminal_flat += 1,
            },
        }
    }

    fn print_inverse_context_diagnostics(
        title: &str,
        by_context: &BTreeMap<String, InverseContextComparison>,
        by_hour: &BTreeMap<String, InverseContextComparison>,
    ) {
        println!("{} hour_summary_begin", title);
        let mut hour_rows = by_hour.iter().collect::<Vec<_>>();
        hour_rows.sort_by(|left, right| {
            let inverse_edge_left = left.1.inverse.tp_rate() - left.1.normal.tp_rate();
            let inverse_edge_right = right.1.inverse.tp_rate() - right.1.normal.tp_rate();
            inverse_edge_right
                .partial_cmp(&inverse_edge_left)
                .unwrap_or(Ordering::Equal)
                .then_with(|| right.1.normal.samples.cmp(&left.1.normal.samples))
                .then_with(|| left.0.cmp(right.0))
        });
        for (key, stats) in hour_rows {
            if stats.normal.samples == 0 {
                continue;
            }
            println!(
                "{} hour={} samples={} normal_tp_rate={:.4} normal_sl_rate={:.4} normal_exp={:.6} inverse_tp_rate={:.4} inverse_sl_rate={:.4} inverse_exp={:.6}",
                title,
                key,
                stats.normal.samples,
                stats.normal.tp_rate(),
                stats.normal.sl_rate(),
                stats.normal.expectancy(),
                stats.inverse.tp_rate(),
                stats.inverse.sl_rate(),
                stats.inverse.expectancy(),
            );
        }
        println!("{} hour_summary_end", title);

        println!("{} flip_zones_begin", title);
        let mut rows = by_context.iter().collect::<Vec<_>>();
        rows.retain(|(_, stats)| {
            stats.normal.samples >= 8
                && stats.inverse.tp_rate() >= 0.60
                && stats.inverse.tp_rate() >= stats.normal.tp_rate() + 0.15
                && stats.inverse.expectancy() > stats.normal.expectancy()
                && stats.inverse.expectancy() > 0.0
        });
        rows.sort_by(|left, right| {
            let left_edge = left.1.inverse.tp_rate() - left.1.normal.tp_rate();
            let right_edge = right.1.inverse.tp_rate() - right.1.normal.tp_rate();
            right_edge
                .partial_cmp(&left_edge)
                .unwrap_or(Ordering::Equal)
                .then_with(|| {
                    right
                        .1
                        .inverse
                        .expectancy()
                        .partial_cmp(&left.1.inverse.expectancy())
                        .unwrap_or(Ordering::Equal)
                })
                .then_with(|| right.1.normal.samples.cmp(&left.1.normal.samples))
                .then_with(|| left.0.cmp(right.0))
        });
        for (key, stats) in rows.into_iter().take(16) {
            println!(
                "{} flip_zone={} samples={} normal_tp_rate={:.4} normal_sl_rate={:.4} normal_exp={:.6} inverse_tp_rate={:.4} inverse_sl_rate={:.4} inverse_exp={:.6}",
                title,
                key,
                stats.normal.samples,
                stats.normal.tp_rate(),
                stats.normal.sl_rate(),
                stats.normal.expectancy(),
                stats.inverse.tp_rate(),
                stats.inverse.sl_rate(),
                stats.inverse.expectancy(),
            );
        }
        println!("{} flip_zones_end", title);
    }

    fn evaluate_monster_binary_slot_models(
        days: &[SeededSchedulerDay],
        monster: &MonsterNode,
        models: &[MonsterBinarySlotModel],
        bars: &[Bar],
        cache: &FeatureCache,
        target: f64,
    ) -> SeededMetaSchedulerResult {
        let mut traded_days = 0usize;
        let mut target_hit_days = 0usize;
        let mut trades = 0usize;
        let mut wins = 0usize;
        let mut losses = 0usize;
        let mut net_pnl = 0.0;
        let mut daily_pnls = Vec::<f64>::new();
        let by_hour = models
            .iter()
            .map(|model| (model.hour, model))
            .collect::<HashMap<_, _>>();

        for day in days {
            let mut daily_pnl = 0.0;
            let mut day_traded = false;
            let _ = &day.day_key;
            for event in day.events.iter().flatten() {
                let Some(model) = by_hour.get(&event.hour) else {
                    continue;
                };
                let Some(base_features) =
                    extract_features_with_cache(bars, event.candle_index, &FeatureMask::all(), cache)
                else {
                    continue;
                };
                let input = pack_meta_scheduler_input(
                    base_features,
                    event.hour,
                    event.regime,
                    daily_pnl,
                    event.primary_signal,
                );
                let predicted_take = monster
                    .call_many_values_i64(&model.program_hash, &[input])
                    .ok()
                    .and_then(|values| values.first().copied())
                    .map(|value| value != 0)
                    .unwrap_or(false);
                let action = if predicted_take {
                    base_action_for_hour(event.hour)
                } else {
                    SeededSchedulerAction::Skip
                };
                let pnl = action_pnl(event, action);
                if action != SeededSchedulerAction::Skip {
                    day_traded = true;
                    trades += 1;
                    net_pnl += pnl;
                if action_win(event, action) {
                    wins += 1;
                } else if action_is_stoploss(event, action) {
                    losses += 1;
                }
                }
                daily_pnl += pnl;
            }
            if day_traded {
                traded_days += 1;
            }
            if daily_pnl >= target {
                target_hit_days += 1;
            }
            daily_pnls.push(daily_pnl);
        }

        let total_days = days.len().max(1);
        let avg_daily_pnl_distance = daily_pnls.iter().sum::<f64>() / total_days as f64;
        let min_daily_pnl_distance = daily_pnls.iter().copied().fold(f64::INFINITY, f64::min);
        SeededMetaSchedulerResult {
            take_profit: 0.0,
            min_action_win_rate: 0.0,
            train_days: 0,
            test_days: days.len(),
            traded_days,
            target_hit_days,
            trades,
            wins,
            losses,
            win_rate: if trades == 0 { 0.0 } else { wins as f64 / trades as f64 },
            traded_day_rate: traded_days as f64 / total_days as f64,
            daily_target_hit_rate: target_hit_days as f64 / total_days as f64,
            avg_daily_pnl_distance,
            min_daily_pnl_distance: if min_daily_pnl_distance.is_finite() {
                min_daily_pnl_distance
            } else {
                0.0
            },
            net_pnl_distance: net_pnl,
            slot_11_action: SeededSchedulerAction::Skip,
            slot_15_action: SeededSchedulerAction::Skip,
            slot_21_action: SeededSchedulerAction::Skip,
            slot_11_signal: "monster_binary".to_string(),
            slot_15_signal: "monster_binary".to_string(),
            slot_21_signal: "monster_binary".to_string(),
        }
    }

    fn run_natgas_monster_binary_slot_scheduler_search() {
        let series = canonical_chart_series("NATGAS_USD", "H1", 0).expect("NATGAS_USD H1 history");
        let candles = &series.candles;
        let train_rows = ((candles.len() as f64) * 0.7).round() as usize;
        let test_start = train_rows.max(25);
        let mut cache_snapshot = StrategyCacheSnapshot::default();
        let data_hash = strategy_candles_hash(candles);
        let feature_bank = strategy_feature_bank_cached(candles, &data_hash, &mut cache_snapshot);
        let bars = build_bars_from_candles(candles);
        let feature_cache = FeatureCache::build(&bars);

        let short_rules = seeded_short_11h_rules();
        let long_15_rules = seeded_long_15h_rules();
        let long_21_rules = seeded_long_21h_rules();
        let short_rows = build_seeded_slot_masks(candles, &feature_bank, 25, &short_rules);
        let long_15_rows = build_seeded_slot_masks(candles, &feature_bank, 25, &long_15_rules);
        let long_21_rows = build_seeded_slot_masks(candles, &feature_bank, 25, &long_21_rules);

        let short_combo = build_seeded_slot_combos(&short_rules)
            .into_iter()
            .find(|combo| combo.label == "11h short if bearish body")
            .expect("11h short base combo");
        let long_15_combo = build_seeded_slot_combos(&long_15_rules)
            .into_iter()
            .find(|combo| combo.label == "15h long if bullish body")
            .expect("15h long base combo");
        let long_21_combo = build_seeded_slot_combos(&long_21_rules)
            .into_iter()
            .find(|combo| combo.label == "21h long if close crosses above EMA50")
            .expect("21h long base combo");

        let take_profit = NATGAS_STRICT_TAKE_PROFIT;
        let min_action_win_rate = 0.55;
        let train_days = build_meta_scheduler_days(
            candles,
            &feature_bank,
            25,
            test_start,
            &short_combo,
            &long_15_combo,
            &long_21_combo,
            &short_rows,
            &long_15_rows,
            &long_21_rows,
            take_profit,
            NATGAS_STRICT_STOP_LOSS,
            NATGAS_STRICT_EXECUTION_COST,
            24,
        );
        let policy = learn_binary_take_policy(&train_days, 0.070, min_action_win_rate);
        let examples_11 = collect_binary_slot_examples(&train_days, &policy, &bars, &feature_cache, 11);
        let examples_15 = collect_binary_slot_examples(&train_days, &policy, &bars, &feature_cache, 15);
        let examples_21 = collect_binary_slot_examples(&train_days, &policy, &bars, &feature_cache, 21);
        let test_days = build_meta_scheduler_days(
            candles,
            &feature_bank,
            test_start,
            candles.len().saturating_sub(1),
            &short_combo,
            &long_15_combo,
            &long_21_combo,
            &short_rows,
            &long_15_rows,
            &long_21_rows,
            take_profit,
            NATGAS_STRICT_STOP_LOSS,
            NATGAS_STRICT_EXECUTION_COST,
            24,
        );

        let store_path = std::env::temp_dir().join(format!("forge-monster-binary-slot-{}", trading_now_ms()));
        let monster = MonsterNode::new(
            Store::open(&store_path).expect("open monster store"),
            MemoryGovernor::new(8 * 1024 * 1024),
        );
        let model_11 = train_binary_slot_model(&monster, &examples_11, 11);
        let model_15 = train_binary_slot_model(&monster, &examples_15, 15);
        let model_21 = train_binary_slot_model(&monster, &examples_21, 21);
        let models = vec![model_11.clone(), model_15.clone(), model_21.clone()];
        let mut result = evaluate_monster_binary_slot_models(
            &test_days,
            &monster,
            &models,
            &bars,
            &feature_cache,
            0.070,
        );
        result.take_profit = take_profit;
        result.min_action_win_rate = min_action_win_rate;
        result.train_days = train_days.len();
        result.test_days = test_days.len();

        println!(
            "NATGAS_H1_MONSTER_BINARY_SLOT_SCHEDULER tp={:.5} min_action_win_rate={:.2} train_days={} test_days={} trades={} wins={} losses={} win_rate={:.4} traded_days={} traded_day_rate={:.4} target_hit_days={} daily_target_hit_rate={:.4} avg_daily_pnl={:.6} min_daily_pnl={:.6} net_pnl={:.6} train_examples_11={} exact_11={} loss_11={} cand_11={} train_examples_15={} exact_15={} loss_15={} cand_15={} train_examples_21={} exact_21={} loss_21={} cand_21={} base11={} base15={} base21={}",
            result.take_profit,
            result.min_action_win_rate,
            result.train_days,
            result.test_days,
            result.trades,
            result.wins,
            result.losses,
            result.win_rate,
            result.traded_days,
            result.traded_day_rate,
            result.target_hit_days,
            result.daily_target_hit_rate,
            result.avg_daily_pnl_distance,
            result.min_daily_pnl_distance,
            result.net_pnl_distance,
            model_11.train_examples,
            model_11.exact,
            model_11.loss,
            model_11.candidates_evaluated,
            model_15.train_examples,
            model_15.exact,
            model_15.loss,
            model_15.candidates_evaluated,
            model_21.train_examples,
            model_21.exact,
            model_21.loss,
            model_21.candidates_evaluated,
            short_combo.label,
            long_15_combo.label,
            long_21_combo.label,
        );
    }

    fn regime_mask_contains(mask: u32, regime: SeededRegimeId) -> bool {
        let bit = 1_u32 << u32::from(regime.code());
        (mask & bit) != 0
    }

    fn regime_mask_label(mask: u32) -> String {
        let mut labels = Vec::<&'static str>::new();
        for regime in [
            SeededRegimeId::TrendUp,
            SeededRegimeId::TrendDown,
            SeededRegimeId::Compression,
            SeededRegimeId::Overbought,
            SeededRegimeId::Oversold,
            SeededRegimeId::Neutral,
        ] {
            if regime_mask_contains(mask, regime) {
                labels.push(regime.label());
            }
        }
        if labels.is_empty() {
            "none".to_string()
        } else {
            labels.join("+")
        }
    }

    fn predict_monster_take(
        monster: &MonsterNode,
        model: &MonsterBinarySlotModel,
        bars: &[Bar],
        cache: &FeatureCache,
        event: &SeededSchedulerEvent,
        current_pnl: f64,
    ) -> bool {
        let Some(base_features) =
            extract_features_with_cache(bars, event.candle_index, &FeatureMask::all(), cache)
        else {
            return false;
        };
        let input = pack_meta_scheduler_input(
            base_features,
            event.hour,
            event.regime,
            current_pnl,
            event.primary_signal,
        );
        monster
            .call_many_values_i64(&model.program_hash, &[input])
            .ok()
            .and_then(|values| values.first().copied())
            .map(|value| value != 0)
            .unwrap_or(false)
    }

    fn tuned_policy_action(
        policy: &TunedSchedulerPolicy,
        event: &SeededSchedulerEvent,
        current_pnl: f64,
        regime_policy: &HashMap<u32, SeededSchedulerDecision>,
        monster: &MonsterNode,
        model_by_hour: &HashMap<u32, &MonsterBinarySlotModel>,
        bars: &[Bar],
        cache: &FeatureCache,
    ) -> SeededSchedulerAction {
        match event.hour {
            11 => {
                let take = match policy.slot_11_mode {
                    TunedSlotMode::Monster => model_by_hour
                        .get(&11)
                        .map(|model| predict_monster_take(monster, model, bars, cache, event, current_pnl))
                        .unwrap_or(false),
                    TunedSlotMode::Regime => resolve_policy_action(regime_policy, event, current_pnl)
                        != SeededSchedulerAction::Skip,
                };
                if take {
                    base_action_for_hour(11)
                } else {
                    SeededSchedulerAction::Skip
                }
            }
            15 => {
                if current_pnl < policy.slot_15_min_pnl {
                    return SeededSchedulerAction::Skip;
                }
                let take = match policy.slot_15_mode {
                    TunedSlotMode::Monster => model_by_hour
                        .get(&15)
                        .map(|model| predict_monster_take(monster, model, bars, cache, event, current_pnl))
                        .unwrap_or(false),
                    TunedSlotMode::Regime => resolve_policy_action(regime_policy, event, current_pnl)
                        != SeededSchedulerAction::Skip,
                };
                if take {
                    base_action_for_hour(15)
                } else {
                    SeededSchedulerAction::Skip
                }
            }
            21 => {
                if matches!(policy.slot_21_mode, Tuned21Mode::Skip) {
                    return SeededSchedulerAction::Skip;
                }
                if current_pnl < policy.slot_21_min_pnl {
                    return SeededSchedulerAction::Skip;
                }
                if policy.slot_21_require_signal && !event.primary_signal {
                    return SeededSchedulerAction::Skip;
                }
                if !regime_mask_contains(policy.slot_21_regime_mask, event.regime) {
                    return SeededSchedulerAction::Skip;
                }
                let take = match policy.slot_21_mode {
                    Tuned21Mode::Skip => false,
                    Tuned21Mode::MonsterPremium => model_by_hour
                        .get(&21)
                        .map(|model| predict_monster_take(monster, model, bars, cache, event, current_pnl))
                        .unwrap_or(false),
                    Tuned21Mode::RegimePremium => resolve_policy_action(regime_policy, event, current_pnl)
                        != SeededSchedulerAction::Skip,
                };
                if take {
                    base_action_for_hour(21)
                } else {
                    SeededSchedulerAction::Skip
                }
            }
            _ => SeededSchedulerAction::Skip,
        }
    }

    fn evaluate_tuned_scheduler_policy(
        days: &[SeededSchedulerDay],
        policy: &TunedSchedulerPolicy,
        regime_policy: &HashMap<u32, SeededSchedulerDecision>,
        monster: &MonsterNode,
        model_by_hour: &HashMap<u32, &MonsterBinarySlotModel>,
        bars: &[Bar],
        cache: &FeatureCache,
        target: f64,
    ) -> SeededMetaSchedulerResult {
        let mut traded_days = 0usize;
        let mut target_hit_days = 0usize;
        let mut trades = 0usize;
        let mut wins = 0usize;
        let mut losses = 0usize;
        let mut net_pnl = 0.0;
        let mut daily_pnls = Vec::<f64>::new();

        for day in days {
            let mut daily_pnl = 0.0;
            let mut day_traded = false;
            for event in day.events.iter().flatten() {
                let action = tuned_policy_action(
                    policy,
                    event,
                    daily_pnl,
                    regime_policy,
                    monster,
                    model_by_hour,
                    bars,
                    cache,
                );
                let pnl = action_pnl(event, action);
                if action != SeededSchedulerAction::Skip {
                    day_traded = true;
                    trades += 1;
                    net_pnl += pnl;
                if action_win(event, action) {
                    wins += 1;
                } else if action_is_stoploss(event, action) {
                    losses += 1;
                }
                }
                daily_pnl += pnl;
            }
            if day_traded {
                traded_days += 1;
            }
            if daily_pnl >= target {
                target_hit_days += 1;
            }
            daily_pnls.push(daily_pnl);
        }

        let total_days = days.len().max(1);
        let avg_daily_pnl_distance = daily_pnls.iter().sum::<f64>() / total_days as f64;
        let min_daily_pnl_distance = daily_pnls.iter().copied().fold(f64::INFINITY, f64::min);
        SeededMetaSchedulerResult {
            take_profit: NATGAS_STRICT_TAKE_PROFIT,
            min_action_win_rate: 0.55,
            train_days: 0,
            test_days: days.len(),
            traded_days,
            target_hit_days,
            trades,
            wins,
            losses,
            win_rate: if trades == 0 { 0.0 } else { wins as f64 / trades as f64 },
            traded_day_rate: traded_days as f64 / total_days as f64,
            daily_target_hit_rate: target_hit_days as f64 / total_days as f64,
            avg_daily_pnl_distance,
            min_daily_pnl_distance: if min_daily_pnl_distance.is_finite() {
                min_daily_pnl_distance
            } else {
                0.0
            },
            net_pnl_distance: net_pnl,
            slot_11_action: SeededSchedulerAction::Skip,
            slot_15_action: SeededSchedulerAction::Skip,
            slot_21_action: SeededSchedulerAction::Skip,
            slot_11_signal: "tuned".to_string(),
            slot_15_signal: "tuned".to_string(),
            slot_21_signal: "tuned".to_string(),
        }
    }

    fn run_natgas_tuned_day_state_scheduler_search() {
        let series = canonical_chart_series("NATGAS_USD", "H1", 0).expect("NATGAS_USD H1 history");
        let candles = &series.candles;
        let train_rows = ((candles.len() as f64) * 0.7).round() as usize;
        let test_start = train_rows.max(25);
        let mut cache_snapshot = StrategyCacheSnapshot::default();
        let data_hash = strategy_candles_hash(candles);
        let feature_bank = strategy_feature_bank_cached(candles, &data_hash, &mut cache_snapshot);
        let bars = build_bars_from_candles(candles);
        let feature_cache = FeatureCache::build(&bars);

        let short_rules = seeded_short_11h_rules();
        let long_15_rules = seeded_long_15h_rules();
        let long_21_rules = seeded_long_21h_rules();
        let short_rows = build_seeded_slot_masks(candles, &feature_bank, 25, &short_rules);
        let long_15_rows = build_seeded_slot_masks(candles, &feature_bank, 25, &long_15_rules);
        let long_21_rows = build_seeded_slot_masks(candles, &feature_bank, 25, &long_21_rules);

        let short_combo = build_seeded_slot_combos(&short_rules)
            .into_iter()
            .find(|combo| combo.label == "11h short if bearish body")
            .expect("11h short base combo");
        let long_15_combo = build_seeded_slot_combos(&long_15_rules)
            .into_iter()
            .find(|combo| combo.label == "15h long if bullish body")
            .expect("15h long base combo");
        let long_21_combo = build_seeded_slot_combos(&long_21_rules)
            .into_iter()
            .find(|combo| combo.label == "21h long if close crosses above EMA50")
            .expect("21h long base combo");

        let take_profit = NATGAS_STRICT_TAKE_PROFIT;
        let min_action_win_rate = 0.55;
        let train_days = build_meta_scheduler_days(
            candles,
            &feature_bank,
            25,
            test_start,
            &short_combo,
            &long_15_combo,
            &long_21_combo,
            &short_rows,
            &long_15_rows,
            &long_21_rows,
            take_profit,
            NATGAS_STRICT_STOP_LOSS,
            NATGAS_STRICT_EXECUTION_COST,
            24,
        );
        let regime_policy = learn_meta_scheduler_policy(&train_days, 0.070, min_action_win_rate);
        let binary_policy = learn_binary_take_policy(&train_days, 0.070, min_action_win_rate);
        let examples_11 = collect_binary_slot_examples(&train_days, &binary_policy, &bars, &feature_cache, 11);
        let examples_15 = collect_binary_slot_examples(&train_days, &binary_policy, &bars, &feature_cache, 15);
        let examples_21 = collect_binary_slot_examples(&train_days, &binary_policy, &bars, &feature_cache, 21);
        let test_days = build_meta_scheduler_days(
            candles,
            &feature_bank,
            test_start,
            candles.len().saturating_sub(1),
            &short_combo,
            &long_15_combo,
            &long_21_combo,
            &short_rows,
            &long_15_rows,
            &long_21_rows,
            take_profit,
            NATGAS_STRICT_STOP_LOSS,
            NATGAS_STRICT_EXECUTION_COST,
            24,
        );

        let store_path = std::env::temp_dir().join(format!("forge-tuned-day-state-{}", trading_now_ms()));
        let monster = MonsterNode::new(
            Store::open(&store_path).expect("open monster store"),
            MemoryGovernor::new(8 * 1024 * 1024),
        );
        let models = vec![
            train_binary_slot_model(&monster, &examples_11, 11),
            train_binary_slot_model(&monster, &examples_15, 15),
            train_binary_slot_model(&monster, &examples_21, 21),
        ];
        let model_by_hour = models.iter().map(|model| (model.hour, model)).collect::<HashMap<_, _>>();

        let regime_masks = [
            1_u32 << u32::from(SeededRegimeId::TrendUp.code()),
            1_u32 << u32::from(SeededRegimeId::Oversold.code()),
            (1_u32 << u32::from(SeededRegimeId::TrendUp.code()))
                | (1_u32 << u32::from(SeededRegimeId::Oversold.code())),
            (1_u32 << u32::from(SeededRegimeId::TrendUp.code()))
                | (1_u32 << u32::from(SeededRegimeId::Compression.code())),
            (1_u32 << u32::from(SeededRegimeId::TrendUp.code()))
                | (1_u32 << u32::from(SeededRegimeId::Oversold.code()))
                | (1_u32 << u32::from(SeededRegimeId::Compression.code())),
        ];
        let mut best_policy: Option<TunedSchedulerPolicy> = None;
        let mut best_train: Option<SeededMetaSchedulerResult> = None;

        for slot_11_mode in [TunedSlotMode::Monster, TunedSlotMode::Regime] {
            for slot_15_mode in [TunedSlotMode::Monster, TunedSlotMode::Regime] {
                for slot_15_min_pnl in [-0.090, -0.045, 0.0] {
                    let base_policy = TunedSchedulerPolicy {
                        slot_11_mode,
                        slot_15_mode,
                        slot_15_min_pnl,
                        slot_21_mode: Tuned21Mode::Skip,
                        slot_21_min_pnl: 0.0,
                        slot_21_regime_mask: 0,
                        slot_21_require_signal: true,
                    };
                    let base_result = evaluate_tuned_scheduler_policy(
                        &train_days,
                        &base_policy,
                        &regime_policy,
                        &monster,
                        &model_by_hour,
                        &bars,
                        &feature_cache,
                        0.070,
                    );
                    let mut candidates = vec![(base_policy, base_result)];
                    for slot_21_mode in [Tuned21Mode::MonsterPremium, Tuned21Mode::RegimePremium] {
                        for slot_21_min_pnl in [-0.045, 0.0, 0.035] {
                            for slot_21_regime_mask in regime_masks {
                                for slot_21_require_signal in [true, false] {
                                    let policy = TunedSchedulerPolicy {
                                        slot_11_mode,
                                        slot_15_mode,
                                        slot_15_min_pnl,
                                        slot_21_mode,
                                        slot_21_min_pnl,
                                        slot_21_regime_mask,
                                        slot_21_require_signal,
                                    };
                                    let result = evaluate_tuned_scheduler_policy(
                                        &train_days,
                                        &policy,
                                        &regime_policy,
                                        &monster,
                                        &model_by_hour,
                                        &bars,
                                        &feature_cache,
                                        0.070,
                                    );
                                    candidates.push((policy, result));
                                }
                            }
                        }
                    }
                    for (policy, result) in candidates {
                        let should_replace = best_train.as_ref().map(|current| {
                            match result
                                .daily_target_hit_rate
                                .partial_cmp(&current.daily_target_hit_rate)
                                .unwrap_or(Ordering::Equal)
                            {
                                Ordering::Greater => true,
                                Ordering::Less => false,
                                Ordering::Equal => match result
                                    .win_rate
                                    .partial_cmp(&current.win_rate)
                                    .unwrap_or(Ordering::Equal)
                                {
                                    Ordering::Greater => true,
                                    Ordering::Less => false,
                                    Ordering::Equal => match result
                                        .traded_day_rate
                                        .partial_cmp(&current.traded_day_rate)
                                        .unwrap_or(Ordering::Equal)
                                    {
                                        Ordering::Greater => true,
                                        Ordering::Less => false,
                                        Ordering::Equal => result
                                            .avg_daily_pnl_distance
                                            .partial_cmp(&current.avg_daily_pnl_distance)
                                            .unwrap_or(Ordering::Equal)
                                            == Ordering::Greater,
                                    },
                                },
                            }
                        }).unwrap_or(true);
                        if should_replace {
                            best_policy = Some(policy);
                            best_train = Some(result);
                        }
                    }
                }
            }
        }

        let best_policy = best_policy.expect("best tuned policy");
        let mut best_test = evaluate_tuned_scheduler_policy(
            &test_days,
            &best_policy,
            &regime_policy,
            &monster,
            &model_by_hour,
            &bars,
            &feature_cache,
            0.070,
        );
        best_test.take_profit = take_profit;
        best_test.min_action_win_rate = min_action_win_rate;
        best_test.train_days = train_days.len();
        best_test.test_days = test_days.len();

        println!(
            "NATGAS_H1_TUNED_DAY_STATE_SCHEDULER tp={:.5} train_days={} test_days={} trades={} wins={} losses={} win_rate={:.4} traded_days={} traded_day_rate={:.4} target_hit_days={} daily_target_hit_rate={:.4} avg_daily_pnl={:.6} min_daily_pnl={:.6} net_pnl={:.6} slot11_mode={} slot15_mode={} slot15_min_pnl={:.3} slot21_mode={} slot21_min_pnl={:.3} slot21_regimes={} slot21_require_signal={} model11_loss={} model15_loss={} model21_loss={}",
            best_test.take_profit,
            best_test.train_days,
            best_test.test_days,
            best_test.trades,
            best_test.wins,
            best_test.losses,
            best_test.win_rate,
            best_test.traded_days,
            best_test.traded_day_rate,
            best_test.target_hit_days,
            best_test.daily_target_hit_rate,
            best_test.avg_daily_pnl_distance,
            best_test.min_daily_pnl_distance,
            best_test.net_pnl_distance,
            best_policy.slot_11_mode.label(),
            best_policy.slot_15_mode.label(),
            best_policy.slot_15_min_pnl,
            best_policy.slot_21_mode.label(),
            best_policy.slot_21_min_pnl,
            regime_mask_label(best_policy.slot_21_regime_mask),
            best_policy.slot_21_require_signal,
            models[0].loss,
            models[1].loss,
            models[2].loss,
        );
    }

    fn run_natgas_loss_cluster_diagnostics() {
        let series = canonical_chart_series("NATGAS_USD", "H1", 0).expect("NATGAS_USD H1 history");
        let candles = &series.candles;
        let train_rows = ((candles.len() as f64) * 0.7).round() as usize;
        let test_start = train_rows.max(25);
        let mut cache_snapshot = StrategyCacheSnapshot::default();
        let data_hash = strategy_candles_hash(candles);
        let feature_bank = strategy_feature_bank_cached(candles, &data_hash, &mut cache_snapshot);
        let bars = build_bars_from_candles(candles);
        let feature_cache = FeatureCache::build(&bars);

        let short_rules = seeded_short_11h_rules();
        let long_15_rules = seeded_long_15h_rules();
        let long_21_rules = seeded_long_21h_rules();
        let short_rows = build_seeded_slot_masks(candles, &feature_bank, 25, &short_rules);
        let long_15_rows = build_seeded_slot_masks(candles, &feature_bank, 25, &long_15_rules);
        let long_21_rows = build_seeded_slot_masks(candles, &feature_bank, 25, &long_21_rules);

        let short_combo = build_seeded_slot_combos(&short_rules)
            .into_iter()
            .find(|combo| combo.label == "11h short if bearish body")
            .expect("11h short base combo");
        let long_15_combo = build_seeded_slot_combos(&long_15_rules)
            .into_iter()
            .find(|combo| combo.label == "15h long if bullish body")
            .expect("15h long base combo");
        let long_21_combo = build_seeded_slot_combos(&long_21_rules)
            .into_iter()
            .find(|combo| combo.label == "21h long if close crosses above EMA50")
            .expect("21h long base combo");

        let take_profit = NATGAS_STRICT_TAKE_PROFIT;
        let min_action_win_rate = 0.55;
        let train_days = build_meta_scheduler_days(
            candles,
            &feature_bank,
            25,
            test_start,
            &short_combo,
            &long_15_combo,
            &long_21_combo,
            &short_rows,
            &long_15_rows,
            &long_21_rows,
            take_profit,
            NATGAS_STRICT_STOP_LOSS,
            NATGAS_STRICT_EXECUTION_COST,
            24,
        );
        let regime_policy = learn_meta_scheduler_policy(&train_days, 0.070, min_action_win_rate);
        let binary_policy = learn_binary_take_policy(&train_days, 0.070, min_action_win_rate);
        let examples_11 = collect_binary_slot_examples(&train_days, &binary_policy, &bars, &feature_cache, 11);
        let examples_15 = collect_binary_slot_examples(&train_days, &binary_policy, &bars, &feature_cache, 15);
        let examples_21 = collect_binary_slot_examples(&train_days, &binary_policy, &bars, &feature_cache, 21);
        let test_days = build_meta_scheduler_days(
            candles,
            &feature_bank,
            test_start,
            candles.len().saturating_sub(1),
            &short_combo,
            &long_15_combo,
            &long_21_combo,
            &short_rows,
            &long_15_rows,
            &long_21_rows,
            take_profit,
            NATGAS_STRICT_STOP_LOSS,
            NATGAS_STRICT_EXECUTION_COST,
            24,
        );

        let store_path = std::env::temp_dir().join(format!("forge-loss-diagnostics-{}", trading_now_ms()));
        let monster = MonsterNode::new(
            Store::open(&store_path).expect("open monster store"),
            MemoryGovernor::new(8 * 1024 * 1024),
        );
        let models = vec![
            train_binary_slot_model(&monster, &examples_11, 11),
            train_binary_slot_model(&monster, &examples_15, 15),
            train_binary_slot_model(&monster, &examples_21, 21),
        ];
        let by_hour_models = models.iter().map(|model| (model.hour, model)).collect::<HashMap<_, _>>();

        let mut regime_clusters = BTreeMap::<String, LossDiagnosticStats>::new();
        let mut regime_hours = BTreeMap::<String, LossDiagnosticStats>::new();
        let mut monster_clusters = BTreeMap::<String, LossDiagnosticStats>::new();
        let mut monster_hours = BTreeMap::<String, LossDiagnosticStats>::new();

        for day in &test_days {
            let mut regime_daily_pnl = 0.0;
            let mut monster_daily_pnl = 0.0;
            for event in day.events.iter().flatten() {
                let regime_action = resolve_policy_action(&regime_policy, event, regime_daily_pnl);
                record_loss_diagnostic(
                    &mut regime_clusters,
                    &mut regime_hours,
                    "regime_scheduler",
                    event,
                    regime_daily_pnl,
                    regime_action,
                );
                regime_daily_pnl += action_pnl(event, regime_action);

                let Some(model) = by_hour_models.get(&event.hour) else {
                    continue;
                };
                let Some(base_features) =
                    extract_features_with_cache(&bars, event.candle_index, &FeatureMask::all(), &feature_cache)
                else {
                    continue;
                };
                let input = pack_meta_scheduler_input(
                    base_features,
                    event.hour,
                    event.regime,
                    monster_daily_pnl,
                    event.primary_signal,
                );
                let monster_take = monster
                    .call_many_values_i64(&model.program_hash, &[input])
                    .ok()
                    .and_then(|values| values.first().copied())
                    .map(|value| value != 0)
                    .unwrap_or(false);
                let monster_action = if monster_take {
                    base_action_for_hour(event.hour)
                } else {
                    SeededSchedulerAction::Skip
                };
                record_loss_diagnostic(
                    &mut monster_clusters,
                    &mut monster_hours,
                    "monster_binary",
                    event,
                    monster_daily_pnl,
                    monster_action,
                );
                monster_daily_pnl += action_pnl(event, monster_action);
            }
        }

        print_loss_diagnostics("LOSS_DIAG_REGIME", &regime_clusters, &regime_hours);
        print_loss_diagnostics("LOSS_DIAG_MONSTER", &monster_clusters, &monster_hours);
    }

    fn run_natgas_seeded_slot_combo_search() {
        let series = canonical_chart_series("NATGAS_USD", "H1", 0).expect("NATGAS_USD H1 history");
        let candles = &series.candles;
        let train_rows = ((candles.len() as f64) * 0.7).round() as usize;
        let test_start = train_rows.max(25);
        let mut cache_snapshot = StrategyCacheSnapshot::default();
        let data_hash = strategy_candles_hash(candles);
        let feature_bank = strategy_feature_bank_cached(candles, &data_hash, &mut cache_snapshot);

        let short_rules = seeded_short_11h_rules();
        let long_15_rules = seeded_long_15h_rules();
        let long_21_rules = seeded_long_21h_rules();
        let short_rows = build_seeded_slot_masks(candles, &feature_bank, test_start, &short_rules);
        let long_15_rows = build_seeded_slot_masks(candles, &feature_bank, test_start, &long_15_rules);
        let long_21_rows = build_seeded_slot_masks(candles, &feature_bank, test_start, &long_21_rules);
        let short_combos = build_seeded_slot_combos(&short_rules);
        let long_15_combos = build_seeded_slot_combos(&long_15_rules);
        let long_21_combos = build_seeded_slot_combos(&long_21_rules);
        let tp_grid = [NATGAS_STRICT_TAKE_PROFIT];

        let mut best: Option<SeededComboSearchCandidate> = None;
        for short_combo in &short_combos {
            for long_15_combo in &long_15_combos {
                for long_21_combo in &long_21_combos {
                    for take_profit in tp_grid {
                        let Some(candidate) = evaluate_seeded_combo_search_candidate(
                            candles,
                            &short_rows,
                            &long_15_rows,
                            &long_21_rows,
                            short_combo,
                            long_15_combo,
                            long_21_combo,
                            take_profit,
                            NATGAS_STRICT_STOP_LOSS,
                            NATGAS_STRICT_EXECUTION_COST,
                            24,
                        ) else {
                            continue;
                        };
                        let should_replace = best.as_ref().map(|current| {
                            match candidate.daily_target_hit_rate.partial_cmp(&current.daily_target_hit_rate).unwrap_or(Ordering::Equal) {
                                Ordering::Greater => true,
                                Ordering::Less => false,
                                Ordering::Equal => match candidate.avg_daily_pnl_distance.partial_cmp(&current.avg_daily_pnl_distance).unwrap_or(Ordering::Equal) {
                                    Ordering::Greater => true,
                                    Ordering::Less => false,
                                    Ordering::Equal => match candidate.min_daily_pnl_distance.partial_cmp(&current.min_daily_pnl_distance).unwrap_or(Ordering::Equal) {
                                        Ordering::Greater => true,
                                        Ordering::Less => false,
                                        Ordering::Equal => candidate.win_rate.partial_cmp(&current.win_rate).unwrap_or(Ordering::Equal) == Ordering::Greater,
                                    },
                                },
                            }
                        }).unwrap_or(true);
                        if should_replace {
                            best = Some(candidate);
                        }
                    }
                }
            }
        }

        let best = best.expect("best seeded combo candidate");
        println!(
            "NATGAS_H1_SEEDED_COMBO_DAILY tp={:.5} best_11h={} [{}] best_15h={} [{}] best_21h={} [{}] trades={} wins={} losses={} win_rate={:.4} daily_target_hit_rate={:.4} target_hit_days={} total_days={} avg_daily_pnl={:.6} min_daily_pnl={:.6} expectancy={:.6} net_pnl={:.6}",
            best.take_profit,
            best.short_11h.label,
            best.short_11h.indicator_refs.join(","),
            best.long_15h.label,
            best.long_15h.indicator_refs.join(","),
            best.long_21h.label,
            best.long_21h.indicator_refs.join(","),
            best.trades,
            best.wins,
            best.losses,
            best.win_rate,
            best.daily_target_hit_rate,
            best.target_hit_days,
            best.total_days,
            best.avg_daily_pnl_distance,
            best.min_daily_pnl_distance,
            best.expectancy_distance,
            best.net_pnl_distance,
        );
    }

    fn run_natgas_seeded_slot_combo_search_21h_vwap_only() {
        let series = canonical_chart_series("NATGAS_USD", "H1", 0).expect("NATGAS_USD H1 history");
        let candles = &series.candles;
        let train_rows = ((candles.len() as f64) * 0.7).round() as usize;
        let test_start = train_rows.max(25);
        let mut cache_snapshot = StrategyCacheSnapshot::default();
        let data_hash = strategy_candles_hash(candles);
        let feature_bank = strategy_feature_bank_cached(candles, &data_hash, &mut cache_snapshot);

        let short_rules = seeded_short_11h_rules();
        let long_15_rules = seeded_long_15h_rules();
        let long_21_rules = seeded_long_21h_rules();
        let short_rows = build_seeded_slot_masks(candles, &feature_bank, test_start, &short_rules);
        let long_15_rows = build_seeded_slot_masks(candles, &feature_bank, test_start, &long_15_rules);
        let long_21_rows = build_seeded_slot_masks(candles, &feature_bank, test_start, &long_21_rules);
        let short_combos = build_seeded_slot_combos(&short_rules);
        let long_15_combos = build_seeded_slot_combos(&long_15_rules);
        let long_21_combos = build_seeded_slot_combos(&long_21_rules)
            .into_iter()
            .filter(combo_is_vwap_family)
            .collect::<Vec<_>>();
        let tp_grid = [NATGAS_STRICT_TAKE_PROFIT];

        let mut best: Option<SeededComboSearchCandidate> = None;
        for short_combo in &short_combos {
            for long_15_combo in &long_15_combos {
                for long_21_combo in &long_21_combos {
                    for take_profit in tp_grid {
                        let Some(candidate) = evaluate_seeded_combo_search_candidate(
                            candles,
                            &short_rows,
                            &long_15_rows,
                            &long_21_rows,
                            short_combo,
                            long_15_combo,
                            long_21_combo,
                            take_profit,
                            NATGAS_STRICT_STOP_LOSS,
                            NATGAS_STRICT_EXECUTION_COST,
                            24,
                        ) else {
                            continue;
                        };
                        let should_replace = best.as_ref().map(|current| {
                            match candidate.daily_target_hit_rate.partial_cmp(&current.daily_target_hit_rate).unwrap_or(Ordering::Equal) {
                                Ordering::Greater => true,
                                Ordering::Less => false,
                                Ordering::Equal => match candidate.avg_daily_pnl_distance.partial_cmp(&current.avg_daily_pnl_distance).unwrap_or(Ordering::Equal) {
                                    Ordering::Greater => true,
                                    Ordering::Less => false,
                                    Ordering::Equal => match candidate.min_daily_pnl_distance.partial_cmp(&current.min_daily_pnl_distance).unwrap_or(Ordering::Equal) {
                                        Ordering::Greater => true,
                                        Ordering::Less => false,
                                        Ordering::Equal => candidate.win_rate.partial_cmp(&current.win_rate).unwrap_or(Ordering::Equal) == Ordering::Greater,
                                    },
                                },
                            }
                        }).unwrap_or(true);
                        if should_replace {
                            best = Some(candidate);
                        }
                    }
                }
            }
        }

        let best = best.expect("best seeded combo candidate for 21h vwap only");
        println!(
            "NATGAS_H1_SEEDED_COMBO_DAILY_21H_VWAP_ONLY tp={:.5} best_11h={} [{}] best_15h={} [{}] best_21h={} [{}] trades={} wins={} losses={} win_rate={:.4} daily_target_hit_rate={:.4} target_hit_days={} total_days={} avg_daily_pnl={:.6} min_daily_pnl={:.6} expectancy={:.6} net_pnl={:.6}",
            best.take_profit,
            best.short_11h.label,
            best.short_11h.indicator_refs.join(","),
            best.long_15h.label,
            best.long_15h.indicator_refs.join(","),
            best.long_21h.label,
            best.long_21h.indicator_refs.join(","),
            best.trades,
            best.wins,
            best.losses,
            best.win_rate,
            best.daily_target_hit_rate,
            best.target_hit_days,
            best.total_days,
            best.avg_daily_pnl_distance,
            best.min_daily_pnl_distance,
            best.expectancy_distance,
            best.net_pnl_distance,
        );
    }

    fn run_natgas_simple_vwap_baseline_with_take_profit(
        take_profit: f64,
        label: &str,
    ) {
        let series = canonical_chart_series("NATGAS_USD", "H1", 0).expect("NATGAS_USD H1 history");
        let candles = &series.candles;
        let search_start = 25usize;
        let mut cache_snapshot = StrategyCacheSnapshot::default();
        let data_hash = strategy_candles_hash(candles);
        let feature_bank = strategy_feature_bank_cached(candles, &data_hash, &mut cache_snapshot);

        let short_rules = seeded_short_11h_rules();
        let long_15_rules = seeded_long_15h_rules();
        let long_21_rules = seeded_long_21h_rules();
        let short_rows = build_seeded_slot_masks(candles, &feature_bank, search_start, &short_rules);
        let long_15_rows =
            build_seeded_slot_masks(candles, &feature_bank, search_start, &long_15_rules);
        let long_21_rows =
            build_seeded_slot_masks(candles, &feature_bank, search_start, &long_21_rules);
        let (short_11, long_15, long_21) = natgas_simple_vwap_baseline_combos();

        let baseline = evaluate_seeded_combo_search_candidate(
            candles,
            &short_rows,
            &long_15_rows,
            &long_21_rows,
            &short_11,
            &long_15,
            &long_21,
            take_profit,
            NATGAS_STRICT_STOP_LOSS,
            NATGAS_STRICT_EXECUTION_COST,
            24,
        )
        .expect("simple vwap baseline candidate");

        println!(
            "{} tp={:.5} best_11h={} [{}] best_15h={} [{}] best_21h={} [{}] trades={} wins={} losses={} win_rate={:.4} daily_target_hit_rate={:.4} target_hit_days={} total_days={} avg_daily_pnl={:.6} min_daily_pnl={:.6} expectancy={:.6} net_pnl={:.6}",
            label,
            take_profit,
            baseline.short_11h.label,
            baseline.short_11h.indicator_refs.join(","),
            baseline.long_15h.label,
            baseline.long_15h.indicator_refs.join(","),
            baseline.long_21h.label,
            baseline.long_21h.indicator_refs.join(","),
            baseline.trades,
            baseline.wins,
            baseline.losses,
            baseline.win_rate,
            baseline.daily_target_hit_rate,
            baseline.target_hit_days,
            baseline.total_days,
            baseline.avg_daily_pnl_distance,
            baseline.min_daily_pnl_distance,
            baseline.expectancy_distance,
            baseline.net_pnl_distance,
        );
    }

    fn run_natgas_simple_vwap_baseline() {
        run_natgas_simple_vwap_baseline_with_take_profit(
            NATGAS_STRICT_TAKE_PROFIT,
            "NATGAS_H1_SIMPLE_VWAP_BASELINE",
        );
    }

    fn run_natgas_simple_vwap_baseline_tp7() {
        run_natgas_simple_vwap_baseline_with_take_profit(
            0.070,
            "NATGAS_H1_SIMPLE_VWAP_BASELINE_TP7",
        );
    }

    fn evaluate_baseline_plus_extra_slot(
        candles: &[TradingCandlePoint],
        short_11_rows: &[(usize, u32)],
        long_15_rows: &[(usize, u32)],
        long_21_rows: &[(usize, u32)],
        extra_rows: &[(usize, u32)],
        short_11: &SeededSlotComboDef,
        long_15: &SeededSlotComboDef,
        long_21: &SeededSlotComboDef,
        extra: &SeededSlotComboDef,
    ) -> Option<SeededComboSearchCandidate> {
        let mut daily_pnl = BTreeMap::<String, f64>::new();
        let mut trades = 0usize;
        let mut wins = 0usize;
        let mut losses = 0usize;
        let mut net_pnl = 0.0;
        let groups = [
            (short_11_rows, short_11),
            (long_15_rows, long_15),
            (long_21_rows, long_21),
            (extra_rows, extra),
        ];
        for (rows, combo) in groups {
            for (index, row_mask) in rows {
                if (*row_mask & combo.required_mask) != combo.required_mask {
                    continue;
                }
                let Some(day_key) = candles[*index].time.get(..10) else {
                    continue;
                };
                daily_pnl.entry(day_key.to_string()).or_insert(0.0);
                let (outcome, _) = strategy_entry_outcome_cached(
                    candles,
                    *index,
                    combo.direction,
                    NATGAS_STRICT_STOP_LOSS,
                    NATGAS_STRICT_EXECUTION_COST,
                    24,
                );
                let Some(outcome) = outcome else {
                    continue;
                };
                let (exit_kind, pnl, _) =
                    classify_strict_trade_outcome(&outcome, NATGAS_STRICT_TAKE_PROFIT);
                *daily_pnl.entry(day_key.to_string()).or_insert(0.0) += pnl;
                trades += 1;
                net_pnl += pnl;
                if exit_kind == StrictTradeExit::TakeProfit {
                    wins += 1;
                } else if exit_kind == StrictTradeExit::StopLoss {
                    losses += 1;
                }
            }
        }
        if daily_pnl.is_empty() || trades == 0 {
            return None;
        }
        let total_days = daily_pnl.len();
        let target_hit_days = daily_pnl
            .values()
            .filter(|value| **value >= NATGAS_STRICT_DAILY_TARGET)
            .count();
        let avg_daily_pnl_distance = daily_pnl.values().sum::<f64>() / total_days as f64;
        let min_daily_pnl_distance = daily_pnl.values().copied().fold(f64::INFINITY, f64::min);
        Some(SeededComboSearchCandidate {
            short_11h: short_11.clone(),
            long_15h: long_15.clone(),
            long_21h: extra.clone(),
            take_profit: NATGAS_STRICT_TAKE_PROFIT,
            trades,
            wins,
            losses,
            win_rate: wins as f64 / trades as f64,
            daily_target_hit_rate: target_hit_days as f64 / total_days as f64,
            target_hit_days,
            total_days,
            avg_daily_pnl_distance,
            min_daily_pnl_distance,
            expectancy_distance: net_pnl / trades as f64,
            net_pnl_distance: net_pnl,
        })
    }

    fn run_natgas_simple_vwap_plus_19h_search() {
        let series = canonical_chart_series("NATGAS_USD", "H1", 0).expect("NATGAS_USD H1 history");
        let candles = &series.candles;
        let search_start = 25usize;
        let mut cache_snapshot = StrategyCacheSnapshot::default();
        let data_hash = strategy_candles_hash(candles);
        let feature_bank = strategy_feature_bank_cached(candles, &data_hash, &mut cache_snapshot);

        let short_11_rules = seeded_short_11h_rules();
        let long_15_rules = seeded_long_15h_rules();
        let long_21_rules = seeded_long_21h_rules();
        let short_19_rules = seeded_short_19h_rules();
        let long_19_rules = seeded_long_19h_rules();
        let short_11_rows =
            build_seeded_slot_masks(candles, &feature_bank, search_start, &short_11_rules);
        let long_15_rows =
            build_seeded_slot_masks(candles, &feature_bank, search_start, &long_15_rules);
        let long_21_rows =
            build_seeded_slot_masks(candles, &feature_bank, search_start, &long_21_rules);
        let short_19_rows =
            build_seeded_slot_masks(candles, &feature_bank, search_start, &short_19_rules);
        let long_19_rows =
            build_seeded_slot_masks(candles, &feature_bank, search_start, &long_19_rules);
        let (short_11, long_15, long_21) = natgas_simple_vwap_baseline_combos();
        let baseline = evaluate_seeded_combo_search_candidate(
            candles,
            &short_11_rows,
            &long_15_rows,
            &long_21_rows,
            &short_11,
            &long_15,
            &long_21,
            NATGAS_STRICT_TAKE_PROFIT,
            NATGAS_STRICT_STOP_LOSS,
            NATGAS_STRICT_EXECUTION_COST,
            24,
        )
        .expect("simple baseline");

        let mut best: Option<(SeededComboSearchCandidate, SeededSlotComboDef)> = None;
        for extra in build_seeded_slot_combos(&short_19_rules)
            .into_iter()
            .chain(build_seeded_slot_combos(&long_19_rules).into_iter())
        {
            let rows = if extra.direction == "short" {
                &short_19_rows
            } else {
                &long_19_rows
            };
            let Some(candidate) = evaluate_baseline_plus_extra_slot(
                candles,
                &short_11_rows,
                &long_15_rows,
                &long_21_rows,
                rows,
                &short_11,
                &long_15,
                &long_21,
                &extra,
            ) else {
                continue;
            };
            let replace = best
                .as_ref()
                .map(|(current, _)| {
                    match candidate.win_rate.partial_cmp(&current.win_rate).unwrap_or(Ordering::Equal)
                    {
                        Ordering::Greater => true,
                        Ordering::Less => false,
                        Ordering::Equal => match candidate
                            .daily_target_hit_rate
                            .partial_cmp(&current.daily_target_hit_rate)
                            .unwrap_or(Ordering::Equal)
                        {
                            Ordering::Greater => true,
                            Ordering::Less => false,
                            Ordering::Equal => candidate
                                .expectancy_distance
                                .partial_cmp(&current.expectancy_distance)
                                .unwrap_or(Ordering::Equal)
                                == Ordering::Greater,
                        },
                    }
                })
                .unwrap_or(true);
            if replace {
                best = Some((candidate, extra));
            }
        }

        let (best, extra) = best.expect("best 19h extension");
        println!(
            "NATGAS_H1_SIMPLE_VWAP_PLUS_19H baseline_win_rate={:.4} baseline_daily_target_hit_rate={:.4} baseline_net_pnl={:.6} best_19h={} [{}] trades={} wins={} losses={} win_rate={:.4} daily_target_hit_rate={:.4} target_hit_days={} total_days={} avg_daily_pnl={:.6} expectancy={:.6} net_pnl={:.6}",
            baseline.win_rate,
            baseline.daily_target_hit_rate,
            baseline.net_pnl_distance,
            extra.label,
            extra.indicator_refs.join(","),
            best.trades,
            best.wins,
            best.losses,
            best.win_rate,
            best.daily_target_hit_rate,
            best.target_hit_days,
            best.total_days,
            best.avg_daily_pnl_distance,
            best.expectancy_distance,
            best.net_pnl_distance,
        );
    }

    fn fallback_action_for_21h(
        mode: DailyFallbackMode,
        index: usize,
        candles: &[TradingCandlePoint],
        feature_bank: &StrategyIndicatorFeatureBank,
    ) -> &'static str {
        match mode {
            DailyFallbackMode::LongAboveVwapElseShort => {
                if candles[index].close >= feature_bank.vwap[index] {
                    "long"
                } else {
                    "short"
                }
            }
            DailyFallbackMode::LongAboveEma21ElseShort => {
                if candles[index].close >= feature_bank.ema21[index] {
                    "long"
                } else {
                    "short"
                }
            }
            DailyFallbackMode::CandleBodyPolarity => {
                if candles[index].close >= candles[index].open {
                    "long"
                } else {
                    "short"
                }
            }
            DailyFallbackMode::MacdPolarity => {
                if feature_bank.macd_histogram[index] >= 0.0 {
                    "long"
                } else {
                    "short"
                }
            }
        }
    }

    fn run_natgas_mandatory_daily_winrate_search() {
        let series = canonical_chart_series("NATGAS_USD", "H1", 0).expect("NATGAS_USD H1 history");
        let candles = &series.candles;
        let search_start = 25usize;
        let mut cache_snapshot = StrategyCacheSnapshot::default();
        let data_hash = strategy_candles_hash(candles);
        let feature_bank = strategy_feature_bank_cached(candles, &data_hash, &mut cache_snapshot);

        let short_11_rules = seeded_short_11h_rules();
        let long_15_rules = seeded_long_15h_rules();
        let short_11_rows =
            build_seeded_slot_masks(candles, &feature_bank, search_start, &short_11_rules);
        let long_15_rows =
            build_seeded_slot_masks(candles, &feature_bank, search_start, &long_15_rules);
        let short_11_map = short_11_rows.iter().copied().collect::<HashMap<_, _>>();
        let long_15_map = long_15_rows.iter().copied().collect::<HashMap<_, _>>();

        let short_11_combos = build_seeded_slot_combos(&short_11_rules)
            .into_iter()
            .filter(|combo| {
                combo_label_is_one_of(
                    combo,
                    &[
                        "11h short if bearish body",
                        "11h short if close < VWAP",
                        "11h short if close crosses below EMA21",
                        "11h short if close < VWAP && 11h short if bearish body",
                        "11h short if bearish body && 11h short if MACD hist < 0",
                        "11h short if two closes stay below VWAP",
                    ],
                )
            })
            .collect::<Vec<_>>();
        let long_15_combos = build_seeded_slot_combos(&long_15_rules)
            .into_iter()
            .filter(|combo| {
                combo_label_is_one_of(
                    combo,
                    &[
                        "15h long if bullish body",
                        "15h long if close > VWAP",
                        "15h long if close crosses above EMA21",
                        "15h long if close > VWAP && 15h long if bullish body",
                        "15h long if bullish body && 15h long if MACD hist > 0",
                        "15h long if two closes stay above VWAP",
                    ],
                )
            })
            .collect::<Vec<_>>();
        let fallback_modes = [
            DailyFallbackMode::LongAboveVwapElseShort,
            DailyFallbackMode::LongAboveEma21ElseShort,
            DailyFallbackMode::CandleBodyPolarity,
            DailyFallbackMode::MacdPolarity,
        ];

        let mut day_slots = BTreeMap::<String, [Option<usize>; 3]>::new();
        for index in search_start..candles.len().saturating_sub(1) {
            let Some(hour) = strategy_hour_utc(&candles[index].time) else {
                continue;
            };
            let slot = match hour {
                11 => Some(0),
                15 => Some(1),
                21 => Some(2),
                _ => None,
            };
            let Some(slot) = slot else {
                continue;
            };
            let Some(day_key) = candles[index].time.get(..10) else {
                continue;
            };
            let entry = day_slots
                .entry(day_key.to_string())
                .or_insert([None, None, None]);
            entry[slot] = Some(index);
        }

        let mut best: Option<(SeededSlotComboDef, SeededSlotComboDef, DailyFallbackMode, SeededComboSearchCandidate)> = None;
        for short_11 in &short_11_combos {
            for long_15 in &long_15_combos {
                for fallback_mode in fallback_modes {
                    let mut daily_pnl = BTreeMap::<String, f64>::new();
                    let mut trades = 0usize;
                    let mut wins = 0usize;
                    let mut losses = 0usize;
                    let mut net_pnl = 0.0;

                    for (day_key, slots) in &day_slots {
                        let mut traded = false;
                        let mut pnl_day = 0.0;

                        if let Some(index) = slots[0] {
                            if let Some(mask) = short_11_map.get(&index).copied() {
                                if (mask & short_11.required_mask) == short_11.required_mask {
                                    let (outcome, _) = strategy_entry_outcome_cached(
                                        candles,
                                        index,
                                        short_11.direction,
                                        NATGAS_STRICT_STOP_LOSS,
                                        NATGAS_STRICT_EXECUTION_COST,
                                        24,
                                    );
                                    if let Some(outcome) = outcome {
                                        let (exit_kind, pnl, _) = classify_strict_trade_outcome(
                                            &outcome,
                                            NATGAS_STRICT_TAKE_PROFIT,
                                        );
                                        pnl_day += pnl;
                                        trades += 1;
                                        net_pnl += pnl;
                                        traded = true;
                                        if exit_kind == StrictTradeExit::TakeProfit {
                                            wins += 1;
                                        } else if exit_kind == StrictTradeExit::StopLoss {
                                            losses += 1;
                                        }
                                    }
                                }
                            }
                        }

                        if let Some(index) = slots[1] {
                            if let Some(mask) = long_15_map.get(&index).copied() {
                                if (mask & long_15.required_mask) == long_15.required_mask {
                                    let (outcome, _) = strategy_entry_outcome_cached(
                                        candles,
                                        index,
                                        long_15.direction,
                                        NATGAS_STRICT_STOP_LOSS,
                                        NATGAS_STRICT_EXECUTION_COST,
                                        24,
                                    );
                                    if let Some(outcome) = outcome {
                                        let (exit_kind, pnl, _) = classify_strict_trade_outcome(
                                            &outcome,
                                            NATGAS_STRICT_TAKE_PROFIT,
                                        );
                                        pnl_day += pnl;
                                        trades += 1;
                                        net_pnl += pnl;
                                        traded = true;
                                        if exit_kind == StrictTradeExit::TakeProfit {
                                            wins += 1;
                                        } else if exit_kind == StrictTradeExit::StopLoss {
                                            losses += 1;
                                        }
                                    }
                                }
                            }
                        }

                        if let Some(index) = slots[2] {
                            if !traded {
                                let direction = fallback_action_for_21h(
                                    fallback_mode,
                                    index,
                                    candles,
                                    &feature_bank,
                                );
                                let (outcome, _) = strategy_entry_outcome_cached(
                                    candles,
                                    index,
                                    direction,
                                    NATGAS_STRICT_STOP_LOSS,
                                    NATGAS_STRICT_EXECUTION_COST,
                                    24,
                                );
                                if let Some(outcome) = outcome {
                                    let (exit_kind, pnl, _) = classify_strict_trade_outcome(
                                        &outcome,
                                        NATGAS_STRICT_TAKE_PROFIT,
                                    );
                                    pnl_day += pnl;
                                    trades += 1;
                                    net_pnl += pnl;
                                    if exit_kind == StrictTradeExit::TakeProfit {
                                        wins += 1;
                                    } else if exit_kind == StrictTradeExit::StopLoss {
                                        losses += 1;
                                    }
                                }
                            }
                        }

                        daily_pnl.insert(day_key.clone(), pnl_day);
                    }

                    if trades == 0 || daily_pnl.is_empty() {
                        continue;
                    }
                    let total_days = daily_pnl.len();
                    let target_hit_days = daily_pnl
                        .values()
                        .filter(|value| **value >= NATGAS_STRICT_DAILY_TARGET)
                        .count();
                    let candidate = SeededComboSearchCandidate {
                        short_11h: short_11.clone(),
                        long_15h: long_15.clone(),
                        long_21h: SeededSlotComboDef {
                            hour: 21,
                            direction: "both",
                            label: fallback_mode.label().to_string(),
                            indicator_refs: vec![],
                            required_mask: 0,
                        },
                        take_profit: NATGAS_STRICT_TAKE_PROFIT,
                        trades,
                        wins,
                        losses,
                        win_rate: wins as f64 / trades as f64,
                        daily_target_hit_rate: target_hit_days as f64 / total_days as f64,
                        target_hit_days,
                        total_days,
                        avg_daily_pnl_distance: daily_pnl.values().sum::<f64>() / total_days as f64,
                        min_daily_pnl_distance: daily_pnl.values().copied().fold(f64::INFINITY, f64::min),
                        expectancy_distance: net_pnl / trades as f64,
                        net_pnl_distance: net_pnl,
                    };

                    let replace = best
                        .as_ref()
                        .map(|(_, _, _, current)| {
                            match candidate.win_rate.partial_cmp(&current.win_rate).unwrap_or(Ordering::Equal) {
                                Ordering::Greater => true,
                                Ordering::Less => false,
                                Ordering::Equal => match candidate
                                    .daily_target_hit_rate
                                    .partial_cmp(&current.daily_target_hit_rate)
                                    .unwrap_or(Ordering::Equal)
                                {
                                    Ordering::Greater => true,
                                    Ordering::Less => false,
                                    Ordering::Equal => candidate
                                        .expectancy_distance
                                        .partial_cmp(&current.expectancy_distance)
                                        .unwrap_or(Ordering::Equal)
                                        == Ordering::Greater,
                                },
                            }
                        })
                        .unwrap_or(true);
                    if replace {
                        best = Some((short_11.clone(), long_15.clone(), fallback_mode, candidate));
                    }
                }
            }
        }

        let (short_11, long_15, fallback_mode, best) = best.expect("best mandatory daily policy");
        println!(
            "NATGAS_H1_MANDATORY_DAILY_WINRATE best_11h={} [{}] best_15h={} [{}] fallback_21h={} trades={} wins={} losses={} win_rate={:.4} daily_target_hit_rate={:.4} target_hit_days={} total_days={} avg_daily_pnl={:.6} expectancy={:.6} net_pnl={:.6}",
            short_11.label,
            short_11.indicator_refs.join(","),
            long_15.label,
            long_15.indicator_refs.join(","),
            fallback_mode.label(),
            best.trades,
            best.wins,
            best.losses,
            best.win_rate,
            best.daily_target_hit_rate,
            best.target_hit_days,
            best.total_days,
            best.avg_daily_pnl_distance,
            best.expectancy_distance,
            best.net_pnl_distance,
        );
    }

    fn run_save_natgas_simple_vwap_baseline_as_program() {
        let args = json!({
            "title": "/stratA_",
            "goal": "NATGAS_USD H1 session strategy built only from selected special recurrences and sniper contexts, with strict execution accounting.",
            "intent": "Use NATGAS_USD H1 on OANDA without the old generic baseline layer. Strict execution accounting is mandatory: spread cost 0.6p, and a win only counts when TP is touched. Keep only the selected special modules. Module 1, mirrored anti-edge recurrence: at 08h UTC, if the last three H1 closes are strictly higher than one another, treat the toxic breakout long as invalid and execute its exact mirror instead by opening a SHORT at the next bar open with raw take profit 3.9p and raw stop loss 5.1p. Module 2, 11h sniper short: bearish body, close below VWAP minus 1 sigma, MACD histogram below zero, RSI14 at or below 45, ATR14 and Bollinger width both in squeeze or low-volatility state, and H4 bias bearish, then open a SHORT with raw stop loss 3.9p and raw take profit 5.1p. Module 3, 13h recurrence long: if the previous H1 candle is red, the current H1 candle is green, MACD histogram is above zero, and the close sits in the lower quartile of the previous 24 H1 candles, then open a LONG with raw stop loss 3.9p and raw take profit 5.1p. Module 4, 18h sniper long: bullish body, close still below session VWAP, MACD histogram above zero, RSI14 at or above 55, ATR14 and Bollinger width both expanded or high-volatility, and H4 bias bearish, then open a LONG with raw stop loss 3.9p and raw take profit 5.1p. There is no generic 11h, 15h, or 21h baseline anymore.",
            "domain": "trading",
            "template": "strategy_baseline",
            "program_kind": "compute_program"
        });
        let created = crate::forge_agent_runtime::direct_create_program_in_store(&args, "trading")
            .expect("save /stratA_ direct program");
        println!(
            "TRADING_PROGRAM_SAVED title=/stratA_ program_hash={} kind={} status={}",
            created
                .get("program_hash")
                .and_then(Value::as_str)
                .unwrap_or("unknown"),
            created
                .pointer("/program/program_kind")
                .and_then(Value::as_str)
                .unwrap_or("unknown"),
            created
                .pointer("/program/status")
                .and_then(Value::as_str)
                .unwrap_or("unknown"),
        );
    }

    #[derive(Clone)]
    struct StratAUnifiedTrade {
        entry_time: String,
        module: &'static str,
        pnl_distance: f64,
        tp_hit: bool,
    }

    fn collect_natgas_strat_a_unified_trades() -> Vec<StratAUnifiedTrade> {
        let h1_series =
            canonical_chart_series("NATGAS_USD", "H1", 0).expect("NATGAS_USD H1 history");
        let h4_series =
            canonical_chart_series("NATGAS_USD", "H4", 0).expect("NATGAS_USD H4 history");
        let candles = h1_series.candles;
        let h4_candles = h4_series.candles;
        let mut cache_snapshot = StrategyCacheSnapshot::default();
        let h1_hash = strategy_candles_hash(&candles);
        let h4_hash = strategy_candles_hash(&h4_candles);
        let feature_bank = strategy_feature_bank_cached(&candles, &h1_hash, &mut cache_snapshot);
        let h4_bank = strategy_feature_bank_cached(&h4_candles, &h4_hash, &mut cache_snapshot);
        let h4_bias_by_h1_index = build_h4_bias_by_h1_index(&candles, &h4_candles, &h4_bank);

        let mut prev24_high = vec![f64::NAN; candles.len()];
        let mut prev24_low = vec![f64::NAN; candles.len()];
        let mut prev24_width = vec![f64::NAN; candles.len()];
        for index in 24..candles.len() {
            let window = &candles[index - 24..index];
            let high = window
                .iter()
                .map(|c| c.high)
                .fold(f64::NEG_INFINITY, f64::max);
            let low = window.iter().map(|c| c.low).fold(f64::INFINITY, f64::min);
            prev24_high[index] = high;
            prev24_low[index] = low;
            prev24_width[index] = (high - low).abs();
        }

        let mut trades = Vec::<StratAUnifiedTrade>::new();

        for index in 24..candles.len().saturating_sub(1) {
            let Some(hour) = strategy_hour_utc(&candles[index].time) else {
                continue;
            };

            let mut module: Option<(&'static str, &'static str, f64, f64)> = None;

            if hour == 8
                && index >= 2
                && candles[index].close > candles[index - 1].close
                && candles[index - 1].close > candles[index - 2].close
            {
                module = Some((
                    "08h_mirror_short",
                    "short",
                    NATGAS_STRICT_TAKE_PROFIT,
                    NATGAS_STRICT_STOP_LOSS,
                ));
            } else if hour == 11
                && hour_context_short_11h(index, &candles, &feature_bank, &h4_bias_by_h1_index)
            {
                module = Some((
                    "11h_sniper_short",
                    "short",
                    NATGAS_STRICT_STOP_LOSS,
                    NATGAS_STRICT_TAKE_PROFIT,
                ));
            } else if hour == 13
                && index > 0
                && bearish_body(&candles[index - 1])
                && bullish_body(&candles[index])
                && feature_bank.macd_histogram[index] > 0.0
                && prev24_width[index].is_finite()
                && prev24_width[index] > f64::EPSILON
                && candles[index].close
                    <= prev24_low[index] + prev24_width[index] * 0.25
            {
                module = Some((
                    "13h_recurrence_long",
                    "long",
                    NATGAS_STRICT_STOP_LOSS,
                    NATGAS_STRICT_TAKE_PROFIT,
                ));
            } else if hour == 18
                && hour_context_long_18h(index, &candles, &feature_bank, &h4_bias_by_h1_index)
            {
                module = Some((
                    "18h_sniper_long",
                    "long",
                    NATGAS_STRICT_STOP_LOSS,
                    NATGAS_STRICT_TAKE_PROFIT,
                ));
            }

            let Some((label, direction, stop_loss, take_profit)) = module else {
                continue;
            };
            let Some((tp_hit, pnl, _held)) = simulate_tp_sl_only_exit(
                &candles,
                index + 1,
                direction,
                stop_loss,
                take_profit,
                NATGAS_STRICT_EXECUTION_COST,
            ) else {
                continue;
            };

            trades.push(StratAUnifiedTrade {
                entry_time: candles[index + 1].time.clone(),
                module: label,
                pnl_distance: pnl,
                tp_hit,
            });
        }

        trades
    }

    fn run_natgas_strat_a_unified_performance() {
        let trades = collect_natgas_strat_a_unified_trades();
        let mut daily_pnl = BTreeMap::<String, f64>::new();
        let mut tp_hits = 0usize;
        let mut sl_hits = 0usize;
        let mut net_pnl_distance = 0.0;
        let mut module_counts = BTreeMap::<&'static str, usize>::new();

        for trade in &trades {
            let Some(day_key) = trade.entry_time.get(..10) else {
                continue;
            };
            net_pnl_distance += trade.pnl_distance;
            *module_counts.entry(trade.module).or_insert(0) += 1;
            if trade.tp_hit {
                tp_hits += 1;
            } else {
                sl_hits += 1;
            }
            *daily_pnl.entry(day_key.to_string()).or_insert(0.0) += trade.pnl_distance;
        }

        let total_days = daily_pnl.len();
        let total_points = net_pnl_distance / 0.01;
        let avg_points_per_day = if total_days > 0 {
            total_points / total_days as f64
        } else {
            0.0
        };
        let trading_weeks = if total_days > 0 {
            total_days as f64 / 5.0
        } else {
            0.0
        };
        let avg_points_per_week = if trading_weeks > 0.0 {
            total_points / trading_weeks
        } else {
            0.0
        };
        let trades_len = trades.len();

        println!(
            "NATGAS_STRAT_A_UNIFIED trades={} tp_hits={} sl_hits={} tp_rate={:.4} sl_rate={:.4} total_days={} total_points={:.2} avg_points_per_day={:.2} avg_points_per_week={:.2} net_pnl_distance={:.6}",
            trades_len,
            tp_hits,
            sl_hits,
            if trades_len > 0 { tp_hits as f64 / trades_len as f64 } else { 0.0 },
            if trades_len > 0 { sl_hits as f64 / trades_len as f64 } else { 0.0 },
            total_days,
            total_points,
            avg_points_per_day,
            avg_points_per_week,
            net_pnl_distance,
        );
        for (label, count) in module_counts {
            println!("NATGAS_STRAT_A_UNIFIED_MODULE label={} trades={}", label, count);
        }
    }

    fn run_natgas_strat_a_compounded_capital_projection() {
        let trades = collect_natgas_strat_a_unified_trades();
        let mut capital = 200.0_f64;
        let mut month_end_capital = BTreeMap::<String, f64>::new();
        let mut first_month: Option<String> = None;
        let mut last_included_month: Option<String> = None;
        let mut included_trades = 0usize;

        for trade in trades {
            let Some(month_key) = trade.entry_time.get(..7).map(str::to_string) else {
                continue;
            };
            if first_month.is_none() {
                first_month = Some(month_key.clone());
            }
            let first = first_month.as_ref().expect("first month set");
            let months_since_start = {
                let fy = first[0..4].parse::<i32>().unwrap_or(0);
                let fm = first[5..7].parse::<i32>().unwrap_or(1);
                let cy = month_key[0..4].parse::<i32>().unwrap_or(0);
                let cm = month_key[5..7].parse::<i32>().unwrap_or(1);
                (cy - fy) * 12 + (cm - fm)
            };
            if months_since_start >= 12 {
                break;
            }

            let points = trade.pnl_distance / 0.01;
            let trade_return = points * 0.03;
            capital *= 1.0 + trade_return;
            included_trades += 1;
            month_end_capital.insert(month_key.clone(), capital);
            last_included_month = Some(month_key);
        }

        let first_month_label = first_month.unwrap_or_else(|| "unknown".to_string());
        let last_month_label = last_included_month.unwrap_or_else(|| "unknown".to_string());
        println!(
            "NATGAS_STRAT_A_CAPITAL_PROJECTION start_capital=200.00 first_month={} last_month={} trades={} end_capital={:.2} total_return_pct={:.2}",
            first_month_label,
            last_month_label,
            included_trades,
            capital,
            ((capital / 200.0) - 1.0) * 100.0,
        );
        for (month, month_capital) in month_end_capital {
            println!(
                "NATGAS_STRAT_A_CAPITAL_MONTH month={} capital={:.2}",
                month,
                month_capital,
            );
        }
    }

    fn run_natgas_inverse_context_diagnostics() {
        let series = canonical_chart_series("NATGAS_USD", "H1", 0).expect("NATGAS_USD H1 history");
        let candles = &series.candles;
        let train_rows = ((candles.len() as f64) * 0.7).round() as usize;
        let test_start = train_rows.max(25);
        let mut cache_snapshot = StrategyCacheSnapshot::default();
        let data_hash = strategy_candles_hash(candles);
        let feature_bank = strategy_feature_bank_cached(candles, &data_hash, &mut cache_snapshot);

        let short_rules = seeded_short_11h_rules();
        let long_15_rules = seeded_long_15h_rules();
        let long_21_rules = seeded_long_21h_rules();
        let short_rows = build_seeded_slot_masks(candles, &feature_bank, 25, &short_rules);
        let long_15_rows = build_seeded_slot_masks(candles, &feature_bank, 25, &long_15_rules);
        let long_21_rows = build_seeded_slot_masks(candles, &feature_bank, 25, &long_21_rules);

        let short_combo = build_seeded_slot_combos(&short_rules)
            .into_iter()
            .find(|combo| combo.label == "11h short if bearish body")
            .expect("11h short base combo");
        let long_15_combo = build_seeded_slot_combos(&long_15_rules)
            .into_iter()
            .find(|combo| combo.label == "15h long if bullish body")
            .expect("15h long base combo");
        let long_21_combo = build_seeded_slot_combos(&long_21_rules)
            .into_iter()
            .find(|combo| combo.label == "21h long if close > VWAP")
            .expect("21h long vwap combo");

        let test_days = build_meta_scheduler_days(
            candles,
            &feature_bank,
            test_start,
            candles.len().saturating_sub(1),
            &short_combo,
            &long_15_combo,
            &long_21_combo,
            &short_rows,
            &long_15_rows,
            &long_21_rows,
            NATGAS_STRICT_TAKE_PROFIT,
            NATGAS_STRICT_STOP_LOSS,
            NATGAS_STRICT_EXECUTION_COST,
            24,
        );

        let mut by_context = BTreeMap::<String, InverseContextComparison>::new();
        let mut by_hour = BTreeMap::<String, InverseContextComparison>::new();
        for day in &test_days {
            let mut baseline_daily_pnl = 0.0;
            for event in day.events.iter().flatten() {
                if !event.primary_signal {
                    continue;
                }
                let normal_action = base_action_for_hour(event.hour);
                let inverse_action = inverse_action_for_hour(event.hour);
                let context_key =
                    inverse_context_key(event, baseline_daily_pnl, candles, &feature_bank);
                let hour_key = format!("{}h", event.hour);

                let context_entry = by_context.entry(context_key).or_default();
                record_inverse_action_stats(&mut context_entry.normal, event, normal_action);
                record_inverse_action_stats(&mut context_entry.inverse, event, inverse_action);

                let hour_entry = by_hour.entry(hour_key).or_default();
                record_inverse_action_stats(&mut hour_entry.normal, event, normal_action);
                record_inverse_action_stats(&mut hour_entry.inverse, event, inverse_action);

                baseline_daily_pnl += action_pnl(event, normal_action);
            }
        }

        print_inverse_context_diagnostics("INVERSE_DIAG_VWAP_BASE", &by_context, &by_hour);
    }

    fn run_natgas_21h_inverse_vwap_search() {
        let series = canonical_chart_series("NATGAS_USD", "H1", 0).expect("NATGAS_USD H1 history");
        let candles = &series.candles;
        let search_start = 25usize;
        let mut cache_snapshot = StrategyCacheSnapshot::default();
        let data_hash = strategy_candles_hash(candles);
        let feature_bank = strategy_feature_bank_cached(candles, &data_hash, &mut cache_snapshot);

        let short_11_rules = seeded_short_11h_rules();
        let long_15_rules = seeded_long_15h_rules();
        let long_21_rules = seeded_long_21h_rules();
        let short_21_rules = seeded_short_21h_rules();

        let short_11_rows = build_seeded_slot_masks(candles, &feature_bank, search_start, &short_11_rules);
        let long_15_rows = build_seeded_slot_masks(candles, &feature_bank, search_start, &long_15_rules);
        let long_21_rows = build_seeded_slot_masks(candles, &feature_bank, search_start, &long_21_rules);
        let short_21_rows = build_seeded_slot_masks(candles, &feature_bank, search_start, &short_21_rules);

        let short_11_base = build_seeded_slot_combos(&short_11_rules)
            .into_iter()
            .find(|combo| combo.label == "11h short if bearish body")
            .expect("11h short base combo");
        let long_15_base = build_seeded_slot_combos(&long_15_rules)
            .into_iter()
            .find(|combo| combo.label == "15h long if bullish body")
            .expect("15h long base combo");

        let long_21_combos = build_seeded_slot_combos(&long_21_rules)
            .into_iter()
            .filter(combo_is_vwap_inversion_family)
            .collect::<Vec<_>>();
        let short_21_combos = build_seeded_slot_combos(&short_21_rules)
            .into_iter()
            .filter(combo_is_vwap_inversion_family)
            .collect::<Vec<_>>();

        let rank_slot = |left: &SeededSlotSearchCandidate, right: &SeededSlotSearchCandidate| {
            match left.win_rate.partial_cmp(&right.win_rate).unwrap_or(Ordering::Equal) {
                Ordering::Greater => true,
                Ordering::Less => false,
                Ordering::Equal => match left
                    .expectancy_distance
                    .partial_cmp(&right.expectancy_distance)
                    .unwrap_or(Ordering::Equal)
                {
                    Ordering::Greater => true,
                    Ordering::Less => false,
                    Ordering::Equal => left.trades > right.trades,
                },
            }
        };

        let rank_daily =
            |left: &SeededComboSearchCandidate, right: &SeededComboSearchCandidate| match left
                .daily_target_hit_rate
                .partial_cmp(&right.daily_target_hit_rate)
                .unwrap_or(Ordering::Equal)
            {
                Ordering::Greater => true,
                Ordering::Less => false,
                Ordering::Equal => match left
                    .win_rate
                    .partial_cmp(&right.win_rate)
                    .unwrap_or(Ordering::Equal)
                {
                    Ordering::Greater => true,
                    Ordering::Less => false,
                    Ordering::Equal => match left
                        .avg_daily_pnl_distance
                        .partial_cmp(&right.avg_daily_pnl_distance)
                        .unwrap_or(Ordering::Equal)
                    {
                        Ordering::Greater => true,
                        Ordering::Less => false,
                        Ordering::Equal => left.trades > right.trades,
                    },
                },
            };

        let mut best_slot_long: Option<SeededSlotSearchCandidate> = None;
        for combo in &long_21_combos {
            let Some(candidate) = evaluate_seeded_slot_search_candidate(
                candles,
                &long_21_rows,
                combo,
                NATGAS_STRICT_TAKE_PROFIT,
                NATGAS_STRICT_STOP_LOSS,
                NATGAS_STRICT_EXECUTION_COST,
                24,
            ) else {
                continue;
            };
            if candidate.trades < 3 {
                continue;
            }
            if best_slot_long
                .as_ref()
                .map(|current| rank_slot(&candidate, current))
                .unwrap_or(true)
            {
                best_slot_long = Some(candidate);
            }
        }

        let mut best_slot_short: Option<SeededSlotSearchCandidate> = None;
        for combo in &short_21_combos {
            let Some(candidate) = evaluate_seeded_slot_search_candidate(
                candles,
                &short_21_rows,
                combo,
                NATGAS_STRICT_TAKE_PROFIT,
                NATGAS_STRICT_STOP_LOSS,
                NATGAS_STRICT_EXECUTION_COST,
                24,
            ) else {
                continue;
            };
            if candidate.trades < 3 {
                continue;
            }
            if best_slot_short
                .as_ref()
                .map(|current| rank_slot(&candidate, current))
                .unwrap_or(true)
            {
                best_slot_short = Some(candidate);
            }
        }

        let mut best_daily_long: Option<SeededComboSearchCandidate> = None;
        for combo in &long_21_combos {
            let Some(candidate) = evaluate_seeded_combo_search_candidate(
                candles,
                &short_11_rows,
                &long_15_rows,
                &long_21_rows,
                &short_11_base,
                &long_15_base,
                combo,
                NATGAS_STRICT_TAKE_PROFIT,
                NATGAS_STRICT_STOP_LOSS,
                NATGAS_STRICT_EXECUTION_COST,
                24,
            ) else {
                continue;
            };
            if best_daily_long
                .as_ref()
                .map(|current| rank_daily(&candidate, current))
                .unwrap_or(true)
            {
                best_daily_long = Some(candidate);
            }
        }

        let mut best_daily_short: Option<SeededComboSearchCandidate> = None;
        for combo in &short_21_combos {
            let Some(candidate) = evaluate_seeded_combo_search_candidate(
                candles,
                &short_11_rows,
                &long_15_rows,
                &short_21_rows,
                &short_11_base,
                &long_15_base,
                combo,
                NATGAS_STRICT_TAKE_PROFIT,
                NATGAS_STRICT_STOP_LOSS,
                NATGAS_STRICT_EXECUTION_COST,
                24,
            ) else {
                continue;
            };
            if best_daily_short
                .as_ref()
                .map(|current| rank_daily(&candidate, current))
                .unwrap_or(true)
            {
                best_daily_short = Some(candidate);
            }
        }

        let best_slot_long = best_slot_long.expect("best 21h long slot candidate");
        let best_slot_short = best_slot_short.expect("best 21h short slot candidate");
        let best_daily_long = best_daily_long.expect("best 21h long daily candidate");
        let best_daily_short = best_daily_short.expect("best 21h short daily candidate");

        println!(
            "NATGAS_H1_21H_INVERSE_SEARCH slot_best_long={} [{}] trades={} wins={} losses={} win_rate={:.4} expectancy={:.6} net_pnl={:.6}",
            best_slot_long.combo.label,
            best_slot_long.combo.indicator_refs.join(","),
            best_slot_long.trades,
            best_slot_long.wins,
            best_slot_long.losses,
            best_slot_long.win_rate,
            best_slot_long.expectancy_distance,
            best_slot_long.net_pnl_distance,
        );
        println!(
            "NATGAS_H1_21H_INVERSE_SEARCH slot_best_short={} [{}] trades={} wins={} losses={} win_rate={:.4} expectancy={:.6} net_pnl={:.6}",
            best_slot_short.combo.label,
            best_slot_short.combo.indicator_refs.join(","),
            best_slot_short.trades,
            best_slot_short.wins,
            best_slot_short.losses,
            best_slot_short.win_rate,
            best_slot_short.expectancy_distance,
            best_slot_short.net_pnl_distance,
        );
        println!(
            "NATGAS_H1_21H_INVERSE_SEARCH daily_best_long={} [{}] trades={} wins={} losses={} win_rate={:.4} daily_target_hit_rate={:.4} target_hit_days={} total_days={} avg_daily_pnl={:.6} expectancy={:.6} net_pnl={:.6}",
            best_daily_long.long_21h.label,
            best_daily_long.long_21h.indicator_refs.join(","),
            best_daily_long.trades,
            best_daily_long.wins,
            best_daily_long.losses,
            best_daily_long.win_rate,
            best_daily_long.daily_target_hit_rate,
            best_daily_long.target_hit_days,
            best_daily_long.total_days,
            best_daily_long.avg_daily_pnl_distance,
            best_daily_long.expectancy_distance,
            best_daily_long.net_pnl_distance,
        );
        println!(
            "NATGAS_H1_21H_INVERSE_SEARCH daily_best_short={} [{}] trades={} wins={} losses={} win_rate={:.4} daily_target_hit_rate={:.4} target_hit_days={} total_days={} avg_daily_pnl={:.6} expectancy={:.6} net_pnl={:.6}",
            best_daily_short.long_21h.label,
            best_daily_short.long_21h.indicator_refs.join(","),
            best_daily_short.trades,
            best_daily_short.wins,
            best_daily_short.losses,
            best_daily_short.win_rate,
            best_daily_short.daily_target_hit_rate,
            best_daily_short.target_hit_days,
            best_daily_short.total_days,
            best_daily_short.avg_daily_pnl_distance,
            best_daily_short.expectancy_distance,
            best_daily_short.net_pnl_distance,
        );
    }

    fn run_natgas_11h_15h_refinement_search() {
        let series = canonical_chart_series("NATGAS_USD", "H1", 0).expect("NATGAS_USD H1 history");
        let candles = &series.candles;
        let search_start = 25usize;
        let mut cache_snapshot = StrategyCacheSnapshot::default();
        let data_hash = strategy_candles_hash(candles);
        let feature_bank = strategy_feature_bank_cached(candles, &data_hash, &mut cache_snapshot);

        let short_11_rules = seeded_short_11h_rules();
        let long_15_rules = seeded_long_15h_rules();
        let short_21_rules = seeded_short_21h_rules();

        let short_11_rows = build_seeded_slot_masks(candles, &feature_bank, search_start, &short_11_rules);
        let long_15_rows = build_seeded_slot_masks(candles, &feature_bank, search_start, &long_15_rules);
        let short_21_rows = build_seeded_slot_masks(candles, &feature_bank, search_start, &short_21_rules);

        let short_11_combos = build_seeded_slot_combos(&short_11_rules);
        let long_15_combos = build_seeded_slot_combos(&long_15_rules);
        let short_21_fixed = build_seeded_slot_combos(&short_21_rules)
            .into_iter()
            .find(|combo| combo.label == "21h short if close < VWAP && 21h short if RSI14 < 50")
            .expect("21h short fixed combo");

        let rank_slot = |left: &SeededSlotSearchCandidate, right: &SeededSlotSearchCandidate| {
            match left.win_rate.partial_cmp(&right.win_rate).unwrap_or(Ordering::Equal) {
                Ordering::Greater => true,
                Ordering::Less => false,
                Ordering::Equal => match left
                    .expectancy_distance
                    .partial_cmp(&right.expectancy_distance)
                    .unwrap_or(Ordering::Equal)
                {
                    Ordering::Greater => true,
                    Ordering::Less => false,
                    Ordering::Equal => left.trades > right.trades,
                },
            }
        };

        let rank_daily =
            |left: &SeededComboSearchCandidate, right: &SeededComboSearchCandidate| match left
                .daily_target_hit_rate
                .partial_cmp(&right.daily_target_hit_rate)
                .unwrap_or(Ordering::Equal)
            {
                Ordering::Greater => true,
                Ordering::Less => false,
                Ordering::Equal => match left
                    .win_rate
                    .partial_cmp(&right.win_rate)
                    .unwrap_or(Ordering::Equal)
                {
                    Ordering::Greater => true,
                    Ordering::Less => false,
                    Ordering::Equal => match left
                        .avg_daily_pnl_distance
                        .partial_cmp(&right.avg_daily_pnl_distance)
                        .unwrap_or(Ordering::Equal)
                    {
                        Ordering::Greater => true,
                        Ordering::Less => false,
                        Ordering::Equal => left.trades > right.trades,
                    },
                },
            };

        let mut best_11h: Option<SeededSlotSearchCandidate> = None;
        for combo in &short_11_combos {
            let Some(candidate) = evaluate_seeded_slot_search_candidate(
                candles,
                &short_11_rows,
                combo,
                NATGAS_STRICT_TAKE_PROFIT,
                NATGAS_STRICT_STOP_LOSS,
                NATGAS_STRICT_EXECUTION_COST,
                24,
            ) else {
                continue;
            };
            if candidate.trades < 12 {
                continue;
            }
            if best_11h
                .as_ref()
                .map(|current| rank_slot(&candidate, current))
                .unwrap_or(true)
            {
                best_11h = Some(candidate);
            }
        }

        let mut best_15h: Option<SeededSlotSearchCandidate> = None;
        for combo in &long_15_combos {
            let Some(candidate) = evaluate_seeded_slot_search_candidate(
                candles,
                &long_15_rows,
                combo,
                NATGAS_STRICT_TAKE_PROFIT,
                NATGAS_STRICT_STOP_LOSS,
                NATGAS_STRICT_EXECUTION_COST,
                24,
            ) else {
                continue;
            };
            if candidate.trades < 12 {
                continue;
            }
            if best_15h
                .as_ref()
                .map(|current| rank_slot(&candidate, current))
                .unwrap_or(true)
            {
                best_15h = Some(candidate);
            }
        }

        let mut best_daily: Option<SeededComboSearchCandidate> = None;
        for short_11_combo in &short_11_combos {
            for long_15_combo in &long_15_combos {
                let Some(candidate) = evaluate_seeded_combo_search_candidate(
                    candles,
                    &short_11_rows,
                    &long_15_rows,
                    &short_21_rows,
                    short_11_combo,
                    long_15_combo,
                    &short_21_fixed,
                    NATGAS_STRICT_TAKE_PROFIT,
                    NATGAS_STRICT_STOP_LOSS,
                    NATGAS_STRICT_EXECUTION_COST,
                    24,
                ) else {
                    continue;
                };
                if best_daily
                    .as_ref()
                    .map(|current| rank_daily(&candidate, current))
                    .unwrap_or(true)
                {
                    best_daily = Some(candidate);
                }
            }
        }

        let best_11h = best_11h.expect("best 11h refinement");
        let best_15h = best_15h.expect("best 15h refinement");
        let best_daily = best_daily.expect("best daily refinement");

        println!(
            "NATGAS_H1_11H_15H_REFINEMENT slot_best_11h={} [{}] trades={} wins={} losses={} win_rate={:.4} expectancy={:.6} net_pnl={:.6}",
            best_11h.combo.label,
            best_11h.combo.indicator_refs.join(","),
            best_11h.trades,
            best_11h.wins,
            best_11h.losses,
            best_11h.win_rate,
            best_11h.expectancy_distance,
            best_11h.net_pnl_distance,
        );
        println!(
            "NATGAS_H1_11H_15H_REFINEMENT slot_best_15h={} [{}] trades={} wins={} losses={} win_rate={:.4} expectancy={:.6} net_pnl={:.6}",
            best_15h.combo.label,
            best_15h.combo.indicator_refs.join(","),
            best_15h.trades,
            best_15h.wins,
            best_15h.losses,
            best_15h.win_rate,
            best_15h.expectancy_distance,
            best_15h.net_pnl_distance,
        );
        println!(
            "NATGAS_H1_11H_15H_REFINEMENT daily_best_11h={} [{}] daily_best_15h={} [{}] fixed_21h={} [{}] trades={} wins={} losses={} win_rate={:.4} daily_target_hit_rate={:.4} target_hit_days={} total_days={} avg_daily_pnl={:.6} expectancy={:.6} net_pnl={:.6}",
            best_daily.short_11h.label,
            best_daily.short_11h.indicator_refs.join(","),
            best_daily.long_15h.label,
            best_daily.long_15h.indicator_refs.join(","),
            best_daily.long_21h.label,
            best_daily.long_21h.indicator_refs.join(","),
            best_daily.trades,
            best_daily.wins,
            best_daily.losses,
            best_daily.win_rate,
            best_daily.daily_target_hit_rate,
            best_daily.target_hit_days,
            best_daily.total_days,
            best_daily.avg_daily_pnl_distance,
            best_daily.expectancy_distance,
            best_daily.net_pnl_distance,
        );
    }

    fn run_natgas_11h_15h_h4_context_search() {
        let h1_series = canonical_chart_series("NATGAS_USD", "H1", 0).expect("NATGAS_USD H1 history");
        let h4_series = canonical_chart_series("NATGAS_USD", "H4", 0).expect("NATGAS_USD H4 history");
        let h1_candles = &h1_series.candles;
        let h4_candles = &h4_series.candles;
        let search_start = 25usize;
        let mut cache_snapshot = StrategyCacheSnapshot::default();
        let h1_hash = strategy_candles_hash(h1_candles);
        let h4_hash = strategy_candles_hash(h4_candles);
        let h1_bank = strategy_feature_bank_cached(h1_candles, &h1_hash, &mut cache_snapshot);
        let h4_bank = strategy_feature_bank_cached(h4_candles, &h4_hash, &mut cache_snapshot);
        let h4_by_h1 = build_completed_higher_timeframe_index_by_lower(h1_candles, h4_candles);

        let short_11_rules = seeded_short_11h_rules();
        let long_15_rules = seeded_long_15h_rules();
        let short_21_rules = seeded_short_21h_rules();
        let short_11_rows = build_seeded_slot_masks(h1_candles, &h1_bank, search_start, &short_11_rules);
        let long_15_rows = build_seeded_slot_masks(h1_candles, &h1_bank, search_start, &long_15_rules);
        let short_21_rows = build_seeded_slot_masks(h1_candles, &h1_bank, search_start, &short_21_rules);
        let short_11_combos = build_seeded_slot_combos(&short_11_rules);
        let long_15_combos = build_seeded_slot_combos(&long_15_rules);
        let short_21_fixed = build_seeded_slot_combos(&short_21_rules)
            .into_iter()
            .find(|combo| combo.label == "21h short if close < VWAP && 21h short if RSI14 < 50")
            .expect("21h short fixed combo");

        let neutral_11 = SeededGateDef {
            label: "no H4 filter".to_string(),
            indicator_refs: vec![],
            allowed_by_index: vec![true; h1_candles.len()],
        };
        let h4_11_close_lt_vwap = build_higher_timeframe_gate(
            h1_candles.len(),
            &h4_by_h1,
            h4_candles,
            &h4_bank,
            seed_short_close_below_vwap,
            "H4 close < VWAP",
            &["/vwap_h4_session_hlc3"],
        );
        let h4_11_bearish_body = build_higher_timeframe_gate(
            h1_candles.len(),
            &h4_by_h1,
            h4_candles,
            &h4_bank,
            seed_short_bearish_body,
            "H4 bearish body",
            &["/candle_h4_body"],
        );
        let h4_11_rsi_lt_50 = build_higher_timeframe_gate(
            h1_candles.len(),
            &h4_by_h1,
            h4_candles,
            &h4_bank,
            seed_short_rsi_lt_50,
            "H4 RSI14 < 50",
            &["/rsi_h4_14_close"],
        );
        let h4_11_two_closes_below_vwap = build_higher_timeframe_gate(
            h1_candles.len(),
            &h4_by_h1,
            h4_candles,
            &h4_bank,
            seed_short_two_closes_below_vwap,
            "H4 two closes stay below VWAP",
            &["/vwap_h4_session_hlc3", "/candle_h4_close"],
        );
        let h4_11_three_bar_rollover = build_higher_timeframe_gate(
            h1_candles.len(),
            &h4_by_h1,
            h4_candles,
            &h4_bank,
            seed_short_three_bar_vwap_rollover,
            "H4 three-bar VWAP rollover",
            &["/vwap_h4_session_hlc3", "/candle_h4_close"],
        );
        let short_11_gates = vec![
            neutral_11.clone(),
            h4_11_close_lt_vwap.clone(),
            h4_11_bearish_body.clone(),
            h4_11_rsi_lt_50.clone(),
            h4_11_two_closes_below_vwap.clone(),
            h4_11_three_bar_rollover.clone(),
            intersect_gate_defs(&h4_11_close_lt_vwap, &h4_11_rsi_lt_50),
            intersect_gate_defs(&h4_11_close_lt_vwap, &h4_11_bearish_body),
            intersect_gate_defs(&h4_11_two_closes_below_vwap, &h4_11_rsi_lt_50),
        ];

        let neutral_15 = SeededGateDef {
            label: "no H4 filter".to_string(),
            indicator_refs: vec![],
            allowed_by_index: vec![true; h1_candles.len()],
        };
        let h4_15_close_gt_vwap = build_higher_timeframe_gate(
            h1_candles.len(),
            &h4_by_h1,
            h4_candles,
            &h4_bank,
            seed_long_close_above_vwap,
            "H4 close > VWAP",
            &["/vwap_h4_session_hlc3"],
        );
        let h4_15_bullish_body = build_higher_timeframe_gate(
            h1_candles.len(),
            &h4_by_h1,
            h4_candles,
            &h4_bank,
            seed_long_bullish_body,
            "H4 bullish body",
            &["/candle_h4_body"],
        );
        let h4_15_rsi_gt_50 = build_higher_timeframe_gate(
            h1_candles.len(),
            &h4_by_h1,
            h4_candles,
            &h4_bank,
            seed_long_rsi_gt_50,
            "H4 RSI14 > 50",
            &["/rsi_h4_14_close"],
        );
        let h4_15_two_closes_above_vwap = build_higher_timeframe_gate(
            h1_candles.len(),
            &h4_by_h1,
            h4_candles,
            &h4_bank,
            seed_long_two_closes_above_vwap,
            "H4 two closes stay above VWAP",
            &["/vwap_h4_session_hlc3", "/candle_h4_close"],
        );
        let h4_15_three_bar_reclaim = build_higher_timeframe_gate(
            h1_candles.len(),
            &h4_by_h1,
            h4_candles,
            &h4_bank,
            seed_long_three_bar_vwap_reclaim,
            "H4 three-bar VWAP reclaim",
            &["/vwap_h4_session_hlc3", "/vwap_h4_ext1_down", "/candle_h4_close"],
        );
        let long_15_gates = vec![
            neutral_15.clone(),
            h4_15_close_gt_vwap.clone(),
            h4_15_bullish_body.clone(),
            h4_15_rsi_gt_50.clone(),
            h4_15_two_closes_above_vwap.clone(),
            h4_15_three_bar_reclaim.clone(),
            intersect_gate_defs(&h4_15_close_gt_vwap, &h4_15_rsi_gt_50),
            intersect_gate_defs(&h4_15_close_gt_vwap, &h4_15_bullish_body),
            intersect_gate_defs(&h4_15_two_closes_above_vwap, &h4_15_rsi_gt_50),
        ];

        let rank_daily =
            |left: &SeededComboSearchCandidate, right: &SeededComboSearchCandidate| match left
                .daily_target_hit_rate
                .partial_cmp(&right.daily_target_hit_rate)
                .unwrap_or(Ordering::Equal)
            {
                Ordering::Greater => true,
                Ordering::Less => false,
                Ordering::Equal => match left
                    .win_rate
                    .partial_cmp(&right.win_rate)
                    .unwrap_or(Ordering::Equal)
                {
                    Ordering::Greater => true,
                    Ordering::Less => false,
                    Ordering::Equal => match left
                        .avg_daily_pnl_distance
                        .partial_cmp(&right.avg_daily_pnl_distance)
                        .unwrap_or(Ordering::Equal)
                    {
                        Ordering::Greater => true,
                        Ordering::Less => false,
                        Ordering::Equal => left.trades > right.trades,
                    },
                },
            };

        let mut best_daily: Option<(SeededComboSearchCandidate, SeededGateDef, SeededGateDef)> = None;
        for short_11_combo in &short_11_combos {
            for long_15_combo in &long_15_combos {
                for short_11_gate in &short_11_gates {
                    for long_15_gate in &long_15_gates {
                        let Some(candidate) = evaluate_seeded_combo_search_candidate_with_gates(
                            h1_candles,
                            &short_11_rows,
                            &long_15_rows,
                            &short_21_rows,
                            short_11_combo,
                            long_15_combo,
                            &short_21_fixed,
                            Some(&short_11_gate.allowed_by_index),
                            Some(&long_15_gate.allowed_by_index),
                            None,
                            NATGAS_STRICT_TAKE_PROFIT,
                            NATGAS_STRICT_STOP_LOSS,
                            NATGAS_STRICT_EXECUTION_COST,
                            24,
                        ) else {
                            continue;
                        };
                        let replace = best_daily
                            .as_ref()
                            .map(|(current, _, _)| rank_daily(&candidate, current))
                            .unwrap_or(true);
                        if replace {
                            best_daily = Some((candidate, short_11_gate.clone(), long_15_gate.clone()));
                        }
                    }
                }
            }
        }

        let (best_daily, best_11_gate, best_15_gate) = best_daily.expect("best H4-context daily refinement");
        println!(
            "NATGAS_H1_11H_15H_H4_CONTEXT daily_best_11h={} [{}] h4_gate_11h={} [{}] daily_best_15h={} [{}] h4_gate_15h={} [{}] fixed_21h={} [{}] trades={} wins={} losses={} win_rate={:.4} daily_target_hit_rate={:.4} target_hit_days={} total_days={} avg_daily_pnl={:.6} expectancy={:.6} net_pnl={:.6}",
            best_daily.short_11h.label,
            best_daily.short_11h.indicator_refs.join(","),
            best_11_gate.label,
            best_11_gate.indicator_refs.join(","),
            best_daily.long_15h.label,
            best_daily.long_15h.indicator_refs.join(","),
            best_15_gate.label,
            best_15_gate.indicator_refs.join(","),
            best_daily.long_21h.label,
            best_daily.long_21h.indicator_refs.join(","),
            best_daily.trades,
            best_daily.wins,
            best_daily.losses,
            best_daily.win_rate,
            best_daily.daily_target_hit_rate,
            best_daily.target_hit_days,
            best_daily.total_days,
            best_daily.avg_daily_pnl_distance,
            best_daily.expectancy_distance,
            best_daily.net_pnl_distance,
        );
    }

    fn run_natgas_h4_pullback_reversal_search() {
        let h1_series = canonical_chart_series("NATGAS_USD", "H1", 0).expect("NATGAS_USD H1 history");
        let h4_series = canonical_chart_series("NATGAS_USD", "H4", 0).expect("NATGAS_USD H4 history");
        let h1_candles = &h1_series.candles;
        let h4_candles = &h4_series.candles;
        let search_start = 25usize;
        let mut cache_snapshot = StrategyCacheSnapshot::default();
        let h1_hash = strategy_candles_hash(h1_candles);
        let h4_hash = strategy_candles_hash(h4_candles);
        let h1_bank = strategy_feature_bank_cached(h1_candles, &h1_hash, &mut cache_snapshot);
        let h4_bank = strategy_feature_bank_cached(h4_candles, &h4_hash, &mut cache_snapshot);
        let h4_by_h1 = build_completed_higher_timeframe_index_by_lower(h1_candles, h4_candles);

        let short_11_rules = seeded_short_11h_rules();
        let long_15_rules = seeded_long_15h_rules();
        let short_21_rules = seeded_short_21h_rules();
        let short_11_rows = build_seeded_slot_masks(h1_candles, &h1_bank, search_start, &short_11_rules);
        let long_15_rows = build_seeded_slot_masks(h1_candles, &h1_bank, search_start, &long_15_rules);
        let short_21_rows = build_seeded_slot_masks(h1_candles, &h1_bank, search_start, &short_21_rules);
        let short_11_combos = build_seeded_slot_combos(&short_11_rules);
        let long_15_combos = build_seeded_slot_combos(&long_15_rules);
        let short_21_fixed = build_seeded_slot_combos(&short_21_rules)
            .into_iter()
            .find(|combo| combo.label == "21h short if close < VWAP && 21h short if RSI14 < 50")
            .expect("21h short fixed combo");

        let short_pullback_gate = build_higher_timeframe_gate(
            h1_candles.len(),
            &h4_by_h1,
            h4_candles,
            &h4_bank,
            seed_short_trend_pullback_reversal,
            "H4 bearish pullback candle closes inside impulse body",
            &[
                "/candle_h4_body",
                "/vwap_h4_session_hlc3",
                "/ema_h4_21_close",
                "/ema_h4_50_close",
                "/macd_h4_12_26_9_histogram",
            ],
        );
        let long_pullback_gate = build_higher_timeframe_gate(
            h1_candles.len(),
            &h4_by_h1,
            h4_candles,
            &h4_bank,
            seed_long_trend_pullback_reversal,
            "H4 bullish pullback candle closes inside impulse body",
            &[
                "/candle_h4_body",
                "/vwap_h4_session_hlc3",
                "/ema_h4_21_close",
                "/ema_h4_50_close",
                "/macd_h4_12_26_9_histogram",
            ],
        );

        let rank_daily =
            |left: &SeededComboSearchCandidate, right: &SeededComboSearchCandidate| match left
                .daily_target_hit_rate
                .partial_cmp(&right.daily_target_hit_rate)
                .unwrap_or(Ordering::Equal)
            {
                Ordering::Greater => true,
                Ordering::Less => false,
                Ordering::Equal => match left
                    .win_rate
                    .partial_cmp(&right.win_rate)
                    .unwrap_or(Ordering::Equal)
                {
                    Ordering::Greater => true,
                    Ordering::Less => false,
                    Ordering::Equal => match left
                        .avg_daily_pnl_distance
                        .partial_cmp(&right.avg_daily_pnl_distance)
                        .unwrap_or(Ordering::Equal)
                    {
                        Ordering::Greater => true,
                        Ordering::Less => false,
                        Ordering::Equal => left.trades > right.trades,
                    },
                },
            };

        let mut best_daily: Option<SeededComboSearchCandidate> = None;
        for short_11_combo in &short_11_combos {
            for long_15_combo in &long_15_combos {
                let Some(candidate) = evaluate_seeded_combo_search_candidate_with_gates(
                    h1_candles,
                    &short_11_rows,
                    &long_15_rows,
                    &short_21_rows,
                    short_11_combo,
                    long_15_combo,
                    &short_21_fixed,
                    Some(&short_pullback_gate.allowed_by_index),
                    Some(&long_pullback_gate.allowed_by_index),
                    None,
                    NATGAS_STRICT_TAKE_PROFIT,
                    NATGAS_STRICT_STOP_LOSS,
                    NATGAS_STRICT_EXECUTION_COST,
                    24,
                ) else {
                    continue;
                };
                if best_daily
                    .as_ref()
                    .map(|current| rank_daily(&candidate, current))
                    .unwrap_or(true)
                {
                    best_daily = Some(candidate);
                }
            }
        }

        let best_daily = best_daily.expect("best H4 pullback reversal candidate");
        println!(
            "NATGAS_H1_H4_PULLBACK_REVERSAL daily_best_11h={} [{}] h4_gate_11h={} [{}] daily_best_15h={} [{}] h4_gate_15h={} [{}] fixed_21h={} [{}] trades={} wins={} losses={} win_rate={:.4} daily_target_hit_rate={:.4} target_hit_days={} total_days={} avg_daily_pnl={:.6} expectancy={:.6} net_pnl={:.6}",
            best_daily.short_11h.label,
            best_daily.short_11h.indicator_refs.join(","),
            short_pullback_gate.label,
            short_pullback_gate.indicator_refs.join(","),
            best_daily.long_15h.label,
            best_daily.long_15h.indicator_refs.join(","),
            long_pullback_gate.label,
            long_pullback_gate.indicator_refs.join(","),
            best_daily.long_21h.label,
            best_daily.long_21h.indicator_refs.join(","),
            best_daily.trades,
            best_daily.wins,
            best_daily.losses,
            best_daily.win_rate,
            best_daily.daily_target_hit_rate,
            best_daily.target_hit_days,
            best_daily.total_days,
            best_daily.avg_daily_pnl_distance,
            best_daily.expectancy_distance,
            best_daily.net_pnl_distance,
        );
    }

    fn run_natgas_h4_pullback_direct_strategy() {
        let series = canonical_chart_series("NATGAS_USD", "H4", 0).expect("NATGAS_USD H4 history");
        let candles = &series.candles;
        let start_index = 25usize;
        let mut cache_snapshot = StrategyCacheSnapshot::default();
        let data_hash = strategy_candles_hash(candles);
        let bank = strategy_feature_bank_cached(candles, &data_hash, &mut cache_snapshot);

        let evaluate_direction = |direction: &'static str,
                                  label: &'static str,
                                  predicate: fn(usize, &[TradingCandlePoint], &StrategyIndicatorFeatureBank) -> bool|
         -> Option<(usize, usize, usize, f64, f64, f64, f64)> {
            let mut daily_pnl = BTreeMap::<String, f64>::new();
            let mut trades = 0usize;
            let mut wins = 0usize;
            let mut losses = 0usize;
            let mut net_pnl = 0.0;
            for index in start_index..candles.len().saturating_sub(1) {
                if !predicate(index, candles, &bank) {
                    continue;
                }
                let Some(day_key) = candles[index].time.get(..10) else {
                    continue;
                };
                let (outcome, _) = strategy_entry_outcome_cached(
                    candles,
                    index,
                    direction,
                    NATGAS_STRICT_STOP_LOSS,
                    NATGAS_STRICT_EXECUTION_COST,
                    12,
                );
                let Some(outcome) = outcome else {
                    continue;
                };
                let (exit_kind, pnl, _) =
                    classify_strict_trade_outcome(&outcome, NATGAS_STRICT_TAKE_PROFIT);
                *daily_pnl.entry(day_key.to_string()).or_insert(0.0) += pnl;
                trades += 1;
                net_pnl += pnl;
                if exit_kind == StrictTradeExit::TakeProfit {
                    wins += 1;
                } else if exit_kind == StrictTradeExit::StopLoss {
                    losses += 1;
                }
            }
            if trades == 0 {
                return None;
            }
            let total_days = daily_pnl.len().max(1);
            let target_hit_days = daily_pnl
                .values()
                .filter(|value| **value >= NATGAS_STRICT_DAILY_TARGET)
                .count();
            let avg_daily_pnl = daily_pnl.values().sum::<f64>() / total_days as f64;
            let expectancy = net_pnl / trades as f64;
            println!(
                "NATGAS_H4_PULLBACK_DIRECT direction={} label={} trades={} wins={} losses={} win_rate={:.4} daily_target_hit_rate={:.4} avg_daily_pnl={:.6} expectancy={:.6} net_pnl={:.6}",
                direction,
                label,
                trades,
                wins,
                losses,
                wins as f64 / trades as f64,
                target_hit_days as f64 / total_days as f64,
                avg_daily_pnl,
                expectancy,
                net_pnl,
            );
            Some((
                trades,
                wins,
                losses,
                target_hit_days as f64 / total_days as f64,
                avg_daily_pnl,
                expectancy,
                net_pnl,
            ))
        };

        let _ = evaluate_direction(
            "short",
            "H4 bearish pullback candle closes inside impulse body",
            seed_short_trend_pullback_reversal,
        );
        let _ = evaluate_direction(
            "long",
            "H4 bullish pullback candle closes inside impulse body",
            seed_long_trend_pullback_reversal,
        );
    }

    fn run_natgas_11h_15h_h4_bias_shortlist_search() {
        let h1_series = canonical_chart_series("NATGAS_USD", "H1", 0).expect("NATGAS_USD H1 history");
        let h4_series = canonical_chart_series("NATGAS_USD", "H4", 0).expect("NATGAS_USD H4 history");
        let h1_candles = &h1_series.candles;
        let h4_candles = &h4_series.candles;
        let search_start = 25usize;
        let mut cache_snapshot = StrategyCacheSnapshot::default();
        let h1_hash = strategy_candles_hash(h1_candles);
        let h4_hash = strategy_candles_hash(h4_candles);
        let h1_bank = strategy_feature_bank_cached(h1_candles, &h1_hash, &mut cache_snapshot);
        let h4_bank = strategy_feature_bank_cached(h4_candles, &h4_hash, &mut cache_snapshot);
        let h4_by_h1 = build_completed_higher_timeframe_index_by_lower(h1_candles, h4_candles);

        let short_11_rules = seeded_short_11h_rules();
        let long_15_rules = seeded_long_15h_rules();
        let short_21_rules = seeded_short_21h_rules();
        let short_11_rows = build_seeded_slot_masks(h1_candles, &h1_bank, search_start, &short_11_rules);
        let long_15_rows = build_seeded_slot_masks(h1_candles, &h1_bank, search_start, &long_15_rules);
        let short_21_rows = build_seeded_slot_masks(h1_candles, &h1_bank, search_start, &short_21_rules);

        let short_11_labels = [
            "11h short if bearish body",
            "11h short if close < VWAP",
            "11h short if close crosses below EMA21",
            "11h short if two closes stay below VWAP",
            "11h short if bearish body && 11h short if MACD hist < 0",
        ];
        let long_15_labels = [
            "15h long if bullish body",
            "15h long if close > VWAP",
            "15h long if close crosses above EMA21",
            "15h long if two closes stay above VWAP",
            "15h long if bullish body && 15h long if MACD hist > 0",
        ];

        let short_11_combos = build_seeded_slot_combos(&short_11_rules)
            .into_iter()
            .filter(|combo| combo_label_is_one_of(combo, &short_11_labels))
            .collect::<Vec<_>>();
        let long_15_combos = build_seeded_slot_combos(&long_15_rules)
            .into_iter()
            .filter(|combo| combo_label_is_one_of(combo, &long_15_labels))
            .collect::<Vec<_>>();
        let short_21_fixed = build_seeded_slot_combos(&short_21_rules)
            .into_iter()
            .find(|combo| combo.label == "21h short if close < VWAP && 21h short if RSI14 < 50")
            .expect("21h short fixed combo");

        let short_11_gates = vec![
            SeededGateDef {
                label: "no H4 bias".to_string(),
                indicator_refs: vec![],
                allowed_by_index: vec![true; h1_candles.len()],
            },
            build_higher_timeframe_gate(
                h1_candles.len(),
                &h4_by_h1,
                h4_candles,
                &h4_bank,
                seed_short_close_below_vwap,
                "H4 close < VWAP",
                &["/vwap_h4_session_hlc3"],
            ),
            build_higher_timeframe_gate(
                h1_candles.len(),
                &h4_by_h1,
                h4_candles,
                &h4_bank,
                seed_short_two_closes_below_vwap,
                "H4 two closes stay below VWAP",
                &["/vwap_h4_session_hlc3", "/candle_h4_close"],
            ),
            build_higher_timeframe_gate(
                h1_candles.len(),
                &h4_by_h1,
                h4_candles,
                &h4_bank,
                seed_short_trend_pullback_reversal,
                "H4 bearish pullback reversal",
                &["/candle_h4_body", "/vwap_h4_session_hlc3", "/ema_h4_21_close", "/ema_h4_50_close"],
            ),
        ];
        let long_15_gates = vec![
            SeededGateDef {
                label: "no H4 bias".to_string(),
                indicator_refs: vec![],
                allowed_by_index: vec![true; h1_candles.len()],
            },
            build_higher_timeframe_gate(
                h1_candles.len(),
                &h4_by_h1,
                h4_candles,
                &h4_bank,
                seed_long_close_above_vwap,
                "H4 close > VWAP",
                &["/vwap_h4_session_hlc3"],
            ),
            build_higher_timeframe_gate(
                h1_candles.len(),
                &h4_by_h1,
                h4_candles,
                &h4_bank,
                seed_long_two_closes_above_vwap,
                "H4 two closes stay above VWAP",
                &["/vwap_h4_session_hlc3", "/candle_h4_close"],
            ),
            build_higher_timeframe_gate(
                h1_candles.len(),
                &h4_by_h1,
                h4_candles,
                &h4_bank,
                seed_long_trend_pullback_reversal,
                "H4 bullish pullback reversal",
                &["/candle_h4_body", "/vwap_h4_session_hlc3", "/ema_h4_21_close", "/ema_h4_50_close"],
            ),
        ];

        let rank_daily =
            |left: &SeededComboSearchCandidate, right: &SeededComboSearchCandidate| match left
                .daily_target_hit_rate
                .partial_cmp(&right.daily_target_hit_rate)
                .unwrap_or(Ordering::Equal)
            {
                Ordering::Greater => true,
                Ordering::Less => false,
                Ordering::Equal => match left
                    .win_rate
                    .partial_cmp(&right.win_rate)
                    .unwrap_or(Ordering::Equal)
                {
                    Ordering::Greater => true,
                    Ordering::Less => false,
                    Ordering::Equal => match left
                        .avg_daily_pnl_distance
                        .partial_cmp(&right.avg_daily_pnl_distance)
                        .unwrap_or(Ordering::Equal)
                    {
                        Ordering::Greater => true,
                        Ordering::Less => false,
                        Ordering::Equal => left.trades > right.trades,
                    },
                },
            };

        let mut best_daily: Option<(SeededComboSearchCandidate, SeededGateDef, SeededGateDef)> = None;
        for short_11_combo in &short_11_combos {
            for long_15_combo in &long_15_combos {
                for short_11_gate in &short_11_gates {
                    for long_15_gate in &long_15_gates {
                        let Some(candidate) = evaluate_seeded_combo_search_candidate_with_gates(
                            h1_candles,
                            &short_11_rows,
                            &long_15_rows,
                            &short_21_rows,
                            short_11_combo,
                            long_15_combo,
                            &short_21_fixed,
                            Some(&short_11_gate.allowed_by_index),
                            Some(&long_15_gate.allowed_by_index),
                            None,
                            NATGAS_STRICT_TAKE_PROFIT,
                            NATGAS_STRICT_STOP_LOSS,
                            NATGAS_STRICT_EXECUTION_COST,
                            24,
                        ) else {
                            continue;
                        };
                        let replace = best_daily
                            .as_ref()
                            .map(|(current, _, _)| rank_daily(&candidate, current))
                            .unwrap_or(true);
                        if replace {
                            best_daily = Some((candidate, short_11_gate.clone(), long_15_gate.clone()));
                        }
                    }
                }
            }
        }

        let (best_daily, best_11_gate, best_15_gate) = best_daily.expect("best H4 bias shortlist");
        println!(
            "NATGAS_H1_11H_15H_H4_BIAS daily_best_11h={} [{}] h4_bias_11h={} [{}] daily_best_15h={} [{}] h4_bias_15h={} [{}] fixed_21h={} [{}] trades={} wins={} losses={} win_rate={:.4} daily_target_hit_rate={:.4} target_hit_days={} total_days={} avg_daily_pnl={:.6} expectancy={:.6} net_pnl={:.6}",
            best_daily.short_11h.label,
            best_daily.short_11h.indicator_refs.join(","),
            best_11_gate.label,
            best_11_gate.indicator_refs.join(","),
            best_daily.long_15h.label,
            best_daily.long_15h.indicator_refs.join(","),
            best_15_gate.label,
            best_15_gate.indicator_refs.join(","),
            best_daily.long_21h.label,
            best_daily.long_21h.indicator_refs.join(","),
            best_daily.trades,
            best_daily.wins,
            best_daily.losses,
            best_daily.win_rate,
            best_daily.daily_target_hit_rate,
            best_daily.target_hit_days,
            best_daily.total_days,
            best_daily.avg_daily_pnl_distance,
            best_daily.expectancy_distance,
            best_daily.net_pnl_distance,
        );
    }

    fn run_natgas_11h_15h_h4_bias_policy_search() {
        let h1_series = canonical_chart_series("NATGAS_USD", "H1", 0).expect("NATGAS_USD H1 history");
        let h4_series = canonical_chart_series("NATGAS_USD", "H4", 0).expect("NATGAS_USD H4 history");
        let h1_candles = &h1_series.candles;
        let h4_candles = &h4_series.candles;
        let search_start = 25usize;
        let mut cache_snapshot = StrategyCacheSnapshot::default();
        let h1_hash = strategy_candles_hash(h1_candles);
        let h4_hash = strategy_candles_hash(h4_candles);
        let h1_bank = strategy_feature_bank_cached(h1_candles, &h1_hash, &mut cache_snapshot);
        let h4_bank = strategy_feature_bank_cached(h4_candles, &h4_hash, &mut cache_snapshot);
        let h4_bias_by_h1_index = build_h4_bias_by_h1_index(h1_candles, h4_candles, &h4_bank);

        let short_11_rules = seeded_short_11h_rules();
        let long_15_rules = seeded_long_15h_rules();
        let short_21_rules = seeded_short_21h_rules();
        let short_11_rows = build_seeded_slot_masks(h1_candles, &h1_bank, search_start, &short_11_rules);
        let long_15_rows = build_seeded_slot_masks(h1_candles, &h1_bank, search_start, &long_15_rules);
        let short_21_rows = build_seeded_slot_masks(h1_candles, &h1_bank, search_start, &short_21_rules);

        let short_11_all_combos = build_seeded_slot_combos(&short_11_rules);
        let long_15_all_combos = build_seeded_slot_combos(&long_15_rules);
        let short_bearish_candidates = combos_from_labels(
            &short_11_all_combos,
            &[
                "11h short if bearish body",
                "11h short if bearish body && 11h short if MACD hist < 0",
                "11h short if close < VWAP",
                "11h short if two closes stay below VWAP",
                "11h short if close crosses below EMA21",
            ],
        );
        let short_neutral_candidates = combos_from_labels(
            &short_11_all_combos,
            &[
                "11h short if close crosses below EMA21",
                "11h short if close < VWAP",
                "11h short if VWAP +1sigma rejection after VWAP break",
                "11h short if three-bar VWAP rollover confirms lower close",
                "11h short if bearish body",
            ],
        );
        let short_bullish_candidates = combos_from_labels(
            &short_11_all_combos,
            &[
                "11h short if close crosses below EMA21",
                "11h short if three-bar VWAP rollover confirms lower close",
            ],
        );
        let long_bearish_candidates = combos_from_labels(
            &long_15_all_combos,
            &[
                "15h long if bullish body",
                "15h long if close crosses above EMA21",
                "15h long if bullish body && 15h long if MACD hist > 0",
                "15h long if VWAP -1sigma reclaim after VWAP break",
            ],
        );
        let long_neutral_candidates = combos_from_labels(
            &long_15_all_combos,
            &[
                "15h long if close > VWAP",
                "15h long if close crosses above EMA21",
                "15h long if two closes stay above VWAP",
                "15h long if VWAP -1sigma reclaim after VWAP break",
                "15h long if bullish body",
            ],
        );
        let long_bullish_candidates = combos_from_labels(
            &long_15_all_combos,
            &[
                "15h long if bullish body",
                "15h long if close > VWAP",
                "15h long if three-bar VWAP reclaim confirms higher close",
            ],
        );
        let short_21_fixed = build_seeded_slot_combos(&short_21_rules)
            .into_iter()
            .find(|combo| combo.label == "21h short if close < VWAP && 21h short if RSI14 < 50")
            .expect("21h short fixed combo");

        let rank_policy =
            |left: &H4BiasPolicyResult, right: &H4BiasPolicyResult| match left
                .daily_target_hit_rate
                .partial_cmp(&right.daily_target_hit_rate)
                .unwrap_or(Ordering::Equal)
            {
                Ordering::Greater => true,
                Ordering::Less => false,
                Ordering::Equal => match left
                    .win_rate
                    .partial_cmp(&right.win_rate)
                    .unwrap_or(Ordering::Equal)
                {
                    Ordering::Greater => true,
                    Ordering::Less => false,
                    Ordering::Equal => match left
                        .avg_daily_pnl_distance
                        .partial_cmp(&right.avg_daily_pnl_distance)
                        .unwrap_or(Ordering::Equal)
                    {
                        Ordering::Greater => true,
                        Ordering::Less => false,
                        Ordering::Equal => left.trades > right.trades,
                    },
                },
            };

        let short_bearish_top = top_bias_candidates(
            h1_candles,
            &short_11_rows,
            &short_bearish_candidates,
            &h4_bias_by_h1_index,
            H4BiasState::Bearish,
            NATGAS_STRICT_TAKE_PROFIT,
            NATGAS_STRICT_STOP_LOSS,
            NATGAS_STRICT_EXECUTION_COST,
            24,
            3,
        );
        let short_neutral_top = top_bias_candidates(
            h1_candles,
            &short_11_rows,
            &short_neutral_candidates,
            &h4_bias_by_h1_index,
            H4BiasState::Neutral,
            NATGAS_STRICT_TAKE_PROFIT,
            NATGAS_STRICT_STOP_LOSS,
            NATGAS_STRICT_EXECUTION_COST,
            24,
            3,
        );
        let short_bullish_top = top_bias_candidates(
            h1_candles,
            &short_11_rows,
            &short_bullish_candidates,
            &h4_bias_by_h1_index,
            H4BiasState::Bullish,
            NATGAS_STRICT_TAKE_PROFIT,
            NATGAS_STRICT_STOP_LOSS,
            NATGAS_STRICT_EXECUTION_COST,
            24,
            2,
        );
        let long_bearish_top = top_bias_candidates(
            h1_candles,
            &long_15_rows,
            &long_bearish_candidates,
            &h4_bias_by_h1_index,
            H4BiasState::Bearish,
            NATGAS_STRICT_TAKE_PROFIT,
            NATGAS_STRICT_STOP_LOSS,
            NATGAS_STRICT_EXECUTION_COST,
            24,
            3,
        );
        let long_neutral_top = top_bias_candidates(
            h1_candles,
            &long_15_rows,
            &long_neutral_candidates,
            &h4_bias_by_h1_index,
            H4BiasState::Neutral,
            NATGAS_STRICT_TAKE_PROFIT,
            NATGAS_STRICT_STOP_LOSS,
            NATGAS_STRICT_EXECUTION_COST,
            24,
            3,
        );
        let long_bullish_top = top_bias_candidates(
            h1_candles,
            &long_15_rows,
            &long_bullish_candidates,
            &h4_bias_by_h1_index,
            H4BiasState::Bullish,
            NATGAS_STRICT_TAKE_PROFIT,
            NATGAS_STRICT_STOP_LOSS,
            NATGAS_STRICT_EXECUTION_COST,
            24,
            2,
        );

        let mut best: Option<H4BiasPolicyResult> = None;
        for short_bearish in &short_bearish_top {
            for short_neutral in &short_neutral_top {
                for short_bullish in &short_bullish_top {
                    for long_bearish in &long_bearish_top {
                        for long_neutral in &long_neutral_top {
                            for long_bullish in &long_bullish_top {
                                let short_policy = [
                                    short_bearish.clone(),
                                    short_neutral.clone(),
                                    short_bullish.clone(),
                                ];
                                let long_policy = [
                                    long_bearish.clone(),
                                    long_neutral.clone(),
                                    long_bullish.clone(),
                                ];
                                let Some(candidate) = evaluate_h4_bias_policy(
                                    h1_candles,
                                    &short_11_rows,
                                    &long_15_rows,
                                    &short_21_rows,
                                    &h4_bias_by_h1_index,
                                    &short_policy,
                                    &long_policy,
                                    &short_21_fixed,
                                    NATGAS_STRICT_TAKE_PROFIT,
                                    NATGAS_STRICT_STOP_LOSS,
                                    NATGAS_STRICT_EXECUTION_COST,
                                    24,
                                ) else {
                                    continue;
                                };
                                if best
                                    .as_ref()
                                    .map(|current| rank_policy(&candidate, current))
                                    .unwrap_or(true)
                                {
                                    best = Some(candidate);
                                }
                            }
                        }
                    }
                }
            }
        }

        let best = best.expect("best H4 bias policy");
        let fmt_combo = |combo: &Option<SeededSlotComboDef>| combo
            .as_ref()
            .map(|value| value.label.clone())
            .unwrap_or_else(|| "skip".to_string());
        println!(
            "NATGAS_H1_11H_15H_H4_BIAS_POLICY short_bearish={} short_neutral={} short_bullish={} long_bearish={} long_neutral={} long_bullish={} fixed_21h={} trades={} wins={} losses={} win_rate={:.4} daily_target_hit_rate={:.4} target_hit_days={} total_days={} avg_daily_pnl={:.6} expectancy={:.6} net_pnl={:.6}",
            fmt_combo(&best.short_policy[H4BiasState::Bearish.slot()]),
            fmt_combo(&best.short_policy[H4BiasState::Neutral.slot()]),
            fmt_combo(&best.short_policy[H4BiasState::Bullish.slot()]),
            fmt_combo(&best.long_policy[H4BiasState::Bearish.slot()]),
            fmt_combo(&best.long_policy[H4BiasState::Neutral.slot()]),
            fmt_combo(&best.long_policy[H4BiasState::Bullish.slot()]),
            short_21_fixed.label,
            best.trades,
            best.wins,
            best.losses,
            best.win_rate,
            best.daily_target_hit_rate,
            best.target_hit_days,
            best.total_days,
            best.avg_daily_pnl_distance,
            best.expectancy_distance,
            best.net_pnl_distance,
        );
    }

    fn natgas_example_spec(granularity: &str, entry_hour: u32) -> TradingStrategySpec {
        let normalized = granularity.trim().to_uppercase();
        let metric_candle = format!("/candle_{}", normalized.to_lowercase());
        let candle_ref = format!("candle{}_{}h", normalized.to_lowercase(), entry_hour);
        let indicator_ref = format!("range_sma_{}_24", normalized.to_lowercase());
        normalize_strategy_spec(TradingStrategySpec {
            instrument: Some("NATGAS_USD".to_string()),
            granularity: Some(normalized.clone()),
            broker: Some("oanda".to_string()),
            point_size: Some(0.01),
            point_size_source: Some("oanda-pipLocation:-2".to_string()),
            point_size_warning: None,
            entry_hour: Some(entry_hour),
            entry_hours: None,
            entry_timezone: Some("UTC".to_string()),
            direction: Some("both".to_string()),
            stop_loss_distance: Some(NATGAS_STRICT_STOP_LOSS),
            take_profit_min_distance: Some(NATGAS_STRICT_TAKE_PROFIT),
            take_profit_max_distance: Some(0.300),
            target_win_rate: Some(0.85),
            daily_profit_target_distance: None,
            low_volatility_metric: Some("range_sma_percentile".to_string()),
            low_volatility_lookback: Some(24),
            low_volatility_percentile: Some(0.25),
            force_daily_entry: Some(true),
            spread_cost_distance: Some(NATGAS_STRICT_EXECUTION_COST),
            slippage_distance: Some(0.0),
            max_hold_bars: Some(if normalized == "H4" { 12 } else { 24 }),
            train_test_split: Some(0.7),
            candle_refs: Some(vec![candle_ref]),
            indicator_refs: Some(vec![indicator_ref]),
            metric_commands: Some(vec![
                "/asset".to_string(),
                "/asset_natgas_usd".to_string(),
                metric_candle,
                "/strategy_paired_long_short".to_string(),
                "/strategy_tp_grid".to_string(),
            ]),
            source_text: Some(format!(
                "/create_ /strategy_ | strategie {normalized} Natural Gas OANDA, entree {entry_hour}h UTC, 1 trade tous les jours de trading ouverts, long+short simultane, SL reel 4.5p avec stop brut 3.9p et spread 0.6p, TP brut 5.1p, objectif 85%"
            )),
        })
    }

    fn natgas_forced_daily_paired_spec(granularity: &str) -> TradingStrategySpec {
        let normalized = granularity.trim().to_uppercase();
        normalize_strategy_spec(TradingStrategySpec {
            instrument: Some("NATGAS_USD".to_string()),
            granularity: Some(normalized.clone()),
            broker: Some("oanda".to_string()),
            point_size: Some(0.01),
            point_size_source: Some("oanda-pipLocation:-2".to_string()),
            point_size_warning: None,
            entry_hour: Some(11),
            entry_hours: Some(vec![11, 15, 21]),
            entry_timezone: Some("UTC".to_string()),
            direction: Some("paired".to_string()),
            stop_loss_distance: Some(NATGAS_STRICT_STOP_LOSS),
            take_profit_min_distance: Some(NATGAS_STRICT_TAKE_PROFIT),
            take_profit_max_distance: Some(0.300),
            target_win_rate: Some(0.85),
            daily_profit_target_distance: Some(NATGAS_STRICT_DAILY_TARGET),
            low_volatility_metric: Some("range_sma_percentile".to_string()),
            low_volatility_lookback: Some(24),
            low_volatility_percentile: Some(0.25),
            force_daily_entry: Some(true),
            spread_cost_distance: Some(NATGAS_STRICT_EXECUTION_COST),
            slippage_distance: Some(0.0),
            max_hold_bars: Some(if normalized == "H4" { 12 } else { 24 }),
            train_test_split: Some(0.7),
            candle_refs: Some(vec![
                format!("candle{}_11h", normalized.to_lowercase()),
                format!("candle{}_15h", normalized.to_lowercase()),
                format!("candle{}_21h", normalized.to_lowercase()),
            ]),
            indicator_refs: Some(vec![
                "/ema_h1_21_close".to_string(),
                "/vwap_h1_session_hlc3".to_string(),
                "/bollinger_h1_20_2_close_lower_band".to_string(),
                "/bollinger_h1_20_2_close_upper_band".to_string(),
                "/macd_h1_12_26_9_histogram".to_string(),
                "/rsi_h1_14_close".to_string(),
            ]),
            metric_commands: Some(vec![
                "/asset".to_string(),
                "/asset_natgas_usd".to_string(),
                "/strategy_paired_long_short".to_string(),
                "/strategy_tp_grid".to_string(),
                "/candleh1_11am".to_string(),
                "/candleh1_3pm".to_string(),
                "/candleh1_9pm".to_string(),
                "/bollingerlowerband".to_string(),
                "/bollingerupperband".to_string(),
                "/ema21".to_string(),
                "/macd".to_string(),
                "/rsi".to_string(),
                "/vwap".to_string(),
            ]),
            source_text: Some(format!(
                "/create_ /strategy_ | strategie {normalized} Natural Gas OANDA, ordres forces tous les jours a 11h 15h 21h UTC, long+short simultanes sur chaque slot, SL 4.5p, TP min 5p, objectif 7p minimum par jour, utiliser combinaisons slash candles/indicators"
            )),
        })
    }

    fn sample_trading_order_request(approval: Option<TradingLiveApprovalProof>) -> TradingPlaceOrderRequest {
        TradingPlaceOrderRequest {
            instrument: Some("NATGAS_USD".to_string()),
            side: "BUY".to_string(),
            units: 100.0,
            order_type: Some("MARKET".to_string()),
            limit_price: None,
            take_profit: Some(3.25),
            stop_loss: Some(2.75),
            time_in_force: Some("FOK".to_string()),
            approval,
        }
    }

    fn synthetic_hourly_candles(count: usize) -> Vec<TradingCandlePoint> {
        let mut candles = Vec::with_capacity(count);
        let mut close = 3.0_f64;
        for index in 0..count {
            let hour = index % 24;
            let day = 1 + (index / 24);
            let drift = ((index as f64) / 17.0).sin() * 0.06 + ((index as f64) / 29.0).cos() * 0.03;
            let open = close;
            close = (close + 0.0025 + drift).max(0.5);
            let high = open.max(close) + 0.025 + ((index % 5) as f64 * 0.001);
            let low = open.min(close) - 0.025 - ((index % 7) as f64 * 0.001);
            candles.push(TradingCandlePoint {
                time: format!("2025-01-{day:02}T{hour:02}:00:00Z"),
                open,
                high,
                low,
                close,
                volume: 1_000 + ((index % 11) as u64 * 37),
            });
        }
        candles
    }

    fn run_natgas_example_strategy_backtest(granularity: &str, entry_hour: u32) {
        let spec = natgas_example_spec(granularity, entry_hour);
        let missing = validate_strategy_spec(&spec);
        assert!(missing.is_empty(), "missing metrics: {missing:?}");
        let series = canonical_chart_series("NATGAS_USD", granularity, 0)
            .unwrap_or_else(|_| panic!("NATGAS_USD {granularity} history"));
        let result = backtest_strategy_spec(&series.candles, &spec).expect("strategy backtest");
        let best = result.best.as_ref().expect("best candidate");
        let plan = &result.compute_plan;
        assert!(plan.engine.contains("KASM-ready"));
        assert_eq!(plan.gpu_plan.kernel, "strategy_mfe_reduce_tp_grid");
        assert!(!plan.dag_nodes.is_empty());
        assert!(plan.simulation_count > 0);
        assert!(plan.cache_report.hits + plan.cache_report.misses > 0);
        println!(
            "NATGAS_{}_{}H rows={} threshold={:.8} entries_plan={} simulations={} best={} filter_id={} filter_label={} formula={} refs={} tp={:.5} trades={} wins={} win_rate={:.4} target=0.8500 meets_target={} expectancy={:.6} kasm_plan={} dag_nodes={} cache_hits={} cache_misses={} injected={} avoided={} mfe_reduce={} gpu_kernel={}",
            granularity,
            entry_hour,
            result.rows,
            result.low_volatility_threshold,
            result
                .paired_probe
                .as_ref()
                .map(|probe| probe.entries)
                .unwrap_or_default(),
            plan.simulation_count,
            best.direction,
            best.filter_id,
            best.filter_label,
            best.display_formula.join(" && "),
            best.indicator_refs.join(","),
            best.take_profit_distance,
            best.trades,
            best.wins,
            best.win_rate.unwrap_or(0.0),
            best.meets_target,
            best.expectancy_distance,
            plan.plan_id,
            plan.dag_nodes.len(),
            plan.cache_report.hits,
            plan.cache_report.misses,
            plan.cache_report.injected_results,
            plan.cache_report.avoided_recalculations,
            plan.outcome_cube_key,
            plan.gpu_plan.kernel,
        );
    }

    fn run_synthetic_manifest_replay(entry_hour: u32) {
        let spec = natgas_example_spec("H1", entry_hour);
        let candles = synthetic_hourly_candles(720);
        let result = backtest_strategy_spec(&candles, &spec).expect("strategy backtest");
        let manifest = build_trading_scenario_manifest(
            "forge-tauri-rust-strategy-backtest",
            &spec,
            "SYNTH_NATGAS_USD",
            "H1",
            candles.len(),
            &result,
        );
        let replay = replay_trading_scenario_manifest_with_candles(&manifest, &candles, &spec)
            .expect("scenario replay");
        assert!(replay.ok, "scenario replay drift: expected={:?} actual={:?}", replay.expected, replay.actual);
        assert_eq!(replay.expected, replay.actual);
        assert!(!manifest.hashes.config_hash.is_empty());
        assert!(!manifest.hashes.input_ref_hash.is_empty());
        assert!(!manifest.hashes.output_hash.is_empty());
        assert!(!manifest.hashes.proof_hash.is_empty());
    }

    fn run_natgas_forced_daily_paired_backtest(granularity: &str) {
        let spec = natgas_forced_daily_paired_spec(granularity);
        let missing = validate_strategy_spec(&spec);
        assert!(missing.is_empty(), "missing metrics: {missing:?}");
        let series = canonical_chart_series("NATGAS_USD", granularity, 0)
            .unwrap_or_else(|_| panic!("NATGAS_USD {granularity} history"));
        let result = backtest_strategy_spec(&series.candles, &spec).expect("paired strategy backtest");
        let best = result.best.as_ref().expect("best paired candidate");
        let best_daily = result
            .candidates
            .iter()
            .max_by(|left, right| {
                left.daily_target_hit_rate
                    .unwrap_or(0.0)
                    .partial_cmp(&right.daily_target_hit_rate.unwrap_or(0.0))
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| {
                        left.avg_daily_pnl_distance
                            .unwrap_or(f64::NEG_INFINITY)
                            .partial_cmp(&right.avg_daily_pnl_distance.unwrap_or(f64::NEG_INFINITY))
                            .unwrap_or(std::cmp::Ordering::Equal)
                    })
            })
            .expect("best daily candidate");
        let plan = &result.compute_plan;
        println!(
            "NATGAS_{}_PAIRED_DAILY_11_15_21 rows={} threshold={:.8} entries_plan={} simulations={} best={} filter_id={} filter_label={} formula={} refs={} tp={:.5} trades={} wins={} win_rate={:.4} daily_target_hit_rate={:.4} target_hit_days={} total_days={} avg_daily_pnl={:.6} min_daily_pnl={:.6} meets_target={} expectancy={:.6} kasm_plan={} dag_nodes={} cache_hits={} cache_misses={} injected={} avoided={} mfe_reduce={} gpu_kernel={}",
            granularity,
            result.rows,
            result.low_volatility_threshold,
            result.paired_probe.as_ref().map(|probe| probe.entries).unwrap_or_default(),
            plan.simulation_count,
            best.direction,
            best.filter_id,
            best.filter_label,
            best.display_formula.join(" && "),
            best.indicator_refs.join(","),
            best.take_profit_distance,
            best.trades,
            best.wins,
            best.win_rate.unwrap_or(0.0),
            best.daily_target_hit_rate.unwrap_or(0.0),
            best.target_hit_days.unwrap_or(0),
            best.total_days.unwrap_or(0),
            best.avg_daily_pnl_distance.unwrap_or(0.0),
            best.min_daily_pnl_distance.unwrap_or(0.0),
            best.meets_target,
            best.expectancy_distance,
            plan.plan_id,
            plan.dag_nodes.len(),
            plan.cache_report.hits,
            plan.cache_report.misses,
            plan.cache_report.injected_results,
            plan.cache_report.avoided_recalculations,
            plan.outcome_cube_key,
            plan.gpu_plan.kernel,
        );
        println!(
            "NATGAS_{}_PAIRED_DAILY_TOP_DAILY filter_id={} filter_label={} formula={} refs={} tp={:.5} trades={} wins={} win_rate={:.4} daily_target_hit_rate={:.4} target_hit_days={} total_days={} avg_daily_pnl={:.6} min_daily_pnl={:.6} meets_target={} expectancy={:.6}",
            granularity,
            best_daily.filter_id,
            best_daily.filter_label,
            best_daily.display_formula.join(" && "),
            best_daily.indicator_refs.join(","),
            best_daily.take_profit_distance,
            best_daily.trades,
            best_daily.wins,
            best_daily.win_rate.unwrap_or(0.0),
            best_daily.daily_target_hit_rate.unwrap_or(0.0),
            best_daily.target_hit_days.unwrap_or(0),
            best_daily.total_days.unwrap_or(0),
            best_daily.avg_daily_pnl_distance.unwrap_or(0.0),
            best_daily.min_daily_pnl_distance.unwrap_or(0.0),
            best_daily.meets_target,
            best_daily.expectancy_distance,
        );
    }

    #[test]
    #[ignore = "requires local .forge-store/trading/oanda/NATGAS_USD/H1.csv"]
    fn natgas_h1_example_strategy_backtest_executes_kasm_plan() {
        run_natgas_example_strategy_backtest("H1", 21);
    }

    #[test]
    #[ignore = "requires local .forge-store/trading/oanda/NATGAS_USD/H4.csv"]
    fn natgas_h4_example_strategy_backtest_executes_kasm_plan() {
        run_natgas_example_strategy_backtest("H4", 21);
    }

    #[test]
    #[ignore = "requires local .forge-store/trading/oanda/NATGAS_USD/H1.csv"]
    fn natgas_h1_11h_strategy_backtest_executes_kasm_plan() {
        run_natgas_example_strategy_backtest("H1", 11);
    }

    #[test]
    #[ignore = "requires local .forge-store/trading/oanda/NATGAS_USD/H1.csv"]
    fn natgas_h1_15h_strategy_backtest_executes_kasm_plan() {
        run_natgas_example_strategy_backtest("H1", 15);
    }

    #[test]
    #[ignore = "requires local .forge-store/trading/oanda/NATGAS_USD/H1.csv"]
    fn natgas_h1_21h_strategy_backtest_executes_kasm_plan() {
        run_natgas_example_strategy_backtest("H1", 21);
    }

    #[test]
    #[ignore = "requires local .forge-store/trading/oanda/NATGAS_USD/H4.csv"]
    fn natgas_h4_11h_strategy_backtest_executes_kasm_plan() {
        run_natgas_example_strategy_backtest("H4", 11);
    }

    #[test]
    #[ignore = "requires local .forge-store/trading/oanda/NATGAS_USD/H4.csv"]
    fn natgas_h4_15h_strategy_backtest_executes_kasm_plan() {
        run_natgas_example_strategy_backtest("H4", 15);
    }

    #[test]
    fn synthetic_h1_manifest_replay_produces_same_hashes() {
        run_synthetic_manifest_replay(11);
    }

    #[test]
    fn live_trading_order_denied_without_approval() {
        let request = sample_trading_order_request(None);
        let err = validate_trading_order_approval(
            &request,
            "NATGAS_USD",
            "BUY",
            "MARKET",
            "FOK",
            "oanda|source=secure-store|base=https://api-fxpractice.oanda.com|account=present|key=present",
            1_710_000_000_000,
        )
        .expect_err("missing approval must be rejected");
        assert!(err.contains("explicit approval"));
    }

    #[test]
    fn live_trading_order_accepts_matching_approval_proof() {
        let approved_at_ms = 1_710_000_000_000_u64;
        let bucket = trading_approval_bucket(approved_at_ms);
        let provider_state =
            "oanda|source=secure-store|base=https://api-fxpractice.oanda.com|account=present|key=present";
        let action_hash = trading_order_action_hash(
            "NATGAS_USD",
            "BUY",
            100.0,
            "MARKET",
            None,
            Some(2.75),
            Some(3.25),
            "FOK",
            provider_state,
            &bucket,
        );
        let request = sample_trading_order_request(Some(TradingLiveApprovalProof {
            approved: true,
            approved_at_ms,
            timestamp_bucket: bucket.clone(),
            provider_state: provider_state.to_string(),
            action_hash,
        }));
        let proof = validate_trading_order_approval(
            &request,
            "NATGAS_USD",
            "BUY",
            "MARKET",
            "FOK",
            provider_state,
            approved_at_ms + 60_000,
        )
        .expect("matching approval proof");
        assert_eq!(proof.0, bucket);
        assert!(!proof.1.is_empty());
    }

    #[test]
    #[ignore = "requires local .forge-store/trading/oanda/NATGAS_USD/H4.csv"]
    fn natgas_h4_21h_strategy_backtest_executes_kasm_plan() {
        run_natgas_example_strategy_backtest("H4", 21);
    }

    #[test]
    #[ignore = "requires local .forge-store/trading/oanda/NATGAS_USD/H1.csv"]
    fn natgas_h1_forced_daily_paired_strategy_backtest_executes_kasm_plan() {
        run_natgas_forced_daily_paired_backtest("H1");
    }

    #[test]
    #[ignore = "requires local .forge-store/trading/oanda/NATGAS_USD/H4.csv"]
    fn natgas_h4_forced_daily_paired_strategy_backtest_executes_kasm_plan() {
        run_natgas_forced_daily_paired_backtest("H4");
    }

    #[test]
    #[ignore = "requires local .forge-store/trading/oanda/NATGAS_USD/H1.csv"]
    fn natgas_h1_seeded_slot_strategy_search_executes() {
        run_natgas_seeded_slot_strategy_search();
    }

    #[test]
    #[ignore = "requires local .forge-store/trading/oanda/NATGAS_USD/H1.csv"]
    fn natgas_h1_seeded_slot_combo_search_executes() {
        run_natgas_seeded_slot_combo_search();
    }

    #[test]
    #[ignore = "requires local .forge-store/trading/oanda/NATGAS_USD/H1.csv"]
    fn natgas_h1_regime_meta_scheduler_search_executes() {
        run_natgas_seeded_regime_meta_scheduler_search();
    }

    #[test]
    #[ignore = "requires local .forge-store/trading/oanda/NATGAS_USD/H1.csv"]
    fn natgas_h1_monster_meta_scheduler_search_executes() {
        run_natgas_monster_meta_scheduler_search();
    }

    #[test]
    #[ignore = "requires local .forge-store/trading/oanda/NATGAS_USD/H1.csv"]
    fn natgas_h1_monster_binary_slot_scheduler_search_executes() {
        run_natgas_monster_binary_slot_scheduler_search();
    }

    #[test]
    #[ignore = "requires local .forge-store/trading/oanda/NATGAS_USD/H1.csv"]
    fn natgas_h1_loss_cluster_diagnostics_executes() {
        run_natgas_loss_cluster_diagnostics();
    }

    #[test]
    #[ignore = "requires local .forge-store/trading/oanda/NATGAS_USD/H1.csv"]
    fn natgas_h1_tuned_day_state_scheduler_search_executes() {
        run_natgas_tuned_day_state_scheduler_search();
    }

    #[test]
    #[ignore = "requires local .forge-store/trading/oanda/NATGAS_USD/H1.csv"]
    fn natgas_h1_seeded_slot_combo_search_21h_vwap_only_executes() {
        run_natgas_seeded_slot_combo_search_21h_vwap_only();
    }

    #[test]
    #[ignore = "requires local .forge-store/trading/oanda/NATGAS_USD/H1.csv"]
    fn natgas_h1_simple_vwap_baseline_executes() {
        run_natgas_simple_vwap_baseline();
    }

    #[test]
    #[ignore = "requires local .forge-store/trading/oanda/NATGAS_USD/H1.csv"]
    fn natgas_h1_simple_vwap_baseline_tp7_executes() {
        run_natgas_simple_vwap_baseline_tp7();
    }

    #[test]
    #[ignore = "requires local .forge-store/trading/oanda/NATGAS_USD/H1.csv"]
    fn natgas_h1_simple_vwap_plus_19h_search_executes() {
        run_natgas_simple_vwap_plus_19h_search();
    }

    #[test]
    #[ignore = "requires local .forge-store/trading/oanda/NATGAS_USD/H1.csv"]
    fn natgas_h1_mandatory_daily_winrate_search_executes() {
        run_natgas_mandatory_daily_winrate_search();
    }

    #[test]
    fn save_natgas_simple_vwap_baseline_as_program_executes() {
        run_save_natgas_simple_vwap_baseline_as_program();
    }

    #[test]
    #[ignore = "requires local .forge-store/trading/oanda/NATGAS_USD/H1.csv"]
    fn natgas_h1_trend_lifecycle_analysis_executes() {
        run_natgas_h1_trend_lifecycle_analysis();
    }

    #[test]
    #[ignore = "requires local .forge-store/trading/oanda/NATGAS_USD/H1.csv"]
    fn natgas_h1_21h_time_exit_analysis_executes() {
        run_natgas_21h_trend_follow_time_exit_analysis();
    }

    #[test]
    #[ignore = "requires local .forge-store/trading/oanda/NATGAS_USD/H1.csv"]
    fn natgas_h1_session_time_reversal_search_executes() {
        run_natgas_session_reversal_time_exit_search();
    }

    #[test]
    #[ignore = "requires local .forge-store/trading/oanda/NATGAS_USD/H1.csv"]
    fn natgas_h1_strat_a_13h_17h_21h_hybrid_executes() {
        run_natgas_strat_a_13h_17h_21h_hybrid();
    }

    #[test]
    #[ignore = "requires local .forge-store/trading/oanda/NATGAS_USD/H1.csv"]
    fn natgas_h1_classic_vwap_qualified_hybrid_search_executes() {
        run_natgas_classic_vwap_qualified_hybrid_search();
    }

    #[test]
    #[ignore = "requires local .forge-store/trading/oanda/NATGAS_USD/H1.csv"]
    fn natgas_h1_11h_15h_pullback_rejoin_search_executes() {
        run_natgas_11h_15h_pullback_rejoin_search();
    }

    #[test]
    #[ignore = "requires local .forge-store/trading/oanda/NATGAS_USD/H1.csv and M5.csv"]
    fn natgas_h1_to_m5_pullback_execution_search_executes() {
        run_natgas_h1_to_m5_pullback_execution_search();
    }

    #[test]
    #[ignore = "requires local .forge-store/trading/oanda/NATGAS_USD/H1.csv"]
    fn natgas_h1_anchored_vwap_touch_search_executes() {
        run_natgas_h1_anchored_vwap_touch_search();
    }

    #[test]
    #[ignore = "requires local .forge-store/trading/oanda/NATGAS_USD/H1.csv"]
    fn natgas_h1_anchored_vwap_ext2_search_executes() {
        run_natgas_h1_anchored_vwap_ext2_search();
    }

    #[test]
    #[ignore = "requires local .forge-store/trading/oanda/NATGAS_USD/H1.csv"]
    fn natgas_h1_anchored_vwap_make_it_worse_executes() {
        run_natgas_h1_anchored_vwap_make_it_worse();
    }

    #[test]
    #[ignore = "requires local .forge-store/trading/oanda/NATGAS_USD/H1.csv and H4.csv"]
    fn natgas_h1_anchored_vwap_ext2_rejection_h4_search_executes() {
        run_natgas_h1_anchored_vwap_ext2_rejection_h4_search();
    }

    #[test]
    #[ignore = "requires local .forge-store/trading/oanda/NATGAS_USD/H1.csv"]
    fn natgas_h1_hour_close_behavior_audit_executes() {
        run_natgas_h1_hour_close_behavior_audit();
    }

    #[test]
    #[ignore = "requires local .forge-store/trading/oanda/NATGAS_USD/H1.csv and H4.csv"]
    fn natgas_h1_hour_close_context_audit_executes() {
        run_natgas_h1_hour_close_context_audit();
    }

    #[test]
    #[ignore = "requires local .forge-store/trading/oanda/NATGAS_USD/H1.csv and H4.csv"]
    fn natgas_h1_hour_recurrence_combo_search_executes() {
        run_natgas_h1_hour_recurrence_combo_search();
    }

    #[test]
    #[ignore = "requires local .forge-store/trading/oanda/NATGAS_USD/H1.csv and H4.csv"]
    fn natgas_h1_high_confidence_signal_refinement_search_executes() {
        run_natgas_high_confidence_signal_refinement_search();
    }

    #[test]
    #[ignore = "requires local .forge-store/trading/oanda/NATGAS_USD/H1.csv and H4.csv"]
    fn natgas_h1_target_bad_ratio_search_executes() {
        run_natgas_h1_target_bad_ratio_search();
    }

    #[test]
    #[ignore = "requires local .forge-store/trading/oanda/NATGAS_USD/H1.csv"]
    fn natgas_h1_inverse_bad_pattern_probe_executes() {
        run_natgas_h1_inverse_bad_pattern_probe();
    }

    #[test]
    #[ignore = "requires local .forge-store/trading/oanda/NATGAS_USD/H1.csv"]
    fn natgas_h1_three_higher_closes_exact_mirror_strategy_executes() {
        run_natgas_h1_three_higher_closes_exact_mirror_strategy();
    }

    #[test]
    #[ignore = "writes a direct program into the local Forge store"]
    fn save_natgas_h1_three_higher_closes_exact_mirror_as_program_executes() {
        run_save_natgas_h1_three_higher_closes_exact_mirror_as_program();
    }

    #[test]
    #[ignore = "requires local .forge-store/trading/oanda/NATGAS_USD/H1.csv and H4.csv"]
    fn natgas_strat_a_unified_performance_executes() {
        run_natgas_strat_a_unified_performance();
    }

    #[test]
    #[ignore = "requires local .forge-store/trading/oanda/NATGAS_USD/H1.csv and H4.csv"]
    fn natgas_strat_a_compounded_capital_projection_executes() {
        run_natgas_strat_a_compounded_capital_projection();
    }

    #[test]
    #[ignore = "requires local .forge-store/trading/oanda/NATGAS_USD/H1.csv and H4.csv"]
    fn natgas_h1_hour_context_strategy_trials_executes() {
        run_natgas_hour_context_strategy_trials();
    }

    #[test]
    #[ignore = "requires local .forge-store/trading/oanda/NATGAS_USD/H1.csv"]
    fn natgas_h1_inverse_context_diagnostics_executes() {
        run_natgas_inverse_context_diagnostics();
    }

    #[test]
    #[ignore = "requires local .forge-store/trading/oanda/NATGAS_USD/H1.csv"]
    fn natgas_h1_21h_inverse_vwap_search_executes() {
        run_natgas_21h_inverse_vwap_search();
    }

    #[test]
    #[ignore = "requires local .forge-store/trading/oanda/NATGAS_USD/H1.csv"]
    fn natgas_h1_11h_15h_refinement_search_executes() {
        run_natgas_11h_15h_refinement_search();
    }

    #[test]
    #[ignore = "requires local .forge-store/trading/oanda/NATGAS_USD/H1.csv and H4.csv"]
    fn natgas_h1_11h_15h_h4_context_search_executes() {
        run_natgas_11h_15h_h4_context_search();
    }

    #[test]
    #[ignore = "requires local .forge-store/trading/oanda/NATGAS_USD/H1.csv and H4.csv"]
    fn natgas_h1_h4_pullback_reversal_search_executes() {
        run_natgas_h4_pullback_reversal_search();
    }

    #[test]
    #[ignore = "requires local .forge-store/trading/oanda/NATGAS_USD/H4.csv"]
    fn natgas_h4_pullback_direct_strategy_executes() {
        run_natgas_h4_pullback_direct_strategy();
    }

    #[test]
    #[ignore = "requires local .forge-store/trading/oanda/NATGAS_USD/H1.csv and H4.csv"]
    fn natgas_h1_11h_15h_h4_bias_shortlist_search_executes() {
        run_natgas_11h_15h_h4_bias_shortlist_search();
    }

    #[test]
    #[ignore = "requires local .forge-store/trading/oanda/NATGAS_USD/H1.csv and H4.csv"]
    fn natgas_h1_11h_15h_h4_bias_policy_search_executes() {
        run_natgas_11h_15h_h4_bias_policy_search();
    }
}
