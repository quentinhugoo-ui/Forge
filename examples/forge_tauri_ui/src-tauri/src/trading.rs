use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
#[cfg(not(target_os = "windows"))]
use keyring::{Entry as KeyringEntry, Error as KeyringError};
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, VecDeque};
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

const DEFAULT_BASE_URL: &str = "https://api-fxpractice.oanda.com";
const DEFAULT_INSTRUMENT: &str = "NATGAS_USD";
const KEEP_STORED_SENTINEL: &str = "__FORGE_KEEP_STORED__";
const HISTORY_START_RFC3339: &str = "2006-01-01T00:00:00Z";
const OANDA_WATCHDOG_TICK_MS: u64 = 30_000;
const OANDA_WATCHDOG_HEARTBEAT_MS: u64 = 5 * 60 * 1000;
const OANDA_WATCHDOG_RECOVERY_MS: u64 = 20_000;
const UNIVERSE_SYNC_GRANULARITIES: &[&str] = &["S10", "S30", "M1", "M5", "M15", "M30", "H1", "H4", "D", "W"];
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
    entry_timezone: Option<String>,
    direction: Option<String>,
    stop_loss_distance: Option<f64>,
    take_profit_min_distance: Option<f64>,
    take_profit_max_distance: Option<f64>,
    target_win_rate: Option<f64>,
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

    let granularity_rank = |value: &str| {
        UNIVERSE_SYNC_GRANULARITIES
            .iter()
            .position(|candidate| candidate.eq_ignore_ascii_case(value))
            .unwrap_or(usize::MAX)
    };

    let mut assets = by_instrument.into_values().collect::<Vec<_>>();
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
    if spec.entry_hour.is_none_or(|hour| hour > 23) {
        push_strategy_missing(
            &mut missing,
            "entry_hour",
            "Heure d'entrée",
            "A quelle heure exacte faut-il ouvrir le trade ?",
            "Une stratégie horaire doit utiliser une heure stable, bornée entre 0 et 23.",
            &["21h", "21:00 UTC"],
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
        "long" | "short" | "buy" | "sell" | "both" | "auto"
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
        "entryTimezone": spec.entry_timezone,
        "direction": spec.direction,
        "stopLossDistance": spec.stop_loss_distance,
        "takeProfitMinDistance": spec.take_profit_min_distance,
        "takeProfitMaxDistance": spec.take_profit_max_distance,
        "targetWinRate": spec.target_win_rate,
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
    TradingStrategyTemplate {
        template_id: format!("strategy-template-{}", &template_hash[..16.min(template_hash.len())]),
        command: "/strategy_".to_string(),
        family: if spec.force_daily_entry.unwrap_or(false) {
            "timed_daily_entry_grid".to_string()
        } else {
            "timed_low_volatility_grid".to_string()
        },
        instrument: spec.instrument.as_deref().unwrap_or(DEFAULT_INSTRUMENT).to_string(),
        granularity: spec.granularity.as_deref().unwrap_or("H1").to_string(),
        broker: spec.broker.as_deref().unwrap_or("oanda").to_string(),
        direction: spec.direction.as_deref().unwrap_or("both").to_string(),
        entry_hour_utc: spec.entry_hour.unwrap_or(21),
        target_win_rate: spec.target_win_rate,
        parameter_hash: template_hash.to_string(),
        data_hash: data_hash.to_string(),
    }
}

fn strategy_plan_lines(spec: &TradingStrategySpec) -> Vec<String> {
    vec![
        "runner=forge-tauri-rust-strategy-backtest".to_string(),
        "kasm_plan=slash metric manifest, condition bytecode, stable cache keys, paired long/short probe".to_string(),
        format!(
            "data={} {} from local canonical history CSV",
            spec.instrument.as_deref().unwrap_or("n/a"),
            spec.granularity.as_deref().unwrap_or("n/a")
        ),
        format!(
            "entry={} when UTC hour == {}",
            if spec.force_daily_entry.unwrap_or(false) {
                "force one trade per open trading day"
            } else {
                "once per candle"
            },
            spec.entry_hour
                .map(|value| value.to_string())
                .unwrap_or_else(|| "n/a".to_string())
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
        _ => vec!["long".to_string(), "short".to_string()],
    }
}

#[derive(Debug, Clone)]
struct StrategyBacktestTrade {
    entry_time: String,
    pnl_distance: f64,
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
}

fn strategy_entry_indices(
    candles: &[TradingCandlePoint],
    low_volatility_values: &[f64],
    threshold: f64,
    start_index: usize,
    end_index: usize,
    entry_hour: u32,
    force_daily_entry: bool,
) -> Vec<usize> {
    if candles.len() < 2 || start_index >= candles.len() {
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
        if strategy_hour_utc(&candles[index].time) != Some(entry_hour) {
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
    entry_hour: u32,
    force_daily_entry: bool,
    data_hash: &str,
    cache_snapshot: &mut StrategyCacheSnapshot,
) -> (Vec<usize>, String) {
    let key = format!(
        "entry-scan-{}",
        &strategy_sha256(&format!(
            "{data_hash}|{threshold:.10}|{start_index}|{end_index}|{entry_hour}|force_daily={force_daily_entry}|{}",
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
        entry_hour,
        force_daily_entry,
    );
    if let Ok(mut cache) = trading_strategy_entry_scan_cache().lock() {
        cache.insert(key.clone(), entries.clone());
    }
    strategy_write_cache_marker(&key, json!({
        "node": "entry_scan",
        "rows": candles.len(),
        "entries": entries.len(),
        "entryHourUtc": entry_hour,
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
        vwap: strategy_vwap(candles),
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
            "Bollinger20x2", "VWAP", "RSI14", "ATR14", "MACD12_26_9",
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
    entry_hour: u32,
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
        entry_hour,
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
    entry_hour: u32,
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
        entry_hour,
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
        entry_hour,
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
                entry_hour,
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
            entry_hour,
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
    entry_hour: u32,
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
    let key = format!(
        "robustness-{}",
        &strategy_sha256(&format!(
            "{data_hash}|{entries_cache_key}|{direction}|sl={stop_loss:.10}|tp={take_profit:.10}|cost={execution_cost:.10}|hold={max_hold}|target={target_win_rate:.6}|point={point_size:.10}|train={train_rows}|hour={entry_hour}|force_daily={force_daily_entry}|threshold={threshold:.10}"
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
        entry_hour,
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
        let win_rate = candidate.win_rate.unwrap_or(0.0) * 1_000.0;
        let expectancy = candidate.expectancy_distance * 100.0;
        let trade_depth = (candidate.trades as f64).ln_1p();
        target_bonus + robust_bonus + win_rate + expectancy + trade_depth
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
    let entry_hour = spec.entry_hour.unwrap_or(21);
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
        entry_hour,
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
                entry_hour,
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
                    "21h daily entries".to_string()
                } else {
                    "21h low-volatility entries".to_string()
                },
                condition_hash: base_mask.mask_hash.clone(),
                mask_ref: base_mask.mask_hash.clone(),
                bytecode_ops: vec![
                    "ENTRY:HOUR_EQ_21UTC".to_string(),
                    if force_daily_entry {
                        "ENTRY:FORCE_ONE_TRADE_PER_OPEN_DAY".to_string()
                    } else {
                        "VOL:RANGE_SMA_LE_TRAIN_Q".to_string()
                    },
                ],
                display_formula: vec![
                    "entry hour == 21 UTC".to_string(),
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
                        robustness: None,
                    });
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
            if candidate.filter_id == "base_low_vol" {
                return Some((test_entry_indices.clone(), shared_entry_scan_cache_key.clone()));
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
    let key = format!(
        "{}|{}|{}|{}|{}|{}|{}",
        spec.instrument.as_deref().unwrap_or(""),
        spec.granularity.as_deref().unwrap_or(""),
        spec.entry_hour.map(|value| value.to_string()).unwrap_or_default(),
        spec.direction.as_deref().unwrap_or(""),
        spec.stop_loss_distance.map(|value| value.to_string()).unwrap_or_default(),
        spec.take_profit_min_distance.map(|value| value.to_string()).unwrap_or_default(),
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
    Ok(TradingPriceSnapshot {
        instrument: instrument.to_string(),
        time,
        bid,
        ask,
        mid: (bid + ask) * 0.5,
        spread: ask - bid,
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

    let mut intraday = requested
        .iter()
        .filter(|value| oanda_is_intraday_rebuildable(value))
        .cloned()
        .collect::<Vec<_>>();
    intraday.sort_by_key(|value| granularity_step_ms(value).unwrap_or(i64::MAX));

    let source = intraday.first().cloned();
    let mut native = Vec::new();
    let mut derived = Vec::new();

    if let Some(source_granularity) = source.as_deref() {
        push_unique_granularity(&mut native, source_granularity);
    }

    for granularity in requested {
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
        UNIVERSE_SYNC_GRANULARITIES
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
    let asset_catalog = build_asset_catalog(&history_files);
    let history_dir = history_root_dir().display().to_string();

    let Some(credentials) = credentials else {
        clear_oanda_runtime();
        return Ok(TradingSnapshotResponse {
            config,
            account: None,
            price: None,
            instruments: vec![TradingInstrumentSummary {
                name: DEFAULT_INSTRUMENT.to_string(),
                display_name: "Natural Gas".to_string(),
                asset_class: "commodity".to_string(),
                pip_location: Some(-2),
                display_precision: Some(3),
                trade_units_precision: Some(0),
                minimum_trade_size: Some(1.0),
                margin_rate: Some(0.05),
            }],
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
            Ok(TradingSnapshotResponse {
                config,
                account: bundle.account,
                price: bundle.price,
                instruments: if bundle.instruments.is_empty() {
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
                    bundle.instruments
                },
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
    let credentials = resolve_credentials();
    let config = config_status_from_credentials(credentials.as_ref());
    let files = build_history_catalog();
    Ok(TradingSyncResponse {
        config,
        history_dir: history_root_dir().display().to_string(),
        assets: build_asset_catalog(&files),
        files,
        notes: vec![
            "Catalog only: no network call performed.".to_string(),
        ],
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
        assets: build_asset_catalog(&catalog_files),
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
            result: None,
        });
    }

    let instrument = spec.instrument.as_deref().unwrap_or(DEFAULT_INSTRUMENT);
    let granularity = spec.granularity.as_deref().unwrap_or("H1");
    let max_rows = request.max_rows.unwrap_or(0);
    let series = canonical_chart_series(instrument, granularity, max_rows)?;
    let result = backtest_strategy_spec(&series.candles, &spec)?;
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
        result: Some(result),
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
    let client = build_oanda_client()?;
    let headers = oanda_headers(&credentials.api_key)?;
    let instrument = request
        .instrument
        .unwrap_or_else(|| DEFAULT_INSTRUMENT.to_string())
        .trim()
        .to_string();
    let side = request.side.trim().to_uppercase();
    let order_type = request
        .order_type
        .unwrap_or_else(|| "MARKET".to_string())
        .trim()
        .to_uppercase();
    let tif = request
        .time_in_force
        .unwrap_or_else(|| if order_type == "LIMIT" { "GTC".to_string() } else { "FOK".to_string() })
        .trim()
        .to_uppercase();
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
        message: "Order submitted to OANDA.".to_string(),
        response: payload,
    })
}

#[cfg(test)]
mod strategy_tests {
    use super::*;

    #[test]
    #[ignore = "requires local .forge-store/trading/oanda/NATGAS_USD/H1.csv"]
    fn natgas_h1_example_strategy_backtest_executes_kasm_plan() {
        let source_text = "/create_ /strategy_ | strategie H1 Natural Gas OANDA, entree 21h UTC, 1 trade tous les jours de trading ouverts, faible volatilite en contexte, SL 4.5p, TP min 3.5p max 30 points, objectif 85%".to_string();
        let spec = normalize_strategy_spec(TradingStrategySpec {
            instrument: Some("NATGAS_USD".to_string()),
            granularity: Some("H1".to_string()),
            broker: Some("oanda".to_string()),
            point_size: Some(0.01),
            point_size_source: Some("oanda-pipLocation:-2".to_string()),
            point_size_warning: None,
            entry_hour: Some(21),
            entry_timezone: Some("UTC".to_string()),
            direction: Some("both".to_string()),
            stop_loss_distance: Some(0.045),
            take_profit_min_distance: Some(0.035),
            take_profit_max_distance: Some(0.300),
            target_win_rate: Some(0.85),
            low_volatility_metric: Some("range_sma_percentile".to_string()),
            low_volatility_lookback: Some(24),
            low_volatility_percentile: Some(0.25),
            force_daily_entry: Some(true),
            spread_cost_distance: Some(0.002),
            slippage_distance: Some(0.001),
            max_hold_bars: Some(24),
            train_test_split: Some(0.7),
            candle_refs: Some(vec!["candleh1_21h".to_string()]),
            indicator_refs: Some(vec!["range_sma_h1_24".to_string()]),
            metric_commands: Some(vec![
                "/asset".to_string(),
                "/asset_natgas_usd".to_string(),
                "/candle_h1".to_string(),
                "/strategy_paired_long_short".to_string(),
                "/strategy_tp_grid".to_string(),
            ]),
            source_text: Some(source_text),
        });
        let missing = validate_strategy_spec(&spec);
        assert!(missing.is_empty(), "missing metrics: {missing:?}");
        let series = canonical_chart_series("NATGAS_USD", "H1", 0).expect("NATGAS_USD H1 history");
        let result = backtest_strategy_spec(&series.candles, &spec).expect("strategy backtest");
        let best = result.best.as_ref().expect("best candidate");
        let plan = &result.compute_plan;
        assert!(plan.engine.contains("KASM-ready"));
        assert_eq!(plan.gpu_plan.kernel, "strategy_mfe_reduce_tp_grid");
        assert!(!plan.dag_nodes.is_empty());
        assert!(plan.simulation_count > 0);
        assert!(plan.cache_report.hits + plan.cache_report.misses > 0);
        println!(
            "NATGAS_H1_EXAMPLE rows={} threshold={:.8} entries_plan={} simulations={} best={} tp={:.5} trades={} wins={} win_rate={:.4} target=0.8500 meets_target={} expectancy={:.6} kasm_plan={} dag_nodes={} cache_hits={} cache_misses={} injected={} avoided={} mfe_reduce={} gpu_kernel={}",
            result.rows,
            result.low_volatility_threshold,
            result
                .paired_probe
                .as_ref()
                .map(|probe| probe.entries)
                .unwrap_or_default(),
            plan.simulation_count,
            best.direction,
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
}
