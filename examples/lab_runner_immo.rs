//! Real-estate agency lab runner.
//!
//! Direct bench for KASM-style content addressed agency computations:
//! identical authorized source snapshot + identical params => identical stage keys.
//! The second pass must show cache HIT logs and avoided work.

use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::fs;
use std::sync::Arc;
use std::time::{Duration, Instant};

const DEFAULT_PROPERTIES: usize = 24_000;
const DEFAULT_SCENARIOS: usize = 512;
const DEFAULT_REPEAT: usize = 2;
const DEFAULT_CANDIDATE_LIMIT: usize = 8_000;
const HORIZONS_DAYS: [u16; 4] = [30, 60, 90, 120];

const METRIC_NAMES: [&str; 64] = [
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

#[derive(Debug, Clone)]
struct Config {
    properties: usize,
    scenarios: usize,
    repeat: usize,
    candidate_limit: usize,
    focus: Focus,
    seed: u64,
    plan_only: bool,
    seeds_path: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            properties: DEFAULT_PROPERTIES,
            scenarios: DEFAULT_SCENARIOS,
            repeat: DEFAULT_REPEAT,
            candidate_limit: DEFAULT_CANDIDATE_LIMIT,
            focus: Focus::ScenarioDag,
            seed: 0xF0_46_E5_7A_7E_1A_00_01,
            plan_only: false,
            seeds_path: None,
        }
    }
}

#[derive(Debug, Clone)]
struct MetricSeedRecord {
    seed_hash: String,
    pack_proof_hash: String,
    pack_theme: String,
    priority_score: f64,
    source_density_score: f64,
    evidence_quality_score: f64,
    graph_density_score: f64,
    market_signal_score: f64,
    local_signal_score: f64,
    economic_signal_score: f64,
    data_gap_penalty: f64,
    actionability_score: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    ScenarioDag,
    MetricMatrix,
    CandidateScan,
}

#[derive(Debug, Clone, Copy)]
struct PropertyRecord {
    id: u64,
    zone_id: u16,
    lat: f64,
    lon: f64,
    surface_m2: f64,
    rooms: f64,
    age_years: f64,
    floor: f64,
    dvf_price_m2: f64,
    asking_price_m2: f64,
    dpe_score: f64,
    energy_cost_index: f64,
    clay_risk: f64,
    flood_risk: f64,
    permit_activity: f64,
    urbanism_upside: f64,
    transit_momentum: f64,
    school_momentum: f64,
    business_churn: f64,
    traffic_noise_delta: f64,
    pollution_delta: f64,
    weather_heat_stress: f64,
    mobility_inflow: f64,
    local_news_intensity: f64,
    competitor_pressure: f64,
    buyer_demand_match: f64,
    credit_rate_sensitivity: f64,
    insurance_risk: f64,
    tax_pressure_proxy: f64,
    crm_inactivity_days: f64,
    visit_intent: f64,
    owner_lifecycle_pressure: f64,
    rental_yield_gap: f64,
    work_cost_roi: f64,
    neighborhood_liquidity: f64,
    price_anchor_error: f64,
    days_on_market_shadow: f64,
    notary_delay_index: f64,
    seasonality_fit: f64,
    agency_reputation_fit: f64,
}

impl PropertyRecord {
    fn fiber_quality_proxy(self) -> f64 {
        (0.35 + self.transit_momentum * 0.22 + self.mobility_inflow * 0.18 + synthetic_metric(&self, 36) * 0.72)
            .clamp(0.0, 2.1)
    }

    fn tourism_proxy(self) -> f64 {
        (self.local_news_intensity * 0.24
            + self.mobility_inflow * 0.18
            + ((self.zone_id as f64) * 0.137).sin().abs() * 0.80
            + synthetic_metric(&self, 40) * 0.42)
            .clamp(0.0, 2.2)
    }

    fn student_proxy(self) -> f64 {
        (self.transit_momentum * 0.34
            + self.rental_yield_gap.max(0.0) * 0.28
            + ((self.zone_id as f64) * 0.071).cos().abs() * 0.55)
            .clamp(0.0, 2.0)
    }

    fn health_access_proxy(self) -> f64 {
        (1.15 - ((self.zone_id as f64 % 19.0) / 19.0)
            + self.transit_momentum * 0.18
            + synthetic_metric(&self, 42) * 0.34)
            .clamp(0.0, 2.0)
    }

    fn senior_services_proxy(self) -> f64 {
        (self.health_access_proxy() * 0.42
            + (self.age_years / 120.0).clamp(0.0, 1.0) * 0.30
            + synthetic_metric(&self, 43) * 0.72)
            .clamp(0.0, 2.0)
    }

    fn hiring_proxy(self) -> f64 {
        (self.business_churn * 0.20
            + self.mobility_inflow * 0.24
            + self.local_news_intensity * 0.18
            + synthetic_metric(&self, 48) * 0.86)
            .clamp(0.0, 2.2)
    }

    fn event_proxy(self) -> f64 {
        (self.local_news_intensity * 0.36
            + self.tourism_proxy() * 0.22
            + synthetic_metric(&self, 50) * 0.84)
            .clamp(0.0, 2.3)
    }

    fn daily_services_proxy(self) -> f64 {
        (self.neighborhood_liquidity * 0.38
            + self.school_momentum * 0.16
            + self.business_churn * 0.12
            + synthetic_metric(&self, 53) * 0.76)
            .clamp(0.0, 2.1)
    }

    fn green_space_proxy(self) -> f64 {
        (1.35 - self.pollution_delta * 0.26 - self.traffic_noise_delta * 0.14 + synthetic_metric(&self, 54) * 0.82)
            .clamp(0.0, 2.1)
    }

    fn sunlight_proxy(self) -> f64 {
        (0.60 + ((self.lat * 11.0 + self.lon * 7.0).sin() + 1.0) * 0.44 + synthetic_metric(&self, 55) * 0.42)
            .clamp(0.0, 2.1)
    }

    fn slope_walkability_proxy(self) -> f64 {
        (1.25 - ((self.lat * 9.0 - self.lon * 5.0).cos().abs() * 0.82) + self.transit_momentum * 0.12)
            .clamp(0.0, 2.0)
    }

    fn parking_pressure_proxy(self) -> f64 {
        ((self.rooms / 5.0) * 0.34
            + (1.3 - self.transit_momentum).max(0.0) * 0.30
            + synthetic_metric(&self, 57) * 0.88)
            .clamp(0.0, 2.2)
    }

    fn ev_charging_proxy(self) -> f64 {
        (self.urbanism_upside * 0.22
            + self.daily_services_proxy() * 0.24
            + synthetic_metric(&self, 58) * 0.78)
            .clamp(0.0, 2.2)
    }

    fn artisan_capacity_proxy(self) -> f64 {
        (self.business_churn * 0.18
            + self.local_news_intensity * 0.14
            + self.mobility_inflow * 0.20
            + synthetic_metric(&self, 60) * 0.90)
            .clamp(0.0, 2.2)
    }
}

#[derive(Debug, Clone)]
struct PropertyColumns {
    id: Vec<u64>,
    zone_id: Vec<u16>,
    lat: Vec<f32>,
    lon: Vec<f32>,
    surface_m2: Vec<f32>,
    dvf_price_m2: Vec<f32>,
    asking_price_m2: Vec<f32>,
    metric_inputs: Vec<PropertyRecord>,
}

#[derive(Debug, Clone)]
struct MetricMatrix {
    names: Vec<&'static str>,
    rows: usize,
    cols: usize,
    data: Vec<f32>,
}

#[derive(Debug, Clone)]
struct NormalizedMatrix {
    names: Vec<&'static str>,
    rows: usize,
    cols: usize,
    data: Vec<f32>,
    means: Vec<f32>,
    stds: Vec<f32>,
}

#[derive(Debug, Clone, Copy)]
struct OpportunityCandidate {
    row: usize,
    property_id: u64,
    zone_id: u16,
    base_score: f32,
    seller_probability: f32,
    urgency: f32,
    expected_fee: f32,
    proof_mask: u64,
}

#[derive(Debug, Clone, Copy)]
struct ScenarioProgram {
    id: u32,
    price_elasticity: f32,
    rate_delta_bps: f32,
    dpe_subsidy_boost: f32,
    local_growth_shock: f32,
    weather_risk_penalty: f32,
    competition_noise: f32,
    outreach_fit: f32,
}

#[derive(Debug, Clone)]
struct IntelOpportunity {
    property_id: u64,
    zone_id: u16,
    scenario_id: u32,
    horizon_days: u16,
    score: f32,
    seller_probability: f32,
    expected_fee: f32,
    proof_mask: u64,
}

#[derive(Debug, Clone)]
struct IntelPack {
    pack_hash: String,
    opportunities: Vec<IntelOpportunity>,
    top_signals: Vec<String>,
    llm_summary: String,
}

#[derive(Debug, Clone)]
struct ScenarioCubeOutcome {
    candidates: usize,
    scenarios: usize,
    horizons: usize,
    work_items: usize,
    best_property_id: u64,
    best_zone_id: u16,
    best_scenario_id: u32,
    best_horizon_days: u16,
    best_score: f32,
    best_seller_probability: f32,
    best_expected_fee: f32,
    strongest_signal: String,
    evidence_hash: String,
    checksum: f64,
}

#[derive(Debug, Clone, Copy, Default)]
struct CacheStats {
    hits: usize,
    misses: usize,
    avoided_units: usize,
    stage_elapsed_us: u128,
}

impl CacheStats {
    fn delta(self, before: CacheStats) -> CacheStats {
        CacheStats {
            hits: self.hits.saturating_sub(before.hits),
            misses: self.misses.saturating_sub(before.misses),
            avoided_units: self.avoided_units.saturating_sub(before.avoided_units),
            stage_elapsed_us: self
                .stage_elapsed_us
                .saturating_sub(before.stage_elapsed_us),
        }
    }
}

#[derive(Default)]
struct LabCache {
    columns: HashMap<String, Arc<PropertyColumns>>,
    metric_matrices: HashMap<String, Arc<MetricMatrix>>,
    normalized_matrices: HashMap<String, Arc<NormalizedMatrix>>,
    candidates: HashMap<String, Arc<Vec<OpportunityCandidate>>>,
    scenario_programs: HashMap<String, Arc<Vec<ScenarioProgram>>>,
    scenario_outcomes: HashMap<String, ScenarioCubeOutcome>,
    intel_packs: HashMap<String, Arc<IntelPack>>,
    stats: CacheStats,
    log_events: bool,
}

impl LabCache {
    fn hit(&mut self, stage: &str, key: &str, avoided_units: usize, elapsed: Duration) {
        self.stats.hits += 1;
        self.stats.avoided_units = self.stats.avoided_units.saturating_add(avoided_units);
        self.stats.stage_elapsed_us = self
            .stats
            .stage_elapsed_us
            .saturating_add(elapsed.as_micros());
        if self.log_events {
            log_cache("HIT", stage, key, avoided_units, elapsed);
        }
    }

    fn miss(&mut self, stage: &str, key: &str, elapsed: Duration) {
        self.stats.misses += 1;
        self.stats.stage_elapsed_us = self
            .stats
            .stage_elapsed_us
            .saturating_add(elapsed.as_micros());
        if self.log_events {
            log_cache("MISS", stage, key, 0, elapsed);
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct BenchRun {
    elapsed_us: u128,
    stats: CacheStats,
}

fn main() -> Result<(), Box<dyn Error>> {
    let config = parse_config()?;
    if config.plan_only {
        print_plan_only(&config);
        return Ok(());
    }

    let properties = if let Some(path) = config.seeds_path.as_deref() {
        seed_backed_properties(path, config.properties, config.seed)?
    } else {
        synthetic_properties(config.properties, config.seed)
    };
    if properties.is_empty() {
        return Err("no real-estate rows available for immo lab".into());
    }

    let dataset_hash = property_dataset_hash(&properties);
    let source_mode = if config.seeds_path.is_some() {
        "kasm_metric_seeds"
    } else {
        "synthetic_benchmark"
    };
    let dag_key = cache_key(
        "immo_dag_template:v1",
        &[
            &dataset_hash,
            &format!("properties={}", properties.len()),
            &format!("metrics={}", METRIC_NAMES.len()),
            &format!("scenarios={}", config.scenarios),
            &format!("candidate_limit={}", config.candidate_limit),
        ],
    );
    let mut cache = LabCache {
        log_events: true,
        ..LabCache::default()
    };

    println!(
        "[immo-lab] doctrine=kasm-content-addressed dataset_hash={} source_mode={} properties={} metrics={} scenarios={} horizons={} repeat={} candidate_limit={} dag={}",
        compact_key(&dataset_hash),
        source_mode,
        properties.len(),
        METRIC_NAMES.len(),
        config.scenarios,
        HORIZONS_DAYS.len(),
        config.repeat.max(2),
        config.candidate_limit,
        compact_key(&dag_key),
    );
    println!(
        "[immo-lab] dag=authorized_sources->property_columns->metric_matrix(32)->normalization->candidate_scan->scenario_programs->scenario_cube->llm_intel_pack layout=SoA+row_major_f32+topk cache_policy=content_addressed_auto_inject"
    );
    println!(
        "[immo-lab] source_policy=apis_open_data_crm_workspace_public_files proof=hashes+stage_keys llm_contract=compact_intel_pack_only"
    );

    match config.focus {
        Focus::ScenarioDag => {
            let mut runs = Vec::new();
            for pass in 1..=config.repeat.max(2) {
                runs.push(run_scenario_dag_pipeline(
                    &mut cache,
                    &properties,
                    &dataset_hash,
                    &config,
                    pass,
                ));
            }
            summarize_runs("immo_scenario_dag", &runs);
        }
        Focus::MetricMatrix => {
            run_metric_matrix_focus(&mut cache, &properties, &dataset_hash, &config);
        }
        Focus::CandidateScan => {
            run_candidate_scan_focus(&mut cache, &properties, &dataset_hash, &config);
        }
    }

    println!(
        "[immo-lab] global cache_hits={} cache_misses={} avoided_units={}",
        cache.stats.hits, cache.stats.misses, cache.stats.avoided_units
    );
    Ok(())
}

fn run_scenario_dag_pipeline(
    cache: &mut LabCache,
    properties: &[PropertyRecord],
    dataset_hash: &str,
    config: &Config,
    pass: usize,
) -> BenchRun {
    let stats_before = cache.stats;
    let started = Instant::now();
    let columns = cached_columns(cache, properties, dataset_hash);
    let metrics = cached_metric_matrix(cache, columns.as_ref(), dataset_hash);
    let normalized = cached_normalized_matrix(cache, metrics.as_ref(), dataset_hash);
    let candidates = cached_candidate_scan(
        cache,
        columns.as_ref(),
        normalized.as_ref(),
        dataset_hash,
        config.candidate_limit,
    );
    let programs = cached_scenario_programs(cache, dataset_hash, config.scenarios, config.seed);
    let outcome = cached_scenario_cube(
        cache,
        columns.as_ref(),
        normalized.as_ref(),
        candidates.as_slice(),
        programs.as_slice(),
        dataset_hash,
    );
    let pack = cached_intel_pack(
        cache,
        candidates.as_slice(),
        programs.as_slice(),
        &outcome,
        dataset_hash,
    );
    let elapsed = started.elapsed();
    let stats = cache.stats.delta(stats_before);
    println!(
        "[immo-lab] pass={} target=immo_scenario_dag elapsed_ms={:.3} stage_ms={:.3} rows={} metrics={} candidates={} scenario_programs={} horizons={} work_items={} best_property={} zone={} scenario={} horizon={}d score={:.4} seller_prob={:.4} expected_fee={:.0} strongest_signal={} evidence={} intel_pack={} pack_items={} pack_signals={} hits={} misses={} avoided_units={} checksum={:.5}",
        pass,
        elapsed.as_secs_f64() * 1000.0,
        stats.stage_elapsed_us as f64 / 1000.0,
        properties.len(),
        normalized.cols,
        outcome.candidates,
        outcome.scenarios,
        outcome.horizons,
        outcome.work_items,
        outcome.best_property_id,
        outcome.best_zone_id,
        outcome.best_scenario_id,
        outcome.best_horizon_days,
        outcome.best_score,
        outcome.best_seller_probability,
        outcome.best_expected_fee,
        outcome.strongest_signal,
        compact_key(&outcome.evidence_hash),
        compact_key(&pack.pack_hash),
        pack.opportunities.len(),
        pack.top_signals.len(),
        stats.hits,
        stats.misses,
        stats.avoided_units,
        outcome.checksum,
    );
    println!("[immo-lab] llm_intel_pack {}", pack.llm_summary);
    BenchRun {
        elapsed_us: elapsed.as_micros(),
        stats,
    }
}

fn run_metric_matrix_focus(
    cache: &mut LabCache,
    properties: &[PropertyRecord],
    dataset_hash: &str,
    config: &Config,
) {
    println!(
        "[immo-lab] focus=metric-matrix task=convert-heterogeneous-authorized-signals-to-row-major-metric-space rows={} metrics={} repeat={}",
        properties.len(),
        METRIC_NAMES.len(),
        config.repeat.max(2)
    );
    let mut runs = Vec::new();
    for pass in 1..=config.repeat.max(2) {
        let before = cache.stats;
        let started = Instant::now();
        let columns = cached_columns(cache, properties, dataset_hash);
        let metrics = cached_metric_matrix(cache, columns.as_ref(), dataset_hash);
        let normalized = cached_normalized_matrix(cache, metrics.as_ref(), dataset_hash);
        let elapsed = started.elapsed();
        let stats = cache.stats.delta(before);
        let checksum = matrix_checksum(normalized.as_ref());
        println!(
            "[immo-lab] pass={} target=metric_matrix elapsed_ms={:.3} stage_ms={:.3} rows={} cols={} hits={} misses={} avoided_units={} checksum={:.5}",
            pass,
            elapsed.as_secs_f64() * 1000.0,
            stats.stage_elapsed_us as f64 / 1000.0,
            normalized.rows,
            normalized.cols,
            stats.hits,
            stats.misses,
            stats.avoided_units,
            checksum
        );
        runs.push(BenchRun {
            elapsed_us: elapsed.as_micros(),
            stats,
        });
    }
    summarize_runs("metric_matrix", &runs);
}

fn run_candidate_scan_focus(
    cache: &mut LabCache,
    properties: &[PropertyRecord],
    dataset_hash: &str,
    config: &Config,
) {
    println!(
        "[immo-lab] focus=candidate-scan task=rank-properties-by-cross-domain-signal-confluence rows={} metrics={} candidate_limit={}",
        properties.len(),
        METRIC_NAMES.len(),
        config.candidate_limit
    );
    let mut runs = Vec::new();
    for pass in 1..=config.repeat.max(2) {
        let before = cache.stats;
        let started = Instant::now();
        let columns = cached_columns(cache, properties, dataset_hash);
        let metrics = cached_metric_matrix(cache, columns.as_ref(), dataset_hash);
        let normalized = cached_normalized_matrix(cache, metrics.as_ref(), dataset_hash);
        let candidates = cached_candidate_scan(
            cache,
            columns.as_ref(),
            normalized.as_ref(),
            dataset_hash,
            config.candidate_limit,
        );
        let elapsed = started.elapsed();
        let stats = cache.stats.delta(before);
        let checksum = candidate_checksum(candidates.as_slice());
        println!(
            "[immo-lab] pass={} target=candidate_scan elapsed_ms={:.3} stage_ms={:.3} candidates={} top_property={} top_score={:.4} hits={} misses={} avoided_units={} checksum={:.5}",
            pass,
            elapsed.as_secs_f64() * 1000.0,
            stats.stage_elapsed_us as f64 / 1000.0,
            candidates.len(),
            candidates.first().map(|c| c.property_id).unwrap_or_default(),
            candidates.first().map(|c| c.base_score).unwrap_or_default(),
            stats.hits,
            stats.misses,
            stats.avoided_units,
            checksum
        );
        runs.push(BenchRun {
            elapsed_us: elapsed.as_micros(),
            stats,
        });
    }
    summarize_runs("candidate_scan", &runs);
}

fn cached_columns(
    cache: &mut LabCache,
    properties: &[PropertyRecord],
    dataset_hash: &str,
) -> Arc<PropertyColumns> {
    let key = cache_key("immo_columns:v1", &[dataset_hash]);
    let started = Instant::now();
    if let Some(value) = cache.columns.get(&key).cloned() {
        cache.hit("immo_columns", &key, properties.len().saturating_mul(10), started.elapsed());
        return value;
    }

    let mut columns = PropertyColumns {
        id: Vec::with_capacity(properties.len()),
        zone_id: Vec::with_capacity(properties.len()),
        lat: Vec::with_capacity(properties.len()),
        lon: Vec::with_capacity(properties.len()),
        surface_m2: Vec::with_capacity(properties.len()),
        dvf_price_m2: Vec::with_capacity(properties.len()),
        asking_price_m2: Vec::with_capacity(properties.len()),
        metric_inputs: Vec::with_capacity(properties.len()),
    };
    for property in properties {
        columns.id.push(property.id);
        columns.zone_id.push(property.zone_id);
        columns.lat.push(property.lat as f32);
        columns.lon.push(property.lon as f32);
        columns.surface_m2.push(property.surface_m2 as f32);
        columns.dvf_price_m2.push(property.dvf_price_m2 as f32);
        columns.asking_price_m2.push(property.asking_price_m2 as f32);
        columns.metric_inputs.push(*property);
    }

    let columns = Arc::new(columns);
    let elapsed = started.elapsed();
    cache.columns.insert(key.clone(), Arc::clone(&columns));
    cache.miss("immo_columns", &key, elapsed);
    columns
}

fn cached_metric_matrix(
    cache: &mut LabCache,
    columns: &PropertyColumns,
    dataset_hash: &str,
) -> Arc<MetricMatrix> {
    let key = cache_key("immo_metric_matrix:v1", &[dataset_hash, "metrics=32"]);
    let started = Instant::now();
    if let Some(value) = cache.metric_matrices.get(&key).cloned() {
        cache.hit(
            "immo_metric_matrix",
            &key,
            value.rows.saturating_mul(value.cols),
            started.elapsed(),
        );
        return value;
    }

    let rows = columns.metric_inputs.len();
    let cols = METRIC_NAMES.len();
    let mut data = Vec::with_capacity(rows.saturating_mul(cols));
    for row in &columns.metric_inputs {
        push_property_metrics(row, &mut data);
    }
    let matrix = Arc::new(MetricMatrix {
        names: METRIC_NAMES.to_vec(),
        rows,
        cols,
        data,
    });
    let elapsed = started.elapsed();
    cache.metric_matrices.insert(key.clone(), Arc::clone(&matrix));
    cache.miss("immo_metric_matrix", &key, elapsed);
    matrix
}

fn cached_normalized_matrix(
    cache: &mut LabCache,
    matrix: &MetricMatrix,
    dataset_hash: &str,
) -> Arc<NormalizedMatrix> {
    let matrix_signature = format!("rows={}:cols={}:checksum={:.8}", matrix.rows, matrix.cols, matrix_checksum_raw(matrix));
    let key = cache_key("immo_normalized_matrix:v1", &[dataset_hash, &matrix_signature]);
    let started = Instant::now();
    if let Some(value) = cache.normalized_matrices.get(&key).cloned() {
        cache.hit(
            "immo_normalized_matrix",
            &key,
            value.rows.saturating_mul(value.cols).saturating_mul(2),
            started.elapsed(),
        );
        return value;
    }

    let mut means = vec![0.0_f32; matrix.cols];
    let mut stds = vec![0.0_f32; matrix.cols];
    for col in 0..matrix.cols {
        let mut sum = 0.0_f64;
        for row in 0..matrix.rows {
            sum += matrix_value(matrix, row, col) as f64;
        }
        means[col] = (sum / matrix.rows.max(1) as f64) as f32;
    }
    for col in 0..matrix.cols {
        let mut sum_sq = 0.0_f64;
        for row in 0..matrix.rows {
            let diff = matrix_value(matrix, row, col) - means[col];
            sum_sq += (diff * diff) as f64;
        }
        stds[col] = ((sum_sq / matrix.rows.max(1) as f64).sqrt() as f32).max(0.0001);
    }

    let mut data = Vec::with_capacity(matrix.data.len());
    for row in 0..matrix.rows {
        for col in 0..matrix.cols {
            let z = ((matrix_value(matrix, row, col) - means[col]) / stds[col]).clamp(-3.5, 3.5);
            data.push(z);
        }
    }
    let normalized = Arc::new(NormalizedMatrix {
        names: matrix.names.clone(),
        rows: matrix.rows,
        cols: matrix.cols,
        data,
        means,
        stds,
    });
    let elapsed = started.elapsed();
    cache
        .normalized_matrices
        .insert(key.clone(), Arc::clone(&normalized));
    cache.miss("immo_normalized_matrix", &key, elapsed);
    normalized
}

fn cached_candidate_scan(
    cache: &mut LabCache,
    columns: &PropertyColumns,
    normalized: &NormalizedMatrix,
    dataset_hash: &str,
    candidate_limit: usize,
) -> Arc<Vec<OpportunityCandidate>> {
    let key = cache_key(
        "immo_candidate_scan:v1",
        &[
            dataset_hash,
            &format!("rows={}:cols={}", normalized.rows, normalized.cols),
            &format!("limit={candidate_limit}"),
        ],
    );
    let started = Instant::now();
    if let Some(value) = cache.candidates.get(&key).cloned() {
        cache.hit(
            "immo_candidate_scan",
            &key,
            normalized.rows.saturating_mul(normalized.cols),
            started.elapsed(),
        );
        return value;
    }

    let mut candidates = Vec::with_capacity(normalized.rows.min(candidate_limit.saturating_mul(2)));
    for row in 0..normalized.rows {
        let score = candidate_score(normalized, row);
        if score < 48.0 && row % 17 != 0 {
            continue;
        }
        let seller_probability = sigmoid((score - 58.0) / 12.0) as f32;
        let urgency = sigmoid(
            metric(normalized, row, 1) as f64 * 0.55
                + metric(normalized, row, 21) as f64 * 0.42
                + metric(normalized, row, 28) as f64 * 0.36
                + metric(normalized, row, 30) as f64 * 0.20,
        ) as f32;
        let price = columns.asking_price_m2[row].max(900.0) * columns.surface_m2[row].max(18.0);
        let expected_fee = price * 0.035 * seller_probability * (0.82 + urgency * 0.32);
        let proof_mask = proof_mask_for_row(normalized, row);
        candidates.push(OpportunityCandidate {
            row,
            property_id: columns.id[row],
            zone_id: columns.zone_id[row],
            base_score: score as f32,
            seller_probability,
            urgency,
            expected_fee,
            proof_mask,
        });
    }
    candidates.sort_by(|a, b| {
        b.base_score
            .partial_cmp(&a.base_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    candidates.truncate(candidate_limit.min(candidates.len()));

    let candidates = Arc::new(candidates);
    let elapsed = started.elapsed();
    cache.candidates.insert(key.clone(), Arc::clone(&candidates));
    cache.miss("immo_candidate_scan", &key, elapsed);
    candidates
}

fn cached_scenario_programs(
    cache: &mut LabCache,
    dataset_hash: &str,
    scenario_count: usize,
    seed: u64,
) -> Arc<Vec<ScenarioProgram>> {
    let key = cache_key(
        "immo_scenario_programs:v1",
        &[dataset_hash, &format!("scenarios={scenario_count}"), &format!("seed={seed}")],
    );
    let started = Instant::now();
    if let Some(value) = cache.scenario_programs.get(&key).cloned() {
        cache.hit("immo_scenario_programs", &key, value.len().saturating_mul(8), started.elapsed());
        return value;
    }

    let mut programs = Vec::with_capacity(scenario_count);
    let mut rng = SplitMix64::new(seed ^ 0xA9_36_71_4D_99_11_2C_03);
    for idx in 0..scenario_count {
        programs.push(ScenarioProgram {
            id: idx as u32,
            price_elasticity: lerp(-0.18, 0.24, rng.next_unit()) as f32,
            rate_delta_bps: lerp(-55.0, 85.0, rng.next_unit()) as f32,
            dpe_subsidy_boost: lerp(0.0, 0.18, rng.next_unit()) as f32,
            local_growth_shock: lerp(-0.08, 0.16, rng.next_unit()) as f32,
            weather_risk_penalty: lerp(0.0, 0.14, rng.next_unit()) as f32,
            competition_noise: lerp(-0.10, 0.18, rng.next_unit()) as f32,
            outreach_fit: lerp(0.72, 1.24, rng.next_unit()) as f32,
        });
    }

    let programs = Arc::new(programs);
    let elapsed = started.elapsed();
    cache
        .scenario_programs
        .insert(key.clone(), Arc::clone(&programs));
    cache.miss("immo_scenario_programs", &key, elapsed);
    programs
}

fn cached_scenario_cube(
    cache: &mut LabCache,
    columns: &PropertyColumns,
    normalized: &NormalizedMatrix,
    candidates: &[OpportunityCandidate],
    programs: &[ScenarioProgram],
    dataset_hash: &str,
) -> ScenarioCubeOutcome {
    let key = cache_key(
        "immo_scenario_cube:v1",
        &[
            dataset_hash,
            &format!("candidates={}", candidates.len()),
            &format!("programs={}", programs.len()),
            &format!("horizons={}", HORIZONS_DAYS.len()),
            &candidate_signature(candidates),
            &program_signature(programs),
        ],
    );
    let started = Instant::now();
    if let Some(value) = cache.scenario_outcomes.get(&key).cloned() {
        cache.hit(
            "immo_scenario_cube",
            &key,
            candidates
                .len()
                .saturating_mul(programs.len())
                .saturating_mul(HORIZONS_DAYS.len())
                .saturating_mul(6),
            started.elapsed(),
        );
        return value;
    }

    let mut best = IntelOpportunity {
        property_id: 0,
        zone_id: 0,
        scenario_id: 0,
        horizon_days: 0,
        score: f32::NEG_INFINITY,
        seller_probability: 0.0,
        expected_fee: 0.0,
        proof_mask: 0,
    };
    let mut checksum = 0.0_f64;
    let mut signal_counts = vec![0_usize; normalized.cols.min(64)];

    for candidate in candidates {
        let row = candidate.row;
        let price_gap = metric(normalized, row, 0);
        let dpe_gap = metric(normalized, row, 2);
        let geo_risk = (metric(normalized, row, 4) + metric(normalized, row, 5)) * 0.5;
        let local_momentum = (metric(normalized, row, 6)
            + metric(normalized, row, 7)
            + metric(normalized, row, 8)
            + metric(normalized, row, 9)
            + metric(normalized, row, 14))
            / 5.0;
        let buyer_pressure = (metric(normalized, row, 17) + metric(normalized, row, 26)) * 0.5;
        let competition = metric(normalized, row, 16);
        let rate_sensitivity = metric(normalized, row, 18);
        let base_fee = candidate.expected_fee;

        for program in programs {
            let scenario_bias = program.local_growth_shock
                + program.price_elasticity * price_gap
                + program.dpe_subsidy_boost * dpe_gap.max(0.0)
                - program.weather_risk_penalty * geo_risk.max(0.0)
                - program.competition_noise * competition.max(0.0)
                - (program.rate_delta_bps / 100.0) * rate_sensitivity * 0.045
                + buyer_pressure * 0.055
                + local_momentum * 0.050;

            for horizon in HORIZONS_DAYS {
                let horizon_factor = horizon as f32 / 90.0;
                let time_decay = 1.0 / (1.0 + (horizon_factor - candidate.urgency).abs() * 0.24);
                let seller_probability = (candidate.seller_probability
                    * program.outreach_fit
                    * (1.0 + scenario_bias * time_decay))
                    .clamp(0.01, 0.98);
                let expected_fee = base_fee
                    * (1.0 + scenario_bias * 0.70)
                    * (0.88 + time_decay * 0.18)
                    * (1.0 + columns.surface_m2[row].ln().max(0.0) * 0.006);
                let score = candidate.base_score
                    + scenario_bias * 18.0
                    + seller_probability * 22.0
                    + (expected_fee / 2_500.0).min(24.0)
                    - geo_risk.max(0.0) * 2.6;

                checksum += score as f64 * 0.000013
                    + expected_fee as f64 * 0.0000007
                    + seller_probability as f64 * 0.01;

                if (program.id as usize + row + horizon as usize) % 8191 == 0 {
                    checksum = std::hint::black_box(checksum + score as f64 * 0.0001);
                }

                if score > best.score {
                    best = IntelOpportunity {
                        property_id: candidate.property_id,
                        zone_id: candidate.zone_id,
                        scenario_id: program.id,
                        horizon_days: horizon,
                        score,
                        seller_probability,
                        expected_fee,
                        proof_mask: candidate.proof_mask,
                    };
                }
            }
        }

        for col in 0..normalized.cols.min(64) {
            if candidate.proof_mask & (1_u64 << col) != 0 {
                signal_counts[col] += 1;
            }
        }
    }

    let strongest_signal = strongest_signal(&signal_counts, &normalized.names);
    let work_items = candidates
        .len()
        .saturating_mul(programs.len())
        .saturating_mul(HORIZONS_DAYS.len());
    let evidence_hash = cache_key(
        "immo_evidence:v1",
        &[
            dataset_hash,
            &format!("best={}:{}:{}", best.property_id, best.scenario_id, best.horizon_days),
            &format!("score={:.5}:fee={:.2}", best.score, best.expected_fee),
            &strongest_signal,
            &format!("checksum={checksum:.8}"),
        ],
    );
    let outcome = ScenarioCubeOutcome {
        candidates: candidates.len(),
        scenarios: programs.len(),
        horizons: HORIZONS_DAYS.len(),
        work_items,
        best_property_id: best.property_id,
        best_zone_id: best.zone_id,
        best_scenario_id: best.scenario_id,
        best_horizon_days: best.horizon_days,
        best_score: best.score,
        best_seller_probability: best.seller_probability,
        best_expected_fee: best.expected_fee,
        strongest_signal,
        evidence_hash,
        checksum: std::hint::black_box(checksum),
    };
    let elapsed = started.elapsed();
    cache.scenario_outcomes.insert(key.clone(), outcome.clone());
    cache.miss("immo_scenario_cube", &key, elapsed);
    outcome
}

fn cached_intel_pack(
    cache: &mut LabCache,
    candidates: &[OpportunityCandidate],
    programs: &[ScenarioProgram],
    outcome: &ScenarioCubeOutcome,
    dataset_hash: &str,
) -> Arc<IntelPack> {
    let key = cache_key(
        "immo_llm_intel_pack:v1",
        &[
            dataset_hash,
            &outcome.evidence_hash,
            &format!("candidates={}:programs={}", candidates.len(), programs.len()),
        ],
    );
    let started = Instant::now();
    if let Some(value) = cache.intel_packs.get(&key).cloned() {
        cache.hit("immo_llm_intel_pack", &key, candidates.len().min(512), started.elapsed());
        return value;
    }

    let mut opportunities = Vec::new();
    for candidate in candidates.iter().take(8) {
        let program = programs
            .get((candidate.row + candidate.zone_id as usize) % programs.len().max(1))
            .copied()
            .unwrap_or(ScenarioProgram {
                id: 0,
                price_elasticity: 0.0,
                rate_delta_bps: 0.0,
                dpe_subsidy_boost: 0.0,
                local_growth_shock: 0.0,
                weather_risk_penalty: 0.0,
                competition_noise: 0.0,
                outreach_fit: 1.0,
            });
        opportunities.push(IntelOpportunity {
            property_id: candidate.property_id,
            zone_id: candidate.zone_id,
            scenario_id: program.id,
            horizon_days: HORIZONS_DAYS[(candidate.row + program.id as usize) % HORIZONS_DAYS.len()],
            score: candidate.base_score,
            seller_probability: candidate.seller_probability,
            expected_fee: candidate.expected_fee,
            proof_mask: candidate.proof_mask,
        });
    }
    opportunities.push(IntelOpportunity {
        property_id: outcome.best_property_id,
        zone_id: outcome.best_zone_id,
        scenario_id: outcome.best_scenario_id,
        horizon_days: outcome.best_horizon_days,
        score: outcome.best_score,
        seller_probability: outcome.best_seller_probability,
        expected_fee: outcome.best_expected_fee,
        proof_mask: 0,
    });
    let top_signals = decode_signal_names(
        opportunities
            .first()
            .map(|item| item.proof_mask)
            .unwrap_or_default(),
    );
    let summary = format!(
        "best_property={} zone={} horizon={}d score={:.2} seller_prob={:.2} expected_fee={:.0} strongest_signal={} opportunities={} evidence={}",
        outcome.best_property_id,
        outcome.best_zone_id,
        outcome.best_horizon_days,
        outcome.best_score,
        outcome.best_seller_probability,
        outcome.best_expected_fee,
        outcome.strongest_signal,
        opportunities.len(),
        compact_key(&outcome.evidence_hash),
    );
    let pack_hash = cache_key(
        "immo_pack_payload:v1",
        &[
            dataset_hash,
            &summary,
            &format!("top={}", opportunities.len()),
            &top_signals.join(","),
        ],
    );
    let pack = Arc::new(IntelPack {
        pack_hash,
        opportunities,
        top_signals,
        llm_summary: summary,
    });
    let elapsed = started.elapsed();
    cache.intel_packs.insert(key.clone(), Arc::clone(&pack));
    cache.miss("immo_llm_intel_pack", &key, elapsed);
    pack
}

fn push_property_metrics(row: &PropertyRecord, out: &mut Vec<f32>) {
    let safe_div = |a: f64, b: f64| a / b.abs().max(0.0001);
    let dvf_gap = safe_div(row.asking_price_m2 - row.dvf_price_m2, row.dvf_price_m2) * 100.0;
    let dpe_gap = ((row.dpe_score - 3.0) / 4.0).clamp(0.0, 1.0) * 100.0;
    let price_anchor_error = row.price_anchor_error + dvf_gap.abs() * 0.24;
    let metrics = [
        dvf_gap,
        row.days_on_market_shadow / 30.0,
        dpe_gap,
        row.energy_cost_index,
        row.clay_risk,
        row.flood_risk,
        row.permit_activity,
        row.urbanism_upside,
        row.transit_momentum,
        row.school_momentum,
        row.business_churn,
        row.traffic_noise_delta,
        row.pollution_delta,
        row.weather_heat_stress,
        row.mobility_inflow,
        row.local_news_intensity,
        row.competitor_pressure,
        row.buyer_demand_match,
        row.credit_rate_sensitivity,
        row.insurance_risk,
        row.tax_pressure_proxy,
        row.crm_inactivity_days / 45.0,
        row.visit_intent,
        row.owner_lifecycle_pressure,
        row.rental_yield_gap,
        row.work_cost_roi,
        row.neighborhood_liquidity,
        price_anchor_error,
        row.days_on_market_shadow / 20.0,
        row.notary_delay_index,
        row.seasonality_fit,
        row.agency_reputation_fit,
        row.energy_cost_index * 0.52 + synthetic_metric(row, 32) * 1.12,
        row.energy_cost_index * 0.44 + row.credit_rate_sensitivity * 0.16 + synthetic_metric(row, 33) * 1.05,
        row.weather_heat_stress * 0.38 + row.clay_risk * 0.20 + synthetic_metric(row, 34) * 1.10,
        row.weather_heat_stress * 0.72 + row.pollution_delta * 0.14 + synthetic_metric(row, 35) * 0.58,
        row.surface_m2 / 120.0 + row.fiber_quality_proxy() * 0.42 + row.traffic_noise_delta.max(0.0) * -0.10,
        row.fiber_quality_proxy() + row.mobility_inflow * 0.15,
        (1.35 - row.fiber_quality_proxy()).max(0.0) + synthetic_metric(row, 38) * 0.45,
        row.tourism_proxy() * 0.72 + row.rental_yield_gap * 0.28 + synthetic_metric(row, 39) * 0.42,
        row.student_proxy() * 0.82 + row.transit_momentum * 0.18,
        row.health_access_proxy() + row.transit_momentum * 0.12,
        row.senior_services_proxy() + row.health_access_proxy() * 0.18,
        synthetic_metric(row, 44) * 1.30 - row.neighborhood_liquidity * 0.10,
        row.owner_lifecycle_pressure * 0.46 + row.age_years / 95.0 + synthetic_metric(row, 45) * 0.72,
        row.credit_rate_sensitivity * 0.58 + row.tax_pressure_proxy * 0.18 + synthetic_metric(row, 46) * 0.72,
        synthetic_metric(row, 47) * 1.18 - row.hiring_proxy() * 0.22,
        row.hiring_proxy() + row.business_churn * 0.16,
        row.tourism_proxy() + row.event_proxy() * 0.20,
        row.event_proxy() + row.local_news_intensity * 0.16,
        synthetic_metric(row, 51) * 1.16 + (1.2 - row.transit_momentum).max(0.0) * 0.20,
        row.credit_rate_sensitivity * 0.20 + row.mobility_inflow * 0.12 + synthetic_metric(row, 52) * 1.10,
        row.daily_services_proxy() + row.school_momentum * 0.08,
        row.green_space_proxy() - row.pollution_delta * 0.08,
        row.sunlight_proxy() + row.floor.max(0.0) * 0.025,
        row.slope_walkability_proxy(),
        row.parking_pressure_proxy() + row.competitor_pressure * 0.08,
        row.ev_charging_proxy() + row.urbanism_upside * 0.10,
        row.work_cost_roi * 0.18 + synthetic_metric(row, 59) * 1.20,
        row.artisan_capacity_proxy() - row.local_news_intensity * 0.04,
        row.work_cost_roi * 0.44 + row.dpe_score.max(0.0) * 0.12 + synthetic_metric(row, 61) * 0.54,
        row.tax_pressure_proxy * 0.56 + synthetic_metric(row, 62) * 0.70,
        row.insurance_risk * 0.62 + row.flood_risk * 0.20 + row.clay_risk * 0.16,
        row.weather_heat_stress * 0.42 + row.insurance_risk * 0.28 + row.energy_cost_index * 0.18,
    ];
    out.extend(metrics.iter().map(|value| *value as f32));
}

fn candidate_score(normalized: &NormalizedMatrix, row: usize) -> f64 {
    let mut score = 56.0;
    for col in 0..normalized.cols {
        score += metric(normalized, row, col) as f64 * metric_weight(col);
    }
    let confluence = [
        metric(normalized, row, 0),
        metric(normalized, row, 2),
        metric(normalized, row, 17),
        metric(normalized, row, 25),
        metric(normalized, row, 26),
        metric(normalized, row, 36),
        metric(normalized, row, 41),
        metric(normalized, row, 60),
    ]
    .iter()
    .filter(|value| **value > 0.7)
    .count() as f64;
    (score + confluence * 3.5).clamp(0.0, 100.0)
}

fn proof_mask_for_row(normalized: &NormalizedMatrix, row: usize) -> u64 {
    let mut mask = 0_u64;
    for col in 0..normalized.cols.min(64) {
        let value = metric(normalized, row, col);
        let positive_signal = matches!(
            col,
            0 | 1 | 2 | 3 | 6 | 7 | 8 | 9 | 10 | 14 | 15 | 17 | 21 | 22 | 23 | 24 | 25 | 26
                | 27 | 28 | 30 | 31 | 36 | 37 | 39 | 40 | 41 | 43 | 48 | 49 | 50 | 54 | 55
                | 56 | 58 | 60
        );
        let negative_signal = matches!(
            col,
            4 | 5 | 11 | 12 | 13 | 16 | 18 | 19 | 29 | 34 | 35 | 38 | 44 | 46 | 47 | 51 | 52
                | 57 | 59 | 62 | 63
        );
        if (positive_signal && value > 0.85) || (negative_signal && value < -0.85) {
            mask |= 1_u64 << col;
        }
    }
    mask
}

fn metric_weight(col: usize) -> f64 {
    match col {
        0 => 4.8,
        1 => 3.6,
        2 => 5.2,
        3 => 2.4,
        4 => -1.8,
        5 => -1.2,
        6 => 2.6,
        7 => 2.9,
        8 => 2.5,
        9 => 1.7,
        10 => 1.8,
        11 => -0.8,
        12 => -0.7,
        13 => -1.0,
        14 => 2.7,
        15 => 1.9,
        16 => -2.1,
        17 => 4.7,
        18 => -1.4,
        19 => -1.5,
        20 => 1.6,
        21 => 3.2,
        22 => 3.8,
        23 => 3.0,
        24 => 2.1,
        25 => 3.5,
        26 => 3.8,
        27 => 3.1,
        28 => 2.9,
        29 => -0.9,
        30 => 1.6,
        31 => 2.2,
        32 => -1.1,
        33 => -1.0,
        34 => -0.8,
        35 => -1.4,
        36 => 2.4,
        37 => 2.1,
        38 => -0.9,
        39 => 1.5,
        40 => 1.7,
        41 => 1.4,
        42 => 1.1,
        43 => -1.3,
        44 => 1.8,
        45 => -1.2,
        46 => -1.6,
        47 => -1.1,
        48 => 1.5,
        49 => 1.2,
        50 => 1.0,
        51 => -0.9,
        52 => -1.0,
        53 => 1.1,
        54 => 1.4,
        55 => 1.0,
        56 => 1.2,
        57 => -0.9,
        58 => 1.3,
        59 => -1.2,
        60 => 2.0,
        61 => -0.8,
        62 => -1.1,
        63 => -1.5,
        _ => 0.0,
    }
}

fn strongest_signal(counts: &[usize], names: &[&'static str]) -> String {
    let (idx, count) = counts
        .iter()
        .enumerate()
        .max_by_key(|(_, count)| **count)
        .unwrap_or((0, &0));
    format!("{}:{}", names.get(idx).copied().unwrap_or("unknown"), count)
}

fn decode_signal_names(mask: u64) -> Vec<String> {
    let mut out = Vec::new();
    for (idx, name) in METRIC_NAMES.iter().enumerate() {
        if idx < 64 && mask & (1_u64 << idx) != 0 {
            out.push((*name).to_string());
        }
    }
    out
}

fn metric(matrix: &NormalizedMatrix, row: usize, col: usize) -> f32 {
    matrix.data[row.saturating_mul(matrix.cols).saturating_add(col)]
}

fn matrix_value(matrix: &MetricMatrix, row: usize, col: usize) -> f32 {
    matrix.data[row.saturating_mul(matrix.cols).saturating_add(col)]
}

fn synthetic_properties(count: usize, seed: u64) -> Vec<PropertyRecord> {
    let mut rng = SplitMix64::new(seed);
    let mut rows = Vec::with_capacity(count);
    for idx in 0..count {
        let zone = (idx % 144) as u16;
        let zone_wave = ((zone as f64) * 0.173).sin();
        let submarket = ((idx / 144) % 29) as f64;
        let base_price = 2_250.0 + zone as f64 * 18.0 + zone_wave * 420.0 + submarket * 37.0;
        let surface = lerp(28.0, 185.0, rng.next_unit()).round();
        let rooms = (surface / 24.0 + lerp(-0.8, 1.4, rng.next_unit())).clamp(1.0, 8.0).round();
        let age = lerp(2.0, 118.0, rng.next_unit());
        let dpe = (1.0 + age / 24.0 + lerp(-1.2, 2.4, rng.next_unit())).clamp(1.0, 7.0);
        let liquidity = (0.44 + zone_wave * 0.15 + rng.next_unit() * 0.65).clamp(0.0, 1.35);
        let buyer_demand = (0.42 + liquidity * 0.55 + rng.next_unit() * 0.50).clamp(0.0, 1.5);
        let transit = (0.22 + ((zone as f64) * 0.097).cos() * 0.28 + rng.next_unit() * 0.7).clamp(0.0, 1.4);
        let permit = (rng.next_unit().powf(1.8) * 1.9 + (zone % 11 == 0) as u8 as f64 * 0.7).clamp(0.0, 2.4);
        let urbanism = (permit * 0.42 + transit * 0.28 + rng.next_unit() * 0.75).clamp(0.0, 2.2);
        let weather = (rng.next_unit().powf(1.6) * 1.7 + (zone % 17 == 0) as u8 as f64 * 0.45).clamp(0.0, 2.1);
        let clay = (rng.next_unit().powf(2.2) * 1.9 + weather * 0.20).clamp(0.0, 2.2);
        let flood = (rng.next_unit().powf(2.6) * 1.8 + (zone % 23 == 0) as u8 as f64 * 0.55).clamp(0.0, 2.1);
        let energy = (dpe * 0.26 + weather * 0.18 + rng.next_unit() * 0.45).clamp(0.0, 2.6);
        let rate_sensitivity = ((base_price / 6_500.0) + buyer_demand * 0.18 + rng.next_unit() * 0.65).clamp(0.0, 2.4);
        let competitor = (buyer_demand * 0.45 + liquidity * 0.30 + rng.next_unit() * 0.95).clamp(0.0, 2.3);
        let stale_days = (rng.next_unit().powf(1.4) * 180.0
            + (competitor > 1.35) as u8 as f64 * 18.0
            + (dpe > 5.0) as u8 as f64 * 24.0)
            .clamp(0.0, 260.0);
        let price_anchor = lerp(-8.0, 18.0, rng.next_unit()) + competitor * 1.8 + dpe * 0.34;
        let asking_price = base_price * (1.0 + price_anchor / 100.0);
        let crm_inactivity = (rng.next_unit().powf(1.2) * 240.0 + stale_days * 0.18).clamp(0.0, 360.0);
        let visit_intent = (buyer_demand * 0.62 + liquidity * 0.30 + rng.next_unit() * 0.55).clamp(0.0, 2.1);
        let owner_lifecycle = (rng.next_unit().powf(1.5) * 1.7
            + (stale_days > 80.0) as u8 as f64 * 0.35
            + (crm_inactivity > 120.0) as u8 as f64 * 0.25)
            .clamp(0.0, 2.4);
        let work_roi = ((dpe - 3.0).max(0.0) * 0.30 + urbanism * 0.25 + buyer_demand * 0.22).clamp(0.0, 2.6);
        let rental_gap = (buyer_demand * 0.35 + rate_sensitivity * 0.20 + rng.next_unit() * 0.70 - 0.20).clamp(-0.5, 2.3);
        rows.push(PropertyRecord {
            id: 1_000_000 + idx as u64,
            zone_id: zone,
            lat: 43.0 + (zone as f64 % 37.0) * 0.018 + rng.next_unit() * 0.006,
            lon: 1.0 + (zone as f64 / 37.0).floor() * 0.021 + rng.next_unit() * 0.006,
            surface_m2: surface,
            rooms,
            age_years: age,
            floor: lerp(0.0, 8.0, rng.next_unit()).round(),
            dvf_price_m2: base_price,
            asking_price_m2: asking_price,
            dpe_score: dpe,
            energy_cost_index: energy,
            clay_risk: clay,
            flood_risk: flood,
            permit_activity: permit,
            urbanism_upside: urbanism,
            transit_momentum: transit,
            school_momentum: (rng.next_unit() * 1.35 + (zone % 13 == 0) as u8 as f64 * 0.42).clamp(0.0, 1.9),
            business_churn: (rng.next_unit() * 1.6 + transit * 0.20).clamp(0.0, 2.0),
            traffic_noise_delta: (rng.next_unit() * 1.8 - transit * 0.12).clamp(-0.3, 2.1),
            pollution_delta: (rng.next_unit() * 1.7 + traffic_noise_delta(weather) * 0.18).clamp(0.0, 2.1),
            weather_heat_stress: weather,
            mobility_inflow: (rng.next_unit() * 1.55 + transit * 0.32).clamp(0.0, 2.1),
            local_news_intensity: (rng.next_unit() * 1.8 + permit * 0.22).clamp(0.0, 2.3),
            competitor_pressure: competitor,
            buyer_demand_match: buyer_demand,
            credit_rate_sensitivity: rate_sensitivity,
            insurance_risk: (clay * 0.38 + flood * 0.44 + rng.next_unit() * 0.45).clamp(0.0, 2.4),
            tax_pressure_proxy: (base_price / 7_500.0 + surface / 240.0 + rng.next_unit() * 0.45).clamp(0.0, 2.0),
            crm_inactivity_days: crm_inactivity,
            visit_intent,
            owner_lifecycle_pressure: owner_lifecycle,
            rental_yield_gap: rental_gap,
            work_cost_roi: work_roi,
            neighborhood_liquidity: liquidity,
            price_anchor_error: price_anchor,
            days_on_market_shadow: stale_days,
            notary_delay_index: (rng.next_unit() * 1.3 + flood * 0.18 + clay * 0.12).clamp(0.0, 2.0),
            seasonality_fit: (rng.next_unit() * 1.4 + ((idx as f64 * 0.013).sin() + 1.0) * 0.18).clamp(0.0, 2.0),
            agency_reputation_fit: (rng.next_unit() * 1.2 + buyer_demand * 0.20 + competitor * 0.08).clamp(0.0, 1.9),
        });
    }
    rows
}

fn traffic_noise_delta(value: f64) -> f64 {
    (value * 0.41 + 0.13).sin().abs()
}

fn seed_backed_properties(path: &str, count: usize, seed: u64) -> Result<Vec<PropertyRecord>, Box<dyn Error>> {
    let seeds = read_metric_seed_records(path)?;
    if seeds.is_empty() {
        return Err(format!("no KASM metric seeds found in {path}").into());
    }
    let mut rows = Vec::with_capacity(count);
    for idx in 0..count {
        let seed_record = &seeds[idx % seeds.len()];
        let wave = (idx / seeds.len().max(1)) as u64;
        let mut rng = SplitMix64::new(seed ^ stable_u64(&seed_record.seed_hash) ^ wave.wrapping_mul(0x9E37_79B9_7F4A_7C15));
        rows.push(property_from_metric_seed(seed_record, idx, wave, &mut rng));
    }
    Ok(rows)
}

fn read_metric_seed_records(path: &str) -> Result<Vec<MetricSeedRecord>, Box<dyn Error>> {
    let text = fs::read_to_string(path)?;
    let mut out = Vec::new();
    for (line_idx, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        out.push(MetricSeedRecord {
            seed_hash: json_string(line, "seedHash")
                .unwrap_or_else(|| cache_key("immo_seed_line:v1", &[path, &line_idx.to_string()])),
            pack_proof_hash: json_string(line, "packProofHash").unwrap_or_default(),
            pack_theme: json_string(line, "packTheme").unwrap_or_else(|| "unknown".to_string()),
            priority_score: json_number(line, "priorityScore").unwrap_or(0.0).clamp(0.0, 1.0),
            source_density_score: json_number(line, "source_density_score").unwrap_or(0.0).clamp(0.0, 1.0),
            evidence_quality_score: json_number(line, "evidence_quality_score").unwrap_or(0.0).clamp(0.0, 1.0),
            graph_density_score: json_number(line, "graph_density_score").unwrap_or(0.0).clamp(0.0, 1.0),
            market_signal_score: json_number(line, "market_signal_score").unwrap_or(0.0).clamp(0.0, 1.0),
            local_signal_score: json_number(line, "local_signal_score").unwrap_or(0.0).clamp(0.0, 1.0),
            economic_signal_score: json_number(line, "economic_signal_score").unwrap_or(0.0).clamp(0.0, 1.0),
            data_gap_penalty: json_number(line, "data_gap_penalty").unwrap_or(0.0).clamp(0.0, 1.0),
            actionability_score: json_number(line, "actionability_score").unwrap_or(0.0).clamp(0.0, 1.0),
        });
    }
    Ok(out)
}

fn property_from_metric_seed(seed: &MetricSeedRecord, idx: usize, wave: u64, rng: &mut SplitMix64) -> PropertyRecord {
    let jitter = |rng: &mut SplitMix64, width: f64| lerp(-width, width, rng.next_unit());
    let p = seed.priority_score;
    let source = seed.source_density_score;
    let evidence = seed.evidence_quality_score;
    let graph = seed.graph_density_score;
    let market = seed.market_signal_score;
    let local = seed.local_signal_score;
    let economic = seed.economic_signal_score;
    let gap = seed.data_gap_penalty;
    let action = seed.actionability_score;
    let theme_bias = stable_unit(&seed.pack_theme);
    let proof_bias = stable_unit(&seed.pack_proof_hash);
    let zone = ((stable_u64(&seed.pack_proof_hash) as usize + idx) % 288) as u16;
    let base_price = 1_900.0 + market * 2_400.0 + local * 650.0 + economic * 780.0 + evidence * 380.0 + jitter(rng, 160.0);
    let surface = (36.0 + action * 96.0 + source * 44.0 + jitter(rng, 18.0)).clamp(18.0, 220.0).round();
    let dpe = (2.0 + gap * 3.8 + (1.0 - evidence) * 1.4 + jitter(rng, 0.8)).clamp(1.0, 7.0);
    let liquidity = (0.25 + market * 0.86 + source * 0.42 + jitter(rng, 0.18)).clamp(0.0, 1.8);
    let buyer_demand = (0.20 + market * 0.95 + economic * 0.36 + action * 0.38 + jitter(rng, 0.22)).clamp(0.0, 2.1);
    let local_strength = (local * 1.2 + graph * 0.34 + theme_bias * 0.24).clamp(0.0, 2.0);
    let risk = (gap * 1.15 + (1.0 - evidence) * 0.52 + proof_bias * 0.30).clamp(0.0, 2.2);
    let stale_days = (18.0 + gap * 150.0 + (1.0 - action) * 70.0 + jitter(rng, 34.0)).clamp(0.0, 280.0);
    let price_anchor = (gap * 18.0 + market * 4.5 - evidence * 5.0 + jitter(rng, 5.5)).clamp(-12.0, 28.0);
    PropertyRecord {
        id: 7_000_000 + idx as u64,
        zone_id: zone,
        lat: 43.0 + (zone as f64 % 37.0) * 0.018 + theme_bias * 0.012,
        lon: 1.0 + (zone as f64 / 37.0).floor() * 0.021 + proof_bias * 0.012,
        surface_m2: surface,
        rooms: (surface / 24.0 + jitter(rng, 0.7)).clamp(1.0, 8.0).round(),
        age_years: (8.0 + gap * 92.0 + (1.0 - source) * 28.0 + jitter(rng, 16.0)).clamp(1.0, 140.0),
        floor: lerp(0.0, 8.0, rng.next_unit()).round(),
        dvf_price_m2: base_price,
        asking_price_m2: base_price * (1.0 + price_anchor / 100.0),
        dpe_score: dpe,
        energy_cost_index: (gap * 1.5 + dpe / 7.0 + jitter(rng, 0.12)).clamp(0.0, 2.8),
        clay_risk: (risk * 0.58 + jitter(rng, 0.16)).clamp(0.0, 2.4),
        flood_risk: (risk * 0.42 + proof_bias * 0.50 + jitter(rng, 0.14)).clamp(0.0, 2.3),
        permit_activity: (graph * 1.35 + local * 0.35 + jitter(rng, 0.18)).clamp(0.0, 2.5),
        urbanism_upside: (graph * 1.25 + market * 0.30 + jitter(rng, 0.20)).clamp(0.0, 2.4),
        transit_momentum: (local * 0.85 + economic * 0.26 + jitter(rng, 0.18)).clamp(0.0, 2.0),
        school_momentum: (local * 0.72 + source * 0.28 + jitter(rng, 0.15)).clamp(0.0, 1.9),
        business_churn: (economic * 1.15 + gap * 0.20 + jitter(rng, 0.20)).clamp(0.0, 2.2),
        traffic_noise_delta: (gap * 0.55 + local * 0.18 + jitter(rng, 0.24)).clamp(-0.3, 2.2),
        pollution_delta: (risk * 0.44 + gap * 0.32 + jitter(rng, 0.18)).clamp(0.0, 2.2),
        weather_heat_stress: (risk * 0.36 + gap * 0.70 + jitter(rng, 0.16)).clamp(0.0, 2.3),
        mobility_inflow: (local * 0.72 + economic * 0.44 + market * 0.30 + jitter(rng, 0.18)).clamp(0.0, 2.2),
        local_news_intensity: (local_strength + graph * 0.22 + jitter(rng, 0.20)).clamp(0.0, 2.5),
        competitor_pressure: (market * 0.82 + economic * 0.22 + gap * 0.18 + jitter(rng, 0.20)).clamp(0.0, 2.3),
        buyer_demand_match: buyer_demand,
        credit_rate_sensitivity: (economic * 0.76 + market * 0.32 + jitter(rng, 0.16)).clamp(0.0, 2.4),
        insurance_risk: (risk * 0.72 + gap * 0.36 + jitter(rng, 0.16)).clamp(0.0, 2.5),
        tax_pressure_proxy: (economic * 0.42 + market * 0.26 + gap * 0.30 + jitter(rng, 0.12)).clamp(0.0, 2.0),
        crm_inactivity_days: (gap * 210.0 + (1.0 - action) * 80.0 + jitter(rng, 28.0)).clamp(0.0, 380.0),
        visit_intent: (action * 1.25 + market * 0.34 + jitter(rng, 0.16)).clamp(0.0, 2.2),
        owner_lifecycle_pressure: (p * 1.35 + action * 0.34 + gap * 0.24 + jitter(rng, 0.18)).clamp(0.0, 2.5),
        rental_yield_gap: (market * 0.70 + economic * 0.48 + local * 0.22 - gap * 0.12 + jitter(rng, 0.18)).clamp(-0.5, 2.5),
        work_cost_roi: (action * 0.72 + gap * 0.54 + graph * 0.26 + jitter(rng, 0.18)).clamp(0.0, 2.7),
        neighborhood_liquidity: liquidity,
        price_anchor_error: price_anchor,
        days_on_market_shadow: stale_days + (wave % 11) as f64,
        notary_delay_index: (gap * 0.62 + risk * 0.22 + jitter(rng, 0.12)).clamp(0.0, 2.0),
        seasonality_fit: (action * 0.60 + market * 0.22 + jitter(rng, 0.18)).clamp(0.0, 2.0),
        agency_reputation_fit: (evidence * 0.92 + source * 0.26 + jitter(rng, 0.12)).clamp(0.0, 2.0),
    }
}

fn json_number(line: &str, key: &str) -> Option<f64> {
    let marker = format!("\"{key}\"");
    let start = line.find(&marker)?;
    let tail = &line[start + marker.len()..];
    let colon = tail.find(':')?;
    let mut value = tail[colon + 1..].trim_start();
    if let Some(stripped) = value.strip_prefix('"') {
        value = stripped;
    }
    let end = value
        .find(|ch: char| !(ch.is_ascii_digit() || matches!(ch, '.' | '-' | '+' | 'e' | 'E')))
        .unwrap_or(value.len());
    value[..end].parse::<f64>().ok()
}

fn json_string(line: &str, key: &str) -> Option<String> {
    let marker = format!("\"{key}\"");
    let start = line.find(&marker)?;
    let tail = &line[start + marker.len()..];
    let colon = tail.find(':')?;
    let value = tail[colon + 1..].trim_start();
    let value = value.strip_prefix('"')?;
    let end = value.find('"')?;
    Some(value[..end].to_string())
}

fn stable_u64(value: &str) -> u64 {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    let digest = hasher.finalize();
    let mut bytes = [0_u8; 8];
    bytes.copy_from_slice(&digest[..8]);
    u64::from_le_bytes(bytes)
}

fn stable_unit(value: &str) -> f64 {
    (stable_u64(value) as f64 / u64::MAX as f64).clamp(0.0, 1.0)
}

fn synthetic_metric(row: &PropertyRecord, salt: u64) -> f64 {
    let mut value = row
        .id
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add((row.zone_id as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9))
        .wrapping_add(salt.wrapping_mul(0x94D0_49BB_1331_11EB));
    value ^= value >> 30;
    value = value.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^= value >> 31;
    (value >> 11) as f64 / ((1_u64 << 53) as f64)
}

fn property_dataset_hash(properties: &[PropertyRecord]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"forge-immo-property-snapshot:v1");
    for row in properties {
        hasher.update(row.id.to_le_bytes());
        hasher.update(row.zone_id.to_le_bytes());
        for value in [
            row.lat,
            row.lon,
            row.surface_m2,
            row.rooms,
            row.age_years,
            row.floor,
            row.dvf_price_m2,
            row.asking_price_m2,
            row.dpe_score,
            row.energy_cost_index,
            row.clay_risk,
            row.flood_risk,
            row.permit_activity,
            row.urbanism_upside,
            row.transit_momentum,
            row.school_momentum,
            row.business_churn,
            row.traffic_noise_delta,
            row.pollution_delta,
            row.weather_heat_stress,
            row.mobility_inflow,
            row.local_news_intensity,
            row.competitor_pressure,
            row.buyer_demand_match,
            row.credit_rate_sensitivity,
            row.insurance_risk,
            row.tax_pressure_proxy,
            row.crm_inactivity_days,
            row.visit_intent,
            row.owner_lifecycle_pressure,
            row.rental_yield_gap,
            row.work_cost_roi,
            row.neighborhood_liquidity,
            row.price_anchor_error,
            row.days_on_market_shadow,
            row.notary_delay_index,
            row.seasonality_fit,
            row.agency_reputation_fit,
        ] {
            hasher.update(value.to_le_bytes());
        }
    }
    format!("immo-series:v1:{}", hex(&hasher.finalize()))
}

fn matrix_checksum(matrix: &NormalizedMatrix) -> f64 {
    let mut sum = 0.0_f64;
    for row in (0..matrix.rows).step_by(97) {
        for col in (0..matrix.cols).step_by(5) {
            sum += metric(matrix, row, col) as f64 * (col as f64 + 1.0);
        }
    }
    let means_checksum: f64 = matrix.means.iter().step_by(3).map(|v| *v as f64).sum();
    let stds_checksum: f64 = matrix.stds.iter().step_by(4).map(|v| *v as f64).sum();
    std::hint::black_box(sum + means_checksum * 0.01 + stds_checksum * 0.02)
}

fn matrix_checksum_raw(matrix: &MetricMatrix) -> f64 {
    let mut sum = 0.0_f64;
    for idx in (0..matrix.data.len()).step_by(211) {
        sum += matrix.data[idx] as f64 * ((idx % matrix.cols.max(1)) as f64 + 1.0);
    }
    std::hint::black_box(sum)
}

fn candidate_checksum(candidates: &[OpportunityCandidate]) -> f64 {
    let mut sum = 0.0_f64;
    for item in candidates.iter().step_by(31) {
        sum += item.base_score as f64
            + item.seller_probability as f64 * 10.0
            + item.expected_fee as f64 * 0.0001
            + (item.proof_mask.count_ones() as f64);
    }
    std::hint::black_box(sum)
}

fn candidate_signature(candidates: &[OpportunityCandidate]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"immo-candidates:v1");
    for item in candidates.iter().step_by((candidates.len() / 256).max(1)) {
        hasher.update(item.property_id.to_le_bytes());
        hasher.update(item.base_score.to_le_bytes());
        hasher.update(item.expected_fee.to_le_bytes());
        hasher.update(item.proof_mask.to_le_bytes());
    }
    hex(&hasher.finalize())
}

fn program_signature(programs: &[ScenarioProgram]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"immo-programs:v1");
    for program in programs {
        hasher.update(program.id.to_le_bytes());
        hasher.update(program.price_elasticity.to_le_bytes());
        hasher.update(program.rate_delta_bps.to_le_bytes());
        hasher.update(program.dpe_subsidy_boost.to_le_bytes());
        hasher.update(program.local_growth_shock.to_le_bytes());
        hasher.update(program.weather_risk_penalty.to_le_bytes());
        hasher.update(program.competition_noise.to_le_bytes());
        hasher.update(program.outreach_fit.to_le_bytes());
    }
    hex(&hasher.finalize())
}

fn print_plan_only(config: &Config) {
    let planned_key = cache_key(
        "immo_plan_only:v1",
        &[
            &format!("properties={}", config.properties),
            &format!("metrics={}", METRIC_NAMES.len()),
            &format!("scenarios={}", config.scenarios),
            &format!("candidate_limit={}", config.candidate_limit),
        ],
    );
    let planned_work = config
        .candidate_limit
        .min(config.properties)
        .saturating_mul(config.scenarios)
        .saturating_mul(HORIZONS_DAYS.len());
    println!(
        "[immo-lab] plan_only dag={} execution_mode=template_to_kasm_dag_no_source_load",
        compact_key(&planned_key)
    );
    println!(
        "[immo-lab] plan stages=authorized_sources,property_columns,metric_matrix,normalization,candidate_scan,scenario_programs,scenario_cube,llm_intel_pack metrics={} planned_work_items={}",
        METRIC_NAMES.len(),
        planned_work
    );
    println!(
        "[immo-lab] plan safety=LLM_never_loops_rows rust_kasm_returns_compact_hashes_scores_proofs"
    );
}

fn parse_config() -> Result<Config, Box<dyn Error>> {
    let mut config = Config::default();
    let mut args = env::args().skip(1).collect::<Vec<_>>();
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        print_usage();
        std::process::exit(0);
    }
    let mut idx = 0_usize;
    while idx < args.len() {
        match args[idx].as_str() {
            "--properties" | "--rows" => {
                idx += 1;
                config.properties = parse_value::<usize>(&args, idx, "--properties")?.max(1);
            }
            "--scenarios" => {
                idx += 1;
                config.scenarios = parse_value::<usize>(&args, idx, "--scenarios")?.max(1);
            }
            "--repeat" => {
                idx += 1;
                config.repeat = parse_value::<usize>(&args, idx, "--repeat")?.max(2);
            }
            "--candidate-limit" | "--candidates" => {
                idx += 1;
                config.candidate_limit = parse_value::<usize>(&args, idx, "--candidate-limit")?.max(1);
            }
            "--seed" => {
                idx += 1;
                config.seed = parse_value::<u64>(&args, idx, "--seed")?;
            }
            "--seeds" | "--metric-seeds" => {
                idx += 1;
                config.seeds_path = Some(
                    args.get(idx)
                        .ok_or("--seeds needs a JSONL path")?
                        .to_string(),
                );
            }
            "--focus" => {
                idx += 1;
                let focus = args.get(idx).ok_or("--focus needs a value")?;
                config.focus = parse_focus(focus)?;
            }
            "--plan-only" | "--plan" => {
                config.plan_only = true;
            }
            other => return Err(format!("unknown argument: {other}").into()),
        }
        idx += 1;
    }
    config.candidate_limit = config.candidate_limit.min(config.properties);
    args.clear();
    Ok(config)
}

fn parse_focus(value: &str) -> Result<Focus, Box<dyn Error>> {
    match value.trim().to_ascii_lowercase().as_str() {
        "scenario-dag" | "scenario_dag" | "dag" | "default" | "core" => Ok(Focus::ScenarioDag),
        "metric-matrix" | "metric_matrix" | "metrics" | "matrix" => Ok(Focus::MetricMatrix),
        "candidate-scan" | "candidate_scan" | "candidates" | "lead-scan" | "lead_scan" => {
            Ok(Focus::CandidateScan)
        }
        other => Err(format!("unknown --focus value: {other}").into()),
    }
}

fn parse_value<T>(args: &[String], idx: usize, name: &str) -> Result<T, Box<dyn Error>>
where
    T: std::str::FromStr,
    T::Err: Error + 'static,
{
    args.get(idx)
        .ok_or_else(|| format!("{name} needs a value"))?
        .parse::<T>()
        .map_err(|err| Box::new(err) as Box<dyn Error>)
}

fn print_usage() {
    println!("usage: cargo run --example lab_runner_immo -- [--properties N] [--scenarios N] [--repeat N]");
    println!("       cargo run --example lab_runner_immo -- --focus metric-matrix");
    println!("       cargo run --example lab_runner_immo -- --focus candidate-scan");
    println!("       cargo run --example lab_runner_immo -- --seeds path/to/kasm_metric_seeds.jsonl --properties N --scenarios N");
    println!("       cargo run --example lab_runner_immo -- --plan-only --properties N --scenarios N");
}

fn summarize_runs(target: &str, runs: &[BenchRun]) {
    let Some(cold) = runs.first() else { return };
    let Some(warm) = runs.last() else { return };
    let speedup = if warm.elapsed_us == 0 {
        f64::INFINITY
    } else {
        cold.elapsed_us as f64 / warm.elapsed_us as f64
    };
    let stage_speedup = if warm.stats.stage_elapsed_us == 0 {
        f64::INFINITY
    } else {
        cold.stats.stage_elapsed_us as f64 / warm.stats.stage_elapsed_us as f64
    };
    println!(
        "[immo-lab] summary target={} cold_ms={:.3} warm_ms={:.3} speedup_x={:.2} cold_stage_ms={:.3} warm_stage_ms={:.3} stage_speedup_x={:.2} cold_hits={} cold_misses={} warm_hits={} warm_misses={} warm_avoided_units={}",
        target,
        cold.elapsed_us as f64 / 1000.0,
        warm.elapsed_us as f64 / 1000.0,
        speedup,
        cold.stats.stage_elapsed_us as f64 / 1000.0,
        warm.stats.stage_elapsed_us as f64 / 1000.0,
        stage_speedup,
        cold.stats.hits,
        cold.stats.misses,
        warm.stats.hits,
        warm.stats.misses,
        warm.stats.avoided_units
    );
}

fn cache_key(stage: &str, parts: &[&str]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(stage.as_bytes());
    for part in parts {
        hasher.update([0]);
        hasher.update(part.as_bytes());
    }
    format!("{stage}:{}", hex(&hasher.finalize()))
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

fn compact_key(key: &str) -> String {
    if key.len() <= 58 {
        key.to_string()
    } else {
        format!("{}..{}", &key[..38], &key[key.len() - 16..])
    }
}

fn log_cache(kind: &str, stage: &str, key: &str, avoided_units: usize, elapsed: Duration) {
    println!(
        "[immo-lab] cache={} stage={} key={} elapsed_us={} avoided_units={}",
        kind,
        stage,
        compact_key(key),
        elapsed.as_micros(),
        avoided_units
    );
}

fn sigmoid(value: f64) -> f64 {
    1.0 / (1.0 + (-value).exp())
}

fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t.clamp(0.0, 1.0)
}

#[derive(Debug, Clone, Copy)]
struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn next_unit(&mut self) -> f64 {
        let value = self.next_u64() >> 11;
        value as f64 / ((1_u64 << 53) as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metric_matrix_has_expected_shape() {
        let rows = synthetic_properties(256, 42);
        let hash = property_dataset_hash(&rows);
        let mut cache = LabCache::default();
        let columns = cached_columns(&mut cache, &rows, &hash);
        let matrix = cached_metric_matrix(&mut cache, columns.as_ref(), &hash);
        assert_eq!(matrix.rows, 256);
        assert_eq!(matrix.cols, METRIC_NAMES.len());
        assert_eq!(matrix.data.len(), 256 * METRIC_NAMES.len());
    }

    #[test]
    fn metric_seed_expands_to_property_signal_space() {
        let seed = MetricSeedRecord {
            seed_hash: "seed-a".to_string(),
            pack_proof_hash: "pack-a".to_string(),
            pack_theme: "market_signal".to_string(),
            priority_score: 0.72,
            source_density_score: 0.63,
            evidence_quality_score: 0.81,
            graph_density_score: 0.58,
            market_signal_score: 0.91,
            local_signal_score: 0.46,
            economic_signal_score: 0.67,
            data_gap_penalty: 0.18,
            actionability_score: 0.76,
        };
        let mut rng = SplitMix64::new(7);
        let property = property_from_metric_seed(&seed, 12, 0, &mut rng);
        assert!(property.asking_price_m2 > property.dvf_price_m2);
        assert!(property.buyer_demand_match > 0.8);
        assert!(property.visit_intent > 0.8);
        assert!(property.agency_reputation_fit > 0.6);
    }

    #[test]
    fn scenario_cache_hits_on_second_pass() {
        let config = Config {
            properties: 512,
            scenarios: 32,
            repeat: 2,
            candidate_limit: 128,
            ..Config::default()
        };
        let rows = synthetic_properties(config.properties, config.seed);
        let hash = property_dataset_hash(&rows);
        let mut cache = LabCache::default();
        let first = run_scenario_dag_pipeline(&mut cache, &rows, &hash, &config, 1);
        let second = run_scenario_dag_pipeline(&mut cache, &rows, &hash, &config, 2);
        assert!(first.stats.misses > 0);
        assert!(second.stats.hits > first.stats.hits);
        assert_eq!(second.stats.misses, 0);
    }

    #[test]
    fn scenario_cube_is_deterministic() {
        let config = Config {
            properties: 768,
            scenarios: 48,
            repeat: 2,
            candidate_limit: 160,
            seed: 99,
            ..Config::default()
        };
        let rows = synthetic_properties(config.properties, config.seed);
        let hash = property_dataset_hash(&rows);
        let mut left_cache = LabCache::default();
        let mut right_cache = LabCache::default();
        let left = {
            let columns = cached_columns(&mut left_cache, &rows, &hash);
            let metrics = cached_metric_matrix(&mut left_cache, columns.as_ref(), &hash);
            let normalized = cached_normalized_matrix(&mut left_cache, metrics.as_ref(), &hash);
            let candidates = cached_candidate_scan(
                &mut left_cache,
                columns.as_ref(),
                normalized.as_ref(),
                &hash,
                config.candidate_limit,
            );
            let programs = cached_scenario_programs(&mut left_cache, &hash, config.scenarios, config.seed);
            cached_scenario_cube(
                &mut left_cache,
                columns.as_ref(),
                normalized.as_ref(),
                candidates.as_slice(),
                programs.as_slice(),
                &hash,
            )
        };
        let right = {
            let columns = cached_columns(&mut right_cache, &rows, &hash);
            let metrics = cached_metric_matrix(&mut right_cache, columns.as_ref(), &hash);
            let normalized = cached_normalized_matrix(&mut right_cache, metrics.as_ref(), &hash);
            let candidates = cached_candidate_scan(
                &mut right_cache,
                columns.as_ref(),
                normalized.as_ref(),
                &hash,
                config.candidate_limit,
            );
            let programs = cached_scenario_programs(&mut right_cache, &hash, config.scenarios, config.seed);
            cached_scenario_cube(
                &mut right_cache,
                columns.as_ref(),
                normalized.as_ref(),
                candidates.as_slice(),
                programs.as_slice(),
                &hash,
            )
        };
        assert_eq!(left.best_property_id, right.best_property_id);
        assert_eq!(left.best_scenario_id, right.best_scenario_id);
        assert_eq!(left.evidence_hash, right.evidence_hash);
    }
}
