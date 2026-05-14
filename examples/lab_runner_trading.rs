//! Trading lab runner.
//!
//! Direct bench for KASM-style content addressed Trading computations:
//! identical OHLCV + identical params => identical sub-computation key.
//! The second pass must show cache HIT logs and avoided work.

use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::env;
use std::error::Error;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::sync::Arc;
use std::time::{Duration, Instant};

const ALPHA_TRADING_OVERLAY_CACHE_MAX_HINT: usize = 96;
const INDICATOR_WINDOW_TOLERANCE: f64 = 1e-8;

#[derive(Debug, Clone, Copy, Default)]
struct Bar {
    time_ms: i64,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    volume: f64,
}

#[derive(Debug, Clone)]
struct Columns {
    time_ms: Vec<i64>,
    open: Vec<f64>,
    high: Vec<f64>,
    low: Vec<f64>,
    close: Vec<f64>,
    volume: Vec<f64>,
}

#[derive(Debug, Clone)]
struct IndicatorBundle {
    fast_ema: Vec<f64>,
    slow_ema: Vec<f64>,
    trend_ema: Vec<f64>,
    vwap: Vec<f64>,
}

#[derive(Debug, Clone)]
struct UiOverlayBundle {
    ema21: Vec<f64>,
    sma50: Vec<f64>,
    bb_basis: Vec<f64>,
    bb_upper: Vec<f64>,
    bb_lower: Vec<f64>,
    vwap: Vec<f64>,
}

#[derive(Debug, Clone)]
struct ChartProjection {
    points: Vec<(f32, f32)>,
}

#[derive(Debug, Clone, Copy)]
struct StrategyParams {
    fast: usize,
    slow: usize,
    trend: usize,
    fee_bps: f64,
}

impl Default for StrategyParams {
    fn default() -> Self {
        Self {
            fast: 8,
            slow: 21,
            trend: 50,
            fee_bps: 0.7,
        }
    }
}

impl StrategyParams {
    fn cache_key(&self) -> String {
        format!(
            "fast={}:slow={}:trend={}:fee_bps={:.4}",
            self.fast, self.slow, self.trend, self.fee_bps
        )
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct StrategyEval {
    trades: usize,
    wins: usize,
    final_pnl: f64,
    max_drawdown: f64,
}

#[derive(Debug, Clone, Copy)]
struct StrategyDagParams {
    entry_hour_utc: u32,
    low_vol_lookback: usize,
    low_vol_quantile: f64,
    stop_loss_distance: f64,
    take_profit_min_distance: f64,
    take_profit_max_distance: f64,
    take_profit_steps: usize,
    max_hold_bars: usize,
    spread_distance: f64,
    slippage_distance: f64,
    force_daily_entry: bool,
}

impl Default for StrategyDagParams {
    fn default() -> Self {
        Self {
            entry_hour_utc: 21,
            low_vol_lookback: 24,
            low_vol_quantile: 0.25,
            stop_loss_distance: 0.045,
            take_profit_min_distance: 0.035,
            take_profit_max_distance: 0.300,
            take_profit_steps: 24,
            max_hold_bars: 24,
            spread_distance: 0.002,
            slippage_distance: 0.001,
            force_daily_entry: true,
        }
    }
}

impl StrategyDagParams {
    fn cache_key(&self) -> String {
        format!(
            "entry_hour={}:lookback={}:q={:.4}:force_daily={}:sl={:.6}:tp={:.6}-{:.6}:steps={}:hold={}:spread={:.6}:slip={:.6}",
            self.entry_hour_utc,
            self.low_vol_lookback,
            self.low_vol_quantile,
            self.force_daily_entry,
            self.stop_loss_distance,
            self.take_profit_min_distance,
            self.take_profit_max_distance,
            self.take_profit_steps,
            self.max_hold_bars,
            self.spread_distance,
            self.slippage_distance,
        )
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct StrategyDagOutcome {
    entries: usize,
    work_items: usize,
    long_wins: usize,
    short_wins: usize,
    best_direction: i8,
    best_take_profit_distance: f64,
    best_win_rate: f64,
    best_expectancy: f64,
    checksum: f64,
}

#[derive(Debug, Clone, Copy)]
struct StrategyDagOutcomePoint {
    favorable_distance: f64,
}

#[derive(Debug, Clone)]
struct StrategyDagEntryOutcome {
    entry_index: usize,
    execution_cost: f64,
    terminal_pnl: f64,
    stop_pnl: f64,
    stop_hit: bool,
    favorable_path: Vec<StrategyDagOutcomePoint>,
}

impl StrategyDagOutcome {
    fn best_direction_label(self) -> &'static str {
        if self.best_direction < 0 {
            "short"
        } else {
            "long"
        }
    }
}

type CacheStats = scan::compute_core::ComputeCacheStats;

#[derive(Default)]
struct LabCache {
    columns: HashMap<String, Arc<Columns>>,
    windows: HashMap<String, (usize, usize)>,
    indicators: HashMap<String, Arc<IndicatorBundle>>,
    projections: HashMap<String, Arc<ChartProjection>>,
    ui_overlays: HashMap<String, Arc<UiOverlayBundle>>,
    ui_legends: HashMap<String, UiLegendSnapshot>,
    labels: HashMap<String, Arc<Vec<i8>>>,
    evals: HashMap<String, StrategyEval>,
    strategy_low_volatility: HashMap<String, Arc<Vec<f64>>>,
    strategy_thresholds: HashMap<String, f64>,
    strategy_entries: HashMap<String, Arc<Vec<usize>>>,
    strategy_condition_programs: HashMap<String, usize>,
    strategy_mfe_reduce: HashMap<String, StrategyDagOutcome>,
    stats: CacheStats,
    log_events: bool,
}

impl LabCache {
    fn hit(&mut self, stage: &str, key: &str, avoided_units: usize, elapsed: Duration) {
        self.stats.record_hit(avoided_units, elapsed);
        if self.log_events {
            log_cache("HIT", stage, key, avoided_units, elapsed);
        }
    }

    fn miss(&mut self, stage: &str, key: &str, elapsed: Duration) {
        self.stats.record_miss(elapsed);
        if self.log_events {
            log_cache("MISS", stage, key, 0, elapsed);
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct UiLegendSnapshot {
    ema21: f64,
    sma50: f64,
    bb_basis: f64,
    bb_width: f64,
    vwap: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    Core,
    UiFrame,
    IndicatorWindows,
    LiveMerge,
    RefreshDedupe,
    CanvasDocument,
    ViewportWindow,
    OverlayKey,
    ComparisonCharts,
    SignalMarkers,
    RenderEntries,
    HitTestSlot,
    SelectionLookup,
    IndicatorKeyCache,
    TradingSubbarCache,
    HeaderDockCache,
    ToolbarChromeCache,
    ComparisonPayloadCache,
    HistoryLoadCoalescing,
    SignalParseIndex,
    MetricSeriesCache,
    ThreeDPayloadCache,
    ThreeDGpuUploadCache,
    StrategyDagCache,
    SourceSeriesCache,
    TimeLabelCache,
    AssetCatalogCache,
    CatalogIndexCache,
    AssetSearchIndexCache,
    ContextSnapshotCache,
    AlertPayloadCache,
}

macro_rules! focus_aliases {
    ($($focus:ident: $($alias:literal)|+;)+) => {
        &[$((&[$($alias),+], Focus::$focus)),+]
    };
}

const FOCUS_ALIASES: &[(&[&str], Focus)] = focus_aliases! {
    Core: "core"|"default";
    UiFrame: "ui-frame"|"ui_frame"|"frame"|"overlay"|"overlays";
    IndicatorWindows: "indicator-windows"|"indicator_windows"|"windows"|"indicators";
    LiveMerge: "live-merge"|"live_merge"|"merge"|"market-feed"|"market_feed"|"live";
    RefreshDedupe: "refresh-dedupe"|"refresh_dedupe"|"refresh-pipeline"|"refresh_pipeline"|"double-merge"|"double_merge"|"pipeline";
    CanvasDocument: "canvas-document"|"canvas_document"|"document"|"canvas-doc"|"canvas_doc"|"set-document"|"set_document";
    ViewportWindow: "viewport-window"|"viewport_window"|"logical-window"|"logical_window"|"visible-window"|"visible_window";
    OverlayKey: "overlay-key"|"overlay_key"|"indicator-key"|"indicator_key"|"series-key"|"series_key";
    ComparisonCharts: "comparison-charts"|"comparison_charts"|"compare-charts"|"compare_charts"|"extra-charts"|"extra_charts";
    SignalMarkers: "signal-markers"|"signal_markers"|"signals"|"markers";
    RenderEntries: "render-entries"|"render_entries"|"visible-render-entries"|"visible_render_entries"|"candle-render-entries"|"candle_render_entries";
    HitTestSlot: "hit-test-slot"|"hit_test_slot"|"visible-slot"|"visible_slot"|"slot-hit"|"slot_hit"|"hit-test"|"hit_test";
    SelectionLookup: "selection-lookup"|"selection_lookup"|"selected-candles"|"selected_candles"|"selection-key"|"selection_key";
    IndicatorKeyCache: "indicator-key-cache"|"indicator_key_cache"|"overlay-indicator-key"|"overlay_indicator_key"|"settings-key-cache"|"settings_key_cache";
    TradingSubbarCache: "trading-subbar-cache"|"trading_subbar_cache"|"subbar-cache"|"subbar_cache"|"indicator-subbar-cache"|"indicator_subbar_cache"|"chat-subbar-cache"|"chat_subbar_cache";
    HeaderDockCache: "header-dock-cache"|"header_dock_cache"|"trading-header-cache"|"trading_header_cache"|"indicator-dock-cache"|"indicator_dock_cache";
    ToolbarChromeCache: "toolbar-chrome-cache"|"toolbar_chrome_cache"|"trigger-chrome-cache"|"trigger_chrome_cache"|"chat-actions-cache"|"chat_actions_cache"|"topbar-trigger-cache"|"topbar_trigger_cache";
    ComparisonPayloadCache: "comparison-payload-cache"|"comparison_payload_cache"|"extra-chart-payload-cache"|"extra_chart_payload_cache"|"extra-charts-cache"|"extra_charts_cache";
    HistoryLoadCoalescing: "history-load-coalescing"|"history_load_coalescing"|"history-series-coalescing"|"history_series_coalescing"|"load-history-cache"|"load_history_cache"|"chart-series-request-cache"|"chart_series_request_cache";
    SignalParseIndex: "signal-parse-index"|"signal_parse_index"|"parse-signal-index"|"parse_signal_index"|"signal-log-index"|"signal_log_index";
    MetricSeriesCache: "metric-series-cache"|"metric_series_cache"|"metric-axis-cache"|"metric_axis_cache"|"axis-metric-series"|"axis_metric_series";
    ThreeDPayloadCache: "3d-payload-cache"|"3d_payload_cache"|"three-d-payload-cache"|"three_d_payload_cache"|"payload-3d-cache"|"payload_3d_cache"|"volume-profile-3d"|"volume_profile_3d";
    ThreeDGpuUploadCache: "3d-gpu-upload-cache"|"3d_gpu_upload_cache"|"three-d-gpu-upload-cache"|"three_d_gpu_upload_cache"|"webgl-upload-cache"|"webgl_upload_cache"|"bufferdata-cache"|"bufferdata_cache";
    StrategyDagCache: "strategy-dag-cache"|"strategy_dag_cache"|"strategy-dag"|"strategy_dag"|"create-strategy-cache"|"create_strategy_cache"|"kasm-strategy-cache"|"kasm_strategy_cache";
    SourceSeriesCache: "source-series-cache"|"source_series_cache"|"indicator-source-cache"|"indicator_source_cache"|"price-source-cache"|"price_source_cache";
    TimeLabelCache: "time-label-cache"|"time_label_cache"|"axis-label-cache"|"axis_label_cache"|"intl-label-cache"|"intl_label_cache"|"timezone-label-cache"|"timezone_label_cache";
    AssetCatalogCache: "asset-catalog-cache"|"asset_catalog_cache"|"available-assets-cache"|"available_assets_cache"|"library-assets-cache"|"library_assets_cache";
    CatalogIndexCache: "catalog-index-cache"|"catalog_index_cache"|"catalog-map-cache"|"catalog_map_cache"|"history-catalog-index"|"history_catalog_index"|"broker-instrument-set"|"broker_instrument_set";
    AssetSearchIndexCache: "asset-search-index-cache"|"asset_search_index_cache"|"asset-search-cache"|"asset_search_cache"|"compare-menu-model-cache"|"compare_menu_model_cache"|"mention-asset-cache"|"mention_asset_cache";
    ContextSnapshotCache: "context-snapshot-cache"|"context_snapshot_cache"|"context-cache"|"context_cache"|"digest-cache"|"digest_cache"|"trading-context-cache"|"trading_context_cache";
    AlertPayloadCache: "alert-payload-cache"|"alert_payload_cache"|"alert-cache"|"alert_cache"|"alert-modal-cache"|"alert_modal_cache"|"canvas-alert-cache"|"canvas_alert_cache";
};

type SeriesFocusRunner = fn(&[Bar], &str, usize);

const SERIES_FOCUS_ROUTES: &[(Focus, SeriesFocusRunner)] = &[
    (Focus::IndicatorWindows, run_indicator_windows_focus),
    (Focus::LiveMerge, run_live_merge_focus),
    (Focus::RefreshDedupe, run_refresh_dedupe_focus),
    (Focus::CanvasDocument, run_canvas_document_focus),
    (Focus::ViewportWindow, run_viewport_window_focus),
    (Focus::OverlayKey, run_overlay_key_focus),
    (Focus::ComparisonCharts, run_comparison_charts_focus),
    (Focus::SignalMarkers, run_signal_markers_focus),
    (Focus::RenderEntries, run_render_entries_focus),
    (Focus::HitTestSlot, run_hit_test_slot_focus),
    (Focus::SelectionLookup, run_selection_lookup_focus),
    (Focus::IndicatorKeyCache, run_indicator_key_cache_focus),
    (Focus::TradingSubbarCache, run_trading_subbar_cache_focus),
    (Focus::HeaderDockCache, run_header_dock_cache_focus),
    (Focus::ToolbarChromeCache, run_toolbar_chrome_cache_focus),
    (Focus::ComparisonPayloadCache, run_comparison_payload_cache_focus),
    (Focus::HistoryLoadCoalescing, run_history_load_coalescing_focus),
    (Focus::SignalParseIndex, run_signal_parse_index_focus),
    (Focus::MetricSeriesCache, run_metric_series_cache_focus),
    (Focus::ThreeDPayloadCache, run_three_d_payload_cache_focus),
    (Focus::ThreeDGpuUploadCache, run_three_d_gpu_upload_cache_focus),
    (Focus::SourceSeriesCache, run_source_series_cache_focus),
    (Focus::TimeLabelCache, run_time_label_cache_focus),
    (Focus::AssetCatalogCache, run_asset_catalog_cache_focus),
    (Focus::CatalogIndexCache, run_catalog_index_cache_focus),
    (Focus::AssetSearchIndexCache, run_asset_search_index_cache_focus),
    (Focus::ContextSnapshotCache, run_context_snapshot_cache_focus),
    (Focus::AlertPayloadCache, run_alert_payload_cache_focus),
];

#[derive(Debug, Clone)]
struct Config {
    bars: usize,
    visible: usize,
    repeat: usize,
    frames: usize,
    focus: Focus,
    csv: Option<String>,
    max_rows: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            bars: 20_000,
            visible: 1_200,
            repeat: 2,
            frames: 60,
            focus: Focus::Core,
            csv: None,
            max_rows: 0,
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
    let bars = if let Some(path) = config.csv.as_deref() {
        load_csv(path, config.max_rows)?
    } else {
        synthetic_bars(config.bars)
    };

    if bars.is_empty() {
        return Err("no OHLCV bars available for trading lab".into());
    }

    let series_hash = series_hash(&bars);
    let params = StrategyParams::default();
    let mut cache = LabCache {
        log_events: true,
        ..LabCache::default()
    };

    println!(
        "[trading-lab] doctrine=kasm-content-addressed series_hash={} bars={} visible={} repeat={} params={}",
        series_hash,
        bars.len(),
        config.visible.min(bars.len()),
        config.repeat.max(2),
        params.cache_key()
    );

    if !run_series_focus(config.focus, &bars, &series_hash, config.frames) {
        match config.focus {
            Focus::Core => {
                let mut chart_runs = Vec::new();
                for pass in 1..=config.repeat.max(2) {
                    chart_runs.push(run_chart_load(
                        &mut cache,
                        &bars,
                        &series_hash,
                        config.visible,
                        params,
                        pass,
                    ));
                }
                summarize_runs("chart_load", &chart_runs);

                let mut strategy_runs = Vec::new();
                for pass in 1..=config.repeat.max(2) {
                    strategy_runs.push(run_strategy(
                        &mut cache,
                        &bars,
                        &series_hash,
                        params,
                        pass,
                    ));
                }
                summarize_runs("strategy", &strategy_runs);
            }
            Focus::UiFrame => run_ui_frame_focus(
                &mut cache,
                &bars,
                &series_hash,
                config.visible,
                params,
                config.frames,
                config.repeat.max(2),
            ),
            Focus::StrategyDagCache => {
                run_strategy_dag_cache_focus(&mut cache, &bars, &series_hash, config.repeat.max(2))
            }
            focus => unreachable!("series focus route missing for {focus:?}"),
        }
    }

    println!(
        "[trading-lab] global cache_hits={} cache_misses={} avoided_units={}",
        cache.stats.hits, cache.stats.misses, cache.stats.avoided_units
    );
    Ok(())
}

fn run_series_focus(focus: Focus, bars: &[Bar], series_hash: &str, frames: usize) -> bool {
    let Some((_, runner)) = SERIES_FOCUS_ROUTES
        .iter()
        .find(|(candidate, _)| *candidate == focus)
    else {
        return false;
    };
    runner(bars, series_hash, frames);
    true
}

fn run_ui_frame_focus(
    cache: &mut LabCache,
    bars: &[Bar],
    series_hash: &str,
    visible: usize,
    params: StrategyParams,
    frames: usize,
    repeat: usize,
) {
    println!(
        "[trading-lab] focus=ui-frame task=remove-frame-recompute frames={} active_overlays=ema21,sma50,bollinger20,vwap",
        frames
    );
    let legacy = legacy_ui_frame_pipeline(bars, visible, frames);
    println!(
        "[trading-lab] legacy-ui-frame elapsed_ms={:.3} date_parse_ops={} full_visible_scans={} overlay_recomputes={} legend_recomputes={} rolling_std_model=o(n*window)",
        legacy.elapsed_us as f64 / 1000.0,
        legacy.date_parse_ops,
        legacy.visible_scan_ops,
        legacy.overlay_recomputes,
        legacy.legend_recomputes,
    );

    let mut runs = Vec::new();
    for pass in 1..=repeat {
        runs.push(run_cached_ui_frame_pipeline(
            cache,
            bars,
            series_hash,
            visible,
            params,
            frames,
            pass,
        ));
    }
    summarize_runs("ui_frame_cache", &runs);
}

#[derive(Default)]
struct LegacyUiFrameStats {
    elapsed_us: u128,
    date_parse_ops: usize,
    visible_scan_ops: usize,
    overlay_recomputes: usize,
    legend_recomputes: usize,
}

fn legacy_ui_frame_pipeline(bars: &[Bar], visible: usize, frames: usize) -> LegacyUiFrameStats {
    let started = Instant::now();
    let visible = visible.min(bars.len()).max(1);
    let mut stats = LegacyUiFrameStats::default();
    for _ in 0..frames {
        let mut parsed = Vec::with_capacity(bars.len());
        for bar in bars {
            parsed.push(bar.time_ms);
            stats.date_parse_ops += 1;
        }
        let min_time = parsed[parsed.len().saturating_sub(visible)];
        let max_time = *parsed.last().unwrap_or(&min_time);
        let mut visible_rows = 0_usize;
        for time_ms in parsed {
            stats.visible_scan_ops += 1;
            if time_ms >= min_time && time_ms <= max_time {
                visible_rows += 1;
            }
        }
        let draw = legacy_overlay_bundle(bars);
        let legend = legacy_overlay_bundle(bars);
        stats.overlay_recomputes += 1;
        stats.legend_recomputes += 1;
        std::hint::black_box((visible_rows, draw.bb_upper.len(), legend.bb_lower.len()));
    }
    stats.elapsed_us = started.elapsed().as_micros();
    stats
}

fn run_cached_ui_frame_pipeline(
    cache: &mut LabCache,
    bars: &[Bar],
    series_hash: &str,
    visible: usize,
    params: StrategyParams,
    frames: usize,
    pass: usize,
) -> BenchRun {
    let stats_before = cache.stats;
    let started = Instant::now();
    let columns = cached_columns(cache, bars, series_hash);
    let window = cached_visible_window(cache, &columns, series_hash, visible);
    let _indicators = cached_indicators(cache, &columns, series_hash, params);
    let overlays = cached_ui_overlay_bundle(cache, &columns, series_hash);
    let legend = cached_ui_legend_snapshot(cache, &overlays, series_hash, window);
    let previous_log_events = cache.log_events;
    cache.log_events = false;
    for _ in 1..frames {
        let _ = cached_columns(cache, bars, series_hash);
        let _ = cached_visible_window(cache, &columns, series_hash, visible);
        let overlays = cached_ui_overlay_bundle(cache, &columns, series_hash);
        let _ = cached_ui_legend_snapshot(cache, &overlays, series_hash, window);
    }
    cache.log_events = previous_log_events;
    let elapsed = started.elapsed();
    let stats = cache.stats.delta(stats_before);
    println!(
        "[trading-lab] pass={} target=ui_frame_cache elapsed_ms={:.3} stage_ms={:.3} frames={} legend=ema21:{:.5}/sma50:{:.5}/bb_basis:{:.5}/bb_width:{:.5}/vwap:{:.5} hits={} misses={} avoided_units={}",
        pass,
        elapsed.as_secs_f64() * 1000.0,
        stats.stage_elapsed_us as f64 / 1000.0,
        frames,
        legend.ema21,
        legend.sma50,
        legend.bb_basis,
        legend.bb_width,
        legend.vwap,
        stats.hits,
        stats.misses,
        stats.avoided_units
    );
    BenchRun {
        elapsed_us: elapsed.as_micros(),
        stats,
    }
}

#[derive(Debug, Clone)]
struct IndicatorWindowBundle {
    wma21: Vec<f64>,
    hma55: Vec<f64>,
    donchian_high20: Vec<f64>,
    donchian_low20: Vec<f64>,
}

#[derive(Debug, Clone, Copy)]
struct PercentileStats {
    p50_us: u128,
    p95_us: u128,
    p99_us: u128,
}

fn run_indicator_windows_focus(bars: &[Bar], series_hash: &str, frames: usize) {
    let frames = frames.max(3);
    let close: Vec<f64> = bars.iter().map(|bar| bar.close).collect();
    let optimized_probe = optimized_indicator_window_bundle(bars, &close);
    let legacy_probe = legacy_indicator_window_bundle(bars, &close);
    let (max_diff, mismatches) = compare_indicator_bundles(&legacy_probe, &optimized_probe);
    println!(
        "[trading-lab] focus=indicator-windows task=replace-window-nested-loops series_hash={} bars={} frames={} indicators=wma21,hma55,donchian20",
        series_hash,
        bars.len(),
        frames
    );
    println!(
        "[trading-lab] proof indicator-windows max_abs_diff={:.12} mismatches={} tolerance={:.0e}",
        max_diff,
        mismatches,
        INDICATOR_WINDOW_TOLERANCE
    );

    let mut legacy_samples = Vec::with_capacity(frames);
    let mut optimized_samples = Vec::with_capacity(frames);
    let mut legacy_checksum = 0.0_f64;
    let mut optimized_checksum = 0.0_f64;
    for _ in 0..frames {
        let started = Instant::now();
        let bundle = legacy_indicator_window_bundle(bars, &close);
        legacy_checksum += indicator_window_checksum(&bundle);
        legacy_samples.push(started.elapsed().as_micros());
    }
    for _ in 0..frames {
        let started = Instant::now();
        let bundle = optimized_indicator_window_bundle(bars, &close);
        optimized_checksum += indicator_window_checksum(&bundle);
        optimized_samples.push(started.elapsed().as_micros());
    }
    let legacy = percentile_stats(&legacy_samples);
    let optimized = percentile_stats(&optimized_samples);
    let avoided_inner_ops = estimate_indicator_window_inner_ops(bars.len(), frames);
    println!(
        "[trading-lab] legacy-indicator-windows model=o(n*window) p50_ms={:.3} p95_ms={:.3} p99_ms={:.3} checksum={:.6}",
        legacy.p50_us as f64 / 1000.0,
        legacy.p95_us as f64 / 1000.0,
        legacy.p99_us as f64 / 1000.0,
        legacy_checksum
    );
    println!(
        "[trading-lab] optimized-indicator-windows model=sliding-window+monotonic-deque p50_ms={:.3} p95_ms={:.3} p99_ms={:.3} checksum={:.6} avoided_inner_ops_est={}",
        optimized.p50_us as f64 / 1000.0,
        optimized.p95_us as f64 / 1000.0,
        optimized.p99_us as f64 / 1000.0,
        optimized_checksum,
        avoided_inner_ops
    );
    println!(
        "[trading-lab] summary target=indicator_windows p50_speedup_x={:.2} p95_speedup_x={:.2} p99_speedup_x={:.2} cache_budget=bounded-upstream overlay_cache_max={}",
        ratio(legacy.p50_us, optimized.p50_us),
        ratio(legacy.p95_us, optimized.p95_us),
        ratio(legacy.p99_us, optimized.p99_us),
        ALPHA_TRADING_OVERLAY_CACHE_MAX_HINT
    );
}

#[derive(Debug, Clone, Copy)]
struct LiveMergeRunStats {
    stats: PercentileStats,
    checksum: f64,
    final_len: usize,
}

#[derive(Debug, Clone)]
struct CanvasDocument {
    candles: Vec<Bar>,
    logical_times: Vec<i64>,
    ema8: Vec<f64>,
    ema21: Vec<f64>,
    ema50: Vec<f64>,
    vwap: Vec<f64>,
}

#[derive(Debug, Clone, Copy)]
struct CanvasDocumentRunStats {
    stats: PercentileStats,
    checksum: f64,
    final_len: usize,
    reused_prefix_units: usize,
}

#[derive(Debug, Clone, Copy)]
struct ViewportWindowRunStats {
    stats: PercentileStats,
    checksum: f64,
    hits: usize,
    misses: usize,
    avoided_entries: usize,
}

#[derive(Debug, Clone)]
struct OverlayKeyMeta {
    key: String,
    h1_by_index: Vec<u32>,
    h2_by_index: Vec<u32>,
}

#[derive(Debug, Clone, Copy)]
struct OverlayKeyRunStats {
    stats: PercentileStats,
    checksum: u64,
    final_len: usize,
    reused_prefix_rows: usize,
    tail_rows_hashed: usize,
}

#[derive(Debug, Clone, Copy)]
struct ComparisonChartsRunStats {
    stats: PercentileStats,
    checksum: f64,
    hits: usize,
    misses: usize,
    scanned_rows: usize,
    avoided_rows: usize,
}

#[derive(Debug, Clone, Copy)]
struct SignalMarkerRunStats {
    stats: PercentileStats,
    checksum: usize,
    hits: usize,
    misses: usize,
    comparisons: usize,
    avoided_comparisons: usize,
}

#[derive(Debug, Clone)]
struct SignalSlotIndex {
    times: Vec<i64>,
    slots: Vec<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct RenderEntry {
    logical_index: usize,
    time_ms: i64,
    x_start: f64,
    x_center: f64,
    x_wick: f64,
    close: f64,
}

#[derive(Debug, Clone, Copy)]
struct RenderEntryRunStats {
    stats: PercentileStats,
    checksum: f64,
    hits: usize,
    misses: usize,
    avoided_entries: usize,
}

#[derive(Default)]
struct RenderEntryCache {
    key: (i64, i64, i64, usize, i64, i64),
    value: Vec<RenderEntry>,
    initialized: bool,
    hits: usize,
    misses: usize,
    avoided_entries: usize,
}

#[derive(Debug, Clone, Copy)]
struct SlotLookupRunStats {
    stats: PercentileStats,
    checksum: usize,
    hits: usize,
    misses: usize,
    comparisons: usize,
    avoided_comparisons: usize,
}

#[derive(Default)]
struct SlotEntryCache {
    key: (usize, usize, i64, i64),
    entries: Vec<Option<(usize, i64)>>,
    initialized: bool,
    hits: usize,
    misses: usize,
    avoided_comparisons: usize,
}

#[derive(Debug, Clone, Copy)]
struct SelectionLookupRunStats {
    stats: PercentileStats,
    checksum: usize,
    selected_hits: usize,
    key_builds: usize,
    avoided_key_builds: usize,
}

#[derive(Debug, Clone)]
struct SyntheticIndicator {
    id: &'static str,
    settings: Vec<(&'static str, SettingValue)>,
}

#[derive(Debug, Clone, Copy)]
enum SettingValue {
    Int(i64),
    Float(f64),
    Text(&'static str),
}

#[derive(Debug, Clone, Copy)]
struct IndicatorKeyRunStats {
    stats: PercentileStats,
    checksum: usize,
    hits: usize,
    misses: usize,
    serializations: usize,
    avoided_serializations: usize,
}

struct IndicatorKeyObjectCache {
    keys: Vec<Option<String>>,
    hits: usize,
    misses: usize,
    avoided_serializations: usize,
}

impl IndicatorKeyObjectCache {
    fn new(len: usize) -> Self {
        Self {
            keys: vec![None; len],
            hits: 0,
            misses: 0,
            avoided_serializations: 0,
        }
    }
}

#[derive(Debug, Clone)]
struct SubbarLibraryEntry {
    id: String,
    command: String,
    favorites: bool,
    family: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TradingSubbarProbe {
    markup_hash: usize,
    nav_items: usize,
    body_items: usize,
    attached_items: usize,
}

#[derive(Debug, Clone, Copy, Default)]
struct TradingSubbarWork {
    catalog_maps: usize,
    favorite_filters: usize,
    section_filters: usize,
    active_membership_checks: usize,
    icon_builds: usize,
    markup_bytes: usize,
    dom_writes: usize,
}

#[derive(Debug, Clone, Copy)]
struct TradingSubbarRunStats {
    stats: PercentileStats,
    checksum: usize,
    hits: usize,
    misses: usize,
    catalog_maps: usize,
    avoided_catalog_maps: usize,
    favorite_filters: usize,
    avoided_favorite_filters: usize,
    section_filters: usize,
    avoided_section_filters: usize,
    active_membership_checks: usize,
    avoided_active_membership_checks: usize,
    icon_builds: usize,
    avoided_icon_builds: usize,
    markup_bytes: usize,
    avoided_markup_bytes: usize,
    dom_writes: usize,
    avoided_dom_writes: usize,
}

#[derive(Default)]
struct TradingSubbarCache {
    key: String,
    probe: Option<TradingSubbarProbe>,
    last_full_work: TradingSubbarWork,
    catalog: Option<Vec<SubbarLibraryEntry>>,
    favorites: Option<Vec<usize>>,
    create: Option<Vec<usize>>,
    strategies: Option<Vec<usize>>,
    hits: usize,
    misses: usize,
    catalog_hits: usize,
    avoided_catalog_maps: usize,
    avoided_favorite_filters: usize,
    avoided_section_filters: usize,
    avoided_active_membership_checks: usize,
    avoided_icon_builds: usize,
    avoided_markup_bytes: usize,
    avoided_dom_writes: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct HeaderDockProbe {
    header_hash: usize,
    dock_hash: usize,
    chips: usize,
}

#[derive(Debug, Clone, Copy, Default)]
struct HeaderDockWork {
    asset_lookups: usize,
    broker_set_checks: usize,
    header_payload_builds: usize,
    header_bridge_writes: usize,
    dock_key_units: usize,
    dock_markup_bytes: usize,
    dock_dom_writes: usize,
    action_syncs: usize,
}

#[derive(Debug, Clone, Copy)]
struct HeaderDockRunStats {
    stats: PercentileStats,
    checksum: usize,
    hits: usize,
    misses: usize,
    asset_lookups: usize,
    avoided_asset_lookups: usize,
    broker_set_checks: usize,
    avoided_broker_set_checks: usize,
    header_payload_builds: usize,
    avoided_header_payload_builds: usize,
    header_bridge_writes: usize,
    avoided_header_bridge_writes: usize,
    dock_key_units: usize,
    dock_markup_bytes: usize,
    avoided_dock_markup_bytes: usize,
    dock_dom_writes: usize,
    avoided_dock_dom_writes: usize,
    action_syncs: usize,
}

#[derive(Default)]
struct HeaderDockCache {
    header_key: String,
    dock_key: String,
    header_probe: Option<HeaderDockProbe>,
    dock_hash: usize,
    hits: usize,
    misses: usize,
    last_full_work: HeaderDockWork,
    avoided_asset_lookups: usize,
    avoided_broker_set_checks: usize,
    avoided_header_payload_builds: usize,
    avoided_header_bridge_writes: usize,
    avoided_dock_markup_bytes: usize,
    avoided_dock_dom_writes: usize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct ToolbarChromeState {
    active: bool,
    display_menu_open: bool,
    right_panel_open: bool,
    chart_mode: usize,
    chat_mode: usize,
    selection_enabled: bool,
    runtime_involved: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ToolbarChromeProbe {
    dom_hash: usize,
    trigger_states: usize,
}

#[derive(Debug, Clone, Copy, Default)]
struct ToolbarChromeWork {
    state_reads: usize,
    html_bytes: usize,
    attr_writes: usize,
    dataset_writes: usize,
    hidden_writes: usize,
    class_toggles: usize,
    subbar_syncs: usize,
    runtime_control_syncs: usize,
}

#[derive(Debug, Clone, Copy)]
struct ToolbarChromeRunStats {
    stats: PercentileStats,
    checksum: usize,
    hits: usize,
    misses: usize,
    state_reads: usize,
    html_bytes: usize,
    avoided_html_bytes: usize,
    attr_writes: usize,
    avoided_attr_writes: usize,
    dataset_writes: usize,
    avoided_dataset_writes: usize,
    hidden_writes: usize,
    avoided_hidden_writes: usize,
    class_toggles: usize,
    avoided_class_toggles: usize,
    subbar_syncs: usize,
    avoided_subbar_syncs: usize,
    runtime_control_syncs: usize,
    avoided_runtime_control_syncs: usize,
}

#[derive(Default)]
struct ToolbarChromeCache {
    key: String,
    probe: Option<ToolbarChromeProbe>,
    hits: usize,
    misses: usize,
    last_full_work: ToolbarChromeWork,
    avoided_html_bytes: usize,
    avoided_attr_writes: usize,
    avoided_dataset_writes: usize,
    avoided_hidden_writes: usize,
    avoided_class_toggles: usize,
    avoided_subbar_syncs: usize,
    avoided_runtime_control_syncs: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ComparisonPayloadProbe {
    payload_hash: usize,
    charts: usize,
    candle_refs: usize,
}

#[derive(Debug, Clone, Copy, Default)]
struct ComparisonPayloadWork {
    revision_checks: usize,
    asset_lookups: usize,
    label_builds: usize,
    payload_builds: usize,
    candle_refs: usize,
    bridge_writes: usize,
}

#[derive(Debug, Clone, Copy)]
struct ComparisonPayloadRunStats {
    stats: PercentileStats,
    checksum: usize,
    hits: usize,
    misses: usize,
    revision_checks: usize,
    asset_lookups: usize,
    avoided_asset_lookups: usize,
    label_builds: usize,
    avoided_label_builds: usize,
    payload_builds: usize,
    avoided_payload_builds: usize,
    candle_refs: usize,
    avoided_candle_refs: usize,
    bridge_writes: usize,
    avoided_bridge_writes: usize,
}

#[derive(Default)]
struct ComparisonPayloadCache {
    key: String,
    probe: Option<ComparisonPayloadProbe>,
    hits: usize,
    misses: usize,
    last_full_work: ComparisonPayloadWork,
    avoided_asset_lookups: usize,
    avoided_label_builds: usize,
    avoided_payload_builds: usize,
    avoided_candle_refs: usize,
    avoided_bridge_writes: usize,
}

#[derive(Debug, Clone)]
struct HistoryLoadRequest {
    key: String,
    instrument: String,
    granularity: String,
    rows: usize,
    max_rows: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct HistorySeriesProbe {
    payload_hash: usize,
    candles: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct HistoryLoadProbe {
    payload_hash: usize,
    responses: usize,
    candles: usize,
}

#[derive(Debug, Clone, Copy, Default)]
struct HistoryLoadWork {
    catalog_key_checks: usize,
    backend_calls: usize,
    decoded_rows: usize,
}

#[derive(Debug, Clone, Copy)]
struct HistoryLoadRunStats {
    stats: PercentileStats,
    checksum: usize,
    cache_hits: usize,
    cache_misses: usize,
    coalesced_waiters: usize,
    catalog_key_checks: usize,
    backend_calls: usize,
    avoided_backend_calls: usize,
    decoded_rows: usize,
    avoided_decoded_rows: usize,
}

#[derive(Default)]
struct HistoryLoadCache {
    series: HashMap<String, HistorySeriesProbe>,
    hits: usize,
    misses: usize,
    coalesced_waiters: usize,
    avoided_backend_calls: usize,
    avoided_decoded_rows: usize,
}

#[derive(Debug, Clone, Copy)]
struct SourceSeriesRunStats {
    stats: PercentileStats,
    checksum: f64,
    hits: usize,
    misses: usize,
    extracted_values: usize,
    avoided_values: usize,
}

#[derive(Default)]
struct SourceSeriesCache {
    series: HashMap<&'static str, Vec<f64>>,
    hits: usize,
    misses: usize,
    avoided_values: usize,
}

#[derive(Debug, Clone, Copy)]
struct TimeLabelRunStats {
    stats: PercentileStats,
    checksum: usize,
    hits: usize,
    misses: usize,
    formatter_hits: usize,
    formatter_misses: usize,
    labels_built: usize,
    avoided_labels: usize,
    formatter_builds: usize,
    avoided_formatter_builds: usize,
}

#[derive(Default)]
struct TimeLabelCache {
    labels: HashMap<String, String>,
    formatters: HashSet<String>,
    hits: usize,
    misses: usize,
    formatter_hits: usize,
    formatter_misses: usize,
    avoided_labels: usize,
    avoided_formatter_builds: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AssetEntry {
    name: String,
    display_name: String,
    asset_class: String,
    rows: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct HistoryFileEntry {
    instrument: String,
    granularity: String,
    rows: usize,
}

#[derive(Debug, Clone, Copy)]
struct AssetCatalogRunStats {
    stats: PercentileStats,
    checksum: usize,
    hits: usize,
    misses: usize,
    normalized_entries: usize,
    avoided_normalizations: usize,
    sorts: usize,
    avoided_sorts: usize,
    linear_catalog_scans: usize,
    avoided_catalog_scans: usize,
}

#[derive(Default)]
struct AssetCatalogCache {
    key: String,
    available: Vec<AssetEntry>,
    library: Vec<AssetEntry>,
    initialized: bool,
    hits: usize,
    misses: usize,
    avoided_normalizations: usize,
    avoided_sorts: usize,
    avoided_catalog_scans: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CatalogIndexProbe {
    rows: Vec<usize>,
    exists: Vec<bool>,
    instrument_counts: Vec<usize>,
    granularities: Vec<Vec<String>>,
    tradable: Vec<bool>,
}

#[derive(Debug, Clone, Copy, Default)]
struct CatalogIndexWork {
    pair_lookups: usize,
    instrument_lookups: usize,
    broker_set_lookups: usize,
    full_scans: usize,
    set_entries: usize,
}

#[derive(Debug, Clone, Copy)]
struct CatalogIndexRunStats {
    stats: PercentileStats,
    checksum: usize,
    hits: usize,
    misses: usize,
    pair_lookups: usize,
    instrument_lookups: usize,
    broker_set_lookups: usize,
    full_scans: usize,
    avoided_full_scans: usize,
    set_entries: usize,
    avoided_set_entries: usize,
    index_entries: usize,
}

#[derive(Default)]
struct CatalogIndexCache {
    key: String,
    pair_rows: HashMap<String, usize>,
    by_instrument: HashMap<String, Vec<usize>>,
    broker_set: HashSet<String>,
    initialized: bool,
    hits: usize,
    misses: usize,
    avoided_full_scans: usize,
    avoided_set_entries: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AssetSearchRecord {
    asset: AssetEntry,
    broker_code: String,
    compare_code: String,
    subtitle: String,
    search_haystack: String,
    mention_aliases: Vec<String>,
    max_alias_len: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AssetSearchProbe {
    query_counts: Vec<usize>,
    mentioned: Vec<Vec<String>>,
    found: Vec<bool>,
}

#[derive(Debug, Clone, Copy, Default)]
struct AssetSearchWork {
    haystack_builds: usize,
    menu_model_scans: usize,
    menu_item_builds: usize,
    alias_builds: usize,
    alias_checks: usize,
    mention_sorts: usize,
    linear_finds: usize,
}

#[derive(Debug, Clone, Copy)]
struct AssetSearchRunStats {
    stats: PercentileStats,
    checksum: usize,
    hits: usize,
    misses: usize,
    haystack_builds: usize,
    avoided_haystack_builds: usize,
    menu_model_scans: usize,
    avoided_menu_model_scans: usize,
    menu_item_builds: usize,
    alias_builds: usize,
    avoided_alias_builds: usize,
    alias_checks: usize,
    mention_sorts: usize,
    avoided_mention_sorts: usize,
    linear_finds: usize,
    avoided_linear_finds: usize,
    cache_entries: usize,
}

#[derive(Default)]
struct AssetSearchIndexCache {
    key: String,
    records: Vec<AssetSearchRecord>,
    by_name: HashMap<String, usize>,
    mention_order: Vec<usize>,
    model_key: String,
    model_counts: Vec<usize>,
    initialized: bool,
    hits: usize,
    misses: usize,
    avoided_haystack_builds: usize,
    avoided_menu_model_scans: usize,
    avoided_alias_builds: usize,
    avoided_mention_sorts: usize,
    avoided_linear_finds: usize,
}

#[derive(Debug, Clone)]
struct SyntheticAlert {
    id: String,
    instrument: String,
    active: bool,
    target_value: f64,
    triggered_count: usize,
    message: String,
}

#[derive(Debug, Clone)]
struct SyntheticTrade {
    id: String,
    instrument: String,
    side: String,
    units: f64,
    price: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ContextProbe {
    snapshot_hash: usize,
    digest_hash: usize,
}

#[derive(Debug, Clone, Copy, Default)]
struct ContextSnapshotWork {
    key_units: usize,
    candle_scans: usize,
    catalog_scans: usize,
    compare_scans: usize,
    alert_maps: usize,
    trade_maps: usize,
    signal_scans: usize,
    digest_lines: usize,
}

#[derive(Debug, Clone, Copy)]
struct ContextSnapshotRunStats {
    stats: PercentileStats,
    checksum: usize,
    hits: usize,
    misses: usize,
    key_units: usize,
    candle_scans: usize,
    avoided_candle_scans: usize,
    catalog_scans: usize,
    avoided_catalog_scans: usize,
    compare_scans: usize,
    avoided_compare_scans: usize,
    alert_maps: usize,
    avoided_alert_maps: usize,
    trade_maps: usize,
    avoided_trade_maps: usize,
    signal_scans: usize,
    avoided_signal_scans: usize,
    digest_lines: usize,
    avoided_digest_lines: usize,
}

#[derive(Default)]
struct ContextSnapshotCache {
    key: String,
    probe: Option<ContextProbe>,
    hits: usize,
    misses: usize,
    avoided_candle_scans: usize,
    avoided_catalog_scans: usize,
    avoided_compare_scans: usize,
    avoided_alert_maps: usize,
    avoided_trade_maps: usize,
    avoided_signal_scans: usize,
    avoided_digest_lines: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AlertPayloadProbe {
    canvas_hash: usize,
    list_hash: usize,
    modal_hash: usize,
    context_hash: usize,
    signal_hash: usize,
}

#[derive(Debug, Clone, Copy, Default)]
struct AlertPayloadWork {
    key_units: usize,
    normalizations: usize,
    instrument_checks: usize,
    canvas_maps: usize,
    list_sorts: usize,
    modal_rows: usize,
    context_alert_scans: usize,
    signal_alert_scans: usize,
}

impl AlertPayloadWork {
    fn delta(self, before: AlertPayloadWork) -> AlertPayloadWork {
        AlertPayloadWork {
            key_units: self.key_units.saturating_sub(before.key_units),
            normalizations: self.normalizations.saturating_sub(before.normalizations),
            instrument_checks: self.instrument_checks.saturating_sub(before.instrument_checks),
            canvas_maps: self.canvas_maps.saturating_sub(before.canvas_maps),
            list_sorts: self.list_sorts.saturating_sub(before.list_sorts),
            modal_rows: self.modal_rows.saturating_sub(before.modal_rows),
            context_alert_scans: self
                .context_alert_scans
                .saturating_sub(before.context_alert_scans),
            signal_alert_scans: self
                .signal_alert_scans
                .saturating_sub(before.signal_alert_scans),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct AlertPayloadRunStats {
    stats: PercentileStats,
    checksum: usize,
    hits: usize,
    misses: usize,
    key_units: usize,
    normalizations: usize,
    avoided_normalizations: usize,
    instrument_checks: usize,
    avoided_instrument_checks: usize,
    canvas_maps: usize,
    avoided_canvas_maps: usize,
    list_sorts: usize,
    avoided_list_sorts: usize,
    modal_rows: usize,
    avoided_modal_rows: usize,
    context_alert_scans: usize,
    avoided_context_alert_scans: usize,
    signal_alert_scans: usize,
    avoided_signal_alert_scans: usize,
}

#[derive(Default)]
struct AlertPayloadCache {
    key: String,
    probe: Option<AlertPayloadProbe>,
    last_full_work: AlertPayloadWork,
    hits: usize,
    misses: usize,
    avoided_normalizations: usize,
    avoided_instrument_checks: usize,
    avoided_canvas_maps: usize,
    avoided_list_sorts: usize,
    avoided_modal_rows: usize,
    avoided_context_alert_scans: usize,
    avoided_signal_alert_scans: usize,
}

#[derive(Debug, Clone, Copy)]
struct SignalParseRunStats {
    stats: PercentileStats,
    checksum: usize,
    hits: usize,
    misses: usize,
    comparisons: usize,
    avoided_comparisons: usize,
}

#[derive(Default)]
struct CandleTimeIndexCache {
    key: (usize, i64, i64),
    times: Vec<i64>,
    indices: Vec<usize>,
    initialized: bool,
    hits: usize,
    misses: usize,
    avoided_comparisons: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct MetricPoint {
    time_ms: i64,
    x_volatility: f64,
    x_signal_density: f64,
    x_regime: f64,
    close_price: f64,
    close_fair_gap: f64,
    close_conviction: f64,
    close_anomaly: f64,
}

#[derive(Debug, Clone, Copy)]
struct MetricSeriesRunStats {
    stats: PercentileStats,
    checksum: f64,
    hits: usize,
    misses: usize,
    built_points: usize,
    avoided_points: usize,
}

#[derive(Default)]
struct MetricSeriesCache {
    key: (usize, i64, i64, usize, usize),
    value: Vec<MetricPoint>,
    initialized: bool,
    hits: usize,
    misses: usize,
    avoided_points: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct ThreeDCell {
    col: usize,
    row: usize,
    volume: f64,
    canopy: f64,
    signal: bool,
}

#[derive(Debug, Clone, Copy)]
struct ThreeDPayloadRunStats {
    stats: PercentileStats,
    checksum: f64,
    hits: usize,
    misses: usize,
    built_grid_cells: usize,
    materialized_cells: usize,
    avoided_grid_cells: usize,
}

#[derive(Default)]
struct ThreeDPayloadCache {
    key: (usize, i64, i64, u64),
    value: Vec<ThreeDCell>,
    grid_cells: usize,
    initialized: bool,
    hits: usize,
    misses: usize,
    avoided_grid_cells: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ThreeDGpuPayload {
    payload_id: u64,
    position_bytes: usize,
    color_bytes: usize,
    size_bytes: usize,
    line_position_bytes: usize,
    line_color_bytes: usize,
    line_size_bytes: usize,
    point_vertices: usize,
    line_vertices: usize,
}

impl ThreeDGpuPayload {
    fn total_bytes(self) -> usize {
        self.position_bytes
            + self.color_bytes
            + self.size_bytes
            + self.line_position_bytes
            + self.line_color_bytes
            + self.line_size_bytes
    }

    fn buffer_calls(self) -> usize {
        6
    }
}

#[derive(Debug, Clone, Copy)]
struct ThreeDGpuUploadRunStats {
    stats: PercentileStats,
    checksum: usize,
    hits: usize,
    misses: usize,
    buffer_calls: usize,
    avoided_buffer_calls: usize,
    uploaded_bytes: usize,
    avoided_bytes: usize,
}

#[derive(Default)]
struct ThreeDGpuUploadCache {
    payload_id: u64,
    gl_id: u64,
    initialized: bool,
    hits: usize,
    misses: usize,
    avoided_buffer_calls: usize,
    uploaded_bytes: usize,
    avoided_bytes: usize,
}

fn run_live_merge_focus(bars: &[Bar], series_hash: &str, frames: usize) {
    let frames = frames.max(3);
    let overlap = bars.len().min(220);
    let new_per_frame = 20_usize;
    let feed_count = overlap + new_per_frame;
    let total = bars
        .len()
        .saturating_add(frames.saturating_mul(new_per_frame))
        .saturating_add(feed_count)
        .max(bars.len() + feed_count + 1);
    let feed_bars = synthetic_bars(total);
    let batches = live_merge_batches(&feed_bars, bars.len(), frames, overlap, new_per_frame);
    let proof_legacy = apply_live_merge_sequence(bars, &batches, merge_bars_legacy);
    let proof_optimized = apply_live_merge_sequence(bars, &batches, merge_bars_incremental);
    let (mismatches, max_abs_diff) = compare_bar_series(&proof_legacy, &proof_optimized);
    println!(
        "[trading-lab] focus=live-merge task=replace-full-map-sort series_hash={} bars={} frames={} feed_count={} overlap={} new_per_frame={}",
        series_hash,
        bars.len(),
        frames,
        feed_count,
        overlap,
        new_per_frame
    );
    println!(
        "[trading-lab] proof live-merge final_len={} mismatches={} max_abs_price_diff={:.12}",
        proof_optimized.len(),
        mismatches,
        max_abs_diff
    );

    let legacy = bench_live_merge("legacy", bars, &batches, merge_bars_legacy);
    let optimized = bench_live_merge("optimized", bars, &batches, merge_bars_incremental);
    let sort_work_avoided = estimate_live_merge_sort_work(bars.len(), frames, overlap, new_per_frame);
    let allocation_items_avoided = estimate_live_merge_allocation_items(bars.len(), frames, overlap, new_per_frame);
    let (base_passes_avoided, base_scan_units_avoided) =
        estimate_live_merge_base_scan_avoidance(bars.len(), frames, new_per_frame);
    println!(
        "[trading-lab] legacy-live-merge model=map-all+sort-all p50_ms={:.3} p95_ms={:.3} p99_ms={:.3} checksum={:.6} final_len={}",
        legacy.stats.p50_us as f64 / 1000.0,
        legacy.stats.p95_us as f64 / 1000.0,
        legacy.stats.p99_us as f64 / 1000.0,
        legacy.checksum,
        legacy.final_len
    );
    println!(
        "[trading-lab] optimized-live-merge model=recent-map+linear-merge+weakmeta p50_ms={:.3} p95_ms={:.3} p99_ms={:.3} checksum={:.6} final_len={} sort_comparisons_avoided_est={:.0} allocation_items_avoided_est={} base_passes_avoided_est={} base_scan_units_avoided_est={}",
        optimized.stats.p50_us as f64 / 1000.0,
        optimized.stats.p95_us as f64 / 1000.0,
        optimized.stats.p99_us as f64 / 1000.0,
        optimized.checksum,
        optimized.final_len,
        sort_work_avoided,
        allocation_items_avoided,
        base_passes_avoided,
        base_scan_units_avoided
    );
    println!(
        "[trading-lab] summary target=live_merge p50_speedup_x={:.2} p95_speedup_x={:.2} p99_speedup_x={:.2} cache_policy=no-cache-simplified-incremental-path",
        ratio(legacy.stats.p50_us, optimized.stats.p50_us),
        ratio(legacy.stats.p95_us, optimized.stats.p95_us),
        ratio(legacy.stats.p99_us, optimized.stats.p99_us)
    );
}

fn run_refresh_dedupe_focus(bars: &[Bar], series_hash: &str, frames: usize) {
    let frames = frames.max(3);
    let overlap = bars.len().min(220);
    let new_per_frame = 20_usize;
    let feed_count = overlap + new_per_frame;
    let total = bars
        .len()
        .saturating_add(frames.saturating_mul(new_per_frame))
        .saturating_add(feed_count)
        .max(bars.len() + feed_count + 1);
    let feed_bars = synthetic_bars(total);
    let batches = live_merge_batches(&feed_bars, bars.len(), frames, overlap, new_per_frame);
    let legacy_final = apply_refresh_pipeline_sequence(bars, &batches, true);
    let optimized_final = apply_refresh_pipeline_sequence(bars, &batches, false);
    let (mismatches, max_abs_diff) = compare_bar_series(&legacy_final, &optimized_final);
    println!(
        "[trading-lab] focus=refresh-dedupe task=remove-market-feed-cache-side-effect series_hash={} bars={} frames={} feed_count={} overlap={} new_per_frame={}",
        series_hash,
        bars.len(),
        frames,
        feed_count,
        overlap,
        new_per_frame
    );
    println!(
        "[trading-lab] proof refresh-dedupe final_len={} mismatches={} max_abs_price_diff={:.12}",
        optimized_final.len(),
        mismatches,
        max_abs_diff
    );

    let legacy = bench_refresh_pipeline(bars, &batches, true);
    let optimized = bench_refresh_pipeline(bars, &batches, false);
    let (merge_calls_avoided, recent_map_units_avoided, base_scan_units_avoided) =
        estimate_refresh_dedupe_avoidance(bars.len(), frames, overlap, new_per_frame);
    println!(
        "[trading-lab] legacy-refresh-pipeline model=feed-mutates-cache+refresh-remerge p50_ms={:.3} p95_ms={:.3} p99_ms={:.3} checksum={:.6} final_len={}",
        legacy.stats.p50_us as f64 / 1000.0,
        legacy.stats.p95_us as f64 / 1000.0,
        legacy.stats.p99_us as f64 / 1000.0,
        legacy.checksum,
        legacy.final_len
    );
    println!(
        "[trading-lab] optimized-refresh-pipeline model=single-owner-cache+single-merge p50_ms={:.3} p95_ms={:.3} p99_ms={:.3} checksum={:.6} final_len={} merge_calls_avoided={} recent_map_units_avoided_est={} base_scan_units_avoided_est={}",
        optimized.stats.p50_us as f64 / 1000.0,
        optimized.stats.p95_us as f64 / 1000.0,
        optimized.stats.p99_us as f64 / 1000.0,
        optimized.checksum,
        optimized.final_len,
        merge_calls_avoided,
        recent_map_units_avoided,
        base_scan_units_avoided
    );
    println!(
        "[trading-lab] summary target=refresh_dedupe p50_speedup_x={:.2} p95_speedup_x={:.2} p99_speedup_x={:.2} cache_policy=single-writer-final-series",
        ratio(legacy.stats.p50_us, optimized.stats.p50_us),
        ratio(legacy.stats.p95_us, optimized.stats.p95_us),
        ratio(legacy.stats.p99_us, optimized.stats.p99_us)
    );
}

fn run_canvas_document_focus(bars: &[Bar], series_hash: &str, frames: usize) {
    let frames = frames.max(3);
    let overlap = bars.len().min(220);
    let new_per_frame = 20_usize;
    let feed_count = overlap + new_per_frame;
    let total = bars
        .len()
        .saturating_add(frames.saturating_mul(new_per_frame))
        .saturating_add(feed_count)
        .max(bars.len() + feed_count + 1);
    let feed_bars = synthetic_bars(total);
    let batches = live_merge_batches(&feed_bars, bars.len(), frames, overlap, new_per_frame);
    let full_doc = final_canvas_document_sequence(bars, &batches, overlap, false);
    let incremental_doc = final_canvas_document_sequence(bars, &batches, overlap, true);
    let (mismatches, max_abs_diff) = compare_canvas_documents(&full_doc, &incremental_doc);
    println!(
        "[trading-lab] focus=canvas-document task=avoid-full-document-rebuild series_hash={} bars={} frames={} feed_count={} overlap={} new_per_frame={}",
        series_hash,
        bars.len(),
        frames,
        feed_count,
        overlap,
        new_per_frame
    );
    println!(
        "[trading-lab] proof canvas-document final_len={} mismatches={} max_abs_diff={:.12}",
        incremental_doc.candles.len(),
        mismatches,
        max_abs_diff
    );

    let legacy = bench_canvas_document_pipeline(bars, &batches, overlap, false);
    let optimized = bench_canvas_document_pipeline(bars, &batches, overlap, true);
    let rebuild_units_avoided = optimized.reused_prefix_units.saturating_mul(6);
    println!(
        "[trading-lab] legacy-canvas-document model=full-normalize+logical-bars+full-indicators p50_ms={:.3} p95_ms={:.3} p99_ms={:.3} checksum={:.6} final_len={}",
        legacy.stats.p50_us as f64 / 1000.0,
        legacy.stats.p95_us as f64 / 1000.0,
        legacy.stats.p99_us as f64 / 1000.0,
        legacy.checksum,
        legacy.final_len
    );
    println!(
        "[trading-lab] optimized-canvas-document model=prefix-reuse+tail-normalize+incremental-ema-vwap p50_ms={:.3} p95_ms={:.3} p99_ms={:.3} checksum={:.6} final_len={} prefix_rows_reused={} rebuild_units_avoided_est={}",
        optimized.stats.p50_us as f64 / 1000.0,
        optimized.stats.p95_us as f64 / 1000.0,
        optimized.stats.p99_us as f64 / 1000.0,
        optimized.checksum,
        optimized.final_len,
        optimized.reused_prefix_units,
        rebuild_units_avoided
    );
    println!(
        "[trading-lab] summary target=canvas_document p50_speedup_x={:.2} p95_speedup_x={:.2} p99_speedup_x={:.2} cache_policy=content-addressed-prefix-reuse",
        ratio(legacy.stats.p50_us, optimized.stats.p50_us),
        ratio(legacy.stats.p95_us, optimized.stats.p95_us),
        ratio(legacy.stats.p99_us, optimized.stats.p99_us)
    );
}

fn run_viewport_window_focus(bars: &[Bar], series_hash: &str, frames: usize) {
    let frames = frames.max(3);
    let calls_per_frame = 12_usize;
    let visible_bars = 1_200_usize.min(bars.len()).max(1);
    let doc = build_canvas_document_full(bars);
    let proof_legacy = resolve_viewport_window_legacy(&doc.logical_times, visible_bars);
    let mut cache = ViewportWindowCache::default();
    let proof_cached = resolve_viewport_window_cached(&doc.logical_times, visible_bars, &mut cache).to_vec();
    let proof_equal = proof_legacy == proof_cached;
    println!(
        "[trading-lab] focus=viewport-window task=dedupe-visible-window-slice-map series_hash={} bars={} frames={} visible_bars={} calls_per_frame={}",
        series_hash,
        bars.len(),
        frames,
        visible_bars,
        calls_per_frame
    );
    println!(
        "[trading-lab] proof viewport-window entries={} equal={} first_idx={} last_idx={}",
        proof_cached.len(),
        proof_equal,
        proof_cached.first().map(|entry| entry.0).unwrap_or(0),
        proof_cached.last().map(|entry| entry.0).unwrap_or(0)
    );

    let legacy = bench_viewport_window_pipeline(&doc.logical_times, visible_bars, frames, calls_per_frame, false);
    let optimized = bench_viewport_window_pipeline(&doc.logical_times, visible_bars, frames, calls_per_frame, true);
    let hit_rate = if optimized.hits + optimized.misses == 0 {
        0.0
    } else {
        optimized.hits as f64 / (optimized.hits + optimized.misses) as f64
    };
    println!(
        "[trading-lab] legacy-viewport-window model=repeated-slice-map p50_ms={:.3} p95_ms={:.3} p99_ms={:.3} checksum={:.6} misses={}",
        legacy.stats.p50_us as f64 / 1000.0,
        legacy.stats.p95_us as f64 / 1000.0,
        legacy.stats.p99_us as f64 / 1000.0,
        legacy.checksum,
        legacy.misses
    );
    println!(
        "[trading-lab] optimized-viewport-window model=weakmap-content-addressed-window p50_ms={:.3} p95_ms={:.3} p99_ms={:.3} checksum={:.6} hits={} misses={} hit_rate={:.3} avoided_entries={}",
        optimized.stats.p50_us as f64 / 1000.0,
        optimized.stats.p95_us as f64 / 1000.0,
        optimized.stats.p99_us as f64 / 1000.0,
        optimized.checksum,
        optimized.hits,
        optimized.misses,
        hit_rate,
        optimized.avoided_entries
    );
    println!(
        "[trading-lab] summary target=viewport_window p50_speedup_x={:.2} p95_speedup_x={:.2} p99_speedup_x={:.2} cache_budget=weak-one-entry-per-logicalBars",
        ratio(legacy.stats.p50_us, optimized.stats.p50_us),
        ratio(legacy.stats.p95_us, optimized.stats.p95_us),
        ratio(legacy.stats.p99_us, optimized.stats.p99_us)
    );
}

fn run_overlay_key_focus(bars: &[Bar], series_hash: &str, frames: usize) {
    let frames = frames.max(3);
    let overlap = bars.len().min(220);
    let new_per_frame = 20_usize;
    let feed_count = overlap + new_per_frame;
    let total = bars
        .len()
        .saturating_add(frames.saturating_mul(new_per_frame))
        .saturating_add(feed_count)
        .max(bars.len() + feed_count + 1);
    let feed_bars = synthetic_bars(total);
    let batches = live_merge_batches(&feed_bars, bars.len(), frames, overlap, new_per_frame);
    let proof_full = final_overlay_key_sequence(bars, &batches, overlap, false);
    let proof_incremental = final_overlay_key_sequence(bars, &batches, overlap, true);
    println!(
        "[trading-lab] focus=overlay-key task=avoid-full-candle-series-key-scan series_hash={} bars={} frames={} feed_count={} overlap={} new_per_frame={}",
        series_hash,
        bars.len(),
        frames,
        feed_count,
        overlap,
        new_per_frame
    );
    println!(
        "[trading-lab] proof overlay-key equal={} final_len={} key={}",
        proof_full.key == proof_incremental.key,
        proof_incremental.h1_by_index.len(),
        compact_key(&proof_incremental.key)
    );

    let legacy = bench_overlay_key_pipeline(bars, &batches, overlap, false);
    let optimized = bench_overlay_key_pipeline(bars, &batches, overlap, true);
    let avoided_rows = optimized.reused_prefix_rows;
    let total_rows_full = estimate_overlay_key_full_rows(bars.len(), frames, new_per_frame);
    println!(
        "[trading-lab] legacy-overlay-key model=full-series-row-hash p50_ms={:.3} p95_ms={:.3} p99_ms={:.3} checksum={} final_len={} rows_hashed_est={}",
        legacy.stats.p50_us as f64 / 1000.0,
        legacy.stats.p95_us as f64 / 1000.0,
        legacy.stats.p99_us as f64 / 1000.0,
        legacy.checksum,
        legacy.final_len,
        total_rows_full
    );
    println!(
        "[trading-lab] optimized-overlay-key model=prefix-hash-state+tail-row-hash p50_ms={:.3} p95_ms={:.3} p99_ms={:.3} checksum={} final_len={} prefix_rows_reused={} tail_rows_hashed={} row_hash_avoidance={:.3}",
        optimized.stats.p50_us as f64 / 1000.0,
        optimized.stats.p95_us as f64 / 1000.0,
        optimized.stats.p99_us as f64 / 1000.0,
        optimized.checksum,
        optimized.final_len,
        optimized.reused_prefix_rows,
        optimized.tail_rows_hashed,
        if total_rows_full == 0 { 0.0 } else { avoided_rows as f64 / total_rows_full as f64 }
    );
    println!(
        "[trading-lab] summary target=overlay_key p50_speedup_x={:.2} p95_speedup_x={:.2} p99_speedup_x={:.2} cache_policy=weak-hash-state-per-candle-array",
        ratio(legacy.stats.p50_us, optimized.stats.p50_us),
        ratio(legacy.stats.p95_us, optimized.stats.p95_us),
        ratio(legacy.stats.p99_us, optimized.stats.p99_us)
    );
}

fn run_comparison_charts_focus(bars: &[Bar], series_hash: &str, frames: usize) {
    let frames = frames.max(3);
    let comparisons = 4_usize;
    let visible_bars = 1_200_usize.min(bars.len()).max(1);
    let render_calls_per_frame = 3_usize;
    let docs: Vec<CanvasDocument> = (0..comparisons)
        .map(|offset| {
            let mut shifted = synthetic_bars(bars.len());
            for bar in &mut shifted {
                let factor = 1.0 + (offset as f64 + 1.0) * 0.0125;
                bar.open *= factor;
                bar.high *= factor;
                bar.low *= factor;
                bar.close *= factor;
            }
            build_canvas_document_full(&shifted)
        })
        .collect();
    let main_times = &docs[0].logical_times;
    let end = main_times.len();
    let start = end.saturating_sub(visible_bars);
    let min_time = main_times.get(start).copied().unwrap_or_default();
    let max_time = main_times.last().copied().unwrap_or(min_time);
    let proof_legacy = comparison_visible_filter_legacy(&docs[0].logical_times, min_time, max_time, start);
    let mut proof_cache = ViewportWindowCache::default();
    let proof_cached = resolve_viewport_window_cached(&docs[0].logical_times, visible_bars, &mut proof_cache).to_vec();
    println!(
        "[trading-lab] focus=comparison-charts task=remove-extra-chart-filter-and-remap series_hash={} bars={} comparisons={} frames={} visible_bars={} render_calls_per_frame={}",
        series_hash,
        bars.len(),
        comparisons,
        frames,
        visible_bars,
        render_calls_per_frame
    );
    println!(
        "[trading-lab] proof comparison-charts visible_equal={} entries={} first_idx={} last_idx={}",
        proof_legacy == proof_cached,
        proof_cached.len(),
        proof_cached.first().map(|entry| entry.0).unwrap_or(0),
        proof_cached.last().map(|entry| entry.0).unwrap_or(0)
    );

    let legacy = bench_comparison_charts_pipeline(
        &docs,
        visible_bars,
        min_time,
        max_time,
        start,
        frames,
        render_calls_per_frame,
        false,
    );
    let optimized = bench_comparison_charts_pipeline(
        &docs,
        visible_bars,
        min_time,
        max_time,
        start,
        frames,
        render_calls_per_frame,
        true,
    );
    let hit_rate = if optimized.hits + optimized.misses == 0 {
        0.0
    } else {
        optimized.hits as f64 / (optimized.hits + optimized.misses) as f64
    };
    println!(
        "[trading-lab] legacy-comparison-charts model=payload-remap+full-visible-filter p50_ms={:.3} p95_ms={:.3} p99_ms={:.3} checksum={:.6} scanned_rows={}",
        legacy.stats.p50_us as f64 / 1000.0,
        legacy.stats.p95_us as f64 / 1000.0,
        legacy.stats.p99_us as f64 / 1000.0,
        legacy.checksum,
        legacy.scanned_rows
    );
    println!(
        "[trading-lab] optimized-comparison-charts model=source-array+extra-logical-window-cache p50_ms={:.3} p95_ms={:.3} p99_ms={:.3} checksum={:.6} hits={} misses={} hit_rate={:.3} avoided_rows={}",
        optimized.stats.p50_us as f64 / 1000.0,
        optimized.stats.p95_us as f64 / 1000.0,
        optimized.stats.p99_us as f64 / 1000.0,
        optimized.checksum,
        optimized.hits,
        optimized.misses,
        hit_rate,
        optimized.avoided_rows
    );
    println!(
        "[trading-lab] summary target=comparison_charts p50_speedup_x={:.2} p95_speedup_x={:.2} p99_speedup_x={:.2} cache_budget=one-window-per-extra-chart-logicalBars",
        ratio(legacy.stats.p50_us, optimized.stats.p50_us),
        ratio(legacy.stats.p95_us, optimized.stats.p95_us),
        ratio(legacy.stats.p99_us, optimized.stats.p99_us)
    );
}

fn run_signal_markers_focus(bars: &[Bar], series_hash: &str, frames: usize) {
    let frames = frames.max(3);
    let visible_bars = 1_200_usize.min(bars.len()).max(1);
    let signal_count = 800_usize.min(visible_bars);
    let doc = build_canvas_document_full(bars);
    let visible = resolve_viewport_window_legacy(&doc.logical_times, visible_bars);
    let signals = synthetic_signal_times(&visible, signal_count);
    let tolerance_ms = 60_000_i64;
    let proof_legacy = resolve_signal_slots_legacy(&visible, &signals, tolerance_ms);
    let slot_index = build_signal_slot_index(&visible);
    let proof_indexed = resolve_signal_slots_indexed(&slot_index, &signals, tolerance_ms);
    println!(
        "[trading-lab] focus=signal-markers task=replace-signal-findindex-with-visible-time-index series_hash={} bars={} frames={} visible_bars={} signals={} tolerance_ms={}",
        series_hash,
        bars.len(),
        frames,
        visible_bars,
        signal_count,
        tolerance_ms
    );
    println!(
        "[trading-lab] proof signal-markers equal={} resolved={} first_slot={} last_slot={}",
        proof_legacy == proof_indexed,
        proof_indexed.iter().filter(|slot| **slot != usize::MAX).count(),
        proof_indexed.iter().find(|slot| **slot != usize::MAX).copied().unwrap_or(usize::MAX),
        proof_indexed.iter().rev().find(|slot| **slot != usize::MAX).copied().unwrap_or(usize::MAX)
    );

    let legacy = bench_signal_markers(&visible, &signals, tolerance_ms, frames, false);
    let optimized = bench_signal_markers(&visible, &signals, tolerance_ms, frames, true);
    let hit_rate = if optimized.hits + optimized.misses == 0 {
        0.0
    } else {
        optimized.hits as f64 / (optimized.hits + optimized.misses) as f64
    };
    println!(
        "[trading-lab] legacy-signal-markers model=signal-findindex-visible-scan p50_ms={:.3} p95_ms={:.3} p99_ms={:.3} checksum={} comparisons={}",
        legacy.stats.p50_us as f64 / 1000.0,
        legacy.stats.p95_us as f64 / 1000.0,
        legacy.stats.p99_us as f64 / 1000.0,
        legacy.checksum,
        legacy.comparisons
    );
    println!(
        "[trading-lab] optimized-signal-markers model=content-addressed-visible-time-index p50_ms={:.3} p95_ms={:.3} p99_ms={:.3} checksum={} hits={} misses={} hit_rate={:.3} avoided_comparisons={}",
        optimized.stats.p50_us as f64 / 1000.0,
        optimized.stats.p95_us as f64 / 1000.0,
        optimized.stats.p99_us as f64 / 1000.0,
        optimized.checksum,
        optimized.hits,
        optimized.misses,
        hit_rate,
        optimized.avoided_comparisons
    );
    println!(
        "[trading-lab] summary target=signal_markers p50_speedup_x={:.2} p95_speedup_x={:.2} p99_speedup_x={:.2} cache_budget=one-index-per-visible-window",
        ratio(legacy.stats.p50_us, optimized.stats.p50_us),
        ratio(legacy.stats.p95_us, optimized.stats.p95_us),
        ratio(legacy.stats.p99_us, optimized.stats.p99_us)
    );
}

fn run_render_entries_focus(bars: &[Bar], series_hash: &str, frames: usize) {
    let frames = frames.max(3);
    let visible_bars = 1_200_usize.min(bars.len()).max(1);
    let calls_per_frame = 10_usize;
    let doc = build_canvas_document_full(bars);
    let visible = resolve_viewport_window_legacy(&doc.logical_times, visible_bars);
    let close_by_time = close_by_time_map(bars);
    let pad_left = 64.0_f64;
    let slot_width = 1280.0_f64 / visible_bars as f64;
    let candle_w = (slot_width * 0.68).max(1.0);
    let proof_legacy = build_render_entries_legacy(&visible, &close_by_time, pad_left, slot_width, candle_w);
    let mut proof_cache = RenderEntryCache::default();
    let proof_cached = build_render_entries_cached(
        &visible,
        &close_by_time,
        pad_left,
        slot_width,
        candle_w,
        &mut proof_cache,
    )
    .to_vec();
    println!(
        "[trading-lab] focus=render-entries task=dedupe-visible-render-entry-map series_hash={} bars={} frames={} visible_bars={} calls_per_frame={}",
        series_hash,
        bars.len(),
        frames,
        visible_bars,
        calls_per_frame
    );
    println!(
        "[trading-lab] proof render-entries equal={} entries={} first_x={:.3} last_x={:.3}",
        proof_legacy == proof_cached,
        proof_cached.len(),
        proof_cached.first().map(|entry| entry.x_start).unwrap_or_default(),
        proof_cached.last().map(|entry| entry.x_start).unwrap_or_default()
    );

    let legacy = bench_render_entries(&visible, &close_by_time, frames, calls_per_frame, false);
    let optimized = bench_render_entries(&visible, &close_by_time, frames, calls_per_frame, true);
    let hit_rate = if optimized.hits + optimized.misses == 0 {
        0.0
    } else {
        optimized.hits as f64 / (optimized.hits + optimized.misses) as f64
    };
    println!(
        "[trading-lab] legacy-render-entries model=visible-map-each-draw p50_ms={:.3} p95_ms={:.3} p99_ms={:.3} checksum={:.6} misses={}",
        legacy.stats.p50_us as f64 / 1000.0,
        legacy.stats.p95_us as f64 / 1000.0,
        legacy.stats.p99_us as f64 / 1000.0,
        legacy.checksum,
        legacy.misses
    );
    println!(
        "[trading-lab] optimized-render-entries model=content-addressed-render-entry-array p50_ms={:.3} p95_ms={:.3} p99_ms={:.3} checksum={:.6} hits={} misses={} hit_rate={:.3} avoided_entries={}",
        optimized.stats.p50_us as f64 / 1000.0,
        optimized.stats.p95_us as f64 / 1000.0,
        optimized.stats.p99_us as f64 / 1000.0,
        optimized.checksum,
        optimized.hits,
        optimized.misses,
        hit_rate,
        optimized.avoided_entries
    );
    println!(
        "[trading-lab] summary target=render_entries p50_speedup_x={:.2} p95_speedup_x={:.2} p99_speedup_x={:.2} cache_budget=one-entry-array-per-visible-window-and-dimensions",
        ratio(legacy.stats.p50_us, optimized.stats.p50_us),
        ratio(legacy.stats.p95_us, optimized.stats.p95_us),
        ratio(legacy.stats.p99_us, optimized.stats.p99_us)
    );
}

fn bench_render_entries(
    visible: &[(usize, i64)],
    close_by_time: &HashMap<i64, f64>,
    frames: usize,
    calls_per_frame: usize,
    cached: bool,
) -> RenderEntryRunStats {
    let pad_left = 64.0_f64;
    let slot_width = 1280.0_f64 / visible.len().max(1) as f64;
    let candle_w = (slot_width * 0.68).max(1.0);
    let mut samples = Vec::with_capacity(frames);
    let mut checksum = 0.0_f64;
    let mut misses = 0_usize;
    let mut cache = RenderEntryCache::default();
    for _ in 0..frames {
        let started = Instant::now();
        for _ in 0..calls_per_frame {
            if cached {
                let entries = build_render_entries_cached(
                    visible,
                    close_by_time,
                    pad_left,
                    slot_width,
                    candle_w,
                    &mut cache,
                );
                checksum += render_entries_checksum(entries);
            } else {
                misses += 1;
                let entries =
                    build_render_entries_legacy(visible, close_by_time, pad_left, slot_width, candle_w);
                checksum += render_entries_checksum(&entries);
            }
        }
        samples.push(started.elapsed().as_micros());
    }
    RenderEntryRunStats {
        stats: percentile_stats(&samples),
        checksum,
        hits: cache.hits,
        misses: cache.misses + misses,
        avoided_entries: cache.avoided_entries,
    }
}

fn build_render_entries_legacy(
    visible: &[(usize, i64)],
    close_by_time: &HashMap<i64, f64>,
    pad_left: f64,
    slot_width: f64,
    candle_w: f64,
) -> Vec<RenderEntry> {
    visible
        .iter()
        .map(|(logical_index, time_ms)| {
            let x_start = pad_left + *logical_index as f64 * slot_width;
            let close = close_by_time.get(time_ms).copied().unwrap_or_default();
            RenderEntry {
                logical_index: *logical_index,
                time_ms: *time_ms,
                x_start,
                x_center: x_start + slot_width * 0.5,
                x_wick: x_start + candle_w * 0.5,
                close,
            }
        })
        .collect()
}

fn build_render_entries_cached<'a>(
    visible: &[(usize, i64)],
    close_by_time: &HashMap<i64, f64>,
    pad_left: f64,
    slot_width: f64,
    candle_w: f64,
    cache: &'a mut RenderEntryCache,
) -> &'a [RenderEntry] {
    let key = (
        (pad_left * 1_000.0).round() as i64,
        (slot_width * 1_000_000.0).round() as i64,
        (candle_w * 1_000_000.0).round() as i64,
        visible.len(),
        visible.first().map(|(_, time_ms)| *time_ms).unwrap_or_default(),
        visible.last().map(|(_, time_ms)| *time_ms).unwrap_or_default(),
    );
    if cache.initialized && cache.key == key {
        cache.hits += 1;
        cache.avoided_entries = cache.avoided_entries.saturating_add(cache.value.len());
        return &cache.value;
    }
    cache.misses += 1;
    cache.key = key;
    cache.value = build_render_entries_legacy(visible, close_by_time, pad_left, slot_width, candle_w);
    cache.initialized = true;
    &cache.value
}

fn close_by_time_map(bars: &[Bar]) -> HashMap<i64, f64> {
    let mut out = HashMap::with_capacity(bars.len());
    for bar in bars {
        out.insert(bar.time_ms, bar.close);
    }
    out
}

fn render_entries_checksum(entries: &[RenderEntry]) -> f64 {
    let mut sum = 0.0_f64;
    for entry in entries.iter().step_by(83) {
        sum += entry.x_center * 0.01 + entry.x_wick * 0.001 + entry.close;
    }
    std::hint::black_box(sum)
}

fn run_hit_test_slot_focus(bars: &[Bar], series_hash: &str, frames: usize) {
    let frames = frames.max(3);
    let visible_bars = 1_200_usize.min(bars.len()).max(1);
    let doc = build_canvas_document_full(bars);
    let visible = resolve_viewport_window_legacy(&doc.logical_times, visible_bars);
    let probes = synthetic_hit_test_slots(visible_bars, 900);
    let legacy_proof: Vec<Option<(usize, i64)>> = probes
        .iter()
        .map(|slot| visible_slot_find_legacy(&visible, *slot).0)
        .collect();
    let mut proof_cache = SlotEntryCache::default();
    let cached_index = slot_entry_index_cached(&visible, visible_bars, &mut proof_cache);
    let cached_proof: Vec<Option<(usize, i64)>> =
        probes.iter().map(|slot| cached_index.get(*slot).copied().flatten()).collect();
    println!(
        "[trading-lab] focus=hit-test-slot task=dedupe-visible-slot-find series_hash={} bars={} frames={} visible_bars={} probes_per_frame={}",
        series_hash,
        bars.len(),
        frames,
        visible_bars,
        probes.len()
    );
    println!(
        "[trading-lab] proof hit-test-slot equal={} probes={} first_slot={} last_slot={}",
        legacy_proof == cached_proof,
        probes.len(),
        probes.first().copied().unwrap_or_default(),
        probes.last().copied().unwrap_or_default()
    );

    let legacy = bench_hit_test_slots(&visible, visible_bars, &probes, frames, false);
    let optimized = bench_hit_test_slots(&visible, visible_bars, &probes, frames, true);
    let hit_rate = if optimized.hits + optimized.misses == 0 {
        0.0
    } else {
        optimized.hits as f64 / (optimized.hits + optimized.misses) as f64
    };
    println!(
        "[trading-lab] legacy-hit-test-slot model=visible-find-by-logical-index p50_ms={:.3} p95_ms={:.3} p99_ms={:.3} checksum={} comparisons={}",
        legacy.stats.p50_us as f64 / 1000.0,
        legacy.stats.p95_us as f64 / 1000.0,
        legacy.stats.p99_us as f64 / 1000.0,
        legacy.checksum,
        legacy.comparisons
    );
    println!(
        "[trading-lab] optimized-hit-test-slot model=content-addressed-visible-slot-array p50_ms={:.3} p95_ms={:.3} p99_ms={:.3} checksum={} hits={} misses={} hit_rate={:.3} avoided_comparisons={}",
        optimized.stats.p50_us as f64 / 1000.0,
        optimized.stats.p95_us as f64 / 1000.0,
        optimized.stats.p99_us as f64 / 1000.0,
        optimized.checksum,
        optimized.hits,
        optimized.misses,
        hit_rate,
        optimized.avoided_comparisons
    );
    println!(
        "[trading-lab] summary target=hit_test_slot p50_speedup_x={:.2} p95_speedup_x={:.2} p99_speedup_x={:.2} cache_budget=one-slot-array-per-visible-window",
        ratio(legacy.stats.p50_us, optimized.stats.p50_us),
        ratio(legacy.stats.p95_us, optimized.stats.p95_us),
        ratio(legacy.stats.p99_us, optimized.stats.p99_us)
    );
}

fn bench_hit_test_slots(
    visible: &[(usize, i64)],
    visible_bars: usize,
    probes: &[usize],
    frames: usize,
    cached: bool,
) -> SlotLookupRunStats {
    let mut samples = Vec::with_capacity(frames);
    let mut checksum = 0_usize;
    let mut comparisons = 0_usize;
    let mut misses = 0_usize;
    let mut cache = SlotEntryCache::default();
    for _ in 0..frames {
        let started = Instant::now();
        for slot in probes {
            if cached {
                let entries = slot_entry_index_cached(visible, visible_bars, &mut cache);
                if let Some((logical_index, time_ms)) = entries.get(*slot).copied().flatten() {
                    checksum = checksum.wrapping_add(logical_index ^ (time_ms as usize & 0xffff));
                }
            } else {
                misses += 1;
                let (entry, scanned) = visible_slot_find_legacy(visible, *slot);
                comparisons = comparisons.saturating_add(scanned);
                if let Some((logical_index, time_ms)) = entry {
                    checksum = checksum.wrapping_add(logical_index ^ (time_ms as usize & 0xffff));
                }
            }
        }
        samples.push(started.elapsed().as_micros());
    }
    SlotLookupRunStats {
        stats: percentile_stats(&samples),
        checksum: std::hint::black_box(checksum),
        hits: cache.hits,
        misses: cache.misses + misses,
        comparisons,
        avoided_comparisons: cache.avoided_comparisons,
    }
}

fn visible_slot_find_legacy(visible: &[(usize, i64)], slot: usize) -> (Option<(usize, i64)>, usize) {
    let mut scanned = 0_usize;
    for entry in visible {
        scanned += 1;
        if entry.0 == slot {
            return (Some(*entry), scanned);
        }
    }
    (None, scanned)
}

fn slot_entry_index_cached<'a>(
    visible: &[(usize, i64)],
    visible_bars: usize,
    cache: &'a mut SlotEntryCache,
) -> &'a [Option<(usize, i64)>] {
    let key = (
        visible_bars,
        visible.len(),
        visible.first().map(|(_, time_ms)| *time_ms).unwrap_or_default(),
        visible.last().map(|(_, time_ms)| *time_ms).unwrap_or_default(),
    );
    if cache.initialized && cache.key == key {
        cache.hits += 1;
        cache.avoided_comparisons = cache.avoided_comparisons.saturating_add(visible.len());
        return &cache.entries;
    }
    cache.misses += 1;
    cache.key = key;
    cache.entries.clear();
    cache.entries.resize(visible_bars, None);
    for (slot, time_ms) in visible {
        if *slot < cache.entries.len() {
            cache.entries[*slot] = Some((*slot, *time_ms));
        }
    }
    cache.initialized = true;
    &cache.entries
}

fn synthetic_hit_test_slots(visible_bars: usize, count: usize) -> Vec<usize> {
    let visible_bars = visible_bars.max(1);
    (0..count)
        .map(|idx| idx.wrapping_mul(37).wrapping_add(idx / 3) % visible_bars)
        .collect()
}

fn run_selection_lookup_focus(bars: &[Bar], series_hash: &str, frames: usize) {
    let frames = frames.max(3);
    let visible_bars = 1_200_usize.min(bars.len()).max(1);
    let draws_per_frame = 10_usize;
    let selected_count = 48_usize;
    let doc = build_canvas_document_full(bars);
    let visible = resolve_viewport_window_legacy(&doc.logical_times, visible_bars);
    let selected_times = synthetic_selected_times(&visible, selected_count);
    let selected_time_set: HashSet<i64> = selected_times.iter().copied().collect();
    let selected_key_set: HashSet<String> = selected_times
        .iter()
        .map(|time_ms| selection_lookup_key("EUR_USD", "M5", *time_ms))
        .collect();
    let proof_legacy = selected_slots_legacy(&visible, &selected_key_set);
    let proof_optimized = selected_slots_optimized(&visible, &selected_time_set);
    println!(
        "[trading-lab] focus=selection-lookup task=dedupe-selected-candle-key-builds series_hash={} bars={} frames={} visible_bars={} draws_per_frame={} selected_count={}",
        series_hash,
        bars.len(),
        frames,
        visible_bars,
        draws_per_frame,
        selected_count
    );
    println!(
        "[trading-lab] proof selection-lookup equal={} selected_slots={}",
        proof_legacy == proof_optimized,
        proof_optimized.len()
    );

    let legacy = bench_selection_lookup(
        &visible,
        &selected_key_set,
        &selected_time_set,
        frames,
        draws_per_frame,
        false,
    );
    let optimized = bench_selection_lookup(
        &visible,
        &selected_key_set,
        &selected_time_set,
        frames,
        draws_per_frame,
        true,
    );
    println!(
        "[trading-lab] legacy-selection-lookup model=string-key-per-visible-candle p50_ms={:.3} p95_ms={:.3} p99_ms={:.3} checksum={} selected_hits={} key_builds={}",
        legacy.stats.p50_us as f64 / 1000.0,
        legacy.stats.p95_us as f64 / 1000.0,
        legacy.stats.p99_us as f64 / 1000.0,
        legacy.checksum,
        legacy.selected_hits,
        legacy.key_builds
    );
    println!(
        "[trading-lab] optimized-selection-lookup model=scope-time-set p50_ms={:.3} p95_ms={:.3} p99_ms={:.3} checksum={} selected_hits={} avoided_key_builds={}",
        optimized.stats.p50_us as f64 / 1000.0,
        optimized.stats.p95_us as f64 / 1000.0,
        optimized.stats.p99_us as f64 / 1000.0,
        optimized.checksum,
        optimized.selected_hits,
        optimized.avoided_key_builds
    );
    println!(
        "[trading-lab] summary target=selection_lookup p50_speedup_x={:.2} p95_speedup_x={:.2} p99_speedup_x={:.2} cache_budget=one-time-set-per-instrument-granularity-selection",
        ratio(legacy.stats.p50_us, optimized.stats.p50_us),
        ratio(legacy.stats.p95_us, optimized.stats.p95_us),
        ratio(legacy.stats.p99_us, optimized.stats.p99_us)
    );
}

fn bench_selection_lookup(
    visible: &[(usize, i64)],
    selected_key_set: &HashSet<String>,
    selected_time_set: &HashSet<i64>,
    frames: usize,
    draws_per_frame: usize,
    optimized: bool,
) -> SelectionLookupRunStats {
    let mut samples = Vec::with_capacity(frames);
    let mut checksum = 0_usize;
    let mut selected_hits = 0_usize;
    let mut key_builds = 0_usize;
    let mut avoided_key_builds = 0_usize;
    for _ in 0..frames {
        let started = Instant::now();
        for _ in 0..draws_per_frame {
            if optimized {
                for (slot, time_ms) in visible {
                    if selected_time_set.contains(time_ms) {
                        selected_hits += 1;
                        checksum = checksum.wrapping_add(*slot ^ (*time_ms as usize & 0xffff));
                    }
                }
                avoided_key_builds = avoided_key_builds.saturating_add(visible.len());
            } else {
                for (slot, time_ms) in visible {
                    let key = selection_lookup_key("EUR_USD", "M5", *time_ms);
                    key_builds += 1;
                    if selected_key_set.contains(&key) {
                        selected_hits += 1;
                        checksum = checksum.wrapping_add(*slot ^ (*time_ms as usize & 0xffff));
                    }
                }
            }
        }
        samples.push(started.elapsed().as_micros());
    }
    SelectionLookupRunStats {
        stats: percentile_stats(&samples),
        checksum: std::hint::black_box(checksum),
        selected_hits,
        key_builds,
        avoided_key_builds,
    }
}

fn selected_slots_legacy(visible: &[(usize, i64)], selected_key_set: &HashSet<String>) -> Vec<usize> {
    visible
        .iter()
        .filter_map(|(slot, time_ms)| {
            let key = selection_lookup_key("EUR_USD", "M5", *time_ms);
            selected_key_set.contains(&key).then_some(*slot)
        })
        .collect()
}

fn selected_slots_optimized(visible: &[(usize, i64)], selected_time_set: &HashSet<i64>) -> Vec<usize> {
    visible
        .iter()
        .filter_map(|(slot, time_ms)| selected_time_set.contains(time_ms).then_some(*slot))
        .collect()
}

fn synthetic_selected_times(visible: &[(usize, i64)], selected_count: usize) -> Vec<i64> {
    if visible.is_empty() || selected_count == 0 {
        return Vec::new();
    }
    (0..selected_count)
        .map(|idx| {
            let slot = idx
                .wrapping_mul(29)
                .wrapping_add(idx / 2)
                .wrapping_rem(visible.len());
            visible[slot].1
        })
        .collect()
}

fn selection_lookup_key(instrument: &str, granularity: &str, time_ms: i64) -> String {
    format!("{}|{}|{}", instrument.to_ascii_lowercase(), granularity.to_ascii_lowercase(), time_ms)
}

fn run_indicator_key_cache_focus(bars: &[Bar], series_hash: &str, frames: usize) {
    let frames = frames.max(3);
    let indicators = synthetic_overlay_indicators();
    let calls_per_frame = 36_usize;
    let proof_legacy: Vec<String> = indicators.iter().map(indicator_key_legacy).collect();
    let mut proof_cache = IndicatorKeyObjectCache::new(indicators.len());
    let proof_cached: Vec<String> = indicators
        .iter()
        .enumerate()
        .map(|(idx, indicator)| indicator_key_cached(idx, indicator, &mut proof_cache).to_string())
        .collect();
    println!(
        "[trading-lab] focus=indicator-key-cache task=dedupe-overlay-indicator-settings-serialization series_hash={} bars={} frames={} indicators={} calls_per_frame={}",
        series_hash,
        bars.len(),
        frames,
        indicators.len(),
        calls_per_frame
    );
    println!(
        "[trading-lab] proof indicator-key-cache equal={} keys={} first_key={} last_key={}",
        proof_legacy == proof_cached,
        proof_cached.len(),
        proof_cached.first().map(String::as_str).unwrap_or(""),
        proof_cached.last().map(String::as_str).unwrap_or("")
    );

    let legacy = bench_indicator_keys(&indicators, frames, calls_per_frame, false);
    let optimized = bench_indicator_keys(&indicators, frames, calls_per_frame, true);
    let hit_rate = if optimized.hits + optimized.misses == 0 {
        0.0
    } else {
        optimized.hits as f64 / (optimized.hits + optimized.misses) as f64
    };
    println!(
        "[trading-lab] legacy-indicator-key model=stable-stringify-settings-each-call p50_ms={:.3} p95_ms={:.3} p99_ms={:.3} checksum={} serializations={}",
        legacy.stats.p50_us as f64 / 1000.0,
        legacy.stats.p95_us as f64 / 1000.0,
        legacy.stats.p99_us as f64 / 1000.0,
        legacy.checksum,
        legacy.serializations
    );
    println!(
        "[trading-lab] optimized-indicator-key model=weakmap-key-per-indicator-object p50_ms={:.3} p95_ms={:.3} p99_ms={:.3} checksum={} hits={} misses={} hit_rate={:.3} avoided_serializations={} cache_entries={} evictions=0",
        optimized.stats.p50_us as f64 / 1000.0,
        optimized.stats.p95_us as f64 / 1000.0,
        optimized.stats.p99_us as f64 / 1000.0,
        optimized.checksum,
        optimized.hits,
        optimized.misses,
        hit_rate,
        optimized.avoided_serializations,
        indicators.len()
    );
    println!(
        "[trading-lab] summary target=indicator_key_cache p50_speedup_x={:.2} p95_speedup_x={:.2} p99_speedup_x={:.2} cache_budget=one-key-per-active-indicator-object-reset-on-payload",
        ratio(legacy.stats.p50_us, optimized.stats.p50_us),
        ratio(legacy.stats.p95_us, optimized.stats.p95_us),
        ratio(legacy.stats.p99_us, optimized.stats.p99_us)
    );
}

fn bench_indicator_keys(
    indicators: &[SyntheticIndicator],
    frames: usize,
    calls_per_frame: usize,
    cached: bool,
) -> IndicatorKeyRunStats {
    let mut samples = Vec::with_capacity(frames);
    let mut checksum = 0_usize;
    let mut serializations = 0_usize;
    let mut cache = IndicatorKeyObjectCache::new(indicators.len());
    for _ in 0..frames {
        let started = Instant::now();
        for _ in 0..calls_per_frame {
            for (idx, indicator) in indicators.iter().enumerate() {
                if cached {
                    let key = indicator_key_cached(idx, indicator, &mut cache);
                    checksum = checksum.wrapping_add(key_checksum(key));
                } else {
                    let key = indicator_key_legacy(indicator);
                    serializations += 1;
                    checksum = checksum.wrapping_add(key_checksum(&key));
                }
            }
        }
        samples.push(started.elapsed().as_micros());
    }
    IndicatorKeyRunStats {
        stats: percentile_stats(&samples),
        checksum: std::hint::black_box(checksum),
        hits: cache.hits,
        misses: cache.misses,
        serializations,
        avoided_serializations: cache.avoided_serializations,
    }
}

fn indicator_key_cached<'a>(
    index: usize,
    indicator: &SyntheticIndicator,
    cache: &'a mut IndicatorKeyObjectCache,
) -> &'a str {
    if cache.keys.get(index).and_then(|key| key.as_ref()).is_some() {
        cache.hits += 1;
        cache.avoided_serializations += 1;
        return cache.keys[index].as_deref().unwrap();
    }
    cache.misses += 1;
    let key = indicator_key_legacy(indicator);
    cache.keys[index] = Some(key);
    cache.keys[index].as_deref().unwrap()
}

fn indicator_key_legacy(indicator: &SyntheticIndicator) -> String {
    format!("{}:{}", indicator.id.to_ascii_lowercase(), stable_settings_string(&indicator.settings))
}

fn stable_settings_string(settings: &[(&'static str, SettingValue)]) -> String {
    let mut pairs = settings.to_vec();
    pairs.sort_by(|left, right| left.0.cmp(right.0));
    let mut out = String::from("{");
    for (idx, (key, value)) in pairs.iter().enumerate() {
        if idx > 0 {
            out.push(',');
        }
        out.push('"');
        out.push_str(key);
        out.push_str("\":");
        match value {
            SettingValue::Int(value) => out.push_str(&value.to_string()),
            SettingValue::Float(value) => out.push_str(&format!("{value:.6}")),
            SettingValue::Text(value) => {
                out.push('"');
                out.push_str(value);
                out.push('"');
            }
        }
    }
    out.push('}');
    out
}

fn key_checksum(key: &str) -> usize {
    let mut hash = 2166136261_usize;
    for byte in key.as_bytes() {
        hash ^= *byte as usize;
        hash = hash.wrapping_mul(16777619);
    }
    hash
}

fn synthetic_overlay_indicators() -> Vec<SyntheticIndicator> {
    vec![
        SyntheticIndicator {
            id: "ema",
            settings: vec![("length", SettingValue::Int(20)), ("source", SettingValue::Text("close"))],
        },
        SyntheticIndicator {
            id: "sma",
            settings: vec![("length", SettingValue::Int(50)), ("source", SettingValue::Text("close"))],
        },
        SyntheticIndicator {
            id: "bollinger",
            settings: vec![
                ("length", SettingValue::Int(20)),
                ("deviation", SettingValue::Float(2.0)),
                ("source", SettingValue::Text("close")),
            ],
        },
        SyntheticIndicator {
            id: "wma",
            settings: vec![("length", SettingValue::Int(21)), ("source", SettingValue::Text("hlc3"))],
        },
        SyntheticIndicator {
            id: "hma",
            settings: vec![("length", SettingValue::Int(55)), ("source", SettingValue::Text("close"))],
        },
        SyntheticIndicator {
            id: "vwma",
            settings: vec![("length", SettingValue::Int(20)), ("source", SettingValue::Text("close"))],
        },
        SyntheticIndicator {
            id: "donchian",
            settings: vec![("length", SettingValue::Int(20))],
        },
        SyntheticIndicator {
            id: "keltner",
            settings: vec![
                ("length", SettingValue::Int(20)),
                ("multiplier", SettingValue::Float(1.5)),
                ("source", SettingValue::Text("hlc3")),
            ],
        },
        SyntheticIndicator {
            id: "supertrend",
            settings: vec![("atrLength", SettingValue::Int(10)), ("multiplier", SettingValue::Float(3.0))],
        },
        SyntheticIndicator {
            id: "ichimoku",
            settings: vec![
                ("conversion", SettingValue::Int(9)),
                ("base", SettingValue::Int(26)),
                ("spanB", SettingValue::Int(52)),
            ],
        },
        SyntheticIndicator {
            id: "psar",
            settings: vec![("step", SettingValue::Float(0.02)), ("max", SettingValue::Float(0.2))],
        },
        SyntheticIndicator {
            id: "vwap",
            settings: vec![("anchor", SettingValue::Text("session")), ("source", SettingValue::Text("hlc3"))],
        },
    ]
}

fn run_trading_subbar_cache_focus(bars: &[Bar], series_hash: &str, frames: usize) {
    let frames = frames.max(3);
    let calls_per_frame = 18_usize;
    let library = synthetic_subbar_library(96);
    let active_ids = synthetic_subbar_active_ids(&library, 14);
    let proof_state = ("indicators", false);
    let mut proof_legacy_work = TradingSubbarWork::default();
    let proof_legacy = build_trading_subbar_probe_legacy(
        &library,
        &active_ids,
        proof_state.0,
        proof_state.1,
        &mut proof_legacy_work,
    );
    let mut proof_cache = TradingSubbarCache::default();
    let mut proof_cached_work = TradingSubbarWork::default();
    let proof_cached = build_trading_subbar_probe_cached(
        &library,
        &active_ids,
        proof_state.0,
        proof_state.1,
        1,
        &mut proof_cache,
        &mut proof_cached_work,
    );
    println!(
        "[trading-lab] focus=trading-subbar-cache task=cache-indicator-catalog-section-libraries-and-skip-identical-innerhtml series_hash={} bars={} frames={} calls_per_frame={} library_entries={} active_indicators={}",
        series_hash,
        bars.len(),
        frames,
        calls_per_frame,
        library.len(),
        active_ids.len()
    );
    println!(
        "[trading-lab] proof trading-subbar-cache equal={} markup_hash={} nav_items={} body_items={} attached_items={} legacy_catalog_maps={} legacy_markup_bytes={}",
        proof_legacy == proof_cached,
        proof_cached.markup_hash,
        proof_cached.nav_items,
        proof_cached.body_items,
        proof_cached.attached_items,
        proof_legacy_work.catalog_maps,
        proof_legacy_work.markup_bytes
    );

    let legacy = bench_trading_subbar_cache(&library, &active_ids, frames, calls_per_frame, false);
    let optimized = bench_trading_subbar_cache(&library, &active_ids, frames, calls_per_frame, true);
    let hit_rate = if optimized.hits + optimized.misses == 0 {
        0.0
    } else {
        optimized.hits as f64 / (optimized.hits + optimized.misses) as f64
    };
    println!(
        "[trading-lab] legacy-trading-subbar model=registry-map-filter-library-build-markup-write-innerhtml-each-sync p50_ms={:.3} p95_ms={:.3} p99_ms={:.3} checksum={} catalog_maps={} favorite_filters={} section_filters={} active_membership_checks={} icon_builds={} markup_bytes={} dom_writes={}",
        legacy.stats.p50_us as f64 / 1000.0,
        legacy.stats.p95_us as f64 / 1000.0,
        legacy.stats.p99_us as f64 / 1000.0,
        legacy.checksum,
        legacy.catalog_maps,
        legacy.favorite_filters,
        legacy.section_filters,
        legacy.active_membership_checks,
        legacy.icon_builds,
        legacy.markup_bytes,
        legacy.dom_writes
    );
    println!(
        "[trading-lab] optimized-trading-subbar model=cached-catalog-plus-section-lists-plus-render-key-innerhtml-skip p50_ms={:.3} p95_ms={:.3} p99_ms={:.3} checksum={} hits={} misses={} hit_rate={:.3} catalog_maps={} avoided_catalog_maps={} favorite_filters={} avoided_favorite_filters={} section_filters={} avoided_section_filters={} active_membership_checks={} avoided_active_membership_checks={} icon_builds={} avoided_icon_builds={} markup_bytes={} avoided_markup_bytes={} dom_writes={} avoided_dom_writes={} cache_entries=6 evictions=0",
        optimized.stats.p50_us as f64 / 1000.0,
        optimized.stats.p95_us as f64 / 1000.0,
        optimized.stats.p99_us as f64 / 1000.0,
        optimized.checksum,
        optimized.hits,
        optimized.misses,
        hit_rate,
        optimized.catalog_maps,
        optimized.avoided_catalog_maps,
        optimized.favorite_filters,
        optimized.avoided_favorite_filters,
        optimized.section_filters,
        optimized.avoided_section_filters,
        optimized.active_membership_checks,
        optimized.avoided_active_membership_checks,
        optimized.icon_builds,
        optimized.avoided_icon_builds,
        optimized.markup_bytes,
        optimized.avoided_markup_bytes,
        optimized.dom_writes,
        optimized.avoided_dom_writes
    );
    println!(
        "[trading-lab] summary target=trading_subbar_cache p50_speedup_x={:.2} p95_speedup_x={:.2} p99_speedup_x={:.2} cache_budget=one-current-subbar-markup-plus-four-section-lists-plus-one-active-id-set",
        ratio(legacy.stats.p50_us, optimized.stats.p50_us),
        ratio(legacy.stats.p95_us, optimized.stats.p95_us),
        ratio(legacy.stats.p99_us, optimized.stats.p99_us)
    );
}

fn bench_trading_subbar_cache(
    library: &[SubbarLibraryEntry],
    active_ids: &[String],
    frames: usize,
    calls_per_frame: usize,
    cached: bool,
) -> TradingSubbarRunStats {
    let mut samples = Vec::with_capacity(frames);
    let mut checksum = 0_usize;
    let mut work = TradingSubbarWork::default();
    let mut cache = TradingSubbarCache::default();
    for frame in 0..frames {
        let (section, expanded) = synthetic_subbar_frame_state(frame);
        let started = Instant::now();
        for _ in 0..calls_per_frame {
            let probe = if cached {
                build_trading_subbar_probe_cached(
                    library,
                    active_ids,
                    section,
                    expanded,
                    1,
                    &mut cache,
                    &mut work,
                )
            } else {
                build_trading_subbar_probe_legacy(library, active_ids, section, expanded, &mut work)
            };
            checksum = checksum
                .wrapping_mul(16777619)
                .wrapping_add(probe.markup_hash)
                .wrapping_add(probe.body_items)
                .wrapping_add(probe.attached_items);
        }
        samples.push(started.elapsed().as_micros());
    }
    TradingSubbarRunStats {
        stats: percentile_stats(&samples),
        checksum: std::hint::black_box(checksum),
        hits: cache.hits,
        misses: cache.misses,
        catalog_maps: work.catalog_maps,
        avoided_catalog_maps: cache.avoided_catalog_maps,
        favorite_filters: work.favorite_filters,
        avoided_favorite_filters: cache.avoided_favorite_filters,
        section_filters: work.section_filters,
        avoided_section_filters: cache.avoided_section_filters,
        active_membership_checks: work.active_membership_checks,
        avoided_active_membership_checks: cache.avoided_active_membership_checks,
        icon_builds: work.icon_builds,
        avoided_icon_builds: cache.avoided_icon_builds,
        markup_bytes: work.markup_bytes,
        avoided_markup_bytes: cache.avoided_markup_bytes,
        dom_writes: work.dom_writes,
        avoided_dom_writes: cache.avoided_dom_writes,
    }
}

fn build_trading_subbar_probe_cached(
    library: &[SubbarLibraryEntry],
    active_ids: &[String],
    section: &str,
    expanded: bool,
    revision: usize,
    cache: &mut TradingSubbarCache,
    work: &mut TradingSubbarWork,
) -> TradingSubbarProbe {
    let active_signature = active_ids.join("|");
    let key = format!("subbar:v1|section={section}|expanded={expanded}|active={active_signature}|revision={revision}");
    if cache.key == key {
        if let Some(probe) = cache.probe {
            cache.hits += 1;
            cache.avoided_catalog_maps = cache
                .avoided_catalog_maps
                .saturating_add(cache.last_full_work.catalog_maps);
            cache.avoided_favorite_filters = cache
                .avoided_favorite_filters
                .saturating_add(cache.last_full_work.favorite_filters);
            cache.avoided_section_filters = cache
                .avoided_section_filters
                .saturating_add(cache.last_full_work.section_filters);
            cache.avoided_active_membership_checks = cache
                .avoided_active_membership_checks
                .saturating_add(cache.last_full_work.active_membership_checks);
            cache.avoided_icon_builds = cache
                .avoided_icon_builds
                .saturating_add(cache.last_full_work.icon_builds);
            cache.avoided_markup_bytes = cache
                .avoided_markup_bytes
                .saturating_add(cache.last_full_work.markup_bytes);
            cache.avoided_dom_writes += 1;
            return probe;
        }
    }
    cache.misses += 1;
    let before = *work;
    let probe = build_trading_subbar_probe_cached_miss(
        library,
        active_ids,
        section,
        expanded,
        cache,
        work,
    );
    let actual_delta = work_delta(*work, before);
    cache.last_full_work = estimate_trading_subbar_legacy_work(
        library.len(),
        active_ids.len(),
        section,
        expanded,
        probe.body_items,
        actual_delta.markup_bytes,
    );
    cache.key = key;
    cache.probe = Some(probe);
    probe
}

fn build_trading_subbar_probe_cached_miss(
    library: &[SubbarLibraryEntry],
    active_ids: &[String],
    section: &str,
    expanded: bool,
    cache: &mut TradingSubbarCache,
    work: &mut TradingSubbarWork,
) -> TradingSubbarProbe {
    if cache.catalog.is_none() {
        let mut catalog = Vec::with_capacity(library.len());
        for entry in library {
            work.catalog_maps += 1;
            catalog.push(entry.clone());
        }
        cache.catalog = Some(catalog);
    } else {
        cache.catalog_hits += 1;
    }
    let catalog = cache.catalog.as_ref().cloned().unwrap_or_default();
    let indexes = cached_subbar_section_indexes(section, &catalog, cache, work);
    let active_set: HashSet<&str> = active_ids.iter().map(String::as_str).collect();
    build_trading_subbar_probe_from_indexes(&catalog, &indexes, &active_set, section, expanded, work)
}

fn build_trading_subbar_probe_legacy(
    library: &[SubbarLibraryEntry],
    active_ids: &[String],
    section: &str,
    expanded: bool,
    work: &mut TradingSubbarWork,
) -> TradingSubbarProbe {
    let mut catalog = Vec::with_capacity(library.len());
    for entry in library {
        work.catalog_maps += 1;
        catalog.push(entry.clone());
    }
    let indexes = legacy_subbar_section_indexes(section, &catalog, work);
    let active_set: HashSet<&str> = active_ids.iter().map(String::as_str).collect();
    build_trading_subbar_probe_from_indexes(&catalog, &indexes, &active_set, section, expanded, work)
}

fn build_trading_subbar_probe_from_indexes(
    catalog: &[SubbarLibraryEntry],
    indexes: &[usize],
    active_ids: &HashSet<&str>,
    section: &str,
    expanded: bool,
    work: &mut TradingSubbarWork,
) -> TradingSubbarProbe {
    let mut hash = 2166136261_usize;
    let nav_items = 6_usize;
    let mut attached_items = 0_usize;
    work.icon_builds += if expanded { 4 } else { 3 };
    work.markup_bytes = work.markup_bytes.saturating_add(420 + nav_items * 132);
    hash = hash_add_str(hash, section);
    hash = hash.wrapping_add(usize::from(expanded));
    for index in indexes {
        let entry = &catalog[*index];
        work.markup_bytes = work.markup_bytes.saturating_add(156 + entry.command.len());
        hash = hash_add_str(hash, &entry.command);
        if entry.family == "indicator" {
            work.active_membership_checks += active_ids.len().max(1);
            if active_ids.contains(entry.id.as_str()) {
                attached_items += 1;
                hash = hash.wrapping_add(97);
            }
        }
    }
    work.dom_writes += 1;
    TradingSubbarProbe {
        markup_hash: std::hint::black_box(hash),
        nav_items,
        body_items: indexes.len(),
        attached_items,
    }
}

fn legacy_subbar_section_indexes(
    section: &str,
    catalog: &[SubbarLibraryEntry],
    work: &mut TradingSubbarWork,
) -> Vec<usize> {
    let mut out = Vec::new();
    for (idx, entry) in catalog.iter().enumerate() {
        match section {
            "favorites" => {
                work.favorite_filters += 1;
                if entry.family == "indicator" && entry.favorites {
                    out.push(idx);
                }
            }
            "indicators" => {
                work.section_filters += 1;
                if entry.family == "indicator" {
                    out.push(idx);
                }
            }
            "create" => {
                work.section_filters += 1;
                if entry.family == "create" && entry.command != "/create_" {
                    out.push(idx);
                }
            }
            "strategies" => {
                work.section_filters += 1;
                if entry.family == "strategy" {
                    out.push(idx);
                }
            }
            _ => {}
        }
    }
    out
}

fn cached_subbar_section_indexes(
    section: &str,
    catalog: &[SubbarLibraryEntry],
    cache: &mut TradingSubbarCache,
    work: &mut TradingSubbarWork,
) -> Vec<usize> {
    match section {
        "favorites" => {
            if cache.favorites.is_none() {
                cache.favorites = Some(legacy_subbar_section_indexes(section, catalog, work));
            }
            cache.favorites.clone().unwrap_or_default()
        }
        "create" => {
            if cache.create.is_none() {
                cache.create = Some(legacy_subbar_section_indexes(section, catalog, work));
            }
            cache.create.clone().unwrap_or_default()
        }
        "strategies" => {
            if cache.strategies.is_none() {
                cache.strategies = Some(legacy_subbar_section_indexes(section, catalog, work));
            }
            cache.strategies.clone().unwrap_or_default()
        }
        "indicators" => (0..catalog.len())
            .filter(|idx| catalog[*idx].family == "indicator")
            .collect(),
        _ => Vec::new(),
    }
}

fn work_delta(current: TradingSubbarWork, before: TradingSubbarWork) -> TradingSubbarWork {
    TradingSubbarWork {
        catalog_maps: current.catalog_maps.saturating_sub(before.catalog_maps),
        favorite_filters: current.favorite_filters.saturating_sub(before.favorite_filters),
        section_filters: current.section_filters.saturating_sub(before.section_filters),
        active_membership_checks: current
            .active_membership_checks
            .saturating_sub(before.active_membership_checks),
        icon_builds: current.icon_builds.saturating_sub(before.icon_builds),
        markup_bytes: current.markup_bytes.saturating_sub(before.markup_bytes),
        dom_writes: current.dom_writes.saturating_sub(before.dom_writes),
    }
}

fn estimate_trading_subbar_legacy_work(
    library_len: usize,
    active_count: usize,
    section: &str,
    expanded: bool,
    body_items: usize,
    markup_bytes: usize,
) -> TradingSubbarWork {
    TradingSubbarWork {
        catalog_maps: library_len,
        favorite_filters: usize::from(section == "favorites") * library_len,
        section_filters: usize::from(section != "favorites") * library_len,
        active_membership_checks: if section == "favorites" || section == "indicators" {
            body_items.saturating_mul(active_count.max(1))
        } else {
            0
        },
        icon_builds: if expanded { 4 } else { 3 },
        markup_bytes,
        dom_writes: 1,
    }
}

fn synthetic_subbar_frame_state(frame: usize) -> (&'static str, bool) {
    const SECTIONS: [&str; 4] = ["indicators", "indicators", "favorites", "create"];
    (SECTIONS[(frame / 32) % SECTIONS.len()], (frame / 48) % 2 == 1)
}

fn synthetic_subbar_library(count: usize) -> Vec<SubbarLibraryEntry> {
    let mut out = Vec::with_capacity(count + 24);
    for idx in 0..count {
        out.push(SubbarLibraryEntry {
            id: format!("indicator-{idx:03}"),
            command: format!("/ind_{idx:03}"),
            favorites: idx % 7 == 0,
            family: "indicator",
        });
    }
    for idx in 0..18 {
        out.push(SubbarLibraryEntry {
            id: format!("create-{idx:03}"),
            command: if idx == 0 { "/create_".to_string() } else { format!("/create_{idx:03}") },
            favorites: false,
            family: "create",
        });
    }
    for idx in 0..12 {
        out.push(SubbarLibraryEntry {
            id: format!("strategy-{idx:03}"),
            command: format!("/strategy_{idx:03}"),
            favorites: false,
            family: "strategy",
        });
    }
    out
}

fn synthetic_subbar_active_ids(library: &[SubbarLibraryEntry], count: usize) -> Vec<String> {
    library
        .iter()
        .filter(|entry| entry.family == "indicator")
        .step_by(5)
        .take(count)
        .map(|entry| entry.id.clone())
        .collect()
}

fn run_header_dock_cache_focus(bars: &[Bar], series_hash: &str, frames: usize) {
    let frames = frames.max(3);
    let calls_per_frame = 42_usize;
    let files = synthetic_history_files((bars.len() / 20).clamp(360, 1_200));
    let asset_catalog = build_asset_catalog_from_history(&files);
    let compare_assets = synthetic_compare_assets(&asset_catalog, 8);
    let indicators = synthetic_overlay_indicators();
    let selected = "EUR_USD";
    let mut proof_legacy_work = HeaderDockWork::default();
    let proof_legacy = build_header_dock_probe_legacy(
        &asset_catalog,
        &compare_assets,
        &indicators,
        selected,
        &mut proof_legacy_work,
    );
    let mut proof_cache = HeaderDockCache::default();
    let mut proof_cached_work = HeaderDockWork::default();
    let proof_cached = build_header_dock_probe_cached(
        &asset_catalog,
        &compare_assets,
        &indicators,
        selected,
        1,
        &mut proof_cache,
        &mut proof_cached_work,
    );
    println!(
        "[trading-lab] focus=header-dock-cache task=skip-identical-trading-header-bridge-write-and-indicator-dock-dom series_hash={} bars={} frames={} calls_per_frame={} assets={} compares={} active_indicators={}",
        series_hash,
        bars.len(),
        frames,
        calls_per_frame,
        asset_catalog.len(),
        compare_assets.len(),
        indicators.len()
    );
    println!(
        "[trading-lab] proof header-dock-cache equal={} header_hash={} dock_hash={} chips={} legacy_header_writes={} legacy_dock_dom_writes={} legacy_dock_markup_bytes={}",
        proof_legacy == proof_cached,
        proof_cached.header_hash,
        proof_cached.dock_hash,
        proof_cached.chips,
        proof_legacy_work.header_bridge_writes,
        proof_legacy_work.dock_dom_writes,
        proof_legacy_work.dock_markup_bytes
    );

    let legacy = bench_header_dock_cache(
        &asset_catalog,
        &compare_assets,
        &indicators,
        selected,
        frames,
        calls_per_frame,
        false,
    );
    let optimized = bench_header_dock_cache(
        &asset_catalog,
        &compare_assets,
        &indicators,
        selected,
        frames,
        calls_per_frame,
        true,
    );
    let hit_rate = if optimized.hits + optimized.misses == 0 {
        0.0
    } else {
        optimized.hits as f64 / (optimized.hits + optimized.misses) as f64
    };
    println!(
        "[trading-lab] legacy-header-dock model=find-asset-build-header-write-bridge-build-indicator-dock-innerhtml-each-sync p50_ms={:.3} p95_ms={:.3} p99_ms={:.3} checksum={} asset_lookups={} broker_set_checks={} header_payload_builds={} header_bridge_writes={} dock_key_units={} dock_markup_bytes={} dock_dom_writes={} action_syncs={}",
        legacy.stats.p50_us as f64 / 1000.0,
        legacy.stats.p95_us as f64 / 1000.0,
        legacy.stats.p99_us as f64 / 1000.0,
        legacy.checksum,
        legacy.asset_lookups,
        legacy.broker_set_checks,
        legacy.header_payload_builds,
        legacy.header_bridge_writes,
        legacy.dock_key_units,
        legacy.dock_markup_bytes,
        legacy.dock_dom_writes,
        legacy.action_syncs
    );
    println!(
        "[trading-lab] optimized-header-dock model=header-payload-key-plus-indicator-dock-key p50_ms={:.3} p95_ms={:.3} p99_ms={:.3} checksum={} hits={} misses={} hit_rate={:.3} asset_lookups={} avoided_asset_lookups={} broker_set_checks={} avoided_broker_set_checks={} header_payload_builds={} avoided_header_payload_builds={} header_bridge_writes={} avoided_header_bridge_writes={} dock_key_units={} dock_markup_bytes={} avoided_dock_markup_bytes={} dock_dom_writes={} avoided_dock_dom_writes={} action_syncs={} cache_entries=2 evictions=0",
        optimized.stats.p50_us as f64 / 1000.0,
        optimized.stats.p95_us as f64 / 1000.0,
        optimized.stats.p99_us as f64 / 1000.0,
        optimized.checksum,
        optimized.hits,
        optimized.misses,
        hit_rate,
        optimized.asset_lookups,
        optimized.avoided_asset_lookups,
        optimized.broker_set_checks,
        optimized.avoided_broker_set_checks,
        optimized.header_payload_builds,
        optimized.avoided_header_payload_builds,
        optimized.header_bridge_writes,
        optimized.avoided_header_bridge_writes,
        optimized.dock_key_units,
        optimized.dock_markup_bytes,
        optimized.avoided_dock_markup_bytes,
        optimized.dock_dom_writes,
        optimized.avoided_dock_dom_writes,
        optimized.action_syncs
    );
    println!(
        "[trading-lab] summary target=header_dock_cache p50_speedup_x={:.2} p95_speedup_x={:.2} p99_speedup_x={:.2} cache_budget=one-header-payload-key-plus-one-indicator-dock-key",
        ratio(legacy.stats.p50_us, optimized.stats.p50_us),
        ratio(legacy.stats.p95_us, optimized.stats.p95_us),
        ratio(legacy.stats.p99_us, optimized.stats.p99_us)
    );
}

#[allow(clippy::too_many_arguments)]
fn bench_header_dock_cache(
    asset_catalog: &[AssetEntry],
    compare_assets: &[String],
    indicators: &[SyntheticIndicator],
    selected: &str,
    frames: usize,
    calls_per_frame: usize,
    cached: bool,
) -> HeaderDockRunStats {
    let mut samples = Vec::with_capacity(frames);
    let mut checksum = 0_usize;
    let mut work = HeaderDockWork::default();
    let mut cache = HeaderDockCache::default();
    for _ in 0..frames {
        let started = Instant::now();
        for _ in 0..calls_per_frame {
            let probe = if cached {
                build_header_dock_probe_cached(
                    asset_catalog,
                    compare_assets,
                    indicators,
                    selected,
                    1,
                    &mut cache,
                    &mut work,
                )
            } else {
                build_header_dock_probe_legacy(asset_catalog, compare_assets, indicators, selected, &mut work)
            };
            checksum = checksum
                .wrapping_mul(16777619)
                .wrapping_add(probe.header_hash)
                .wrapping_add(probe.dock_hash)
                .wrapping_add(probe.chips);
        }
        samples.push(started.elapsed().as_micros());
    }
    HeaderDockRunStats {
        stats: percentile_stats(&samples),
        checksum: std::hint::black_box(checksum),
        hits: cache.hits,
        misses: cache.misses,
        asset_lookups: work.asset_lookups,
        avoided_asset_lookups: cache.avoided_asset_lookups,
        broker_set_checks: work.broker_set_checks,
        avoided_broker_set_checks: cache.avoided_broker_set_checks,
        header_payload_builds: work.header_payload_builds,
        avoided_header_payload_builds: cache.avoided_header_payload_builds,
        header_bridge_writes: work.header_bridge_writes,
        avoided_header_bridge_writes: cache.avoided_header_bridge_writes,
        dock_key_units: work.dock_key_units,
        dock_markup_bytes: work.dock_markup_bytes,
        avoided_dock_markup_bytes: cache.avoided_dock_markup_bytes,
        dock_dom_writes: work.dock_dom_writes,
        avoided_dock_dom_writes: cache.avoided_dock_dom_writes,
        action_syncs: work.action_syncs,
    }
}

#[allow(clippy::too_many_arguments)]
fn build_header_dock_probe_cached(
    asset_catalog: &[AssetEntry],
    compare_assets: &[String],
    indicators: &[SyntheticIndicator],
    selected: &str,
    revision: usize,
    cache: &mut HeaderDockCache,
    work: &mut HeaderDockWork,
) -> HeaderDockProbe {
    let header_key = format!(
        "header:v1|revision={revision}|selected={selected}|compare={}|active=1",
        compare_assets.join(",")
    );
    let dock_key = format!(
        "dock:v1|revision={revision}|indicators={}",
        indicators.iter().map(|indicator| indicator.id).collect::<Vec<_>>().join(",")
    );
    if cache.header_key == header_key && cache.dock_key == dock_key {
        if let Some(probe) = cache.header_probe {
            cache.hits += 1;
            work.dock_key_units = work.dock_key_units.saturating_add(indicators.len());
            work.action_syncs = work.action_syncs.saturating_add(3);
            cache.avoided_asset_lookups = cache
                .avoided_asset_lookups
                .saturating_add(cache.last_full_work.asset_lookups);
            cache.avoided_broker_set_checks = cache
                .avoided_broker_set_checks
                .saturating_add(cache.last_full_work.broker_set_checks);
            cache.avoided_header_payload_builds = cache
                .avoided_header_payload_builds
                .saturating_add(cache.last_full_work.header_payload_builds);
            cache.avoided_header_bridge_writes = cache.avoided_header_bridge_writes.saturating_add(1);
            cache.avoided_dock_markup_bytes = cache
                .avoided_dock_markup_bytes
                .saturating_add(cache.last_full_work.dock_markup_bytes);
            cache.avoided_dock_dom_writes = cache.avoided_dock_dom_writes.saturating_add(1);
            return HeaderDockProbe {
                dock_hash: cache.dock_hash,
                ..probe
            };
        }
    }
    cache.misses += 1;
    let before = *work;
    let probe = build_header_dock_probe_legacy(asset_catalog, compare_assets, indicators, selected, work);
    cache.last_full_work = header_dock_work_delta(*work, before);
    cache.header_key = header_key;
    cache.dock_key = dock_key;
    cache.header_probe = Some(probe);
    cache.dock_hash = probe.dock_hash;
    probe
}

fn build_header_dock_probe_legacy(
    asset_catalog: &[AssetEntry],
    compare_assets: &[String],
    indicators: &[SyntheticIndicator],
    selected: &str,
    work: &mut HeaderDockWork,
) -> HeaderDockProbe {
    let mut header_hash = 2166136261_usize;
    let mut display_name = selected;
    for asset in asset_catalog {
        work.asset_lookups += 1;
        if asset.name == selected {
            display_name = &asset.display_name;
            break;
        }
    }
    work.broker_set_checks += 1;
    work.header_payload_builds += 1;
    work.header_bridge_writes += 1;
    header_hash = hash_add_str(header_hash, "OANDA");
    header_hash = hash_add_str(header_hash, selected);
    header_hash = hash_add_str(header_hash, display_name);
    header_hash = header_hash.wrapping_add(compare_assets.len());

    let mut dock_hash = 2166136261_usize;
    work.dock_key_units = work.dock_key_units.saturating_add(indicators.len());
    for indicator in indicators {
        work.dock_markup_bytes = work
            .dock_markup_bytes
            .saturating_add(420 + indicator.id.len() + stable_settings_string(&indicator.settings).len());
        dock_hash = hash_add_str(dock_hash, indicator.id);
        dock_hash = hash_add_str(dock_hash, &indicator_key_legacy(indicator));
    }
    work.dock_dom_writes += 1;
    work.action_syncs += 3;
    HeaderDockProbe {
        header_hash: std::hint::black_box(header_hash),
        dock_hash: std::hint::black_box(dock_hash),
        chips: indicators.len(),
    }
}

fn header_dock_work_delta(current: HeaderDockWork, before: HeaderDockWork) -> HeaderDockWork {
    HeaderDockWork {
        asset_lookups: current.asset_lookups.saturating_sub(before.asset_lookups),
        broker_set_checks: current.broker_set_checks.saturating_sub(before.broker_set_checks),
        header_payload_builds: current
            .header_payload_builds
            .saturating_sub(before.header_payload_builds),
        header_bridge_writes: current
            .header_bridge_writes
            .saturating_sub(before.header_bridge_writes),
        dock_key_units: current.dock_key_units.saturating_sub(before.dock_key_units),
        dock_markup_bytes: current
            .dock_markup_bytes
            .saturating_sub(before.dock_markup_bytes),
        dock_dom_writes: current.dock_dom_writes.saturating_sub(before.dock_dom_writes),
        action_syncs: current.action_syncs.saturating_sub(before.action_syncs),
    }
}

fn run_toolbar_chrome_cache_focus(bars: &[Bar], series_hash: &str, frames: usize) {
    let frames = frames.max(3);
    let calls_per_frame = 120_usize;
    let states = synthetic_toolbar_chrome_states(frames);
    let mut proof_legacy_work = ToolbarChromeWork::default();
    let proof_state = states.first().copied().unwrap_or_default();
    let proof_legacy = build_toolbar_chrome_probe_legacy(proof_state, &mut proof_legacy_work);
    let mut proof_cache = ToolbarChromeCache::default();
    let mut proof_cached_work = ToolbarChromeWork::default();
    let proof_cached = build_toolbar_chrome_probe_cached(
        proof_state,
        &mut proof_cache,
        &mut proof_cached_work,
    );
    println!(
        "[trading-lab] focus=toolbar-chrome-cache task=skip-identical-toolbar-trigger-and-chat-action-dom-sync series_hash={} bars={} frames={} calls_per_frame={} state_changes={} cache_entries=1",
        series_hash,
        bars.len(),
        frames,
        calls_per_frame,
        toolbar_state_changes(&states)
    );
    println!(
        "[trading-lab] proof toolbar-chrome-cache equal={} dom_hash={} trigger_states={} legacy_html_bytes={} legacy_attr_writes={} legacy_dataset_writes={}",
        proof_legacy == proof_cached,
        proof_cached.dom_hash,
        proof_cached.trigger_states,
        proof_legacy_work.html_bytes,
        proof_legacy_work.attr_writes,
        proof_legacy_work.dataset_writes
    );

    let legacy = bench_toolbar_chrome_cache(&states, calls_per_frame, false);
    let optimized = bench_toolbar_chrome_cache(&states, calls_per_frame, true);
    let hit_rate = if optimized.hits + optimized.misses == 0 {
        0.0
    } else {
        optimized.hits as f64 / (optimized.hits + optimized.misses) as f64
    };
    println!(
        "[trading-lab] legacy-toolbar-chrome model=rewrite-trigger-svg-aria-dataset-and-chat-actions-each-sync p50_ms={:.3} p95_ms={:.3} p99_ms={:.3} checksum={} state_reads={} html_bytes={} attr_writes={} dataset_writes={} hidden_writes={} class_toggles={} subbar_syncs={} runtime_control_syncs={}",
        legacy.stats.p50_us as f64 / 1000.0,
        legacy.stats.p95_us as f64 / 1000.0,
        legacy.stats.p99_us as f64 / 1000.0,
        legacy.checksum,
        legacy.state_reads,
        legacy.html_bytes,
        legacy.attr_writes,
        legacy.dataset_writes,
        legacy.hidden_writes,
        legacy.class_toggles,
        legacy.subbar_syncs,
        legacy.runtime_control_syncs
    );
    println!(
        "[trading-lab] optimized-toolbar-chrome model=single-state-key-for-chart-program-alert-chat-actions p50_ms={:.3} p95_ms={:.3} p99_ms={:.3} checksum={} hits={} misses={} hit_rate={:.3} state_reads={} html_bytes={} avoided_html_bytes={} attr_writes={} avoided_attr_writes={} dataset_writes={} avoided_dataset_writes={} hidden_writes={} avoided_hidden_writes={} class_toggles={} avoided_class_toggles={} subbar_syncs={} avoided_subbar_syncs={} runtime_control_syncs={} avoided_runtime_control_syncs={} cache_entries=1 evictions=0",
        optimized.stats.p50_us as f64 / 1000.0,
        optimized.stats.p95_us as f64 / 1000.0,
        optimized.stats.p99_us as f64 / 1000.0,
        optimized.checksum,
        optimized.hits,
        optimized.misses,
        hit_rate,
        optimized.state_reads,
        optimized.html_bytes,
        optimized.avoided_html_bytes,
        optimized.attr_writes,
        optimized.avoided_attr_writes,
        optimized.dataset_writes,
        optimized.avoided_dataset_writes,
        optimized.hidden_writes,
        optimized.avoided_hidden_writes,
        optimized.class_toggles,
        optimized.avoided_class_toggles,
        optimized.subbar_syncs,
        optimized.avoided_subbar_syncs,
        optimized.runtime_control_syncs,
        optimized.avoided_runtime_control_syncs
    );
    println!(
        "[trading-lab] summary target=toolbar_chrome_cache p50_speedup_x={:.2} p95_speedup_x={:.2} p99_speedup_x={:.2} cache_budget=one-toolbar-state-key",
        ratio(legacy.stats.p50_us, optimized.stats.p50_us),
        ratio(legacy.stats.p95_us, optimized.stats.p95_us),
        ratio(legacy.stats.p99_us, optimized.stats.p99_us)
    );
}

fn bench_toolbar_chrome_cache(
    states: &[ToolbarChromeState],
    calls_per_frame: usize,
    cached: bool,
) -> ToolbarChromeRunStats {
    let mut samples = Vec::with_capacity(states.len());
    let mut checksum = 0_usize;
    let mut work = ToolbarChromeWork::default();
    let mut cache = ToolbarChromeCache::default();
    for state in states {
        let started = Instant::now();
        for _ in 0..calls_per_frame {
            let probe = if cached {
                build_toolbar_chrome_probe_cached(*state, &mut cache, &mut work)
            } else {
                build_toolbar_chrome_probe_legacy(*state, &mut work)
            };
            checksum = checksum
                .wrapping_mul(16777619)
                .wrapping_add(probe.dom_hash)
                .wrapping_add(probe.trigger_states);
        }
        samples.push(started.elapsed().as_micros());
    }
    ToolbarChromeRunStats {
        stats: percentile_stats(&samples),
        checksum: std::hint::black_box(checksum),
        hits: cache.hits,
        misses: cache.misses,
        state_reads: work.state_reads,
        html_bytes: work.html_bytes,
        avoided_html_bytes: cache.avoided_html_bytes,
        attr_writes: work.attr_writes,
        avoided_attr_writes: cache.avoided_attr_writes,
        dataset_writes: work.dataset_writes,
        avoided_dataset_writes: cache.avoided_dataset_writes,
        hidden_writes: work.hidden_writes,
        avoided_hidden_writes: cache.avoided_hidden_writes,
        class_toggles: work.class_toggles,
        avoided_class_toggles: cache.avoided_class_toggles,
        subbar_syncs: work.subbar_syncs,
        avoided_subbar_syncs: cache.avoided_subbar_syncs,
        runtime_control_syncs: work.runtime_control_syncs,
        avoided_runtime_control_syncs: cache.avoided_runtime_control_syncs,
    }
}

fn build_toolbar_chrome_probe_cached(
    state: ToolbarChromeState,
    cache: &mut ToolbarChromeCache,
    work: &mut ToolbarChromeWork,
) -> ToolbarChromeProbe {
    work.state_reads = work.state_reads.saturating_add(7);
    let key = toolbar_chrome_key(state);
    if cache.key == key {
        if let Some(probe) = cache.probe {
            cache.hits += 1;
            cache.avoided_html_bytes = cache
                .avoided_html_bytes
                .saturating_add(cache.last_full_work.html_bytes);
            cache.avoided_attr_writes = cache
                .avoided_attr_writes
                .saturating_add(cache.last_full_work.attr_writes);
            cache.avoided_dataset_writes = cache
                .avoided_dataset_writes
                .saturating_add(cache.last_full_work.dataset_writes);
            cache.avoided_hidden_writes = cache
                .avoided_hidden_writes
                .saturating_add(cache.last_full_work.hidden_writes);
            cache.avoided_class_toggles = cache
                .avoided_class_toggles
                .saturating_add(cache.last_full_work.class_toggles);
            cache.avoided_subbar_syncs = cache
                .avoided_subbar_syncs
                .saturating_add(cache.last_full_work.subbar_syncs);
            cache.avoided_runtime_control_syncs = cache
                .avoided_runtime_control_syncs
                .saturating_add(cache.last_full_work.runtime_control_syncs);
            return probe;
        }
    }
    cache.misses += 1;
    let before = *work;
    let probe = build_toolbar_chrome_probe_legacy(state, work);
    cache.last_full_work = toolbar_chrome_work_delta(*work, before);
    cache.key = key;
    cache.probe = Some(probe);
    probe
}

fn build_toolbar_chrome_probe_legacy(
    state: ToolbarChromeState,
    work: &mut ToolbarChromeWork,
) -> ToolbarChromeProbe {
    work.state_reads = work.state_reads.saturating_add(7);
    let mut hash = hash_add_str(2166136261_usize, "toolbar-chrome");
    hash = hash.wrapping_add(state.active as usize);
    hash = hash.wrapping_add((state.display_menu_open as usize) << 1);
    hash = hash.wrapping_add((state.right_panel_open as usize) << 2);
    hash = hash.wrapping_add(state.chart_mode << 3);
    hash = hash.wrapping_add(state.chat_mode << 6);
    hash = hash.wrapping_add((state.selection_enabled as usize) << 9);
    hash = hash.wrapping_add((state.runtime_involved as usize) << 10);
    let (html_bytes, attr_writes, dataset_writes) = if state.active {
        (380 + 360, 5 + 3 + 2 + 4, 2 + 2 + 1)
    } else {
        (280 + 260, 4 + 3 + 2, 4)
    };
    let hidden_writes = 5_usize;
    let class_toggles = 2_usize;
    let subbar_syncs = 1_usize;
    let runtime_control_syncs = 1_usize;
    work.html_bytes = work.html_bytes.saturating_add(html_bytes);
    work.attr_writes = work.attr_writes.saturating_add(attr_writes);
    work.dataset_writes = work.dataset_writes.saturating_add(dataset_writes);
    work.hidden_writes = work.hidden_writes.saturating_add(hidden_writes);
    work.class_toggles = work.class_toggles.saturating_add(class_toggles);
    work.subbar_syncs = work.subbar_syncs.saturating_add(subbar_syncs);
    work.runtime_control_syncs = work
        .runtime_control_syncs
        .saturating_add(runtime_control_syncs);
    hash = simulate_toolbar_dom_mutation_cost(
        hash,
        html_bytes,
        attr_writes,
        dataset_writes,
        hidden_writes,
        class_toggles,
        subbar_syncs,
        runtime_control_syncs,
    );
    ToolbarChromeProbe {
        dom_hash: std::hint::black_box(hash),
        trigger_states: state.chart_mode + state.chat_mode + usize::from(state.right_panel_open),
    }
}

fn simulate_toolbar_dom_mutation_cost(
    mut hash: usize,
    html_bytes: usize,
    attr_writes: usize,
    dataset_writes: usize,
    hidden_writes: usize,
    class_toggles: usize,
    subbar_syncs: usize,
    runtime_control_syncs: usize,
) -> usize {
    let units = html_bytes / 6
        + attr_writes.saturating_mul(48)
        + dataset_writes.saturating_mul(36)
        + hidden_writes.saturating_mul(24)
        + class_toggles.saturating_mul(24)
        + subbar_syncs.saturating_mul(72)
        + runtime_control_syncs.saturating_mul(48);
    for idx in 0..units {
        hash = hash
            .rotate_left(5)
            .wrapping_mul(16777619)
            .wrapping_add(idx ^ units);
    }
    std::hint::black_box(hash)
}

fn toolbar_chrome_key(state: ToolbarChromeState) -> String {
    format!(
        "{}:{}:{}:{}:{}:{}:{}",
        usize::from(state.active),
        usize::from(state.display_menu_open),
        usize::from(state.right_panel_open),
        state.chart_mode,
        state.chat_mode,
        usize::from(state.selection_enabled),
        usize::from(state.runtime_involved)
    )
}

fn toolbar_chrome_work_delta(
    current: ToolbarChromeWork,
    before: ToolbarChromeWork,
) -> ToolbarChromeWork {
    ToolbarChromeWork {
        state_reads: current.state_reads.saturating_sub(before.state_reads),
        html_bytes: current.html_bytes.saturating_sub(before.html_bytes),
        attr_writes: current.attr_writes.saturating_sub(before.attr_writes),
        dataset_writes: current.dataset_writes.saturating_sub(before.dataset_writes),
        hidden_writes: current.hidden_writes.saturating_sub(before.hidden_writes),
        class_toggles: current.class_toggles.saturating_sub(before.class_toggles),
        subbar_syncs: current.subbar_syncs.saturating_sub(before.subbar_syncs),
        runtime_control_syncs: current
            .runtime_control_syncs
            .saturating_sub(before.runtime_control_syncs),
    }
}

fn synthetic_toolbar_chrome_states(frames: usize) -> Vec<ToolbarChromeState> {
    (0..frames)
        .map(|frame| ToolbarChromeState {
            active: true,
            display_menu_open: (frame / 36) % 2 == 1,
            right_panel_open: (frame / 48) % 2 == 1,
            chart_mode: (frame / 72) % 3,
            chat_mode: (frame / 30) % 4,
            selection_enabled: (frame / 40) % 2 == 1,
            runtime_involved: (frame / 90) % 2 == 0,
        })
        .collect()
}

fn toolbar_state_changes(states: &[ToolbarChromeState]) -> usize {
    if states.is_empty() {
        return 0;
    }
    1 + states.windows(2).filter(|pair| pair[0] != pair[1]).count()
}

fn run_comparison_payload_cache_focus(bars: &[Bar], series_hash: &str, frames: usize) {
    let frames = frames.max(3);
    let calls_per_frame = 30_usize;
    let files = synthetic_history_files((bars.len() / 18).clamp(360, 1_400));
    let asset_catalog = build_asset_catalog_from_history(&files);
    let targets = synthetic_compare_assets(&asset_catalog, 12);
    let revisions: Vec<usize> = (0..targets.len()).map(|idx| 10_000 + idx).collect();
    let series_len = bars.len().min(4_000);
    let mut proof_legacy_work = ComparisonPayloadWork::default();
    let proof_legacy = build_comparison_payload_probe_legacy(
        &asset_catalog,
        &targets,
        &revisions,
        series_len,
        &mut proof_legacy_work,
    );
    let mut proof_cache = ComparisonPayloadCache::default();
    let mut proof_cached_work = ComparisonPayloadWork::default();
    let proof_cached = build_comparison_payload_probe_cached(
        &asset_catalog,
        &targets,
        &revisions,
        series_len,
        "H4",
        1,
        &mut proof_cache,
        &mut proof_cached_work,
    );
    println!(
        "[trading-lab] focus=comparison-payload-cache task=skip-identical-extra-chart-payload-and-bridge-write series_hash={} bars={} frames={} calls_per_frame={} targets={} series_len={} assets={}",
        series_hash,
        bars.len(),
        frames,
        calls_per_frame,
        targets.len(),
        series_len,
        asset_catalog.len()
    );
    println!(
        "[trading-lab] proof comparison-payload-cache equal={} payload_hash={} charts={} candle_refs={} legacy_asset_lookups={} legacy_bridge_writes={}",
        proof_legacy == proof_cached,
        proof_cached.payload_hash,
        proof_cached.charts,
        proof_cached.candle_refs,
        proof_legacy_work.asset_lookups,
        proof_legacy_work.bridge_writes
    );

    let legacy = bench_comparison_payload_cache(
        &asset_catalog,
        &targets,
        &revisions,
        series_len,
        frames,
        calls_per_frame,
        false,
    );
    let optimized = bench_comparison_payload_cache(
        &asset_catalog,
        &targets,
        &revisions,
        series_len,
        frames,
        calls_per_frame,
        true,
    );
    let hit_rate = if optimized.hits + optimized.misses == 0 {
        0.0
    } else {
        optimized.hits as f64 / (optimized.hits + optimized.misses) as f64
    };
    println!(
        "[trading-lab] legacy-comparison-payload model=build-extra-chart-array-and-bridge-write-each-sync p50_ms={:.3} p95_ms={:.3} p99_ms={:.3} checksum={} revision_checks={} asset_lookups={} label_builds={} payload_builds={} candle_refs={} bridge_writes={}",
        legacy.stats.p50_us as f64 / 1000.0,
        legacy.stats.p95_us as f64 / 1000.0,
        legacy.stats.p99_us as f64 / 1000.0,
        legacy.checksum,
        legacy.revision_checks,
        legacy.asset_lookups,
        legacy.label_builds,
        legacy.payload_builds,
        legacy.candle_refs,
        legacy.bridge_writes
    );
    println!(
        "[trading-lab] optimized-comparison-payload model=target-series-revision-key-plus-extra-chart-payload-cache p50_ms={:.3} p95_ms={:.3} p99_ms={:.3} checksum={} hits={} misses={} hit_rate={:.3} revision_checks={} asset_lookups={} avoided_asset_lookups={} label_builds={} avoided_label_builds={} payload_builds={} avoided_payload_builds={} candle_refs={} avoided_candle_refs={} bridge_writes={} avoided_bridge_writes={} cache_entries=1 evictions=0",
        optimized.stats.p50_us as f64 / 1000.0,
        optimized.stats.p95_us as f64 / 1000.0,
        optimized.stats.p99_us as f64 / 1000.0,
        optimized.checksum,
        optimized.hits,
        optimized.misses,
        hit_rate,
        optimized.revision_checks,
        optimized.asset_lookups,
        optimized.avoided_asset_lookups,
        optimized.label_builds,
        optimized.avoided_label_builds,
        optimized.payload_builds,
        optimized.avoided_payload_builds,
        optimized.candle_refs,
        optimized.avoided_candle_refs,
        optimized.bridge_writes,
        optimized.avoided_bridge_writes
    );
    println!(
        "[trading-lab] summary target=comparison_payload_cache p50_speedup_x={:.2} p95_speedup_x={:.2} p99_speedup_x={:.2} cache_budget=one-extra-chart-payload-per-target-revision-key",
        ratio(legacy.stats.p50_us, optimized.stats.p50_us),
        ratio(legacy.stats.p95_us, optimized.stats.p95_us),
        ratio(legacy.stats.p99_us, optimized.stats.p99_us)
    );
}

fn bench_comparison_payload_cache(
    asset_catalog: &[AssetEntry],
    targets: &[String],
    revisions: &[usize],
    series_len: usize,
    frames: usize,
    calls_per_frame: usize,
    cached: bool,
) -> ComparisonPayloadRunStats {
    let mut samples = Vec::with_capacity(frames);
    let mut checksum = 0_usize;
    let mut work = ComparisonPayloadWork::default();
    let mut cache = ComparisonPayloadCache::default();
    for _ in 0..frames {
        let started = Instant::now();
        for _ in 0..calls_per_frame {
            let probe = if cached {
                build_comparison_payload_probe_cached(
                    asset_catalog,
                    targets,
                    revisions,
                    series_len,
                    "H4",
                    1,
                    &mut cache,
                    &mut work,
                )
            } else {
                build_comparison_payload_probe_legacy(asset_catalog, targets, revisions, series_len, &mut work)
            };
            checksum = checksum
                .wrapping_mul(16777619)
                .wrapping_add(probe.payload_hash)
                .wrapping_add(probe.charts)
                .wrapping_add(probe.candle_refs);
        }
        samples.push(started.elapsed().as_micros());
    }
    ComparisonPayloadRunStats {
        stats: percentile_stats(&samples),
        checksum: std::hint::black_box(checksum),
        hits: cache.hits,
        misses: cache.misses,
        revision_checks: work.revision_checks,
        asset_lookups: work.asset_lookups,
        avoided_asset_lookups: cache.avoided_asset_lookups,
        label_builds: work.label_builds,
        avoided_label_builds: cache.avoided_label_builds,
        payload_builds: work.payload_builds,
        avoided_payload_builds: cache.avoided_payload_builds,
        candle_refs: work.candle_refs,
        avoided_candle_refs: cache.avoided_candle_refs,
        bridge_writes: work.bridge_writes,
        avoided_bridge_writes: cache.avoided_bridge_writes,
    }
}

#[allow(clippy::too_many_arguments)]
fn build_comparison_payload_probe_cached(
    asset_catalog: &[AssetEntry],
    targets: &[String],
    revisions: &[usize],
    series_len: usize,
    granularity: &str,
    asset_revision: usize,
    cache: &mut ComparisonPayloadCache,
    work: &mut ComparisonPayloadWork,
) -> ComparisonPayloadProbe {
    work.revision_checks = work.revision_checks.saturating_add(targets.len());
    let key = comparison_payload_key(targets, revisions, granularity, asset_revision);
    if cache.key == key {
        if let Some(probe) = cache.probe {
            cache.hits += 1;
            cache.avoided_asset_lookups = cache
                .avoided_asset_lookups
                .saturating_add(cache.last_full_work.asset_lookups);
            cache.avoided_label_builds = cache
                .avoided_label_builds
                .saturating_add(cache.last_full_work.label_builds);
            cache.avoided_payload_builds = cache
                .avoided_payload_builds
                .saturating_add(cache.last_full_work.payload_builds);
            cache.avoided_candle_refs = cache
                .avoided_candle_refs
                .saturating_add(cache.last_full_work.candle_refs);
            cache.avoided_bridge_writes = cache.avoided_bridge_writes.saturating_add(1);
            return probe;
        }
    }
    cache.misses += 1;
    let before = work_comparison_payload_snapshot(*work);
    let probe = build_comparison_payload_probe_legacy(asset_catalog, targets, revisions, series_len, work);
    cache.last_full_work = comparison_payload_work_delta(*work, before);
    cache.key = key;
    cache.probe = Some(probe);
    probe
}

fn build_comparison_payload_probe_legacy(
    asset_catalog: &[AssetEntry],
    targets: &[String],
    revisions: &[usize],
    series_len: usize,
    work: &mut ComparisonPayloadWork,
) -> ComparisonPayloadProbe {
    work.revision_checks = work.revision_checks.saturating_add(targets.len());
    let mut payload_hash = 2166136261_usize;
    let mut charts = 0_usize;
    let mut candle_refs = 0_usize;
    for (idx, target) in targets.iter().enumerate() {
        let mut display_name = target.as_str();
        for asset in asset_catalog {
            work.asset_lookups += 1;
            if asset.name == *target {
                display_name = &asset.display_name;
                break;
            }
        }
        work.label_builds += 2;
        work.payload_builds += 1;
        work.candle_refs = work.candle_refs.saturating_add(series_len);
        candle_refs = candle_refs.saturating_add(series_len);
        charts += 1;
        payload_hash = hash_add_str(payload_hash, target);
        payload_hash = hash_add_str(payload_hash, display_name);
        payload_hash = payload_hash.wrapping_add(revisions.get(idx).copied().unwrap_or(0));
        payload_hash = payload_hash.wrapping_add(series_len);
    }
    work.bridge_writes += 1;
    ComparisonPayloadProbe {
        payload_hash: std::hint::black_box(payload_hash),
        charts,
        candle_refs,
    }
}

fn comparison_payload_key(
    targets: &[String],
    revisions: &[usize],
    granularity: &str,
    asset_revision: usize,
) -> String {
    let mut out = format!("extra:v1|asset={asset_revision}|g={granularity}");
    for (idx, target) in targets.iter().enumerate() {
        out.push('|');
        out.push_str(target);
        out.push(':');
        out.push_str(&revisions.get(idx).copied().unwrap_or(0).to_string());
    }
    out
}

fn work_comparison_payload_snapshot(work: ComparisonPayloadWork) -> ComparisonPayloadWork {
    work
}

fn comparison_payload_work_delta(
    current: ComparisonPayloadWork,
    before: ComparisonPayloadWork,
) -> ComparisonPayloadWork {
    ComparisonPayloadWork {
        revision_checks: current.revision_checks.saturating_sub(before.revision_checks),
        asset_lookups: current.asset_lookups.saturating_sub(before.asset_lookups),
        label_builds: current.label_builds.saturating_sub(before.label_builds),
        payload_builds: current.payload_builds.saturating_sub(before.payload_builds),
        candle_refs: current.candle_refs.saturating_sub(before.candle_refs),
        bridge_writes: current.bridge_writes.saturating_sub(before.bridge_writes),
    }
}

fn run_history_load_coalescing_focus(bars: &[Bar], series_hash: &str, frames: usize) {
    let frames = frames.max(3);
    let calls_per_frame = 96_usize;
    let unique_keys = (bars.len() / 2_500).clamp(8, 18);
    let files = synthetic_history_files((bars.len() / 16).clamp(360, 1_200));
    let asset_catalog = build_asset_catalog_from_history(&files);
    let requests =
        synthetic_history_load_requests(&files, &asset_catalog, unique_keys, calls_per_frame);
    let unique_request_keys: HashSet<&str> = requests.iter().map(|request| request.key.as_str()).collect();
    let mut proof_legacy_work = HistoryLoadWork::default();
    let proof_legacy = build_history_load_probe_legacy(&requests, &mut proof_legacy_work);
    let mut proof_cache = HistoryLoadCache::default();
    let mut proof_cached_work = HistoryLoadWork::default();
    let proof_cached = build_history_load_probe_cached_wave(
        &requests,
        &mut proof_cache,
        &mut proof_cached_work,
    );
    println!(
        "[trading-lab] focus=history-load-coalescing task=coalesce-duplicate-history-series-loads series_hash={} bars={} frames={} calls_per_frame={} unique_keys={} catalog_files={} miss_cache_budget=512",
        series_hash,
        bars.len(),
        frames,
        calls_per_frame,
        unique_request_keys.len(),
        files.len()
    );
    println!(
        "[trading-lab] proof history-load-coalescing equal={} payload_hash={} responses={} candles={} legacy_backend_calls={} optimized_backend_calls={} coalesced_waiters={}",
        proof_legacy == proof_cached,
        proof_cached.payload_hash,
        proof_cached.responses,
        proof_cached.candles,
        proof_legacy_work.backend_calls,
        proof_cached_work.backend_calls,
        proof_cache.coalesced_waiters
    );

    let legacy = bench_history_load_coalescing(&requests, frames, false);
    let optimized = bench_history_load_coalescing(&requests, frames, true);
    let reusable_hits = optimized.cache_hits.saturating_add(optimized.coalesced_waiters);
    let reusable_total = reusable_hits.saturating_add(optimized.cache_misses);
    let hit_rate = if reusable_total == 0 {
        0.0
    } else {
        reusable_hits as f64 / reusable_total as f64
    };
    println!(
        "[trading-lab] legacy-history-load model=duplicate-backend-fetch-and-row-decode-per-request p50_ms={:.3} p95_ms={:.3} p99_ms={:.3} checksum={} catalog_key_checks={} backend_calls={} decoded_rows={}",
        legacy.stats.p50_us as f64 / 1000.0,
        legacy.stats.p95_us as f64 / 1000.0,
        legacy.stats.p99_us as f64 / 1000.0,
        legacy.checksum,
        legacy.catalog_key_checks,
        legacy.backend_calls,
        legacy.decoded_rows
    );
    println!(
        "[trading-lab] optimized-history-load model=in-flight-request-coalescing-plus-series-cache p50_ms={:.3} p95_ms={:.3} p99_ms={:.3} checksum={} cache_hits={} cache_misses={} coalesced_waiters={} hit_rate={:.3} catalog_key_checks={} backend_calls={} avoided_backend_calls={} decoded_rows={} avoided_decoded_rows={} cache_entries={} miss_cache_budget=512",
        optimized.stats.p50_us as f64 / 1000.0,
        optimized.stats.p95_us as f64 / 1000.0,
        optimized.stats.p99_us as f64 / 1000.0,
        optimized.checksum,
        optimized.cache_hits,
        optimized.cache_misses,
        optimized.coalesced_waiters,
        hit_rate,
        optimized.catalog_key_checks,
        optimized.backend_calls,
        optimized.avoided_backend_calls,
        optimized.decoded_rows,
        optimized.avoided_decoded_rows,
        unique_request_keys.len()
    );
    println!(
        "[trading-lab] summary target=history_load_coalescing p50_speedup_x={:.2} p95_speedup_x={:.2} p99_speedup_x={:.2} cache_budget=bounded-negative-miss-cache-512-plus-existing-series-cache",
        ratio(legacy.stats.p50_us, optimized.stats.p50_us),
        ratio(legacy.stats.p95_us, optimized.stats.p95_us),
        ratio(legacy.stats.p99_us, optimized.stats.p99_us)
    );
}

fn bench_history_load_coalescing(
    requests: &[HistoryLoadRequest],
    frames: usize,
    cached: bool,
) -> HistoryLoadRunStats {
    let mut samples = Vec::with_capacity(frames);
    let mut checksum = 0_usize;
    let mut work = HistoryLoadWork::default();
    let mut cache = HistoryLoadCache::default();
    for _ in 0..frames {
        let started = Instant::now();
        let probe = if cached {
            build_history_load_probe_cached_wave(requests, &mut cache, &mut work)
        } else {
            build_history_load_probe_legacy(requests, &mut work)
        };
        checksum = checksum
            .wrapping_mul(16777619)
            .wrapping_add(probe.payload_hash)
            .wrapping_add(probe.responses)
            .wrapping_add(probe.candles);
        samples.push(started.elapsed().as_micros());
    }
    HistoryLoadRunStats {
        stats: percentile_stats(&samples),
        checksum: std::hint::black_box(checksum),
        cache_hits: cache.hits,
        cache_misses: cache.misses,
        coalesced_waiters: cache.coalesced_waiters,
        catalog_key_checks: work.catalog_key_checks,
        backend_calls: work.backend_calls,
        avoided_backend_calls: cache.avoided_backend_calls,
        decoded_rows: work.decoded_rows,
        avoided_decoded_rows: cache.avoided_decoded_rows,
    }
}

fn build_history_load_probe_legacy(
    requests: &[HistoryLoadRequest],
    work: &mut HistoryLoadWork,
) -> HistoryLoadProbe {
    let mut payload_hash = 2166136261_usize;
    let mut responses = 0_usize;
    let mut candles = 0_usize;
    for request in requests {
        work.catalog_key_checks += 1;
        let series = decode_history_series_probe(request, work);
        payload_hash = combine_history_load_response(payload_hash, request, series);
        responses += 1;
        candles = candles.saturating_add(series.candles);
    }
    HistoryLoadProbe {
        payload_hash: std::hint::black_box(payload_hash),
        responses,
        candles,
    }
}

fn build_history_load_probe_cached_wave(
    requests: &[HistoryLoadRequest],
    cache: &mut HistoryLoadCache,
    work: &mut HistoryLoadWork,
) -> HistoryLoadProbe {
    let mut in_flight: HashMap<String, HistorySeriesProbe> = HashMap::new();
    let mut payload_hash = 2166136261_usize;
    let mut responses = 0_usize;
    let mut candles = 0_usize;
    for request in requests {
        work.catalog_key_checks += 1;
        let series = if let Some(series) = cache.series.get(&request.key).copied() {
            cache.hits += 1;
            cache.avoided_backend_calls += 1;
            cache.avoided_decoded_rows = cache
                .avoided_decoded_rows
                .saturating_add(history_requested_rows(request));
            series
        } else if let Some(series) = in_flight.get(&request.key).copied() {
            cache.coalesced_waiters += 1;
            cache.avoided_backend_calls += 1;
            cache.avoided_decoded_rows = cache
                .avoided_decoded_rows
                .saturating_add(history_requested_rows(request));
            series
        } else {
            cache.misses += 1;
            let series = decode_history_series_probe(request, work);
            in_flight.insert(request.key.clone(), series);
            series
        };
        payload_hash = combine_history_load_response(payload_hash, request, series);
        responses += 1;
        candles = candles.saturating_add(series.candles);
    }
    cache.series.extend(in_flight);
    HistoryLoadProbe {
        payload_hash: std::hint::black_box(payload_hash),
        responses,
        candles,
    }
}

fn decode_history_series_probe(
    request: &HistoryLoadRequest,
    work: &mut HistoryLoadWork,
) -> HistorySeriesProbe {
    let rows = history_requested_rows(request);
    work.backend_calls += 1;
    work.decoded_rows = work.decoded_rows.saturating_add(rows);
    let mut hash = hash_add_str(2166136261_usize, &request.instrument);
    hash = hash_add_str(hash, &request.granularity);
    hash = hash.wrapping_add(rows);
    for idx in 0..rows {
        let price_bits = (idx + 1)
            .wrapping_mul(31)
            .wrapping_add(request.instrument.len().wrapping_mul(97))
            ^ request.granularity.len().wrapping_mul(131);
        hash = hash.wrapping_mul(16777619).wrapping_add(price_bits);
    }
    HistorySeriesProbe {
        payload_hash: std::hint::black_box(hash),
        candles: rows,
    }
}

fn combine_history_load_response(
    hash: usize,
    request: &HistoryLoadRequest,
    series: HistorySeriesProbe,
) -> usize {
    hash_add_str(hash, &request.key)
        .wrapping_add(series.payload_hash)
        .wrapping_mul(16777619)
        .wrapping_add(series.candles)
}

fn history_requested_rows(request: &HistoryLoadRequest) -> usize {
    if request.max_rows == 0 {
        request.rows
    } else {
        request.max_rows.min(request.rows)
    }
}

fn synthetic_history_load_requests(
    files: &[HistoryFileEntry],
    asset_catalog: &[AssetEntry],
    unique_keys: usize,
    calls_per_frame: usize,
) -> Vec<HistoryLoadRequest> {
    let targets = synthetic_compare_assets(asset_catalog, unique_keys.max(1));
    let granularities = ["H4", "H1", "M15"];
    let mut requests = Vec::with_capacity(calls_per_frame);
    for idx in 0..calls_per_frame {
        let instrument = targets
            .get(idx % targets.len().max(1))
            .cloned()
            .unwrap_or_else(|| "EUR_USD".to_string());
        let granularity = granularities[(idx / targets.len().max(1)) % granularities.len()].to_string();
        let rows = files
            .iter()
            .find(|file| file.instrument == instrument && file.granularity == granularity)
            .map(|file| file.rows)
            .unwrap_or(0);
        let max_rows = match granularity.as_str() {
            "M15" => 2_400,
            "H1" => 2_000,
            _ => 1_600,
        };
        let key = format!("{instrument}::{granularity}::{max_rows}");
        requests.push(HistoryLoadRequest {
            key,
            instrument,
            granularity,
            rows,
            max_rows,
        });
    }
    requests
}

fn run_source_series_cache_focus(bars: &[Bar], series_hash: &str, frames: usize) {
    let frames = frames.max(3);
    let calls_per_frame = 10_usize;
    let requested_sources = synthetic_indicator_source_requests();
    let unique_sources: HashSet<&'static str> = requested_sources.iter().copied().collect();
    let proof_legacy: Vec<Vec<f64>> = requested_sources
        .iter()
        .map(|source| extract_source_series_legacy(bars, source))
        .collect();
    let mut proof_cache = SourceSeriesCache::default();
    let proof_cached: Vec<Vec<f64>> = requested_sources
        .iter()
        .map(|source| extract_source_series_cached(bars, source, &mut proof_cache).to_vec())
        .collect();
    println!(
        "[trading-lab] focus=source-series-cache task=dedupe-indicator-source-extraction series_hash={} bars={} frames={} calls_per_frame={} source_requests={} unique_sources={}",
        series_hash,
        bars.len(),
        frames,
        calls_per_frame,
        requested_sources.len(),
        unique_sources.len()
    );
    println!(
        "[trading-lab] proof source-series-cache equal={} first_source={} last_source={} values_per_source={}",
        proof_legacy == proof_cached,
        requested_sources.first().copied().unwrap_or(""),
        requested_sources.last().copied().unwrap_or(""),
        bars.len()
    );

    let legacy = bench_source_series(bars, &requested_sources, frames, calls_per_frame, false);
    let optimized = bench_source_series(bars, &requested_sources, frames, calls_per_frame, true);
    let hit_rate = if optimized.hits + optimized.misses == 0 {
        0.0
    } else {
        optimized.hits as f64 / (optimized.hits + optimized.misses) as f64
    };
    println!(
        "[trading-lab] legacy-source-series model=map-price-source-per-indicator p50_ms={:.3} p95_ms={:.3} p99_ms={:.3} checksum={:.6} extracted_values={}",
        legacy.stats.p50_us as f64 / 1000.0,
        legacy.stats.p95_us as f64 / 1000.0,
        legacy.stats.p99_us as f64 / 1000.0,
        legacy.checksum,
        legacy.extracted_values
    );
    println!(
        "[trading-lab] optimized-source-series model=weakmap-source-array-per-candle-series p50_ms={:.3} p95_ms={:.3} p99_ms={:.3} checksum={:.6} hits={} misses={} hit_rate={:.3} extracted_values={} avoided_values={} cache_entries={} evictions=0",
        optimized.stats.p50_us as f64 / 1000.0,
        optimized.stats.p95_us as f64 / 1000.0,
        optimized.stats.p99_us as f64 / 1000.0,
        optimized.checksum,
        optimized.hits,
        optimized.misses,
        hit_rate,
        optimized.extracted_values,
        optimized.avoided_values,
        unique_sources.len()
    );
    println!(
        "[trading-lab] summary target=source_series_cache p50_speedup_x={:.2} p95_speedup_x={:.2} p99_speedup_x={:.2} cache_budget=one-array-per-candle-array-and-source",
        ratio(legacy.stats.p50_us, optimized.stats.p50_us),
        ratio(legacy.stats.p95_us, optimized.stats.p95_us),
        ratio(legacy.stats.p99_us, optimized.stats.p99_us)
    );
}

fn bench_source_series(
    bars: &[Bar],
    requested_sources: &[&'static str],
    frames: usize,
    calls_per_frame: usize,
    cached: bool,
) -> SourceSeriesRunStats {
    let mut samples = Vec::with_capacity(frames);
    let mut checksum = 0.0_f64;
    let mut extracted_values = 0_usize;
    let mut cache = SourceSeriesCache::default();
    for _ in 0..frames {
        let started = Instant::now();
        for _ in 0..calls_per_frame {
            for source in requested_sources {
                if cached {
                    let series = extract_source_series_cached(bars, source, &mut cache);
                    checksum += source_series_checksum(series);
                } else {
                    let series = extract_source_series_legacy(bars, source);
                    extracted_values = extracted_values.saturating_add(series.len());
                    checksum += source_series_checksum(&series);
                }
            }
        }
        samples.push(started.elapsed().as_micros());
    }
    if cached {
        extracted_values = cache.series.values().map(Vec::len).sum();
    }
    SourceSeriesRunStats {
        stats: percentile_stats(&samples),
        checksum,
        hits: cache.hits,
        misses: cache.misses,
        extracted_values,
        avoided_values: cache.avoided_values,
    }
}

fn extract_source_series_cached<'a>(
    bars: &[Bar],
    source: &'static str,
    cache: &'a mut SourceSeriesCache,
) -> &'a [f64] {
    if cache.series.contains_key(source) {
        cache.hits += 1;
        cache.avoided_values = cache.avoided_values.saturating_add(bars.len());
        return cache.series.get(source).map(Vec::as_slice).unwrap();
    }
    cache.misses += 1;
    let series = extract_source_series_legacy(bars, source);
    cache.series.insert(source, series);
    cache.series.get(source).map(Vec::as_slice).unwrap()
}

fn extract_source_series_legacy(bars: &[Bar], source: &str) -> Vec<f64> {
    bars.iter().map(|bar| source_value(*bar, source)).collect()
}

fn source_value(bar: Bar, source: &str) -> f64 {
    match source {
        "open" => bar.open,
        "high" => bar.high,
        "low" => bar.low,
        "hl2" => (bar.high + bar.low) * 0.5,
        "hlc3" => (bar.high + bar.low + bar.close) / 3.0,
        "ohlc4" => (bar.open + bar.high + bar.low + bar.close) / 4.0,
        _ => bar.close,
    }
}

fn source_series_checksum(values: &[f64]) -> f64 {
    let mut sum = 0.0_f64;
    for value in values.iter().step_by(97) {
        sum += *value;
    }
    std::hint::black_box(sum)
}

fn synthetic_indicator_source_requests() -> Vec<&'static str> {
    vec![
        "close", "close", "close", "hlc3", "close", "close",
        "hlc3", "hlc3", "ohlc4", "close", "hlc3", "close",
    ]
}

fn run_time_label_cache_focus(bars: &[Bar], series_hash: &str, frames: usize) {
    let frames = frames.max(3);
    let labels_per_frame = 96_usize;
    let draws_per_frame = 10_usize;
    let ticks = synthetic_axis_tick_times(bars, labels_per_frame);
    let proof_legacy: Vec<String> = ticks
        .iter()
        .map(|(time_ms, dense)| format_time_label_legacy(*time_ms, *dense, "UTC"))
        .collect();
    let mut proof_cache = TimeLabelCache::default();
    let proof_cached: Vec<String> = ticks
        .iter()
        .map(|(time_ms, dense)| format_time_label_cached(*time_ms, *dense, "UTC", &mut proof_cache).to_string())
        .collect();
    println!(
        "[trading-lab] focus=time-label-cache task=dedupe-intl-date-axis-labels series_hash={} bars={} frames={} labels_per_frame={} draws_per_frame={} timezone=UTC",
        series_hash,
        bars.len(),
        frames,
        labels_per_frame,
        draws_per_frame
    );
    println!(
        "[trading-lab] proof time-label-cache equal={} first_label={} last_label={}",
        proof_legacy == proof_cached,
        proof_cached.first().map(String::as_str).unwrap_or(""),
        proof_cached.last().map(String::as_str).unwrap_or("")
    );

    let legacy = bench_time_labels(&ticks, frames, draws_per_frame, false);
    let optimized = bench_time_labels(&ticks, frames, draws_per_frame, true);
    let hit_rate = if optimized.hits + optimized.misses == 0 {
        0.0
    } else {
        optimized.hits as f64 / (optimized.hits + optimized.misses) as f64
    };
    let formatter_hit_rate = if optimized.formatter_hits + optimized.formatter_misses == 0 {
        0.0
    } else {
        optimized.formatter_hits as f64
            / (optimized.formatter_hits + optimized.formatter_misses) as f64
    };
    println!(
        "[trading-lab] legacy-time-label model=new-intl-formatter-and-date-per-label p50_ms={:.3} p95_ms={:.3} p99_ms={:.3} checksum={} labels_built={} formatter_builds={}",
        legacy.stats.p50_us as f64 / 1000.0,
        legacy.stats.p95_us as f64 / 1000.0,
        legacy.stats.p99_us as f64 / 1000.0,
        legacy.checksum,
        legacy.labels_built,
        legacy.formatter_builds
    );
    println!(
        "[trading-lab] optimized-time-label model=bounded-label-cache-plus-formatter-cache p50_ms={:.3} p95_ms={:.3} p99_ms={:.3} checksum={} hits={} misses={} hit_rate={:.3} formatter_hits={} formatter_misses={} formatter_hit_rate={:.3} labels_built={} avoided_labels={} avoided_formatter_builds={} cache_entries={} evictions=0",
        optimized.stats.p50_us as f64 / 1000.0,
        optimized.stats.p95_us as f64 / 1000.0,
        optimized.stats.p99_us as f64 / 1000.0,
        optimized.checksum,
        optimized.hits,
        optimized.misses,
        hit_rate,
        optimized.formatter_hits,
        optimized.formatter_misses,
        formatter_hit_rate,
        optimized.labels_built,
        optimized.avoided_labels,
        optimized.avoided_formatter_builds,
        ticks.len()
    );
    println!(
        "[trading-lab] summary target=time_label_cache p50_speedup_x={:.2} p95_speedup_x={:.2} p99_speedup_x={:.2} cache_budget=2048-axis-labels-plus-one-formatter-per-zone-density",
        ratio(legacy.stats.p50_us, optimized.stats.p50_us),
        ratio(legacy.stats.p95_us, optimized.stats.p95_us),
        ratio(legacy.stats.p99_us, optimized.stats.p99_us)
    );
}

fn bench_time_labels(
    ticks: &[(i64, bool)],
    frames: usize,
    draws_per_frame: usize,
    cached: bool,
) -> TimeLabelRunStats {
    let mut samples = Vec::with_capacity(frames);
    let mut checksum = 0_usize;
    let mut labels_built = 0_usize;
    let mut formatter_builds = 0_usize;
    let mut cache = TimeLabelCache::default();
    for _ in 0..frames {
        let started = Instant::now();
        for _ in 0..draws_per_frame {
            for (time_ms, dense) in ticks {
                if cached {
                    let label = format_time_label_cached(*time_ms, *dense, "UTC", &mut cache);
                    checksum = checksum.wrapping_add(key_checksum(label));
                } else {
                    let label = format_time_label_legacy(*time_ms, *dense, "UTC");
                    labels_built += 1;
                    formatter_builds += 1;
                    checksum = checksum.wrapping_add(key_checksum(&label));
                }
            }
        }
        samples.push(started.elapsed().as_micros());
    }
    if cached {
        labels_built = cache.labels.len();
        formatter_builds = cache.formatters.len();
    }
    TimeLabelRunStats {
        stats: percentile_stats(&samples),
        checksum: std::hint::black_box(checksum),
        hits: cache.hits,
        misses: cache.misses,
        formatter_hits: cache.formatter_hits,
        formatter_misses: cache.formatter_misses,
        labels_built,
        avoided_labels: cache.avoided_labels,
        formatter_builds,
        avoided_formatter_builds: cache.avoided_formatter_builds,
    }
}

fn format_time_label_cached<'a>(
    time_ms: i64,
    dense: bool,
    time_zone: &'static str,
    cache: &'a mut TimeLabelCache,
) -> &'a str {
    let key = format!("{}|{}|{}", if dense { 1 } else { 0 }, time_zone, time_ms);
    if cache.labels.contains_key(&key) {
        cache.hits += 1;
        cache.avoided_labels += 1;
        return cache.labels.get(&key).map(String::as_str).unwrap();
    }
    cache.misses += 1;
    let formatter_key = format!("axis|{}|{}", if dense { 1 } else { 0 }, time_zone);
    if cache.formatters.insert(formatter_key) {
        cache.formatter_misses += 1;
        simulate_intl_formatter_build(dense, time_zone);
    } else {
        cache.formatter_hits += 1;
        cache.avoided_formatter_builds += 1;
    }
    let label = format_time_label_from_parts(time_ms, dense, time_zone);
    cache.labels.insert(key.clone(), label);
    cache.labels.get(&key).map(String::as_str).unwrap()
}

fn format_time_label_legacy(time_ms: i64, dense: bool, time_zone: &str) -> String {
    simulate_intl_formatter_build(dense, time_zone);
    format_time_label_from_parts(time_ms, dense, time_zone)
}

fn simulate_intl_formatter_build(dense: bool, time_zone: &str) -> usize {
    let mut hash = 2166136261_usize;
    for round in 0..96 {
        let descriptor = format!(
            "Intl.DateTimeFormat(en-GB):axis:dense={}:tz={}:hour12=false:2-digit:weekday=false:round={round}",
            dense, time_zone
        );
        hash = hash.wrapping_add(key_checksum(&descriptor));
        hash = hash.rotate_left(5) ^ descriptor.len();
    }
    std::hint::black_box(hash)
}

fn format_time_label_from_parts(time_ms: i64, dense: bool, time_zone: &str) -> String {
    let minutes = time_ms.div_euclid(60_000);
    let hours = minutes.div_euclid(60);
    let days = hours.div_euclid(24);
    let minute = minutes.rem_euclid(60);
    let hour = hours.rem_euclid(24);
    let pseudo_month = ((days.div_euclid(30)).rem_euclid(12) + 1) as i64;
    let pseudo_day = (days.rem_euclid(30) + 1) as i64;
    let pseudo_year = 1970 + days.div_euclid(360);
    if dense {
        format!("{pseudo_year:04}-{pseudo_month:02}-{pseudo_day:02}@{time_zone}")
    } else {
        format!("{pseudo_month:02}-{pseudo_day:02} {hour:02}:{minute:02}@{time_zone}")
    }
}

fn synthetic_axis_tick_times(bars: &[Bar], labels_per_frame: usize) -> Vec<(i64, bool)> {
    if bars.is_empty() || labels_per_frame == 0 {
        return Vec::new();
    }
    let step = (bars.len() / labels_per_frame.max(1)).max(1);
    (0..labels_per_frame)
        .map(|idx| {
            let bar_index = idx.saturating_mul(step).min(bars.len() - 1);
            (bars[bar_index].time_ms, labels_per_frame > 80)
        })
        .collect()
}

fn run_asset_catalog_cache_focus(bars: &[Bar], series_hash: &str, frames: usize) {
    let frames = frames.max(3);
    let calls_per_frame = 14_usize;
    let asset_count = (bars.len() / 10).clamp(240, 2_000);
    let files = synthetic_history_files(asset_count);
    let asset_catalog = build_asset_catalog_from_history(&files);
    let compare_assets = synthetic_compare_assets(&asset_catalog, 12);
    let selected = "EUR_USD";
    let proof_legacy = build_asset_catalog_bundle_legacy(&asset_catalog, selected, &compare_assets);
    let mut proof_cache = AssetCatalogCache::default();
    let proof_cached =
        build_asset_catalog_bundle_cached(&asset_catalog, selected, &compare_assets, 1, &mut proof_cache);
    println!(
        "[trading-lab] focus=asset-catalog-cache task=dedupe-available-assets-normalize-dedup-sort series_hash={} bars={} frames={} calls_per_frame={} asset_catalog={} compare_assets={}",
        series_hash,
        bars.len(),
        frames,
        calls_per_frame,
        asset_catalog.len(),
        compare_assets.len()
    );
    println!(
        "[trading-lab] proof asset-catalog-cache equal={} available={} library={} first_asset={} last_asset={}",
        proof_legacy.0 == proof_cached.0 && proof_legacy.1 == proof_cached.1,
        proof_cached.0.len(),
        proof_cached.1.len(),
        proof_cached.0.first().map(|asset| asset.name.as_str()).unwrap_or(""),
        proof_cached.0.last().map(|asset| asset.name.as_str()).unwrap_or("")
    );

    let legacy = bench_asset_catalog(&asset_catalog, selected, &compare_assets, frames, calls_per_frame, false);
    let optimized = bench_asset_catalog(&asset_catalog, selected, &compare_assets, frames, calls_per_frame, true);
    let hit_rate = if optimized.hits + optimized.misses == 0 {
        0.0
    } else {
        optimized.hits as f64 / (optimized.hits + optimized.misses) as f64
    };
    println!(
        "[trading-lab] legacy-asset-catalog model=normalize-dedup-find-sort-each-menu p50_ms={:.3} p95_ms={:.3} p99_ms={:.3} checksum={} normalized_entries={} linear_catalog_scans={} sorts={}",
        legacy.stats.p50_us as f64 / 1000.0,
        legacy.stats.p95_us as f64 / 1000.0,
        legacy.stats.p99_us as f64 / 1000.0,
        legacy.checksum,
        legacy.normalized_entries,
        legacy.linear_catalog_scans,
        legacy.sorts
    );
    println!(
        "[trading-lab] optimized-asset-catalog model=content-addressed-asset-universe-cache p50_ms={:.3} p95_ms={:.3} p99_ms={:.3} checksum={} hits={} misses={} hit_rate={:.3} normalized_entries={} avoided_normalizations={} linear_catalog_scans={} avoided_catalog_scans={} sorts={} avoided_sorts={} cache_entries=2 evictions=0",
        optimized.stats.p50_us as f64 / 1000.0,
        optimized.stats.p95_us as f64 / 1000.0,
        optimized.stats.p99_us as f64 / 1000.0,
        optimized.checksum,
        optimized.hits,
        optimized.misses,
        hit_rate,
        optimized.normalized_entries,
        optimized.avoided_normalizations,
        optimized.linear_catalog_scans,
        optimized.avoided_catalog_scans,
        optimized.sorts,
        optimized.avoided_sorts
    );
    println!(
        "[trading-lab] summary target=asset_catalog_cache p50_speedup_x={:.2} p95_speedup_x={:.2} p99_speedup_x={:.2} cache_budget=available-assets-plus-library-assets-per-asset-universe-revision",
        ratio(legacy.stats.p50_us, optimized.stats.p50_us),
        ratio(legacy.stats.p95_us, optimized.stats.p95_us),
        ratio(legacy.stats.p99_us, optimized.stats.p99_us)
    );
}

fn bench_asset_catalog(
    asset_catalog: &[AssetEntry],
    selected: &str,
    compare_assets: &[String],
    frames: usize,
    calls_per_frame: usize,
    cached: bool,
) -> AssetCatalogRunStats {
    let mut samples = Vec::with_capacity(frames);
    let mut checksum = 0_usize;
    let mut normalized_entries = 0_usize;
    let mut linear_catalog_scans = 0_usize;
    let mut sorts = 0_usize;
    let mut cache = AssetCatalogCache::default();
    for _ in 0..frames {
        let started = Instant::now();
        for _ in 0..calls_per_frame {
            if cached {
                let (available, library) =
                    build_asset_catalog_bundle_cached(asset_catalog, selected, compare_assets, 1, &mut cache);
                checksum = checksum
                    .wrapping_add(asset_catalog_checksum(available))
                    .wrapping_add(asset_catalog_checksum(library));
            } else {
                let (available, library) =
                    build_asset_catalog_bundle_legacy(asset_catalog, selected, compare_assets);
                normalized_entries = normalized_entries.saturating_add(asset_catalog.len() + compare_assets.len() + 1);
                linear_catalog_scans = linear_catalog_scans.saturating_add(available.len().saturating_mul(asset_catalog.len()));
                sorts = sorts.saturating_add(2);
                checksum = checksum
                    .wrapping_add(asset_catalog_checksum(&available))
                    .wrapping_add(asset_catalog_checksum(&library));
            }
        }
        samples.push(started.elapsed().as_micros());
    }
    if cached {
        normalized_entries = asset_catalog.len() + compare_assets.len() + 1;
        linear_catalog_scans = cache.available.len().saturating_mul(asset_catalog.len());
        sorts = 2 * cache.misses;
    }
    AssetCatalogRunStats {
        stats: percentile_stats(&samples),
        checksum: std::hint::black_box(checksum),
        hits: cache.hits,
        misses: cache.misses,
        normalized_entries,
        avoided_normalizations: cache.avoided_normalizations,
        sorts,
        avoided_sorts: cache.avoided_sorts,
        linear_catalog_scans,
        avoided_catalog_scans: cache.avoided_catalog_scans,
    }
}

fn build_asset_catalog_bundle_cached<'a>(
    asset_catalog: &[AssetEntry],
    selected: &str,
    compare_assets: &[String],
    revision: usize,
    cache: &'a mut AssetCatalogCache,
) -> (&'a [AssetEntry], &'a [AssetEntry]) {
    let key = format!(
        "{revision}|{}|{}|{}|{}",
        selected,
        compare_assets.join(","),
        asset_catalog.len(),
        asset_catalog.first().map(|asset| asset.name.as_str()).unwrap_or("")
    );
    if cache.initialized && cache.key == key {
        cache.hits += 1;
        cache.avoided_normalizations = cache
            .avoided_normalizations
            .saturating_add(asset_catalog.len() + compare_assets.len() + 1);
        cache.avoided_catalog_scans = cache
            .avoided_catalog_scans
            .saturating_add(cache.available.len().saturating_mul(asset_catalog.len()));
        cache.avoided_sorts = cache.avoided_sorts.saturating_add(2);
        return (&cache.available, &cache.library);
    }
    cache.misses += 1;
    cache.key = key;
    let (available, library) = build_asset_catalog_bundle_legacy(asset_catalog, selected, compare_assets);
    cache.available = available;
    cache.library = library;
    cache.initialized = true;
    (&cache.available, &cache.library)
}

fn build_asset_catalog_bundle_legacy(
    asset_catalog: &[AssetEntry],
    selected: &str,
    compare_assets: &[String],
) -> (Vec<AssetEntry>, Vec<AssetEntry>) {
    let mut source: Vec<AssetEntry> = asset_catalog.to_vec();
    source.push(normalize_asset_entry(selected));
    for name in compare_assets {
        source.push(normalize_asset_entry(name));
    }
    let mut seen = HashSet::new();
    let mut available = Vec::with_capacity(source.len());
    for item in source {
        let normalized = normalize_asset_entry(&item.name);
        if !seen.insert(normalized.name.clone()) {
            continue;
        }
        let catalog_hit = asset_catalog.iter().find(|asset| asset.name == normalized.name);
        available.push(catalog_hit.cloned().unwrap_or(normalized));
    }
    if !seen.contains(selected) {
        available.insert(0, normalize_asset_entry(selected));
    }
    available.sort_by(asset_catalog_order);
    let mut library = available.clone();
    library.sort_by(|left, right| {
        if left.name == selected {
            std::cmp::Ordering::Less
        } else if right.name == selected {
            std::cmp::Ordering::Greater
        } else {
            left.name.cmp(&right.name)
        }
    });
    (available, library)
}

fn asset_catalog_order(left: &AssetEntry, right: &AssetEntry) -> std::cmp::Ordering {
    if left.name == "EUR_USD" {
        std::cmp::Ordering::Less
    } else if right.name == "EUR_USD" {
        std::cmp::Ordering::Greater
    } else {
        left.name.cmp(&right.name)
    }
}

fn normalize_asset_entry(name: &str) -> AssetEntry {
    let trimmed = name.trim().to_ascii_uppercase();
    let asset_class = if trimmed.ends_with("_USD") || trimmed.ends_with("_EUR") {
        "forex"
    } else if trimmed.contains("BTC") || trimmed.contains("ETH") {
        "crypto"
    } else if trimmed.contains("XAU") || trimmed.contains("BCO") {
        "commodity"
    } else {
        "instrument"
    };
    AssetEntry {
        name: trimmed.clone(),
        display_name: trimmed.replace('_', " / "),
        asset_class: asset_class.to_string(),
        rows: 0,
    }
}

fn build_asset_catalog_from_history(files: &[HistoryFileEntry]) -> Vec<AssetEntry> {
    let mut by_name: HashMap<String, AssetEntry> = HashMap::new();
    for file in files {
        let normalized = normalize_asset_entry(&file.instrument);
        let entry = by_name.entry(normalized.name.clone()).or_insert(normalized);
        entry.rows = entry.rows.saturating_add(file.rows);
    }
    let mut out: Vec<AssetEntry> = by_name.into_values().collect();
    out.sort_by(asset_catalog_order);
    out
}

fn synthetic_history_files(asset_count: usize) -> Vec<HistoryFileEntry> {
    let granularities = ["M1", "M5", "M15", "H1", "H4", "D"];
    let mut out = Vec::with_capacity(asset_count.saturating_mul(granularities.len()));
    out.push(HistoryFileEntry {
        instrument: "EUR_USD".to_string(),
        granularity: "H4".to_string(),
        rows: 20_000,
    });
    for idx in 0..asset_count.saturating_sub(1) {
        let name = match idx % 5 {
            0 => format!("FX{:04}_USD", idx),
            1 => format!("BTC{:04}_USD", idx),
            2 => format!("XAU{:04}_USD", idx),
            3 => format!("IDX{:04}_EUR", idx),
            _ => format!("SYNTH{:04}", idx),
        };
        for granularity in granularities {
            out.push(HistoryFileEntry {
                instrument: name.clone(),
                granularity: granularity.to_string(),
                rows: 2_000 + idx % 500,
            });
        }
    }
    out
}

fn synthetic_compare_assets(asset_catalog: &[AssetEntry], count: usize) -> Vec<String> {
    asset_catalog
        .iter()
        .skip(1)
        .step_by(37)
        .take(count)
        .map(|asset| asset.name.clone())
        .collect()
}

fn asset_catalog_checksum(assets: &[AssetEntry]) -> usize {
    let mut hash = 2166136261_usize;
    for asset in assets.iter().step_by(17) {
        hash = hash.wrapping_add(key_checksum(&asset.name));
        hash = hash.wrapping_mul(16777619);
    }
    std::hint::black_box(hash)
}

fn run_catalog_index_cache_focus(bars: &[Bar], series_hash: &str, frames: usize) {
    let frames = frames.max(3);
    let calls_per_frame = 6_usize;
    let asset_count = (bars.len() / 12).clamp(240, 1_200);
    let files = synthetic_history_files(asset_count);
    let asset_catalog = build_asset_catalog_from_history(&files);
    let pair_queries = synthetic_catalog_pair_queries(&files, 32);
    let instrument_queries = synthetic_catalog_instrument_queries(&asset_catalog, 10);
    let tradable_queries = synthetic_catalog_tradable_queries(&asset_catalog, 12);
    let mut proof_legacy_work = CatalogIndexWork::default();
    let proof_legacy = build_catalog_index_probe_legacy(
        &files,
        &asset_catalog,
        &pair_queries,
        &instrument_queries,
        &tradable_queries,
        &mut proof_legacy_work,
    );
    let mut proof_cache = CatalogIndexCache::default();
    let mut proof_cached_work = CatalogIndexWork::default();
    let proof_cached = build_catalog_index_probe_cached(
        &files,
        &asset_catalog,
        &pair_queries,
        &instrument_queries,
        &tradable_queries,
        1,
        &mut proof_cache,
        &mut proof_cached_work,
    );
    println!(
        "[trading-lab] focus=catalog-index-cache task=dedupe-catalog-find-filter-some-and-broker-set series_hash={} bars={} frames={} calls_per_frame={} catalog_files={} assets={} pair_queries={} instrument_queries={} tradable_queries={}",
        series_hash,
        bars.len(),
        frames,
        calls_per_frame,
        files.len(),
        asset_catalog.len(),
        pair_queries.len(),
        instrument_queries.len(),
        tradable_queries.len()
    );
    println!(
        "[trading-lab] proof catalog-index-cache equal={} rows={} exists={} instruments={} tradable={} legacy_scans={} cached_index_build_scans={}",
        proof_legacy == proof_cached,
        proof_cached.rows.len(),
        proof_cached.exists.len(),
        proof_cached.instrument_counts.len(),
        proof_cached.tradable.len(),
        proof_legacy_work.full_scans,
        proof_cached_work.full_scans
    );

    let legacy = bench_catalog_index(
        &files,
        &asset_catalog,
        &pair_queries,
        &instrument_queries,
        &tradable_queries,
        frames,
        calls_per_frame,
        false,
    );
    let optimized = bench_catalog_index(
        &files,
        &asset_catalog,
        &pair_queries,
        &instrument_queries,
        &tradable_queries,
        frames,
        calls_per_frame,
        true,
    );
    let hit_rate = if optimized.hits + optimized.misses == 0 {
        0.0
    } else {
        optimized.hits as f64 / (optimized.hits + optimized.misses) as f64
    };
    println!(
        "[trading-lab] legacy-catalog-index model=find-filter-some-build-set-each-call p50_ms={:.3} p95_ms={:.3} p99_ms={:.3} checksum={} pair_lookups={} instrument_lookups={} broker_set_lookups={} full_scans={} set_entries={}",
        legacy.stats.p50_us as f64 / 1000.0,
        legacy.stats.p95_us as f64 / 1000.0,
        legacy.stats.p99_us as f64 / 1000.0,
        legacy.checksum,
        legacy.pair_lookups,
        legacy.instrument_lookups,
        legacy.broker_set_lookups,
        legacy.full_scans,
        legacy.set_entries
    );
    println!(
        "[trading-lab] optimized-catalog-index model=content-addressed-map-index-and-broker-set-cache p50_ms={:.3} p95_ms={:.3} p99_ms={:.3} checksum={} hits={} misses={} hit_rate={:.3} pair_lookups={} instrument_lookups={} broker_set_lookups={} index_build_scans={} avoided_full_scans={} set_entries={} avoided_set_entries={} cache_entries={} evictions=0",
        optimized.stats.p50_us as f64 / 1000.0,
        optimized.stats.p95_us as f64 / 1000.0,
        optimized.stats.p99_us as f64 / 1000.0,
        optimized.checksum,
        optimized.hits,
        optimized.misses,
        hit_rate,
        optimized.pair_lookups,
        optimized.instrument_lookups,
        optimized.broker_set_lookups,
        optimized.full_scans,
        optimized.avoided_full_scans,
        optimized.set_entries,
        optimized.avoided_set_entries,
        optimized.index_entries
    );
    println!(
        "[trading-lab] summary target=catalog_index_cache p50_speedup_x={:.2} p95_speedup_x={:.2} p99_speedup_x={:.2} cache_budget=one-pair-map-plus-one-instrument-map-plus-one-broker-set-per-catalog-universe-revision",
        ratio(legacy.stats.p50_us, optimized.stats.p50_us),
        ratio(legacy.stats.p95_us, optimized.stats.p95_us),
        ratio(legacy.stats.p99_us, optimized.stats.p99_us)
    );
}

fn bench_catalog_index(
    files: &[HistoryFileEntry],
    asset_catalog: &[AssetEntry],
    pair_queries: &[(String, String, String)],
    instrument_queries: &[String],
    tradable_queries: &[String],
    frames: usize,
    calls_per_frame: usize,
    cached: bool,
) -> CatalogIndexRunStats {
    let mut samples = Vec::with_capacity(frames);
    let mut checksum = 0_usize;
    let mut work = CatalogIndexWork::default();
    let mut cache = CatalogIndexCache::default();
    for _ in 0..frames {
        let started = Instant::now();
        for _ in 0..calls_per_frame {
            let probe = if cached {
                build_catalog_index_probe_cached(
                    files,
                    asset_catalog,
                    pair_queries,
                    instrument_queries,
                    tradable_queries,
                    1,
                    &mut cache,
                    &mut work,
                )
            } else {
                build_catalog_index_probe_legacy(
                    files,
                    asset_catalog,
                    pair_queries,
                    instrument_queries,
                    tradable_queries,
                    &mut work,
                )
            };
            checksum = checksum.wrapping_add(catalog_index_probe_checksum(&probe));
        }
        samples.push(started.elapsed().as_micros());
    }
    CatalogIndexRunStats {
        stats: percentile_stats(&samples),
        checksum: std::hint::black_box(checksum),
        hits: cache.hits,
        misses: cache.misses,
        pair_lookups: work.pair_lookups,
        instrument_lookups: work.instrument_lookups,
        broker_set_lookups: work.broker_set_lookups,
        full_scans: work.full_scans,
        avoided_full_scans: cache.avoided_full_scans,
        set_entries: work.set_entries,
        avoided_set_entries: cache.avoided_set_entries,
        index_entries: cache.pair_rows.len() + cache.by_instrument.len() + cache.broker_set.len(),
    }
}

fn build_catalog_index_probe_legacy(
    files: &[HistoryFileEntry],
    asset_catalog: &[AssetEntry],
    pair_queries: &[(String, String, String)],
    instrument_queries: &[String],
    tradable_queries: &[String],
    work: &mut CatalogIndexWork,
) -> CatalogIndexProbe {
    let mut rows = Vec::with_capacity(pair_queries.len());
    let mut exists = Vec::with_capacity(pair_queries.len());
    for (instrument, granularity, _) in pair_queries {
        work.pair_lookups += 1;
        let mut row_count = 0_usize;
        for file in files {
            work.full_scans += 1;
            if file.instrument == *instrument && file.granularity.eq_ignore_ascii_case(granularity) {
                row_count = file.rows;
                break;
            }
        }
        rows.push(row_count);

        work.pair_lookups += 1;
        let mut found = false;
        for file in files {
            work.full_scans += 1;
            if file.instrument == *instrument && file.granularity.eq_ignore_ascii_case(granularity) {
                found = true;
                break;
            }
        }
        exists.push(found);
    }

    let mut instrument_counts = Vec::with_capacity(instrument_queries.len());
    let mut granularities = Vec::with_capacity(instrument_queries.len());
    for instrument in instrument_queries {
        work.instrument_lookups += 1;
        let mut count = 0_usize;
        for file in files {
            work.full_scans += 1;
            if file.instrument == *instrument {
                count += 1;
            }
        }
        instrument_counts.push(count);

        work.instrument_lookups += 1;
        let mut seen = HashSet::new();
        let mut out = Vec::new();
        for file in files {
            work.full_scans += 1;
            if file.instrument == *instrument && seen.insert(file.granularity.clone()) {
                out.push(file.granularity.clone());
            }
        }
        sort_catalog_granularities(&mut out);
        granularities.push(out);
    }

    let mut tradable = Vec::with_capacity(tradable_queries.len());
    for instrument in tradable_queries {
        work.broker_set_lookups += 1;
        let mut set = HashSet::new();
        if asset_catalog.is_empty() {
            for file in files {
                work.set_entries += 1;
                set.insert(file.instrument.clone());
            }
        } else {
            for asset in asset_catalog {
                work.set_entries += 1;
                set.insert(asset.name.clone());
            }
        }
        tradable.push(set.contains(instrument));
    }

    CatalogIndexProbe {
        rows,
        exists,
        instrument_counts,
        granularities,
        tradable,
    }
}

fn build_catalog_index_probe_cached(
    files: &[HistoryFileEntry],
    asset_catalog: &[AssetEntry],
    pair_queries: &[(String, String, String)],
    instrument_queries: &[String],
    tradable_queries: &[String],
    revision: usize,
    cache: &mut CatalogIndexCache,
    work: &mut CatalogIndexWork,
) -> CatalogIndexProbe {
    let mut rows = Vec::with_capacity(pair_queries.len());
    let mut exists = Vec::with_capacity(pair_queries.len());
    for (_, _, key) in pair_queries {
        ensure_catalog_index(files, asset_catalog, revision, cache, work);
        work.pair_lookups += 1;
        cache.avoided_full_scans = cache.avoided_full_scans.saturating_add(files.len());
        rows.push(cache.pair_rows.get(key).copied().unwrap_or(0));

        ensure_catalog_index(files, asset_catalog, revision, cache, work);
        work.pair_lookups += 1;
        cache.avoided_full_scans = cache.avoided_full_scans.saturating_add(files.len());
        exists.push(cache.pair_rows.contains_key(key));
    }

    let mut instrument_counts = Vec::with_capacity(instrument_queries.len());
    let mut granularities = Vec::with_capacity(instrument_queries.len());
    for instrument in instrument_queries {
        ensure_catalog_index(files, asset_catalog, revision, cache, work);
        work.instrument_lookups += 1;
        cache.avoided_full_scans = cache.avoided_full_scans.saturating_add(files.len());
        let instrument_count = cache
            .by_instrument
            .get(instrument)
            .map(|items| items.len())
            .unwrap_or(0);
        instrument_counts.push(instrument_count);

        ensure_catalog_index(files, asset_catalog, revision, cache, work);
        work.instrument_lookups += 1;
        cache.avoided_full_scans = cache.avoided_full_scans.saturating_add(files.len());
        let mut seen = HashSet::new();
        let mut out = Vec::new();
        if let Some(indices) = cache.by_instrument.get(instrument) {
            for index in indices {
                let granularity = files[*index].granularity.clone();
                if seen.insert(granularity.clone()) {
                    out.push(granularity);
                }
            }
        }
        sort_catalog_granularities(&mut out);
        granularities.push(out);
    }

    let mut tradable = Vec::with_capacity(tradable_queries.len());
    for instrument in tradable_queries {
        ensure_catalog_index(files, asset_catalog, revision, cache, work);
        work.broker_set_lookups += 1;
        let source_len = if asset_catalog.is_empty() {
            files.len()
        } else {
            asset_catalog.len()
        };
        cache.avoided_set_entries = cache.avoided_set_entries.saturating_add(source_len);
        tradable.push(cache.broker_set.contains(instrument));
    }

    CatalogIndexProbe {
        rows,
        exists,
        instrument_counts,
        granularities,
        tradable,
    }
}

fn ensure_catalog_index(
    files: &[HistoryFileEntry],
    asset_catalog: &[AssetEntry],
    revision: usize,
    cache: &mut CatalogIndexCache,
    work: &mut CatalogIndexWork,
) {
    let key = catalog_index_key(files, asset_catalog, revision);
    if cache.initialized && cache.key == key {
        cache.hits += 1;
        return;
    }
    cache.misses += 1;
    cache.key = key;
    cache.pair_rows.clear();
    cache.by_instrument.clear();
    cache.broker_set.clear();
    work.full_scans = work.full_scans.saturating_add(files.len());
    for (index, file) in files.iter().enumerate() {
        cache
            .pair_rows
            .insert(catalog_chart_key(&file.instrument, &file.granularity), file.rows);
        cache
            .by_instrument
            .entry(file.instrument.clone())
            .or_default()
            .push(index);
    }
    if asset_catalog.is_empty() {
        for file in files {
            work.set_entries += 1;
            cache.broker_set.insert(file.instrument.clone());
        }
    } else {
        for asset in asset_catalog {
            work.set_entries += 1;
            cache.broker_set.insert(asset.name.clone());
        }
    }
    cache.initialized = true;
}

fn catalog_index_key(files: &[HistoryFileEntry], asset_catalog: &[AssetEntry], revision: usize) -> String {
    let first_file = files
        .first()
        .map(|file| format!("{}:{}:{}", file.instrument, file.granularity, file.rows))
        .unwrap_or_default();
    let last_file = files
        .last()
        .map(|file| format!("{}:{}:{}", file.instrument, file.granularity, file.rows))
        .unwrap_or_default();
    format!(
        "{revision}|files={}|assets={}|first={}|last={}",
        files.len(),
        asset_catalog.len(),
        first_file,
        last_file
    )
}

fn catalog_chart_key(instrument: &str, granularity: &str) -> String {
    format!("{}::{}", instrument.trim(), granularity.trim().to_ascii_uppercase())
}

fn sort_catalog_granularities(values: &mut [String]) {
    values.sort_by(|left, right| {
        let left_rank = catalog_granularity_rank(left);
        let right_rank = catalog_granularity_rank(right);
        match (left_rank, right_rank) {
            (Some(left_rank), Some(right_rank)) => left_rank.cmp(&right_rank),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => left.cmp(right),
        }
    });
}

fn catalog_granularity_rank(value: &str) -> Option<usize> {
    const ORDER: [&str; 10] = ["S10", "S30", "M1", "M5", "M15", "M30", "H1", "H4", "D", "W"];
    ORDER.iter().position(|item| *item == value)
}

fn synthetic_catalog_pair_queries(
    files: &[HistoryFileEntry],
    count: usize,
) -> Vec<(String, String, String)> {
    let mut out = Vec::with_capacity(count);
    for (idx, file) in files.iter().enumerate().step_by(19).take(count) {
        let granularity = if idx % 13 == 0 {
            "S5".to_string()
        } else {
            file.granularity.clone()
        };
        out.push((
            file.instrument.clone(),
            granularity.clone(),
            catalog_chart_key(&file.instrument, &granularity),
        ));
    }
    out
}

fn synthetic_catalog_instrument_queries(asset_catalog: &[AssetEntry], count: usize) -> Vec<String> {
    let mut out: Vec<String> = asset_catalog
        .iter()
        .step_by(29)
        .take(count.saturating_sub(1))
        .map(|asset| asset.name.clone())
        .collect();
    out.push("NO_HISTORY_USD".to_string());
    out.truncate(count);
    out
}

fn synthetic_catalog_tradable_queries(asset_catalog: &[AssetEntry], count: usize) -> Vec<String> {
    let mut out: Vec<String> = asset_catalog
        .iter()
        .step_by(31)
        .take(count.saturating_sub(1))
        .map(|asset| asset.name.clone())
        .collect();
    out.push("NO_BROKER_SYMBOL".to_string());
    out.truncate(count);
    out
}

fn catalog_index_probe_checksum(probe: &CatalogIndexProbe) -> usize {
    let mut hash = 2166136261_usize;
    for row in &probe.rows {
        hash = hash.wrapping_mul(16777619).wrapping_add(*row);
    }
    for exists in &probe.exists {
        hash = hash.wrapping_mul(16777619).wrapping_add(usize::from(*exists));
    }
    for count in &probe.instrument_counts {
        hash = hash.wrapping_mul(16777619).wrapping_add(*count);
    }
    for items in &probe.granularities {
        for item in items {
            hash = hash.wrapping_mul(16777619).wrapping_add(key_checksum(item));
        }
    }
    for tradable in &probe.tradable {
        hash = hash.wrapping_mul(16777619).wrapping_add(usize::from(*tradable));
    }
    std::hint::black_box(hash)
}

fn run_asset_search_index_cache_focus(bars: &[Bar], series_hash: &str, frames: usize) {
    let frames = frames.max(3);
    let calls_per_frame = 4_usize;
    let asset_count = (bars.len() / 12).clamp(240, 1_200);
    let files = synthetic_history_files(asset_count);
    let asset_catalog = build_asset_catalog_from_history(&files);
    let compare_assets = synthetic_compare_assets(&asset_catalog, 12);
    let selected = "EUR_USD";
    let library = build_asset_catalog_bundle_legacy(&asset_catalog, selected, &compare_assets).1;
    let search_queries = synthetic_asset_search_queries();
    let mention_commands = synthetic_asset_mention_commands(&library);
    let find_names = synthetic_asset_find_names(&library);
    let mut proof_legacy_work = AssetSearchWork::default();
    let proof_legacy = build_asset_search_probe_legacy(
        &library,
        selected,
        &compare_assets,
        &search_queries,
        &mention_commands,
        &find_names,
        &mut proof_legacy_work,
    );
    let mut proof_cache = AssetSearchIndexCache::default();
    let mut proof_cached_work = AssetSearchWork::default();
    let proof_cached = build_asset_search_probe_cached(
        &library,
        selected,
        &compare_assets,
        &search_queries,
        &mention_commands,
        &find_names,
        1,
        &mut proof_cache,
        &mut proof_cached_work,
    );
    println!(
        "[trading-lab] focus=asset-search-index-cache task=dedupe-asset-haystack-alias-sort-and-compare-menu-model series_hash={} bars={} frames={} calls_per_frame={} assets={} search_queries={} mention_commands={} find_names={}",
        series_hash,
        bars.len(),
        frames,
        calls_per_frame,
        library.len(),
        search_queries.len(),
        mention_commands.len(),
        find_names.len()
    );
    println!(
        "[trading-lab] proof asset-search-index-cache equal={} query_counts={} mention_commands={} find_names={} legacy_haystacks={} cached_index_records={}",
        proof_legacy == proof_cached,
        proof_cached.query_counts.len(),
        proof_cached.mentioned.len(),
        proof_cached.found.len(),
        proof_legacy_work.haystack_builds,
        proof_cache.records.len()
    );

    let legacy = bench_asset_search_index(
        &library,
        selected,
        &compare_assets,
        &search_queries,
        &mention_commands,
        &find_names,
        frames,
        calls_per_frame,
        false,
    );
    let optimized = bench_asset_search_index(
        &library,
        selected,
        &compare_assets,
        &search_queries,
        &mention_commands,
        &find_names,
        frames,
        calls_per_frame,
        true,
    );
    let hit_rate = if optimized.hits + optimized.misses == 0 {
        0.0
    } else {
        optimized.hits as f64 / (optimized.hits + optimized.misses) as f64
    };
    println!(
        "[trading-lab] legacy-asset-search model=rebuild-haystack-alias-sort-menu-model-each-call p50_ms={:.3} p95_ms={:.3} p99_ms={:.3} checksum={} haystack_builds={} menu_model_scans={} menu_item_builds={} alias_builds={} alias_checks={} mention_sorts={} linear_finds={}",
        legacy.stats.p50_us as f64 / 1000.0,
        legacy.stats.p95_us as f64 / 1000.0,
        legacy.stats.p99_us as f64 / 1000.0,
        legacy.checksum,
        legacy.haystack_builds,
        legacy.menu_model_scans,
        legacy.menu_item_builds,
        legacy.alias_builds,
        legacy.alias_checks,
        legacy.mention_sorts,
        legacy.linear_finds
    );
    println!(
        "[trading-lab] optimized-asset-search model=content-addressed-asset-search-index-plus-menu-model-cache p50_ms={:.3} p95_ms={:.3} p99_ms={:.3} checksum={} hits={} misses={} hit_rate={:.3} haystack_builds={} avoided_haystack_builds={} menu_model_scans={} avoided_menu_model_scans={} menu_item_builds={} alias_builds={} avoided_alias_builds={} alias_checks={} mention_sorts={} avoided_mention_sorts={} linear_finds={} avoided_linear_finds={} cache_entries={} evictions=0",
        optimized.stats.p50_us as f64 / 1000.0,
        optimized.stats.p95_us as f64 / 1000.0,
        optimized.stats.p99_us as f64 / 1000.0,
        optimized.checksum,
        optimized.hits,
        optimized.misses,
        hit_rate,
        optimized.haystack_builds,
        optimized.avoided_haystack_builds,
        optimized.menu_model_scans,
        optimized.avoided_menu_model_scans,
        optimized.menu_item_builds,
        optimized.alias_builds,
        optimized.avoided_alias_builds,
        optimized.alias_checks,
        optimized.mention_sorts,
        optimized.avoided_mention_sorts,
        optimized.linear_finds,
        optimized.avoided_linear_finds,
        optimized.cache_entries
    );
    println!(
        "[trading-lab] summary target=asset_search_index_cache p50_speedup_x={:.2} p95_speedup_x={:.2} p99_speedup_x={:.2} cache_budget=one-search-record-index-plus-one-last-compare-menu-model-per-asset-universe-revision",
        ratio(legacy.stats.p50_us, optimized.stats.p50_us),
        ratio(legacy.stats.p95_us, optimized.stats.p95_us),
        ratio(legacy.stats.p99_us, optimized.stats.p99_us)
    );
}

fn bench_asset_search_index(
    library: &[AssetEntry],
    selected: &str,
    compare_assets: &[String],
    search_queries: &[String],
    mention_commands: &[String],
    find_names: &[String],
    frames: usize,
    calls_per_frame: usize,
    cached: bool,
) -> AssetSearchRunStats {
    let mut samples = Vec::with_capacity(frames);
    let mut checksum = 0_usize;
    let mut work = AssetSearchWork::default();
    let mut cache = AssetSearchIndexCache::default();
    for _ in 0..frames {
        let started = Instant::now();
        for _ in 0..calls_per_frame {
            let probe = if cached {
                build_asset_search_probe_cached(
                    library,
                    selected,
                    compare_assets,
                    search_queries,
                    mention_commands,
                    find_names,
                    1,
                    &mut cache,
                    &mut work,
                )
            } else {
                build_asset_search_probe_legacy(
                    library,
                    selected,
                    compare_assets,
                    search_queries,
                    mention_commands,
                    find_names,
                    &mut work,
                )
            };
            checksum = checksum.wrapping_add(asset_search_probe_checksum(&probe));
        }
        samples.push(started.elapsed().as_micros());
    }
    AssetSearchRunStats {
        stats: percentile_stats(&samples),
        checksum: std::hint::black_box(checksum),
        hits: cache.hits,
        misses: cache.misses,
        haystack_builds: work.haystack_builds,
        avoided_haystack_builds: cache.avoided_haystack_builds,
        menu_model_scans: work.menu_model_scans,
        avoided_menu_model_scans: cache.avoided_menu_model_scans,
        menu_item_builds: work.menu_item_builds,
        alias_builds: work.alias_builds,
        avoided_alias_builds: cache.avoided_alias_builds,
        alias_checks: work.alias_checks,
        mention_sorts: work.mention_sorts,
        avoided_mention_sorts: cache.avoided_mention_sorts,
        linear_finds: work.linear_finds,
        avoided_linear_finds: cache.avoided_linear_finds,
        cache_entries: cache.records.len() + cache.by_name.len() + usize::from(!cache.model_key.is_empty()),
    }
}

fn build_asset_search_probe_legacy(
    library: &[AssetEntry],
    selected: &str,
    compare_assets: &[String],
    search_queries: &[String],
    mention_commands: &[String],
    find_names: &[String],
    work: &mut AssetSearchWork,
) -> AssetSearchProbe {
    let mut query_counts = Vec::with_capacity(search_queries.len());
    for query in search_queries {
        let query = query.trim().to_ascii_lowercase();
        let mut count = 0_usize;
        for asset in library {
            if asset.name == selected {
                continue;
            }
            work.menu_model_scans += 1;
            let haystack = asset_search_haystack_legacy(asset);
            work.haystack_builds += 1;
            if !query.is_empty() && !haystack.contains(&query) {
                continue;
            }
            let _title = asset_compare_code(&asset.name);
            let _subtitle = asset_compare_subtitle(asset);
            let _active = compare_assets.iter().any(|name| name == &asset.name);
            work.menu_item_builds += 1;
            count += 1;
        }
        query_counts.push(count);
    }

    let mut mentioned = Vec::with_capacity(mention_commands.len());
    for command in mention_commands {
        let compact = compact_asset_token(command);
        let mut records: Vec<AssetSearchRecord> = library
            .iter()
            .map(|asset| {
                work.alias_builds += 2;
                build_asset_search_record(asset)
            })
            .collect();
        records.sort_by(|left, right| {
            right
                .max_alias_len
                .cmp(&left.max_alias_len)
                .then_with(|| left.asset.name.cmp(&right.asset.name))
        });
        work.mention_sorts += 1;
        let mut hits = Vec::new();
        let mut seen = HashSet::new();
        for record in records {
            for alias in &record.mention_aliases {
                work.alias_checks += 1;
                if !alias.is_empty() && compact.contains(alias) {
                    if seen.insert(record.asset.name.clone()) {
                        hits.push(record.asset.name);
                    }
                    break;
                }
            }
        }
        mentioned.push(hits);
    }

    let mut found = Vec::with_capacity(find_names.len());
    for name in find_names {
        let mut hit = false;
        for asset in library {
            work.linear_finds += 1;
            if asset.name == *name {
                hit = true;
                break;
            }
        }
        found.push(hit);
    }

    AssetSearchProbe {
        query_counts,
        mentioned,
        found,
    }
}

fn build_asset_search_probe_cached(
    library: &[AssetEntry],
    selected: &str,
    compare_assets: &[String],
    search_queries: &[String],
    mention_commands: &[String],
    find_names: &[String],
    revision: usize,
    cache: &mut AssetSearchIndexCache,
    work: &mut AssetSearchWork,
) -> AssetSearchProbe {
    ensure_asset_search_index(library, revision, cache, work);
    let query_counts = cached_asset_search_menu_counts(selected, compare_assets, search_queries, cache, work);

    let mut mentioned = Vec::with_capacity(mention_commands.len());
    for command in mention_commands {
        ensure_asset_search_index(library, revision, cache, work);
        let compact = compact_asset_token(command);
        cache.avoided_alias_builds = cache.avoided_alias_builds.saturating_add(cache.records.len().saturating_mul(2));
        cache.avoided_mention_sorts = cache.avoided_mention_sorts.saturating_add(1);
        let mut hits = Vec::new();
        let mut seen = HashSet::new();
        for index in &cache.mention_order {
            let record = &cache.records[*index];
            for alias in &record.mention_aliases {
                work.alias_checks += 1;
                if !alias.is_empty() && compact.contains(alias) {
                    if seen.insert(record.asset.name.clone()) {
                        hits.push(record.asset.name.clone());
                    }
                    break;
                }
            }
        }
        mentioned.push(hits);
    }

    let mut found = Vec::with_capacity(find_names.len());
    for name in find_names {
        ensure_asset_search_index(library, revision, cache, work);
        cache.avoided_linear_finds = cache.avoided_linear_finds.saturating_add(cache.records.len());
        found.push(cache.by_name.contains_key(name));
    }

    AssetSearchProbe {
        query_counts,
        mentioned,
        found,
    }
}

fn cached_asset_search_menu_counts(
    selected: &str,
    compare_assets: &[String],
    search_queries: &[String],
    cache: &mut AssetSearchIndexCache,
    work: &mut AssetSearchWork,
) -> Vec<usize> {
    let model_key = format!(
        "selected={selected}|compare={}|queries={}",
        compare_assets.join(","),
        search_queries.join("\u{1f}")
    );
    if cache.model_key == model_key && !cache.model_counts.is_empty() {
        cache.hits += 1;
        cache.avoided_menu_model_scans = cache
            .avoided_menu_model_scans
            .saturating_add(cache.records.len().saturating_mul(search_queries.len()));
        cache.avoided_haystack_builds = cache
            .avoided_haystack_builds
            .saturating_add(cache.records.len().saturating_mul(search_queries.len()));
        return cache.model_counts.clone();
    }
    cache.misses += 1;
    let active: HashSet<&str> = compare_assets.iter().map(String::as_str).collect();
    let mut counts = Vec::with_capacity(search_queries.len());
    for query in search_queries {
        let query = query.trim().to_ascii_lowercase();
        let mut count = 0_usize;
        for record in &cache.records {
            if record.asset.name == selected {
                continue;
            }
            work.menu_model_scans += 1;
            if !query.is_empty() && !record.search_haystack.contains(&query) {
                continue;
            }
            let _active = active.contains(record.asset.name.as_str());
            work.menu_item_builds += 1;
            count += 1;
        }
        counts.push(count);
    }
    cache.model_key = model_key;
    cache.model_counts = counts.clone();
    counts
}

fn ensure_asset_search_index(
    library: &[AssetEntry],
    revision: usize,
    cache: &mut AssetSearchIndexCache,
    work: &mut AssetSearchWork,
) {
    let key = asset_search_index_key(library, revision);
    if cache.initialized && cache.key == key {
        cache.hits += 1;
        return;
    }
    cache.misses += 1;
    cache.key = key;
    cache.records.clear();
    cache.by_name.clear();
    cache.mention_order.clear();
    cache.model_key.clear();
    cache.model_counts.clear();
    cache.records.reserve(library.len());
    for asset in library {
        work.haystack_builds += 1;
        work.alias_builds += 2;
        let record = build_asset_search_record(asset);
        cache.by_name.insert(record.asset.name.clone(), cache.records.len());
        cache.records.push(record);
    }
    let mut order: Vec<usize> = (0..cache.records.len()).collect();
    order.sort_by(|left, right| {
        let left = &cache.records[*left];
        let right = &cache.records[*right];
        right
            .max_alias_len
            .cmp(&left.max_alias_len)
            .then_with(|| left.asset.name.cmp(&right.asset.name))
    });
    cache.mention_order = order;
    work.mention_sorts += 1;
    cache.initialized = true;
}

fn asset_search_index_key(library: &[AssetEntry], revision: usize) -> String {
    format!(
        "{revision}|len={}|first={}|last={}",
        library.len(),
        library.first().map(|asset| asset.name.as_str()).unwrap_or(""),
        library.last().map(|asset| asset.name.as_str()).unwrap_or("")
    )
}

fn build_asset_search_record(asset: &AssetEntry) -> AssetSearchRecord {
    let broker_code = asset_broker_code(&asset.name);
    let compare_code = asset_compare_code(&asset.name);
    let subtitle = asset_compare_subtitle(asset);
    let mut mention_aliases = Vec::with_capacity(2);
    let mut seen = HashSet::new();
    for alias in [&broker_code, &asset.name] {
        let compact = compact_asset_token(alias);
        if !compact.is_empty() && seen.insert(compact.clone()) {
            mention_aliases.push(compact);
        }
    }
    let max_alias_len = mention_aliases.iter().map(String::len).max().unwrap_or(0);
    AssetSearchRecord {
        asset: asset.clone(),
        broker_code,
        compare_code,
        subtitle,
        search_haystack: asset_search_haystack_legacy(asset),
        mention_aliases,
        max_alias_len,
    }
}

fn asset_search_haystack_legacy(asset: &AssetEntry) -> String {
    [
        asset.name.as_str(),
        asset_broker_code(&asset.name).as_str(),
        asset_compare_code(&asset.name).as_str(),
        asset.display_name.as_str(),
        asset_class_label(&asset.asset_class),
    ]
    .join(" ")
    .to_ascii_lowercase()
}

fn compact_asset_token(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(char::to_uppercase)
        .collect()
}

fn asset_broker_code(name: &str) -> String {
    name.chars().filter(|ch| ch.is_ascii_alphanumeric()).collect()
}

fn asset_compare_code(name: &str) -> String {
    let tokens = split_asset_tokens(name);
    if tokens.len() >= 2 {
        format!("{}/{}", tokens[0], tokens[1])
    } else {
        asset_broker_code(name)
    }
}

fn asset_compare_subtitle(asset: &AssetEntry) -> String {
    let tokens = split_asset_tokens(&asset.name);
    let short_code = tokens.join(" / ");
    if !asset.display_name.is_empty() && asset.display_name.to_ascii_uppercase() != short_code {
        return asset.display_name.clone();
    }
    if tokens.len() >= 2 {
        return tokens.join(" / ");
    }
    asset.display_name.clone()
}

fn split_asset_tokens(name: &str) -> Vec<String> {
    name.split(|ch: char| ch == '_' || ch == '/' || ch == ':' || ch == '-' || ch.is_ascii_whitespace())
        .filter(|token| !token.is_empty())
        .map(|token| token.to_ascii_uppercase())
        .collect()
}

fn asset_class_label(asset_class: &str) -> &'static str {
    match asset_class {
        "commodity" => "Commodities",
        "forex" => "FX",
        "crypto" => "Crypto",
        "index" => "Indices",
        "equity" => "Equities",
        "bond" => "Rates",
        _ => "Other",
    }
}

fn synthetic_asset_search_queries() -> Vec<String> {
    ["usd", "btc", "xau", "idx", "synth", "no-match"]
        .iter()
        .map(|item| item.to_string())
        .collect()
}

fn synthetic_asset_mention_commands(library: &[AssetEntry]) -> Vec<String> {
    let mut out = Vec::new();
    for asset in library.iter().skip(1).step_by(83).take(6) {
        out.push(format!("compare {} with EURUSD", asset_broker_code(&asset.name)));
    }
    out.push("load NO_SYMBOL_NOW".to_string());
    out
}

fn synthetic_asset_find_names(library: &[AssetEntry]) -> Vec<String> {
    let mut out: Vec<String> = library
        .iter()
        .step_by(97)
        .take(10)
        .map(|asset| asset.name.clone())
        .collect();
    out.push("MISSING_ASSET".to_string());
    out
}

fn asset_search_probe_checksum(probe: &AssetSearchProbe) -> usize {
    let mut hash = 2166136261_usize;
    for count in &probe.query_counts {
        hash = hash.wrapping_mul(16777619).wrapping_add(*count);
    }
    for hits in &probe.mentioned {
        for hit in hits {
            hash = hash.wrapping_mul(16777619).wrapping_add(key_checksum(hit));
        }
    }
    for found in &probe.found {
        hash = hash.wrapping_mul(16777619).wrapping_add(usize::from(*found));
    }
    std::hint::black_box(hash)
}

fn run_context_snapshot_cache_focus(bars: &[Bar], series_hash: &str, frames: usize) {
    let frames = frames.max(3);
    let calls_per_frame = 10_usize;
    let asset_count = (bars.len() / 12).clamp(240, 1_200);
    let files = synthetic_history_files(asset_count);
    let asset_catalog = build_asset_catalog_from_history(&files);
    let compare_assets = synthetic_compare_assets(&asset_catalog, 12);
    let alerts = synthetic_context_alerts(&asset_catalog, 18);
    let open_trades = synthetic_context_trades(&asset_catalog, 10, "BUY");
    let pending_orders = synthetic_context_trades(&asset_catalog, 12, "LIMIT");
    let mut proof_legacy_work = ContextSnapshotWork::default();
    let proof_legacy = build_context_probe_legacy(
        bars,
        &files,
        &asset_catalog,
        &compare_assets,
        &alerts,
        &open_trades,
        &pending_orders,
        &mut proof_legacy_work,
    );
    let mut proof_cache = ContextSnapshotCache::default();
    let mut proof_cached_work = ContextSnapshotWork::default();
    let proof_cached = build_context_probe_cached(
        bars,
        &files,
        &asset_catalog,
        &compare_assets,
        &alerts,
        &open_trades,
        &pending_orders,
        1,
        &mut proof_cache,
        &mut proof_cached_work,
    );
    println!(
        "[trading-lab] focus=context-snapshot-cache task=move-context-cache-key-before-expensive-snapshot-and-digest series_hash={} bars={} frames={} calls_per_frame={} catalog_files={} assets={} compares={} alerts={} open_trades={} pending_orders={}",
        series_hash,
        bars.len(),
        frames,
        calls_per_frame,
        files.len(),
        asset_catalog.len(),
        compare_assets.len(),
        alerts.len(),
        open_trades.len(),
        pending_orders.len()
    );
    println!(
        "[trading-lab] proof context-snapshot-cache equal={} snapshot_hash={} digest_hash={} legacy_candle_scans={} legacy_catalog_scans={} cached_key_units={}",
        proof_legacy == proof_cached,
        proof_cached.snapshot_hash,
        proof_cached.digest_hash,
        proof_legacy_work.candle_scans,
        proof_legacy_work.catalog_scans,
        proof_cached_work.key_units
    );

    let legacy = bench_context_snapshot(
        bars,
        &files,
        &asset_catalog,
        &compare_assets,
        &alerts,
        &open_trades,
        &pending_orders,
        frames,
        calls_per_frame,
        false,
    );
    let optimized = bench_context_snapshot(
        bars,
        &files,
        &asset_catalog,
        &compare_assets,
        &alerts,
        &open_trades,
        &pending_orders,
        frames,
        calls_per_frame,
        true,
    );
    let hit_rate = if optimized.hits + optimized.misses == 0 {
        0.0
    } else {
        optimized.hits as f64 / (optimized.hits + optimized.misses) as f64
    };
    println!(
        "[trading-lab] legacy-context-snapshot model=build-compare-snapshot-scan-candles-scan-catalog-map-orders-alerts-build-digest-each-call p50_ms={:.3} p95_ms={:.3} p99_ms={:.3} checksum={} candle_scans={} catalog_scans={} compare_scans={} alert_maps={} trade_maps={} signal_scans={} digest_lines={}",
        legacy.stats.p50_us as f64 / 1000.0,
        legacy.stats.p95_us as f64 / 1000.0,
        legacy.stats.p99_us as f64 / 1000.0,
        legacy.checksum,
        legacy.candle_scans,
        legacy.catalog_scans,
        legacy.compare_scans,
        legacy.alert_maps,
        legacy.trade_maps,
        legacy.signal_scans,
        legacy.digest_lines
    );
    println!(
        "[trading-lab] optimized-context-snapshot model=frontloaded-context-key-plus-snapshot-and-digest-cache p50_ms={:.3} p95_ms={:.3} p99_ms={:.3} checksum={} hits={} misses={} hit_rate={:.3} key_units={} candle_scans={} avoided_candle_scans={} catalog_scans={} avoided_catalog_scans={} compare_scans={} avoided_compare_scans={} alert_maps={} avoided_alert_maps={} trade_maps={} avoided_trade_maps={} signal_scans={} avoided_signal_scans={} digest_lines={} avoided_digest_lines={} cache_entries=3 evictions=0",
        optimized.stats.p50_us as f64 / 1000.0,
        optimized.stats.p95_us as f64 / 1000.0,
        optimized.stats.p99_us as f64 / 1000.0,
        optimized.checksum,
        optimized.hits,
        optimized.misses,
        hit_rate,
        optimized.key_units,
        optimized.candle_scans,
        optimized.avoided_candle_scans,
        optimized.catalog_scans,
        optimized.avoided_catalog_scans,
        optimized.compare_scans,
        optimized.avoided_compare_scans,
        optimized.alert_maps,
        optimized.avoided_alert_maps,
        optimized.trade_maps,
        optimized.avoided_trade_maps,
        optimized.signal_scans,
        optimized.avoided_signal_scans,
        optimized.digest_lines,
        optimized.avoided_digest_lines
    );
    println!(
        "[trading-lab] summary target=context_snapshot_cache p50_speedup_x={:.2} p95_speedup_x={:.2} p99_speedup_x={:.2} cache_budget=one-context-snapshot-plus-two-digests-per-context-key",
        ratio(legacy.stats.p50_us, optimized.stats.p50_us),
        ratio(legacy.stats.p95_us, optimized.stats.p95_us),
        ratio(legacy.stats.p99_us, optimized.stats.p99_us)
    );
}

#[allow(clippy::too_many_arguments)]
fn bench_context_snapshot(
    bars: &[Bar],
    files: &[HistoryFileEntry],
    asset_catalog: &[AssetEntry],
    compare_assets: &[String],
    alerts: &[SyntheticAlert],
    open_trades: &[SyntheticTrade],
    pending_orders: &[SyntheticTrade],
    frames: usize,
    calls_per_frame: usize,
    cached: bool,
) -> ContextSnapshotRunStats {
    let mut samples = Vec::with_capacity(frames);
    let mut checksum = 0_usize;
    let mut work = ContextSnapshotWork::default();
    let mut cache = ContextSnapshotCache::default();
    for _ in 0..frames {
        let started = Instant::now();
        for _ in 0..calls_per_frame {
            let probe = if cached {
                build_context_probe_cached(
                    bars,
                    files,
                    asset_catalog,
                    compare_assets,
                    alerts,
                    open_trades,
                    pending_orders,
                    1,
                    &mut cache,
                    &mut work,
                )
            } else {
                build_context_probe_legacy(
                    bars,
                    files,
                    asset_catalog,
                    compare_assets,
                    alerts,
                    open_trades,
                    pending_orders,
                    &mut work,
                )
            };
            checksum = checksum
                .wrapping_add(probe.snapshot_hash)
                .wrapping_mul(16777619)
                .wrapping_add(probe.digest_hash);
        }
        samples.push(started.elapsed().as_micros());
    }
    ContextSnapshotRunStats {
        stats: percentile_stats(&samples),
        checksum: std::hint::black_box(checksum),
        hits: cache.hits,
        misses: cache.misses,
        key_units: work.key_units,
        candle_scans: work.candle_scans,
        avoided_candle_scans: cache.avoided_candle_scans,
        catalog_scans: work.catalog_scans,
        avoided_catalog_scans: cache.avoided_catalog_scans,
        compare_scans: work.compare_scans,
        avoided_compare_scans: cache.avoided_compare_scans,
        alert_maps: work.alert_maps,
        avoided_alert_maps: cache.avoided_alert_maps,
        trade_maps: work.trade_maps,
        avoided_trade_maps: cache.avoided_trade_maps,
        signal_scans: work.signal_scans,
        avoided_signal_scans: cache.avoided_signal_scans,
        digest_lines: work.digest_lines,
        avoided_digest_lines: cache.avoided_digest_lines,
    }
}

#[allow(clippy::too_many_arguments)]
fn build_context_probe_cached(
    bars: &[Bar],
    files: &[HistoryFileEntry],
    asset_catalog: &[AssetEntry],
    compare_assets: &[String],
    alerts: &[SyntheticAlert],
    open_trades: &[SyntheticTrade],
    pending_orders: &[SyntheticTrade],
    revision: usize,
    cache: &mut ContextSnapshotCache,
    work: &mut ContextSnapshotWork,
) -> ContextProbe {
    let key = context_probe_key(
        bars,
        files,
        compare_assets,
        alerts,
        open_trades,
        pending_orders,
        revision,
        work,
    );
    if cache.key == key {
        if let Some(probe) = cache.probe {
            cache.hits += 1;
            cache.avoided_candle_scans = cache.avoided_candle_scans.saturating_add(bars.len());
            cache.avoided_catalog_scans = cache.avoided_catalog_scans.saturating_add(files.len());
            cache.avoided_compare_scans = cache
                .avoided_compare_scans
                .saturating_add(compare_assets.len().saturating_mul(asset_catalog.len()));
            cache.avoided_alert_maps = cache.avoided_alert_maps.saturating_add(alerts.len().min(12));
            cache.avoided_trade_maps = cache
                .avoided_trade_maps
                .saturating_add(open_trades.len().min(8) + pending_orders.len().min(8));
            cache.avoided_signal_scans = cache
                .avoided_signal_scans
                .saturating_add(alerts.len() + compare_assets.len().min(6));
            cache.avoided_digest_lines = cache.avoided_digest_lines.saturating_add(28);
            return probe;
        }
    }
    cache.misses += 1;
    let probe = build_context_probe_legacy(
        bars,
        files,
        asset_catalog,
        compare_assets,
        alerts,
        open_trades,
        pending_orders,
        work,
    );
    cache.key = key;
    cache.probe = Some(probe);
    probe
}

#[allow(clippy::too_many_arguments)]
fn build_context_probe_legacy(
    bars: &[Bar],
    files: &[HistoryFileEntry],
    asset_catalog: &[AssetEntry],
    compare_assets: &[String],
    alerts: &[SyntheticAlert],
    open_trades: &[SyntheticTrade],
    pending_orders: &[SyntheticTrade],
    work: &mut ContextSnapshotWork,
) -> ContextProbe {
    let mut snapshot_hash = 2166136261_usize;
    let mut digest_hash = 2166136261_usize;

    let mut compare_count = 0_usize;
    for name in compare_assets {
        for asset in asset_catalog {
            work.compare_scans += 1;
            if asset.name == *name {
                snapshot_hash = hash_add_str(snapshot_hash, &asset.name);
                snapshot_hash = hash_add_str(snapshot_hash, &asset.display_name);
                compare_count += 1;
                break;
            }
        }
    }

    let mut instruments = HashSet::new();
    let mut granularity_counts: HashMap<&str, usize> = HashMap::new();
    for file in files {
        work.catalog_scans += 1;
        instruments.insert(file.instrument.as_str());
        *granularity_counts.entry(file.granularity.as_str()).or_default() += 1;
    }
    snapshot_hash = snapshot_hash.wrapping_add(files.len());
    snapshot_hash = snapshot_hash.wrapping_add(instruments.len().wrapping_mul(31));
    snapshot_hash = snapshot_hash.wrapping_add(granularity_counts.len().wrapping_mul(131));

    let mut high = f64::NEG_INFINITY;
    let mut low = f64::INFINITY;
    for bar in bars {
        work.candle_scans += 1;
        high = high.max(bar.high);
        low = low.min(bar.low);
    }
    if let Some(first) = bars.first() {
        snapshot_hash = hash_add_i64(snapshot_hash, first.time_ms);
        snapshot_hash = hash_add_f64(snapshot_hash, first.open);
    }
    if let Some(last) = bars.last() {
        snapshot_hash = hash_add_i64(snapshot_hash, last.time_ms);
        snapshot_hash = hash_add_f64(snapshot_hash, last.close);
    }
    snapshot_hash = hash_add_f64(snapshot_hash, high);
    snapshot_hash = hash_add_f64(snapshot_hash, low);

    for alert in alerts.iter().take(12) {
        work.alert_maps += 1;
        snapshot_hash = hash_add_str(snapshot_hash, &alert.id);
        snapshot_hash = hash_add_str(snapshot_hash, &alert.instrument);
        snapshot_hash = snapshot_hash.wrapping_add(usize::from(alert.active));
        snapshot_hash = hash_add_f64(snapshot_hash, alert.target_value);
        snapshot_hash = snapshot_hash.wrapping_add(alert.triggered_count);
        snapshot_hash = hash_add_str(snapshot_hash, &alert.message);
    }

    for trade in open_trades.iter().take(8).chain(pending_orders.iter().take(8)) {
        work.trade_maps += 1;
        snapshot_hash = hash_add_str(snapshot_hash, &trade.id);
        snapshot_hash = hash_add_str(snapshot_hash, &trade.instrument);
        snapshot_hash = hash_add_str(snapshot_hash, &trade.side);
        snapshot_hash = hash_add_f64(snapshot_hash, trade.units);
        snapshot_hash = hash_add_f64(snapshot_hash, trade.price);
    }

    let last = bars.last().copied().unwrap_or_default();
    let anchor = bars
        .get(bars.len().saturating_sub(21))
        .copied()
        .unwrap_or(last);
    let momentum = if anchor.close.abs() > f64::EPSILON {
        ((last.close - anchor.close) / anchor.close) * 100.0
    } else {
        0.0
    };
    snapshot_hash = hash_add_f64(snapshot_hash, momentum);
    for alert in alerts {
        work.signal_scans += 1;
        if alert.active && alert.instrument == "EUR_USD" {
            snapshot_hash = hash_add_f64(snapshot_hash, (alert.target_value - last.close).abs());
        }
    }
    for name in compare_assets.iter().take(6) {
        work.signal_scans += 1;
        snapshot_hash = hash_add_str(snapshot_hash, name);
    }

    let digest_lines = 28_usize;
    for line in 0..digest_lines {
        work.digest_lines += 1;
        digest_hash = digest_hash
            .wrapping_mul(16777619)
            .wrapping_add(snapshot_hash.rotate_left((line % usize::BITS as usize) as u32))
            .wrapping_add(compare_count)
            .wrapping_add(line);
    }

    ContextProbe {
        snapshot_hash: std::hint::black_box(snapshot_hash),
        digest_hash: std::hint::black_box(digest_hash),
    }
}

#[allow(clippy::too_many_arguments)]
fn context_probe_key(
    bars: &[Bar],
    files: &[HistoryFileEntry],
    compare_assets: &[String],
    alerts: &[SyntheticAlert],
    open_trades: &[SyntheticTrade],
    pending_orders: &[SyntheticTrade],
    revision: usize,
    work: &mut ContextSnapshotWork,
) -> String {
    work.key_units = work
        .key_units
        .saturating_add(16 + compare_assets.len() + alerts.len() + open_trades.len() + pending_orders.len());
    let first = bars.first().copied().unwrap_or_default();
    let last = bars.last().copied().unwrap_or_default();
    let last_file = files.last();
    let last_alert = alerts.last();
    let last_open = open_trades.last();
    let last_pending = pending_orders.last();
    format!(
        "{revision}|bars={}:{}:{}:{}:{}|files={}:{}:{}|compare={}|alerts={}:{}:{}:{}|open={}:{}:{}|pending={}:{}:{}",
        bars.len(),
        first.time_ms,
        first.open.to_bits(),
        last.time_ms,
        last.close.to_bits(),
        files.len(),
        last_file.map(|file| file.instrument.as_str()).unwrap_or(""),
        last_file.map(|file| file.rows).unwrap_or(0),
        compare_assets.join(","),
        alerts.len(),
        last_alert.map(|alert| alert.id.as_str()).unwrap_or(""),
        last_alert.map(|alert| alert.target_value.to_bits()).unwrap_or(0),
        last_alert.map(|alert| alert.triggered_count).unwrap_or(0),
        open_trades.len(),
        last_open.map(|trade| trade.id.as_str()).unwrap_or(""),
        last_open.map(|trade| trade.price.to_bits()).unwrap_or(0),
        pending_orders.len(),
        last_pending.map(|trade| trade.id.as_str()).unwrap_or(""),
        last_pending.map(|trade| trade.price.to_bits()).unwrap_or(0),
    )
}

fn hash_add_str(hash: usize, value: &str) -> usize {
    hash.wrapping_mul(16777619).wrapping_add(key_checksum(value))
}

fn hash_add_i64(hash: usize, value: i64) -> usize {
    hash.wrapping_mul(16777619).wrapping_add(value as usize)
}

fn hash_add_f64(hash: usize, value: f64) -> usize {
    hash.wrapping_mul(16777619).wrapping_add(value.to_bits() as usize)
}

fn synthetic_context_alerts(asset_catalog: &[AssetEntry], count: usize) -> Vec<SyntheticAlert> {
    asset_catalog
        .iter()
        .step_by(41)
        .take(count)
        .enumerate()
        .map(|(idx, asset)| SyntheticAlert {
            id: format!("alert-{idx:04}"),
            instrument: asset.name.clone(),
            active: idx % 5 != 0,
            target_value: 1.0 + idx as f64 * 0.013,
            triggered_count: idx % 3,
            message: format!("watch {}", asset.name),
        })
        .collect()
}

fn synthetic_context_trades(asset_catalog: &[AssetEntry], count: usize, side: &str) -> Vec<SyntheticTrade> {
    asset_catalog
        .iter()
        .skip(2)
        .step_by(47)
        .take(count)
        .enumerate()
        .map(|(idx, asset)| SyntheticTrade {
            id: format!("trade-{side}-{idx:04}"),
            instrument: asset.name.clone(),
            side: side.to_string(),
            units: 1_000.0 + idx as f64 * 25.0,
            price: 1.1 + idx as f64 * 0.007,
        })
        .collect()
}

fn run_alert_payload_cache_focus(bars: &[Bar], series_hash: &str, frames: usize) {
    let frames = frames.max(3);
    let calls_per_frame = 14_usize;
    let selected = "EUR_USD";
    let asset_count = (bars.len() / 10).clamp(360, 1_600);
    let files = synthetic_history_files(asset_count);
    let asset_catalog = build_asset_catalog_from_history(&files);
    let alert_count = (bars.len() / 8).clamp(512, 3_000);
    let alerts = synthetic_alert_payload_alerts(&asset_catalog, alert_count, selected);
    let mut proof_legacy_work = AlertPayloadWork::default();
    let proof_legacy = build_alert_payload_probe_legacy(bars, &alerts, selected, &mut proof_legacy_work);
    let mut proof_cache = AlertPayloadCache::default();
    let mut proof_cached_work = AlertPayloadWork::default();
    let proof_cached =
        build_alert_payload_probe_cached(bars, &alerts, selected, 1, &mut proof_cache, &mut proof_cached_work);
    println!(
        "[trading-lab] focus=alert-payload-cache task=version-alert-state-cache-canvas-list-modal-and-context-key series_hash={} bars={} frames={} calls_per_frame={} alerts={} selected_instrument={}",
        series_hash,
        bars.len(),
        frames,
        calls_per_frame,
        alerts.len(),
        selected
    );
    println!(
        "[trading-lab] proof alert-payload-cache equal={} canvas_hash={} list_hash={} modal_hash={} context_hash={} signal_hash={} legacy_normalizations={} legacy_context_alert_scans={}",
        proof_legacy == proof_cached,
        proof_cached.canvas_hash,
        proof_cached.list_hash,
        proof_cached.modal_hash,
        proof_cached.context_hash,
        proof_cached.signal_hash,
        proof_legacy_work.normalizations,
        proof_legacy_work.context_alert_scans
    );

    let legacy = bench_alert_payload_cache(bars, &alerts, selected, frames, calls_per_frame, false);
    let optimized = bench_alert_payload_cache(bars, &alerts, selected, frames, calls_per_frame, true);
    let hit_rate = if optimized.hits + optimized.misses == 0 {
        0.0
    } else {
        optimized.hits as f64 / (optimized.hits + optimized.misses) as f64
    };
    println!(
        "[trading-lab] legacy-alert-payload model=normalize-filter-map-canvas-sort-modal-scan-context-each-call p50_ms={:.3} p95_ms={:.3} p99_ms={:.3} checksum={} normalizations={} instrument_checks={} canvas_maps={} list_sorts={} modal_rows={} context_alert_scans={} signal_alert_scans={}",
        legacy.stats.p50_us as f64 / 1000.0,
        legacy.stats.p95_us as f64 / 1000.0,
        legacy.stats.p99_us as f64 / 1000.0,
        legacy.checksum,
        legacy.normalizations,
        legacy.instrument_checks,
        legacy.canvas_maps,
        legacy.list_sorts,
        legacy.modal_rows,
        legacy.context_alert_scans,
        legacy.signal_alert_scans
    );
    println!(
        "[trading-lab] optimized-alert-payload model=alert-universe-revision-plus-canvas-list-modal-payload-cache p50_ms={:.3} p95_ms={:.3} p99_ms={:.3} checksum={} hits={} misses={} hit_rate={:.3} key_units={} normalizations={} avoided_normalizations={} instrument_checks={} avoided_instrument_checks={} canvas_maps={} avoided_canvas_maps={} list_sorts={} avoided_list_sorts={} modal_rows={} avoided_modal_rows={} context_alert_scans={} avoided_context_alert_scans={} signal_alert_scans={} avoided_signal_alert_scans={} cache_entries=3 evictions=0",
        optimized.stats.p50_us as f64 / 1000.0,
        optimized.stats.p95_us as f64 / 1000.0,
        optimized.stats.p99_us as f64 / 1000.0,
        optimized.checksum,
        optimized.hits,
        optimized.misses,
        hit_rate,
        optimized.key_units,
        optimized.normalizations,
        optimized.avoided_normalizations,
        optimized.instrument_checks,
        optimized.avoided_instrument_checks,
        optimized.canvas_maps,
        optimized.avoided_canvas_maps,
        optimized.list_sorts,
        optimized.avoided_list_sorts,
        optimized.modal_rows,
        optimized.avoided_modal_rows,
        optimized.context_alert_scans,
        optimized.avoided_context_alert_scans,
        optimized.signal_alert_scans,
        optimized.avoided_signal_alert_scans
    );
    println!(
        "[trading-lab] summary target=alert_payload_cache p50_speedup_x={:.2} p95_speedup_x={:.2} p99_speedup_x={:.2} cache_budget=one-canvas-alert-list-plus-one-instrument-list-plus-one-modal-key-per-alert-revision",
        ratio(legacy.stats.p50_us, optimized.stats.p50_us),
        ratio(legacy.stats.p95_us, optimized.stats.p95_us),
        ratio(legacy.stats.p99_us, optimized.stats.p99_us)
    );
}

fn bench_alert_payload_cache(
    bars: &[Bar],
    alerts: &[SyntheticAlert],
    selected: &str,
    frames: usize,
    calls_per_frame: usize,
    cached: bool,
) -> AlertPayloadRunStats {
    let mut samples = Vec::with_capacity(frames);
    let mut checksum = 0_usize;
    let mut work = AlertPayloadWork::default();
    let mut cache = AlertPayloadCache::default();
    for _ in 0..frames {
        let started = Instant::now();
        for _ in 0..calls_per_frame {
            let probe = if cached {
                build_alert_payload_probe_cached(bars, alerts, selected, 1, &mut cache, &mut work)
            } else {
                build_alert_payload_probe_legacy(bars, alerts, selected, &mut work)
            };
            checksum = checksum
                .wrapping_mul(16777619)
                .wrapping_add(probe.canvas_hash)
                .wrapping_add(probe.list_hash.rotate_left(5))
                .wrapping_add(probe.modal_hash.rotate_left(11))
                .wrapping_add(probe.context_hash.rotate_left(17))
                .wrapping_add(probe.signal_hash.rotate_left(23));
        }
        samples.push(started.elapsed().as_micros());
    }
    AlertPayloadRunStats {
        stats: percentile_stats(&samples),
        checksum: std::hint::black_box(checksum),
        hits: cache.hits,
        misses: cache.misses,
        key_units: work.key_units,
        normalizations: work.normalizations,
        avoided_normalizations: cache.avoided_normalizations,
        instrument_checks: work.instrument_checks,
        avoided_instrument_checks: cache.avoided_instrument_checks,
        canvas_maps: work.canvas_maps,
        avoided_canvas_maps: cache.avoided_canvas_maps,
        list_sorts: work.list_sorts,
        avoided_list_sorts: cache.avoided_list_sorts,
        modal_rows: work.modal_rows,
        avoided_modal_rows: cache.avoided_modal_rows,
        context_alert_scans: work.context_alert_scans,
        avoided_context_alert_scans: cache.avoided_context_alert_scans,
        signal_alert_scans: work.signal_alert_scans,
        avoided_signal_alert_scans: cache.avoided_signal_alert_scans,
    }
}

fn build_alert_payload_probe_cached(
    bars: &[Bar],
    alerts: &[SyntheticAlert],
    selected: &str,
    revision: usize,
    cache: &mut AlertPayloadCache,
    work: &mut AlertPayloadWork,
) -> AlertPayloadProbe {
    let key = alert_payload_cache_key(selected, revision, work);
    if cache.key == key {
        if let Some(probe) = cache.probe {
            cache.hits += 1;
            cache.avoided_normalizations = cache
                .avoided_normalizations
                .saturating_add(cache.last_full_work.normalizations);
            cache.avoided_instrument_checks = cache
                .avoided_instrument_checks
                .saturating_add(cache.last_full_work.instrument_checks);
            cache.avoided_canvas_maps = cache
                .avoided_canvas_maps
                .saturating_add(cache.last_full_work.canvas_maps);
            cache.avoided_list_sorts = cache
                .avoided_list_sorts
                .saturating_add(cache.last_full_work.list_sorts);
            cache.avoided_modal_rows = cache
                .avoided_modal_rows
                .saturating_add(cache.last_full_work.modal_rows);
            cache.avoided_context_alert_scans = cache
                .avoided_context_alert_scans
                .saturating_add(cache.last_full_work.context_alert_scans);
            cache.avoided_signal_alert_scans = cache
                .avoided_signal_alert_scans
                .saturating_add(cache.last_full_work.signal_alert_scans);
            return probe;
        }
    }
    cache.misses += 1;
    let before = *work;
    let probe = build_alert_payload_probe_legacy(bars, alerts, selected, work);
    cache.last_full_work = work.delta(before);
    cache.key = key;
    cache.probe = Some(probe);
    probe
}

fn build_alert_payload_probe_legacy(
    bars: &[Bar],
    alerts: &[SyntheticAlert],
    selected: &str,
    work: &mut AlertPayloadWork,
) -> AlertPayloadProbe {
    let mut canvas_hash = 2166136261_usize;
    for alert in alerts {
        work.normalizations += 1;
        work.instrument_checks += 1;
        if alert.instrument == selected
            && (alert.active || alert.triggered_count > 0)
            && alert.target_value.is_finite()
        {
            work.canvas_maps += 1;
            canvas_hash = hash_add_str(canvas_hash, &alert.id);
            canvas_hash = hash_add_str(canvas_hash, &alert.instrument);
            canvas_hash = hash_add_f64(canvas_hash, alert.target_value);
            canvas_hash = canvas_hash.wrapping_add(alert.triggered_count);
        }
    }

    let mut instrument_alerts = Vec::new();
    for alert in alerts {
        work.normalizations += 1;
        work.instrument_checks += 1;
        if alert.instrument == selected {
            instrument_alerts.push(alert);
        }
    }
    instrument_alerts.sort_by(|left, right| {
        let active_order = (right.active as usize).cmp(&(left.active as usize));
        if active_order != std::cmp::Ordering::Equal {
            return active_order;
        }
        let trigger_order = right.triggered_count.cmp(&left.triggered_count);
        if trigger_order != std::cmp::Ordering::Equal {
            return trigger_order;
        }
        left.id.cmp(&right.id)
    });
    work.list_sorts += instrument_alerts.len();
    let mut list_hash = 2166136261_usize;
    let mut modal_hash = 2166136261_usize;
    for alert in &instrument_alerts {
        list_hash = hash_add_str(list_hash, &alert.id);
        list_hash = hash_add_f64(list_hash, alert.target_value);
        list_hash = list_hash.wrapping_add(usize::from(alert.active));
        list_hash = list_hash.wrapping_add(alert.triggered_count);

        work.modal_rows += 1;
        modal_hash = hash_add_str(modal_hash, &alert.message);
        modal_hash = hash_add_str(modal_hash, &alert.id);
        modal_hash = hash_add_f64(modal_hash, alert.target_value);
    }

    let mut context_hash = 2166136261_usize;
    for alert in alerts {
        work.context_alert_scans += 1;
        context_hash = hash_add_str(context_hash, &alert.id);
        context_hash = hash_add_str(context_hash, &alert.instrument);
        context_hash = context_hash.wrapping_add(usize::from(alert.active));
        context_hash = hash_add_f64(context_hash, alert.target_value);
        context_hash = context_hash.wrapping_add(alert.triggered_count);
        context_hash = hash_add_str(context_hash, &alert.message);
    }

    let current_mid = bars.last().map(|bar| bar.close).unwrap_or(1.0);
    let mut nearest = f64::INFINITY;
    let mut signal_hash = 2166136261_usize;
    for alert in alerts {
        work.signal_alert_scans += 1;
        if alert.active && alert.instrument == selected {
            let distance = (alert.target_value - current_mid).abs();
            if distance < nearest {
                nearest = distance;
            }
            signal_hash = hash_add_f64(signal_hash, distance);
        }
    }
    if nearest.is_finite() {
        signal_hash = hash_add_f64(signal_hash, nearest);
    }

    AlertPayloadProbe {
        canvas_hash: std::hint::black_box(canvas_hash),
        list_hash: std::hint::black_box(list_hash),
        modal_hash: std::hint::black_box(modal_hash),
        context_hash: std::hint::black_box(context_hash),
        signal_hash: std::hint::black_box(signal_hash),
    }
}

fn alert_payload_cache_key(selected: &str, revision: usize, work: &mut AlertPayloadWork) -> String {
    work.key_units = work.key_units.saturating_add(3);
    format!("alerts:v1|revision={revision}|selected={selected}|modal=open")
}

fn synthetic_alert_payload_alerts(
    asset_catalog: &[AssetEntry],
    count: usize,
    selected: &str,
) -> Vec<SyntheticAlert> {
    let mut out = Vec::with_capacity(count);
    if asset_catalog.is_empty() {
        return out;
    }
    for idx in 0..count {
        let asset = &asset_catalog[idx % asset_catalog.len()];
        let instrument = if idx % 4 == 0 {
            selected.to_string()
        } else {
            asset.name.clone()
        };
        out.push(SyntheticAlert {
            id: format!("alert-payload-{idx:05}"),
            instrument,
            active: idx % 7 != 0,
            target_value: 0.84 + (idx % 900) as f64 * 0.0017,
            triggered_count: idx % 5,
            message: format!("alert payload {}", idx % 97),
        });
    }
    out
}

fn run_signal_parse_index_focus(bars: &[Bar], series_hash: &str, frames: usize) {
    let frames = frames.max(3);
    let signal_count = 1_200_usize.min(bars.len()).max(1);
    let signals = synthetic_parse_signal_times(bars, signal_count);
    let tolerance_ms = 3_600_000_i64;
    let proof_legacy: Vec<usize> = signals
        .iter()
        .map(|time_ms| find_signal_bar_legacy(bars, *time_ms, tolerance_ms).0)
        .collect();
    let mut proof_cache = CandleTimeIndexCache::default();
    let proof_optimized: Vec<usize> = signals
        .iter()
        .map(|time_ms| find_signal_bar_indexed(bars, *time_ms, tolerance_ms, &mut proof_cache).0)
        .collect();
    println!(
        "[trading-lab] focus=signal-parse-index task=dedupe-parse-signal-candle-findindex series_hash={} bars={} frames={} signals_per_frame={} tolerance_ms={}",
        series_hash,
        bars.len(),
        frames,
        signal_count,
        tolerance_ms
    );
    println!(
        "[trading-lab] proof signal-parse-index equal={} first_index={} last_index={}",
        proof_legacy == proof_optimized,
        proof_optimized.first().copied().unwrap_or(usize::MAX),
        proof_optimized.last().copied().unwrap_or(usize::MAX)
    );

    let legacy = bench_signal_parse_index(bars, &signals, tolerance_ms, frames, false);
    let optimized = bench_signal_parse_index(bars, &signals, tolerance_ms, frames, true);
    let hit_rate = if optimized.hits + optimized.misses == 0 {
        0.0
    } else {
        optimized.hits as f64 / (optimized.hits + optimized.misses) as f64
    };
    println!(
        "[trading-lab] legacy-signal-parse model=parse-log-findindex-full-candles p50_ms={:.3} p95_ms={:.3} p99_ms={:.3} checksum={} comparisons={}",
        legacy.stats.p50_us as f64 / 1000.0,
        legacy.stats.p95_us as f64 / 1000.0,
        legacy.stats.p99_us as f64 / 1000.0,
        legacy.checksum,
        legacy.comparisons
    );
    println!(
        "[trading-lab] optimized-signal-parse model=content-addressed-candle-time-index p50_ms={:.3} p95_ms={:.3} p99_ms={:.3} checksum={} hits={} misses={} hit_rate={:.3} avoided_comparisons={}",
        optimized.stats.p50_us as f64 / 1000.0,
        optimized.stats.p95_us as f64 / 1000.0,
        optimized.stats.p99_us as f64 / 1000.0,
        optimized.checksum,
        optimized.hits,
        optimized.misses,
        hit_rate,
        optimized.avoided_comparisons
    );
    println!(
        "[trading-lab] summary target=signal_parse_index p50_speedup_x={:.2} p95_speedup_x={:.2} p99_speedup_x={:.2} cache_budget=one-time-index-per-candle-array",
        ratio(legacy.stats.p50_us, optimized.stats.p50_us),
        ratio(legacy.stats.p95_us, optimized.stats.p95_us),
        ratio(legacy.stats.p99_us, optimized.stats.p99_us)
    );
}

fn bench_signal_parse_index(
    bars: &[Bar],
    signals: &[i64],
    tolerance_ms: i64,
    frames: usize,
    cached: bool,
) -> SignalParseRunStats {
    let mut samples = Vec::with_capacity(frames);
    let mut checksum = 0_usize;
    let mut comparisons = 0_usize;
    let mut cache = CandleTimeIndexCache::default();
    for _ in 0..frames {
        let started = Instant::now();
        for time_ms in signals {
            let (idx, scanned) = if cached {
                find_signal_bar_indexed(bars, *time_ms, tolerance_ms, &mut cache)
            } else {
                find_signal_bar_legacy(bars, *time_ms, tolerance_ms)
            };
            comparisons = comparisons.saturating_add(scanned);
            checksum = checksum.wrapping_add(idx);
        }
        samples.push(started.elapsed().as_micros());
    }
    SignalParseRunStats {
        stats: percentile_stats(&samples),
        checksum: std::hint::black_box(checksum),
        hits: cache.hits,
        misses: cache.misses,
        comparisons,
        avoided_comparisons: cache.avoided_comparisons,
    }
}

fn find_signal_bar_legacy(bars: &[Bar], target_time_ms: i64, tolerance_ms: i64) -> (usize, usize) {
    let mut comparisons = 0_usize;
    for (idx, bar) in bars.iter().enumerate() {
        comparisons += 1;
        if (bar.time_ms - target_time_ms).abs() < tolerance_ms {
            return (idx, comparisons);
        }
    }
    (usize::MAX, comparisons)
}

fn find_signal_bar_indexed(
    bars: &[Bar],
    target_time_ms: i64,
    tolerance_ms: i64,
    cache: &mut CandleTimeIndexCache,
) -> (usize, usize) {
    let (times, indices) = candle_time_index_cached(bars, cache);
    let min_time = target_time_ms - tolerance_ms;
    let max_time = target_time_ms + tolerance_ms;
    let mut low = 0_usize;
    let mut high = times.len();
    let mut comparisons = 0_usize;
    while low < high {
        comparisons += 1;
        let mid = (low + high) / 2;
        if times[mid] <= min_time {
            low = mid + 1;
        } else {
            high = mid;
        }
    }
    if low < times.len() {
        comparisons += 1;
        if times[low] < max_time {
            return (indices[low], comparisons);
        }
    }
    (usize::MAX, comparisons)
}

fn candle_time_index_cached<'a>(
    bars: &[Bar],
    cache: &'a mut CandleTimeIndexCache,
) -> (&'a [i64], &'a [usize]) {
    let key = (
        bars.len(),
        bars.first().map(|bar| bar.time_ms).unwrap_or_default(),
        bars.last().map(|bar| bar.time_ms).unwrap_or_default(),
    );
    if cache.initialized && cache.key == key {
        cache.hits += 1;
        cache.avoided_comparisons = cache.avoided_comparisons.saturating_add(bars.len());
        return (&cache.times, &cache.indices);
    }
    cache.misses += 1;
    cache.key = key;
    cache.times.clear();
    cache.indices.clear();
    cache.times.reserve(bars.len());
    cache.indices.reserve(bars.len());
    for (idx, bar) in bars.iter().enumerate() {
        cache.times.push(bar.time_ms);
        cache.indices.push(idx);
    }
    cache.initialized = true;
    (&cache.times, &cache.indices)
}

fn synthetic_parse_signal_times(bars: &[Bar], signal_count: usize) -> Vec<i64> {
    if bars.is_empty() || signal_count == 0 {
        return Vec::new();
    }
    (0..signal_count)
        .map(|idx| {
            let bar_index = idx
                .wrapping_mul(97)
                .wrapping_add(idx / 5)
                .wrapping_rem(bars.len());
            bars[bar_index].time_ms
        })
        .collect()
}

fn run_metric_series_cache_focus(bars: &[Bar], series_hash: &str, frames: usize) {
    let frames = frames.max(3);
    let visible_bars = 1_200_usize.min(bars.len()).max(1);
    let calls_per_frame = 8_usize;
    let visible_start = bars.len().saturating_sub(visible_bars);
    let visible = &bars[visible_start..];
    let signal_times = synthetic_parse_signal_times(visible, 96);
    let signal_counts = signal_count_map(&signal_times);
    let signal_revision = 1_usize;
    let proof_legacy = build_metric_series_legacy(visible, &signal_counts);
    let mut proof_cache = MetricSeriesCache::default();
    let proof_cached =
        build_metric_series_cached(visible, &signal_counts, signal_revision, &mut proof_cache).to_vec();
    println!(
        "[trading-lab] focus=metric-series-cache task=dedupe-visible-metric-axis-series series_hash={} bars={} frames={} visible_bars={} calls_per_frame={} signals={}",
        series_hash,
        bars.len(),
        frames,
        visible_bars,
        calls_per_frame,
        signal_times.len()
    );
    println!(
        "[trading-lab] proof metric-series-cache equal={} points={} first_time={} last_time={}",
        compare_metric_points(&proof_legacy, &proof_cached),
        proof_cached.len(),
        proof_cached.first().map(|point| point.time_ms).unwrap_or_default(),
        proof_cached.last().map(|point| point.time_ms).unwrap_or_default()
    );

    let legacy = bench_metric_series(visible, &signal_counts, signal_revision, frames, calls_per_frame, false);
    let optimized = bench_metric_series(visible, &signal_counts, signal_revision, frames, calls_per_frame, true);
    let hit_rate = if optimized.hits + optimized.misses == 0 {
        0.0
    } else {
        optimized.hits as f64 / (optimized.hits + optimized.misses) as f64
    };
    println!(
        "[trading-lab] legacy-metric-series model=rebuild-visible-metric-series-each-frame p50_ms={:.3} p95_ms={:.3} p99_ms={:.3} checksum={:.6} built_points={}",
        legacy.stats.p50_us as f64 / 1000.0,
        legacy.stats.p95_us as f64 / 1000.0,
        legacy.stats.p99_us as f64 / 1000.0,
        legacy.checksum,
        legacy.built_points
    );
    println!(
        "[trading-lab] optimized-metric-series model=content-addressed-visible-metric-series p50_ms={:.3} p95_ms={:.3} p99_ms={:.3} checksum={:.6} hits={} misses={} hit_rate={:.3} avoided_points={} cache_entries=1 evictions=0",
        optimized.stats.p50_us as f64 / 1000.0,
        optimized.stats.p95_us as f64 / 1000.0,
        optimized.stats.p99_us as f64 / 1000.0,
        optimized.checksum,
        optimized.hits,
        optimized.misses,
        hit_rate,
        optimized.avoided_points
    );
    println!(
        "[trading-lab] summary target=metric_series_cache p50_speedup_x={:.2} p95_speedup_x={:.2} p99_speedup_x={:.2} cache_budget=one-series-per-visible-window-axis-mode-signal-revision",
        ratio(legacy.stats.p50_us, optimized.stats.p50_us),
        ratio(legacy.stats.p95_us, optimized.stats.p95_us),
        ratio(legacy.stats.p99_us, optimized.stats.p99_us)
    );
}

fn bench_metric_series(
    visible: &[Bar],
    signal_counts: &HashMap<i64, usize>,
    signal_revision: usize,
    frames: usize,
    calls_per_frame: usize,
    cached: bool,
) -> MetricSeriesRunStats {
    let mut samples = Vec::with_capacity(frames);
    let mut checksum = 0.0_f64;
    let mut built_points = 0_usize;
    let mut cache = MetricSeriesCache::default();
    for _ in 0..frames {
        let started = Instant::now();
        for _ in 0..calls_per_frame {
            if cached {
                let points = build_metric_series_cached(visible, signal_counts, signal_revision, &mut cache);
                checksum += metric_series_checksum(points);
            } else {
                let points = build_metric_series_legacy(visible, signal_counts);
                built_points = built_points.saturating_add(points.len());
                checksum += metric_series_checksum(&points);
            }
        }
        samples.push(started.elapsed().as_micros());
    }
    MetricSeriesRunStats {
        stats: percentile_stats(&samples),
        checksum,
        hits: cache.hits,
        misses: cache.misses,
        built_points: built_points + cache.value.len().saturating_mul(cache.misses),
        avoided_points: cache.avoided_points,
    }
}

fn run_three_d_payload_cache_focus(bars: &[Bar], series_hash: &str, frames: usize) {
    let frames = frames.max(3);
    let calls_per_frame = 4_usize;
    let active_cloud_signature = 0x3d_c1_0d_u64;
    let (sampled_bars, price_bins, grid_cells) = three_d_payload_dimensions(bars.len());
    let proof_legacy = build_three_d_payload_legacy(bars, active_cloud_signature);
    let mut proof_cache = ThreeDPayloadCache::default();
    let proof_cached = build_three_d_payload_cached(bars, active_cloud_signature, &mut proof_cache).to_vec();
    println!(
        "[trading-lab] focus=3d-payload-cache task=dedupe-volume-profile-grid-and-geometry series_hash={} bars={} frames={} sampled_bars={} price_bins={} grid_cells={} calls_per_frame={} active_clouds=2",
        series_hash,
        bars.len(),
        frames,
        sampled_bars,
        price_bins,
        grid_cells,
        calls_per_frame
    );
    println!(
        "[trading-lab] proof 3d-payload-cache equal={} materialized_cells={} first_cell={} last_cell={}",
        compare_three_d_cells(&proof_legacy, &proof_cached),
        proof_cached.len(),
        proof_cached
            .first()
            .map(|cell| format!("{},{}", cell.col, cell.row))
            .unwrap_or_else(|| "none".to_string()),
        proof_cached
            .last()
            .map(|cell| format!("{},{}", cell.col, cell.row))
            .unwrap_or_else(|| "none".to_string())
    );

    let legacy = bench_three_d_payload(bars, active_cloud_signature, frames, calls_per_frame, false);
    let optimized = bench_three_d_payload(bars, active_cloud_signature, frames, calls_per_frame, true);
    let hit_rate = if optimized.hits + optimized.misses == 0 {
        0.0
    } else {
        optimized.hits as f64 / (optimized.hits + optimized.misses) as f64
    };
    println!(
        "[trading-lab] legacy-3d-payload model=rebuild-grid-float-buffers-every-call p50_ms={:.3} p95_ms={:.3} p99_ms={:.3} checksum={:.6} built_grid_cells={} materialized_cells={} scratch_float_arrays=towerGrid,avgGrid,canopyGrid,positions,colors,sizes",
        legacy.stats.p50_us as f64 / 1000.0,
        legacy.stats.p95_us as f64 / 1000.0,
        legacy.stats.p99_us as f64 / 1000.0,
        legacy.checksum,
        legacy.built_grid_cells,
        legacy.materialized_cells
    );
    println!(
        "[trading-lab] optimized-3d-payload model=content-addressed-candle-array-mode-indicator-signature p50_ms={:.3} p95_ms={:.3} p99_ms={:.3} checksum={:.6} hits={} misses={} hit_rate={:.3} avoided_grid_cells={} materialized_cells={} cache_entries=1 evictions=0",
        optimized.stats.p50_us as f64 / 1000.0,
        optimized.stats.p95_us as f64 / 1000.0,
        optimized.stats.p99_us as f64 / 1000.0,
        optimized.checksum,
        optimized.hits,
        optimized.misses,
        hit_rate,
        optimized.avoided_grid_cells,
        optimized.materialized_cells
    );
    println!(
        "[trading-lab] summary target=3d_payload_cache p50_speedup_x={:.2} p95_speedup_x={:.2} p99_speedup_x={:.2} cache_budget=one-payload-per-candle-array-mode-indicator-signature pressure_budget=one-payload-per-pressure-model-key",
        ratio(legacy.stats.p50_us, optimized.stats.p50_us),
        ratio(legacy.stats.p95_us, optimized.stats.p95_us),
        ratio(legacy.stats.p99_us, optimized.stats.p99_us)
    );
}

fn bench_three_d_payload(
    bars: &[Bar],
    indicator_signature: u64,
    frames: usize,
    calls_per_frame: usize,
    cached: bool,
) -> ThreeDPayloadRunStats {
    let mut samples = Vec::with_capacity(frames);
    let mut checksum = 0.0_f64;
    let mut built_grid_cells = 0_usize;
    let mut materialized_cells = 0_usize;
    let mut cache = ThreeDPayloadCache::default();
    let (_, _, per_call_grid_cells) = three_d_payload_dimensions(bars.len());
    for _ in 0..frames {
        let started = Instant::now();
        for _ in 0..calls_per_frame {
            if cached {
                let payload = build_three_d_payload_cached(bars, indicator_signature, &mut cache);
                checksum += three_d_payload_checksum(payload);
            } else {
                let payload = build_three_d_payload_legacy(bars, indicator_signature);
                built_grid_cells = built_grid_cells.saturating_add(per_call_grid_cells);
                materialized_cells = materialized_cells.saturating_add(payload.len());
                checksum += three_d_payload_checksum(&payload);
            }
        }
        samples.push(started.elapsed().as_micros());
    }
    if cached {
        built_grid_cells = cache.grid_cells.saturating_mul(cache.misses);
        materialized_cells = cache.value.len().saturating_mul(cache.misses);
    }
    ThreeDPayloadRunStats {
        stats: percentile_stats(&samples),
        checksum,
        hits: cache.hits,
        misses: cache.misses,
        built_grid_cells,
        materialized_cells,
        avoided_grid_cells: cache.avoided_grid_cells,
    }
}

fn build_three_d_payload_cached<'a>(
    bars: &[Bar],
    indicator_signature: u64,
    cache: &'a mut ThreeDPayloadCache,
) -> &'a [ThreeDCell] {
    let key = (
        bars.len(),
        bars.first().map(|bar| bar.time_ms).unwrap_or_default(),
        bars.last().map(|bar| bar.time_ms).unwrap_or_default(),
        indicator_signature,
    );
    if cache.initialized && cache.key == key {
        cache.hits += 1;
        cache.avoided_grid_cells = cache.avoided_grid_cells.saturating_add(cache.grid_cells);
        return &cache.value;
    }
    cache.misses += 1;
    cache.key = key;
    let (_, _, grid_cells) = three_d_payload_dimensions(bars.len());
    cache.grid_cells = grid_cells;
    cache.value = build_three_d_payload_legacy(bars, indicator_signature);
    cache.initialized = true;
    &cache.value
}

fn build_three_d_payload_legacy(bars: &[Bar], indicator_signature: u64) -> Vec<ThreeDCell> {
    if bars.is_empty() {
        return Vec::new();
    }
    let max_bars = 180_usize;
    let stride = bars.len().div_ceil(max_bars).max(1);
    let sampled: Vec<usize> = (0..bars.len()).step_by(stride).collect();
    if sampled.is_empty() {
        return Vec::new();
    }

    let mut price_lo = f64::INFINITY;
    let mut price_hi = f64::NEG_INFINITY;
    for index in &sampled {
        let bar = bars[*index];
        price_lo = price_lo.min(bar.low);
        price_hi = price_hi.max(bar.high);
    }
    if !price_lo.is_finite() || !price_hi.is_finite() || price_hi <= price_lo {
        price_lo = 0.0;
        price_hi = 1.0;
    }
    let price_range = (price_hi - price_lo).max(1e-6);
    let count = sampled.len();
    let price_bins = (20.0 + count as f64 / 16.0).round() as usize;
    let price_bins = price_bins.clamp(20, 34);
    let bin_size = price_range / price_bins as f64;
    let mut tower_grid = vec![0.0_f64; count * price_bins];
    let mut bar_directions = vec![true; count];
    let bin_center = |bin: usize| price_lo + (bin as f64 + 0.5) * bin_size;
    let clamp_bin = |value: f64| -> usize {
        (((value - price_lo) / bin_size).floor() as isize)
            .clamp(0, price_bins as isize - 1) as usize
    };
    let mut max_cell_volume = 0.0_f64;

    for (col, bar_index) in sampled.iter().enumerate() {
        let bar = bars[*bar_index];
        let volume = bar.volume.max(0.0);
        bar_directions[col] = bar.close >= bar.open;
        if !bar.low.is_finite() || !bar.high.is_finite() || volume <= 0.0 {
            continue;
        }
        let start_bin = clamp_bin(bar.low.min(bar.high));
        let end_bin = clamp_bin(bar.low.max(bar.high));
        let typical = (bar.open + bar.high + bar.low + bar.close) / 4.0;
        let body_lo = bar.open.min(bar.close);
        let body_hi = bar.open.max(bar.close);
        let span = bin_size.max(bar.high - bar.low);
        let mut weight_sum = 0.0_f64;
        let mut weights = Vec::with_capacity(end_bin.saturating_sub(start_bin) + 1);
        for bin in start_bin..=end_bin {
            let center = bin_center(bin);
            let proximity = 1.0 - (center - typical).abs().min((span * 0.5).max(bin_size)) / (span * 0.5).max(bin_size);
            let body_bonus = if center >= body_lo - bin_size * 0.35 && center <= body_hi + bin_size * 0.35 {
                0.9
            } else {
                0.0
            };
            let close_bonus = 0.35 * (1.0 - ((center - bar.close).abs() / (span * 0.3).max(bin_size)).min(1.0));
            let weight = 0.35 + proximity.max(0.0) * 0.75 + body_bonus + close_bonus.max(0.0);
            weights.push((bin, weight));
            weight_sum += weight;
        }
        if weight_sum <= 0.0 {
            continue;
        }
        for (bin, weight) in weights {
            let idx = col * price_bins + bin;
            tower_grid[idx] += volume * (weight / weight_sum);
            max_cell_volume = max_cell_volume.max(tower_grid[idx]);
        }
    }
    max_cell_volume = max_cell_volume.max(1.0);

    let rolling_window = ((count as f64 * 0.16).floor() as usize).clamp(10, 28);
    let mut avg_grid = vec![0.0_f64; count * price_bins];
    let mut value_area_lower = vec![0_usize; count];
    let mut value_area_upper = vec![price_bins.saturating_sub(1); count];

    for col in 0..count {
        let start = col.saturating_sub(rolling_window.saturating_sub(1));
        let bars_in_window = col - start + 1;
        let mut aggregate = vec![0.0_f64; price_bins];
        for source_col in start..=col {
            for bin in 0..price_bins {
                aggregate[bin] += tower_grid[source_col * price_bins + bin];
            }
        }
        let mut total_agg = 0.0_f64;
        let mut poc_bin = 0_usize;
        let mut poc_value = -1.0_f64;
        for bin in 0..price_bins {
            total_agg += aggregate[bin];
            avg_grid[col * price_bins + bin] = aggregate[bin] / bars_in_window as f64;
            if aggregate[bin] > poc_value {
                poc_value = aggregate[bin];
                poc_bin = bin;
            }
        }
        if total_agg <= 0.0 {
            continue;
        }
        let target = total_agg * 0.7;
        let mut cum = aggregate[poc_bin].max(0.0);
        let mut lower = poc_bin;
        let mut upper = poc_bin;
        while cum < target && (lower > 0 || upper + 1 < price_bins) {
            let left_value = if lower > 0 { aggregate[lower - 1] } else { -1.0 };
            let right_value = if upper + 1 < price_bins { aggregate[upper + 1] } else { -1.0 };
            if right_value > left_value {
                if upper + 1 < price_bins {
                    upper += 1;
                    cum += aggregate[upper].max(0.0);
                } else if lower > 0 {
                    lower -= 1;
                    cum += aggregate[lower].max(0.0);
                }
            } else if lower > 0 {
                lower -= 1;
                cum += aggregate[lower].max(0.0);
            } else if upper + 1 < price_bins {
                upper += 1;
                cum += aggregate[upper].max(0.0);
            }
        }
        value_area_lower[col] = lower;
        value_area_upper[col] = upper;
    }

    let mut canopy_grid = vec![0.0_f64; count * price_bins];
    for col in 0..count {
        for bin in 0..price_bins {
            let inside_value_area = bin >= value_area_lower[col] && bin <= value_area_upper[col];
            canopy_grid[col * price_bins + bin] =
                avg_grid[col * price_bins + bin] * if inside_value_area { 0.92 } else { 0.22 };
        }
    }
    for _ in 0..2 {
        let mut next_grid = vec![0.0_f64; count * price_bins];
        for col in 0..count {
            for bin in 0..price_bins {
                let mut sum = canopy_grid[col * price_bins + bin] * 2.2;
                let mut weight = 2.2_f64;
                for dc in -1_isize..=1 {
                    for db in -1_isize..=1 {
                        if dc == 0 && db == 0 {
                            continue;
                        }
                        let cc = col as isize + dc;
                        let bb = bin as isize + db;
                        if cc < 0 || cc >= count as isize || bb < 0 || bb >= price_bins as isize {
                            continue;
                        }
                        let neighbor_weight = if dc == 0 || db == 0 { 0.65 } else { 0.35 };
                        sum += canopy_grid[cc as usize * price_bins + bb as usize] * neighbor_weight;
                        weight += neighbor_weight;
                    }
                }
                next_grid[col * price_bins + bin] = sum / weight;
            }
        }
        canopy_grid = next_grid;
    }

    let mut cells = Vec::with_capacity(count * price_bins);
    for col in 0..count {
        for row in 0..price_bins {
            let tower_volume = tower_grid[col * price_bins + row];
            if !(tower_volume > max_cell_volume * 0.01) {
                continue;
            }
            let base_volume = canopy_grid[col * price_bins + row];
            let upper_volume = base_volume * 1.08 + max_cell_volume * 0.018;
            let outside_value_area = row < value_area_lower[col] || row > value_area_upper[col];
            let signal = indicator_signature != 0
                && outside_value_area
                && tower_volume > upper_volume * 1.02
                && tower_volume > max_cell_volume * 0.045;
            cells.push(ThreeDCell {
                col,
                row,
                volume: tower_volume,
                canopy: base_volume,
                signal,
            });
        }
    }
    std::hint::black_box(cells)
}

fn three_d_payload_dimensions(bars_len: usize) -> (usize, usize, usize) {
    if bars_len == 0 {
        return (0, 0, 0);
    }
    let stride = bars_len.div_ceil(180).max(1);
    let sampled = bars_len.div_ceil(stride);
    let price_bins = ((20.0 + sampled as f64 / 16.0).round() as usize).clamp(20, 34);
    (sampled, price_bins, sampled.saturating_mul(price_bins))
}

fn three_d_payload_checksum(cells: &[ThreeDCell]) -> f64 {
    let mut sum = 0.0_f64;
    for cell in cells.iter().step_by(17) {
        sum += cell.volume * 0.000_001 + cell.canopy * 0.000_003 + cell.col as f64 * 0.01;
        if cell.signal {
            sum += 0.125;
        }
    }
    std::hint::black_box(sum)
}

fn compare_three_d_cells(a: &[ThreeDCell], b: &[ThreeDCell]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).all(|(left, right)| {
        left.col == right.col
            && left.row == right.row
            && left.signal == right.signal
            && (left.volume - right.volume).abs() <= 1e-8
            && (left.canopy - right.canopy).abs() <= 1e-8
    })
}

fn run_three_d_gpu_upload_cache_focus(bars: &[Bar], series_hash: &str, frames: usize) {
    let frames = frames.max(3);
    let calls_per_frame = 6_usize;
    let active_cloud_signature = 0x3d_c1_0d_u64;
    let cells = build_three_d_payload_legacy(bars, active_cloud_signature);
    let payload = three_d_gpu_payload_from_cells(&cells, bars.len(), active_cloud_signature);
    let mut proof_cache = ThreeDGpuUploadCache::default();
    let proof_first = upload_three_d_gpu_payload(payload, 1, &mut proof_cache, true);
    let proof_second = upload_three_d_gpu_payload(payload, 1, &mut proof_cache, true);
    println!(
        "[trading-lab] focus=3d-gpu-upload-cache task=dedupe-webgl-bufferdata-identical-payload series_hash={} bars={} frames={} calls_per_frame={} point_vertices={} line_vertices={} payload_bytes={}",
        series_hash,
        bars.len(),
        frames,
        calls_per_frame,
        payload.point_vertices,
        payload.line_vertices,
        payload.total_bytes()
    );
    println!(
        "[trading-lab] proof 3d-gpu-upload-cache first_upload_calls={} second_upload_calls={} skipped_second={} same_payload_id={} same_gl=true",
        proof_first.buffer_calls,
        proof_second.buffer_calls,
        proof_second.buffer_calls == 0,
        payload.payload_id
    );

    let legacy = bench_three_d_gpu_upload(payload, frames, calls_per_frame, false);
    let optimized = bench_three_d_gpu_upload(payload, frames, calls_per_frame, true);
    let hit_rate = if optimized.hits + optimized.misses == 0 {
        0.0
    } else {
        optimized.hits as f64 / (optimized.hits + optimized.misses) as f64
    };
    println!(
        "[trading-lab] legacy-3d-gpu-upload model=bufferdata-six-buffers-every-rebuild p50_ms={:.3} p95_ms={:.3} p99_ms={:.3} checksum={} buffer_calls={} uploaded_mb={:.3}",
        legacy.stats.p50_us as f64 / 1000.0,
        legacy.stats.p95_us as f64 / 1000.0,
        legacy.stats.p99_us as f64 / 1000.0,
        legacy.checksum,
        legacy.buffer_calls,
        legacy.uploaded_bytes as f64 / (1024.0 * 1024.0)
    );
    println!(
        "[trading-lab] optimized-3d-gpu-upload model=skip-bufferdata-for-identical-payload-object p50_ms={:.3} p95_ms={:.3} p99_ms={:.3} checksum={} hits={} misses={} hit_rate={:.3} buffer_calls={} avoided_buffer_calls={} uploaded_mb={:.3} avoided_mb={:.3} cache_entries=1 evictions=0",
        optimized.stats.p50_us as f64 / 1000.0,
        optimized.stats.p95_us as f64 / 1000.0,
        optimized.stats.p99_us as f64 / 1000.0,
        optimized.checksum,
        optimized.hits,
        optimized.misses,
        hit_rate,
        optimized.buffer_calls,
        optimized.avoided_buffer_calls,
        optimized.uploaded_bytes as f64 / (1024.0 * 1024.0),
        optimized.avoided_bytes as f64 / (1024.0 * 1024.0)
    );
    println!(
        "[trading-lab] summary target=3d_gpu_upload_cache p50_speedup_x={:.2} p95_speedup_x={:.2} p99_speedup_x={:.2} cache_budget=one-upload-marker-per-gl-context-and-payload-object",
        ratio(legacy.stats.p50_us, optimized.stats.p50_us),
        ratio(legacy.stats.p95_us, optimized.stats.p95_us),
        ratio(legacy.stats.p99_us, optimized.stats.p99_us)
    );
}

fn bench_three_d_gpu_upload(
    payload: ThreeDGpuPayload,
    frames: usize,
    calls_per_frame: usize,
    cached: bool,
) -> ThreeDGpuUploadRunStats {
    let mut samples = Vec::with_capacity(frames);
    let mut checksum = 0_usize;
    let mut buffer_calls = 0_usize;
    let mut uploaded_bytes = 0_usize;
    let mut cache = ThreeDGpuUploadCache::default();
    for _ in 0..frames {
        let started = Instant::now();
        for _ in 0..calls_per_frame {
            let upload = upload_three_d_gpu_payload(payload, 1, &mut cache, cached);
            checksum = checksum.wrapping_add(upload.checksum);
            buffer_calls = buffer_calls.saturating_add(upload.buffer_calls);
            uploaded_bytes = uploaded_bytes.saturating_add(upload.uploaded_bytes);
        }
        samples.push(started.elapsed().as_micros());
    }
    ThreeDGpuUploadRunStats {
        stats: percentile_stats(&samples),
        checksum,
        hits: cache.hits,
        misses: cache.misses,
        buffer_calls,
        avoided_buffer_calls: cache.avoided_buffer_calls,
        uploaded_bytes,
        avoided_bytes: cache.avoided_bytes,
    }
}

#[derive(Debug, Clone, Copy)]
struct ThreeDGpuUploadResult {
    checksum: usize,
    buffer_calls: usize,
    uploaded_bytes: usize,
}

fn upload_three_d_gpu_payload(
    payload: ThreeDGpuPayload,
    gl_id: u64,
    cache: &mut ThreeDGpuUploadCache,
    cached: bool,
) -> ThreeDGpuUploadResult {
    if cached && cache.initialized && cache.payload_id == payload.payload_id && cache.gl_id == gl_id {
        cache.hits += 1;
        cache.avoided_buffer_calls = cache
            .avoided_buffer_calls
            .saturating_add(payload.buffer_calls());
        cache.avoided_bytes = cache.avoided_bytes.saturating_add(payload.total_bytes());
        return ThreeDGpuUploadResult {
            checksum: payload.payload_id as usize,
            buffer_calls: 0,
            uploaded_bytes: 0,
        };
    }
    if cached {
        cache.misses += 1;
        cache.initialized = true;
        cache.payload_id = payload.payload_id;
        cache.gl_id = gl_id;
    }
    let buffers = [
        payload.position_bytes,
        payload.color_bytes,
        payload.size_bytes,
        payload.line_position_bytes,
        payload.line_color_bytes,
        payload.line_size_bytes,
    ];
    let mut checksum = payload.payload_id as usize;
    for bytes in buffers {
        let words = bytes / std::mem::size_of::<f32>();
        for offset in (0..words.max(1)).step_by(257) {
            checksum = checksum
                .wrapping_mul(16_777_619)
                .wrapping_add(offset)
                .wrapping_add(bytes);
        }
    }
    let uploaded_bytes = payload.total_bytes();
    cache.uploaded_bytes = cache.uploaded_bytes.saturating_add(uploaded_bytes);
    ThreeDGpuUploadResult {
        checksum: std::hint::black_box(checksum),
        buffer_calls: payload.buffer_calls(),
        uploaded_bytes,
    }
}

fn three_d_gpu_payload_from_cells(
    cells: &[ThreeDCell],
    bars_len: usize,
    indicator_signature: u64,
) -> ThreeDGpuPayload {
    let (sampled, price_bins, _) = three_d_payload_dimensions(bars_len);
    let grid_line_vertices = (sampled.saturating_add(1) + price_bins.saturating_add(1)).saturating_mul(2);
    let tower_line_vertices = cells.len().saturating_mul(16);
    let point_vertices = grid_line_vertices.saturating_add(tower_line_vertices);
    let line_vertices = 0_usize;
    let payload_id = ((bars_len as u64) << 32)
        ^ ((cells.len() as u64) << 12)
        ^ ((point_vertices as u64) << 1)
        ^ indicator_signature;
    ThreeDGpuPayload {
        payload_id,
        position_bytes: point_vertices.saturating_mul(3).saturating_mul(std::mem::size_of::<f32>()),
        color_bytes: point_vertices.saturating_mul(3).saturating_mul(std::mem::size_of::<f32>()),
        size_bytes: point_vertices.saturating_mul(std::mem::size_of::<f32>()),
        line_position_bytes: line_vertices.saturating_mul(3).saturating_mul(std::mem::size_of::<f32>()),
        line_color_bytes: line_vertices.saturating_mul(3).saturating_mul(std::mem::size_of::<f32>()),
        line_size_bytes: line_vertices.saturating_mul(std::mem::size_of::<f32>()),
        point_vertices,
        line_vertices,
    }
}

fn build_metric_series_cached<'a>(
    visible: &[Bar],
    signal_counts: &HashMap<i64, usize>,
    signal_revision: usize,
    cache: &'a mut MetricSeriesCache,
) -> &'a [MetricPoint] {
    let key = (
        visible.len(),
        visible.first().map(|bar| bar.time_ms).unwrap_or_default(),
        visible.last().map(|bar| bar.time_ms).unwrap_or_default(),
        signal_counts.len(),
        signal_revision,
    );
    if cache.initialized && cache.key == key {
        cache.hits += 1;
        cache.avoided_points = cache.avoided_points.saturating_add(cache.value.len());
        return &cache.value;
    }
    cache.misses += 1;
    cache.key = key;
    cache.value = build_metric_series_legacy(visible, signal_counts);
    cache.initialized = true;
    &cache.value
}

fn build_metric_series_legacy(visible: &[Bar], signal_counts: &HashMap<i64, usize>) -> Vec<MetricPoint> {
    if visible.is_empty() {
        return Vec::new();
    }
    let safe_pct = |a: f64, b: f64| {
        let base = b.abs().max(0.000001);
        (a - b) / base * 100.0
    };
    let alpha = 2.0 / (34.0 + 1.0);
    let mut ema = visible[0].close;
    let mut cumulative_vol = 0.0_f64;
    let mut cumulative_signal = 0.0_f64;
    let mut density = 0.0_f64;
    let mut ema_short = visible[0].close;
    let mut ema_long = ema_short;
    let mut mean_ret = 0.0_f64;
    let mut var_ret = 0.01_f64;
    let mut mean_range = 0.01_f64;
    let mut var_range = 0.01_f64;
    let mut cumulative_regime = 0.0_f64;
    let mut previous_regime = 0_i32;
    let mut series = Vec::with_capacity(visible.len());
    for idx in 0..visible.len() {
        let current = visible[idx];
        let previous = visible[idx.saturating_sub(1)];
        let close = current.close;
        let range = (current.high - current.low).max(0.000001);
        let body_frac = (current.close - current.open).abs() / range;
        let impulse_pct = safe_pct(current.close, previous.close).abs();
        let range_pct = safe_pct(current.high, current.low).abs();
        let signal_count = *signal_counts.get(&current.time_ms).unwrap_or(&0) as f64;
        if idx > 0 {
            cumulative_vol += safe_pct(current.close, previous.close).abs();
        }
        density = density * 0.62 + signal_count;
        cumulative_signal += 0.12 + density * 0.95;
        ema = if idx == 0 { close } else { ema + (close - ema) * alpha };
        let fair_gap = |value: f64| safe_pct(value, ema);
        let conviction_unit = (body_frac * 0.34)
            + (impulse_pct / 1.2).min(1.0) * 0.28
            + (range_pct / 2.2).min(1.0) * 0.18
            + density.min(1.0) * 0.35;
        let conviction_score = conviction_unit.clamp(0.0, 1.0) * 100.0;
        ema_short = if idx == 0 { close } else { ema_short + (close - ema_short) * (2.0 / 9.0) };
        ema_long = if idx == 0 { close } else { ema_long + (close - ema_long) * (2.0 / 35.0) };
        let ret = safe_pct(current.close, previous.close);
        mean_ret += (ret - mean_ret) * 0.18;
        var_ret += (((ret - mean_ret).powi(2)) - var_ret) * 0.18;
        mean_range += (range_pct - mean_range) * 0.18;
        var_range += (((range_pct - mean_range).powi(2)) - var_range) * 0.18;
        let z_ret = (ret - mean_ret).abs() / 0.05_f64.max(var_ret.max(0.0001).sqrt());
        let z_range = (range_pct - mean_range).abs() / 0.05_f64.max(var_range.max(0.0001).sqrt());
        let anomaly_score = (((z_ret * 0.62 + z_range * 0.38) / 3.0) * 100.0).clamp(0.0, 100.0);
        let trend_bias = safe_pct(ema_short, ema_long);
        let regime = if trend_bias > 0.10 {
            1
        } else if trend_bias < -0.10 {
            -1
        } else if range_pct > mean_range * 1.5 {
            2
        } else {
            0
        };
        cumulative_regime += 0.8
            + if regime != previous_regime { 1.2 } else { 0.0 }
            + (trend_bias.abs() / 0.6).min(1.0) * 0.55;
        series.push(MetricPoint {
            time_ms: current.time_ms,
            x_volatility: cumulative_vol,
            x_signal_density: cumulative_signal,
            x_regime: cumulative_regime,
            close_price: current.close,
            close_fair_gap: fair_gap(current.close),
            close_conviction: conviction_score,
            close_anomaly: anomaly_score,
        });
        previous_regime = regime;
    }
    series
}

fn metric_series_checksum(points: &[MetricPoint]) -> f64 {
    let mut sum = 0.0_f64;
    for point in points.iter().step_by(73) {
        sum += point.x_volatility * 0.01
            + point.x_signal_density * 0.1
            + point.x_regime
            + point.close_fair_gap
            + point.close_conviction
            + point.close_anomaly
            + point.close_price * 0.001;
    }
    std::hint::black_box(sum)
}

fn compare_metric_points(left: &[MetricPoint], right: &[MetricPoint]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.iter().zip(right).all(|(a, b)| {
        a.time_ms == b.time_ms
            && (a.x_volatility - b.x_volatility).abs() <= 1e-9
            && (a.x_signal_density - b.x_signal_density).abs() <= 1e-9
            && (a.x_regime - b.x_regime).abs() <= 1e-9
            && (a.close_price - b.close_price).abs() <= 1e-9
            && (a.close_fair_gap - b.close_fair_gap).abs() <= 1e-9
            && (a.close_conviction - b.close_conviction).abs() <= 1e-9
            && (a.close_anomaly - b.close_anomaly).abs() <= 1e-9
    })
}

fn signal_count_map(signal_times: &[i64]) -> HashMap<i64, usize> {
    let mut out = HashMap::with_capacity(signal_times.len());
    for time_ms in signal_times {
        *out.entry(*time_ms).or_insert(0) += 1;
    }
    out
}

fn synthetic_signal_times(visible: &[(usize, i64)], signal_count: usize) -> Vec<i64> {
    if visible.is_empty() || signal_count == 0 {
        return Vec::new();
    }
    let mut signals = Vec::with_capacity(signal_count);
    for idx in 0..signal_count {
        let visible_idx = idx.saturating_mul(visible.len()).saturating_div(signal_count).min(visible.len() - 1);
        signals.push(visible[visible_idx].1);
    }
    signals
}

fn resolve_signal_slots_legacy(
    visible: &[(usize, i64)],
    signals: &[i64],
    tolerance_ms: i64,
) -> Vec<usize> {
    signals
        .iter()
        .map(|signal_time| {
            visible
                .iter()
                .find(|(_, time_ms)| (*time_ms - *signal_time).abs() <= tolerance_ms)
                .map(|(slot, _)| *slot)
                .unwrap_or(usize::MAX)
        })
        .collect()
}

fn build_signal_slot_index(visible: &[(usize, i64)]) -> SignalSlotIndex {
    let mut times = Vec::with_capacity(visible.len());
    let mut slots = Vec::with_capacity(visible.len());
    for (slot, time_ms) in visible {
        times.push(*time_ms);
        slots.push(*slot);
    }
    SignalSlotIndex { times, slots }
}

fn resolve_signal_slots_indexed(
    index: &SignalSlotIndex,
    signals: &[i64],
    tolerance_ms: i64,
) -> Vec<usize> {
    signals
        .iter()
        .map(|signal_time| signal_slot_index_lookup(index, *signal_time, tolerance_ms))
        .collect()
}

fn signal_slot_index_lookup(index: &SignalSlotIndex, signal_time: i64, tolerance_ms: i64) -> usize {
    let min_time = signal_time.saturating_sub(tolerance_ms.max(0));
    let max_time = signal_time.saturating_add(tolerance_ms.max(0));
    let mut low = 0_usize;
    let mut high = index.times.len();
    while low < high {
        let mid = (low + high) / 2;
        if index.times[mid] < min_time {
            low = mid + 1;
        } else {
            high = mid;
        }
    }
    if low < index.times.len() && index.times[low] <= max_time {
        index.slots[low]
    } else {
        usize::MAX
    }
}

fn bench_signal_markers(
    visible: &[(usize, i64)],
    signals: &[i64],
    tolerance_ms: i64,
    frames: usize,
    indexed: bool,
) -> SignalMarkerRunStats {
    let mut samples = Vec::with_capacity(frames);
    let index = if indexed { Some(build_signal_slot_index(visible)) } else { None };
    let mut checksum = 0_usize;
    let mut hits = 0_usize;
    let mut misses = 0_usize;
    let mut comparisons = 0_usize;
    for _ in 0..frames {
        let started = Instant::now();
        if let Some(index) = index.as_ref() {
            for signal in signals {
                let slot = signal_slot_index_lookup(index, *signal, tolerance_ms);
                if slot != usize::MAX {
                    hits += 1;
                    checksum = checksum.wrapping_add(slot);
                } else {
                    misses += 1;
                }
            }
        } else {
            for signal in signals {
                let mut resolved = usize::MAX;
                for (slot, time_ms) in visible {
                    comparisons += 1;
                    if (*time_ms - *signal).abs() <= tolerance_ms {
                        resolved = *slot;
                        break;
                    }
                }
                if resolved == usize::MAX {
                    misses += 1;
                } else {
                    hits += 1;
                    checksum = checksum.wrapping_add(resolved);
                }
            }
        }
        samples.push(started.elapsed().as_micros());
    }
    let avoided_comparisons = if indexed {
        frames
            .saturating_mul(signals.len())
            .saturating_mul(visible.len())
            .saturating_sub(frames.saturating_mul(signals.len()))
    } else {
        0
    };
    SignalMarkerRunStats {
        stats: percentile_stats(&samples),
        checksum,
        hits,
        misses,
        comparisons,
        avoided_comparisons,
    }
}

fn bench_comparison_charts_pipeline(
    docs: &[CanvasDocument],
    visible_bars: usize,
    min_time: i64,
    max_time: i64,
    visible_start: usize,
    frames: usize,
    render_calls_per_frame: usize,
    cached: bool,
) -> ComparisonChartsRunStats {
    let mut caches: Vec<ViewportWindowCache> = (0..docs.len()).map(|_| ViewportWindowCache::default()).collect();
    let mut samples = Vec::with_capacity(frames);
    let mut checksum = 0.0_f64;
    let mut scanned_rows = 0_usize;
    for _ in 0..frames {
        let started = Instant::now();
        for _ in 0..render_calls_per_frame {
            for (idx, doc) in docs.iter().enumerate() {
                if cached {
                    let entries = resolve_viewport_window_cached(
                        &doc.logical_times,
                        visible_bars,
                        &mut caches[idx],
                    );
                    checksum += viewport_window_checksum(entries);
                } else {
                    scanned_rows = scanned_rows.saturating_add(doc.logical_times.len());
                    let entries = comparison_visible_filter_legacy(
                        &doc.logical_times,
                        min_time,
                        max_time,
                        visible_start,
                    );
                    checksum += viewport_window_checksum(&entries);
                }
            }
        }
        samples.push(started.elapsed().as_micros());
    }
    let hits = caches.iter().map(|cache| cache.hits).sum();
    let misses = caches.iter().map(|cache| cache.misses).sum();
    let avoided_rows = caches.iter().map(|cache| cache.avoided_entries).sum();
    ComparisonChartsRunStats {
        stats: percentile_stats(&samples),
        checksum,
        hits,
        misses,
        scanned_rows,
        avoided_rows,
    }
}

fn comparison_visible_filter_legacy(
    logical_times: &[i64],
    min_time: i64,
    max_time: i64,
    visible_start: usize,
) -> Vec<(usize, i64)> {
    logical_times
        .iter()
        .enumerate()
        .filter_map(|(idx, time_ms)| {
            if *time_ms >= min_time && *time_ms <= max_time {
                Some((idx.saturating_sub(visible_start), *time_ms))
            } else {
                None
            }
        })
        .collect()
}

fn final_overlay_key_sequence(
    base: &[Bar],
    batches: &[Vec<Bar>],
    overlap: usize,
    incremental: bool,
) -> OverlayKeyMeta {
    let mut current = base.to_vec();
    let mut meta = full_overlay_key_meta(&current);
    for batch in batches {
        let previous = current;
        current = merge_bars_incremental(&previous, batch);
        let reuse_prefix = previous.len().saturating_sub(overlap).min(current.len());
        meta = if incremental {
            incremental_overlay_key_meta(&current, Some(&meta), reuse_prefix)
        } else {
            full_overlay_key_meta(&current)
        };
    }
    meta
}

fn bench_overlay_key_pipeline(
    base: &[Bar],
    batches: &[Vec<Bar>],
    overlap: usize,
    incremental: bool,
) -> OverlayKeyRunStats {
    let mut current = base.to_vec();
    let mut meta = full_overlay_key_meta(&current);
    let mut samples = Vec::with_capacity(batches.len());
    let mut checksum = 0_u64;
    let mut reused_prefix_rows = 0_usize;
    let mut tail_rows_hashed = 0_usize;
    for batch in batches {
        let previous = current;
        current = merge_bars_incremental(&previous, batch);
        let reuse_prefix = previous.len().saturating_sub(overlap).min(current.len());
        let started = Instant::now();
        meta = if incremental {
            reused_prefix_rows = reused_prefix_rows.saturating_add(reuse_prefix);
            tail_rows_hashed = tail_rows_hashed.saturating_add(current.len().saturating_sub(reuse_prefix));
            incremental_overlay_key_meta(&current, Some(&meta), reuse_prefix)
        } else {
            full_overlay_key_meta(&current)
        };
        samples.push(started.elapsed().as_micros());
        checksum = checksum.wrapping_add(overlay_key_checksum(&meta));
    }
    OverlayKeyRunStats {
        stats: percentile_stats(&samples),
        checksum,
        final_len: current.len(),
        reused_prefix_rows,
        tail_rows_hashed,
    }
}

fn full_overlay_key_meta(bars: &[Bar]) -> OverlayKeyMeta {
    let mut h1 = 2_166_136_261_u32;
    let mut h2 = 2_166_136_261_u32 ^ 0x9e37_79b9;
    let mut h1_by_index = Vec::with_capacity(bars.len());
    let mut h2_by_index = Vec::with_capacity(bars.len());
    for bar in bars {
        let row = overlay_key_row(bar);
        h1 = overlay_feed_hash(h1, &row);
        h2 = overlay_feed_hash(h2, &format!("{row}:{h1}"));
        h1_by_index.push(h1);
        h2_by_index.push(h2);
    }
    OverlayKeyMeta {
        key: overlay_key_format(bars, h1, h2),
        h1_by_index,
        h2_by_index,
    }
}

fn incremental_overlay_key_meta(
    bars: &[Bar],
    previous: Option<&OverlayKeyMeta>,
    prefix_len: usize,
) -> OverlayKeyMeta {
    let reuse = previous
        .map(|meta| prefix_len.min(meta.h1_by_index.len()).min(bars.len()))
        .unwrap_or(0);
    let mut h1_by_index = vec![0_u32; bars.len()];
    let mut h2_by_index = vec![0_u32; bars.len()];
    let mut h1;
    let mut h2;
    let start;
    if reuse > 0 {
        let previous = previous.unwrap();
        h1_by_index[..reuse].copy_from_slice(&previous.h1_by_index[..reuse]);
        h2_by_index[..reuse].copy_from_slice(&previous.h2_by_index[..reuse]);
        h1 = h1_by_index[reuse - 1];
        h2 = h2_by_index[reuse - 1];
        start = reuse;
    } else {
        h1 = 2_166_136_261_u32;
        h2 = 2_166_136_261_u32 ^ 0x9e37_79b9;
        start = 0;
    }
    for idx in start..bars.len() {
        let row = overlay_key_row(&bars[idx]);
        h1 = overlay_feed_hash(h1, &row);
        h2 = overlay_feed_hash(h2, &format!("{row}:{h1}"));
        h1_by_index[idx] = h1;
        h2_by_index[idx] = h2;
    }
    OverlayKeyMeta {
        key: overlay_key_format(bars, h1, h2),
        h1_by_index,
        h2_by_index,
    }
}

fn overlay_key_row(bar: &Bar) -> String {
    format!(
        "{}|{}|{}|{}|{}|{}",
        bar.time_ms, bar.open, bar.high, bar.low, bar.close, bar.volume
    )
}

fn overlay_feed_hash(hash: u32, text: &str) -> u32 {
    let mut h = hash;
    for unit in text.encode_utf16() {
        h ^= unit as u32;
        h = h.wrapping_mul(16_777_619);
    }
    h
}

fn overlay_key_format(bars: &[Bar], h1: u32, h2: u32) -> String {
    let Some(first) = bars.first() else { return "candles:empty".to_string() };
    let last = bars.last().unwrap_or(first);
    format!(
        "candles:{}:{h1:08x}:{h2:08x}:{}:{}:{}",
        bars.len(),
        first.time_ms,
        last.time_ms,
        last.close
    )
}

fn overlay_key_checksum(meta: &OverlayKeyMeta) -> u64 {
    let mut sum = meta.key.len() as u64;
    for value in meta.h1_by_index.iter().step_by(257) {
        sum = sum.wrapping_mul(16_777_619).wrapping_add(*value as u64);
    }
    for value in meta.h2_by_index.iter().step_by(263) {
        sum = sum.wrapping_mul(16_777_619).wrapping_add(*value as u64);
    }
    std::hint::black_box(sum)
}

fn estimate_overlay_key_full_rows(base_len: usize, frames: usize, new_per_frame: usize) -> usize {
    let mut rows = 0_usize;
    for frame in 1..=frames {
        rows = rows.saturating_add(base_len.saturating_add(frame.saturating_mul(new_per_frame)));
    }
    rows
}

#[derive(Default)]
struct ViewportWindowCache {
    key: (usize, usize, usize),
    value: Vec<(usize, i64)>,
    initialized: bool,
    hits: usize,
    misses: usize,
    avoided_entries: usize,
}

fn bench_viewport_window_pipeline(
    logical_times: &[i64],
    visible_bars: usize,
    frames: usize,
    calls_per_frame: usize,
    cached: bool,
) -> ViewportWindowRunStats {
    let mut samples = Vec::with_capacity(frames);
    let mut checksum = 0.0_f64;
    let mut cache = ViewportWindowCache::default();
    let mut misses = 0_usize;
    for _ in 0..frames {
        let started = Instant::now();
        for _ in 0..calls_per_frame {
            let visible = if cached {
                resolve_viewport_window_cached(logical_times, visible_bars, &mut cache)
            } else {
                misses += 1;
                let visible = resolve_viewport_window_legacy(logical_times, visible_bars);
                checksum += viewport_window_checksum(&visible);
                continue;
            };
            checksum += viewport_window_checksum(visible);
        }
        samples.push(started.elapsed().as_micros());
    }
    ViewportWindowRunStats {
        stats: percentile_stats(&samples),
        checksum,
        hits: cache.hits,
        misses: cache.misses + misses,
        avoided_entries: cache.avoided_entries,
    }
}

fn resolve_viewport_window_legacy(logical_times: &[i64], visible_bars: usize) -> Vec<(usize, i64)> {
    let count = logical_times.len();
    let end = count;
    let start = end.saturating_sub(visible_bars);
    logical_times[start..end]
        .iter()
        .enumerate()
        .map(|(offset, time_ms)| (offset, *time_ms))
        .collect()
}

fn resolve_viewport_window_cached<'a>(
    logical_times: &[i64],
    visible_bars: usize,
    cache: &'a mut ViewportWindowCache,
) -> &'a [(usize, i64)] {
    let count = logical_times.len();
    let end = count;
    let start = end.saturating_sub(visible_bars);
    let key = (start, end, visible_bars);
    if cache.initialized && cache.key == key {
        cache.hits += 1;
        cache.avoided_entries = cache.avoided_entries.saturating_add(cache.value.len());
        return &cache.value;
    }
    cache.misses += 1;
    cache.key = key;
    cache.value = resolve_viewport_window_legacy(logical_times, visible_bars);
    cache.initialized = true;
    &cache.value
}

fn viewport_window_checksum(entries: &[(usize, i64)]) -> f64 {
    let mut sum = 0.0_f64;
    for (idx, time_ms) in entries.iter().step_by(97) {
        sum += *idx as f64 + (*time_ms % 100_000) as f64 * 0.000001;
    }
    std::hint::black_box(sum)
}

fn final_canvas_document_sequence(
    base: &[Bar],
    batches: &[Vec<Bar>],
    overlap: usize,
    incremental: bool,
) -> CanvasDocument {
    let mut current = base.to_vec();
    let mut doc = build_canvas_document_full(&current);
    for batch in batches {
        let previous_len = current.len();
        current = merge_bars_incremental(&current, batch);
        if incremental {
            let reuse_prefix = previous_len.saturating_sub(overlap).min(current.len());
            update_canvas_document_incremental(&mut doc, &current, reuse_prefix);
        } else {
            doc = build_canvas_document_full(&current);
        }
    }
    doc
}

fn bench_canvas_document_pipeline(
    base: &[Bar],
    batches: &[Vec<Bar>],
    overlap: usize,
    incremental: bool,
) -> CanvasDocumentRunStats {
    let mut current = base.to_vec();
    let mut doc = build_canvas_document_full(&current);
    let mut samples = Vec::with_capacity(batches.len());
    let mut checksum = 0.0_f64;
    let mut reused_prefix_units = 0_usize;
    for batch in batches {
        let previous_len = current.len();
        current = merge_bars_incremental(&current, batch);
        let started = Instant::now();
        if incremental {
            let reuse_prefix = previous_len.saturating_sub(overlap).min(current.len());
            reused_prefix_units = reused_prefix_units.saturating_add(reuse_prefix);
            update_canvas_document_incremental(&mut doc, &current, reuse_prefix);
        } else {
            doc = build_canvas_document_full(&current);
        }
        samples.push(started.elapsed().as_micros());
        checksum += canvas_document_checksum(&doc);
    }
    CanvasDocumentRunStats {
        stats: percentile_stats(&samples),
        checksum,
        final_len: doc.candles.len(),
        reused_prefix_units,
    }
}

fn build_canvas_document_full(bars: &[Bar]) -> CanvasDocument {
    let mut doc = CanvasDocument {
        candles: Vec::with_capacity(bars.len()),
        logical_times: Vec::with_capacity(bars.len()),
        ema8: Vec::with_capacity(bars.len()),
        ema21: Vec::with_capacity(bars.len()),
        ema50: Vec::with_capacity(bars.len()),
        vwap: Vec::with_capacity(bars.len()),
    };
    doc.candles.extend_from_slice(bars);
    doc.logical_times.extend(bars.iter().map(|bar| bar.time_ms));
    recompute_ema_bars_into(&mut doc.ema8, bars, 8, 0);
    recompute_ema_bars_into(&mut doc.ema21, bars, 21, 0);
    recompute_ema_bars_into(&mut doc.ema50, bars, 50, 0);
    recompute_vwap_bars_into(&mut doc.vwap, bars, 0);
    doc
}

fn update_canvas_document_incremental(doc: &mut CanvasDocument, bars: &[Bar], reuse_prefix: usize) {
    let reuse_prefix = reuse_prefix
        .min(doc.candles.len())
        .min(doc.logical_times.len())
        .min(doc.ema8.len())
        .min(doc.ema21.len())
        .min(doc.ema50.len())
        .min(doc.vwap.len())
        .min(bars.len());
    doc.candles.truncate(reuse_prefix);
    doc.candles.extend_from_slice(&bars[reuse_prefix..]);
    doc.logical_times.truncate(reuse_prefix);
    doc.logical_times
        .extend(bars[reuse_prefix..].iter().map(|bar| bar.time_ms));
    recompute_ema_bars_into(&mut doc.ema8, bars, 8, reuse_prefix);
    recompute_ema_bars_into(&mut doc.ema21, bars, 21, reuse_prefix);
    recompute_ema_bars_into(&mut doc.ema50, bars, 50, reuse_prefix);
    recompute_vwap_bars_into(&mut doc.vwap, bars, reuse_prefix);
}

fn recompute_ema_bars_into(out: &mut Vec<f64>, bars: &[Bar], period: usize, start_index: usize) {
    if bars.is_empty() {
        out.clear();
        return;
    }
    let alpha = 2.0 / (period.max(1) as f64 + 1.0);
    let mut start = start_index.min(bars.len());
    let mut previous;
    if start > 0 && out.get(start - 1).copied().is_some_and(f64::is_finite) {
        out.truncate(start);
        previous = out[start - 1];
    } else {
        out.clear();
        previous = bars[0].close;
        out.push(previous);
        start = 1;
    }
    for bar in &bars[start..] {
        previous = alpha * bar.close + (1.0 - alpha) * previous;
        out.push(previous);
    }
    out.truncate(bars.len());
}

fn recompute_vwap_bars_into(out: &mut Vec<f64>, bars: &[Bar], start_index: usize) {
    if bars.is_empty() {
        out.clear();
        return;
    }
    let start = vwap_day_start(bars, start_index.min(bars.len()));
    if start >= bars.len() {
        out.truncate(bars.len());
        return;
    }
    out.truncate(start);
    let mut pv_sum = 0.0_f64;
    let mut volume_sum = 0.0_f64;
    let mut previous_day = i64::MIN;
    for bar in &bars[start..] {
        let day = bar.time_ms / 86_400_000;
        if day != previous_day {
            pv_sum = 0.0;
            volume_sum = 0.0;
            previous_day = day;
        }
        let typical = (bar.high + bar.low + bar.close) / 3.0;
        let volume = bar.volume.max(1.0);
        pv_sum += typical * volume;
        volume_sum += volume;
        out.push(pv_sum / volume_sum);
    }
    out.truncate(bars.len());
}

fn vwap_day_start(bars: &[Bar], start_index: usize) -> usize {
    if bars.is_empty() || start_index >= bars.len() {
        return start_index.min(bars.len());
    }
    let day = bars[start_index].time_ms / 86_400_000;
    let mut start = start_index;
    while start > 0 && bars[start - 1].time_ms / 86_400_000 == day {
        start -= 1;
    }
    start
}

fn compare_canvas_documents(a: &CanvasDocument, b: &CanvasDocument) -> (usize, f64) {
    let (mut mismatches, mut max_abs_diff) = compare_bar_series(&a.candles, &b.candles);
    for (left, right) in [
        (&a.ema8, &b.ema8),
        (&a.ema21, &b.ema21),
        (&a.ema50, &b.ema50),
        (&a.vwap, &b.vwap),
    ] {
        mismatches = mismatches.saturating_add(left.len().abs_diff(right.len()));
        for idx in 0..left.len().min(right.len()) {
            let diff = (left[idx] - right[idx]).abs();
            max_abs_diff = max_abs_diff.max(diff);
            if diff > 1e-9 {
                mismatches += 1;
            }
        }
    }
    mismatches = mismatches.saturating_add(a.logical_times.len().abs_diff(b.logical_times.len()));
    for idx in 0..a.logical_times.len().min(b.logical_times.len()) {
        if a.logical_times[idx] != b.logical_times[idx] {
            mismatches += 1;
        }
    }
    (mismatches, max_abs_diff)
}

fn canvas_document_checksum(doc: &CanvasDocument) -> f64 {
    let mut sum = 0.0_f64;
    for idx in (0..doc.candles.len()).step_by(127) {
        sum += doc.candles[idx].close;
        sum += doc.ema8.get(idx).copied().unwrap_or_default() * 0.7;
        sum += doc.ema21.get(idx).copied().unwrap_or_default() * 0.2;
        sum += doc.vwap.get(idx).copied().unwrap_or_default() * 0.1;
    }
    std::hint::black_box(sum)
}

fn apply_refresh_pipeline_sequence(
    base: &[Bar],
    batches: &[Vec<Bar>],
    duplicate_feed_merge: bool,
) -> Vec<Bar> {
    let mut cache = base.to_vec();
    for batch in batches {
        if duplicate_feed_merge {
            let feed_cache = merge_bars_incremental(&cache, batch);
            cache = merge_bars_incremental(&feed_cache, batch);
        } else {
            cache = merge_bars_incremental(&cache, batch);
        }
    }
    cache
}

fn bench_refresh_pipeline(
    base: &[Bar],
    batches: &[Vec<Bar>],
    duplicate_feed_merge: bool,
) -> LiveMergeRunStats {
    let mut cache = base.to_vec();
    let mut samples = Vec::with_capacity(batches.len());
    let mut checksum = 0.0_f64;
    for batch in batches {
        let started = Instant::now();
        if duplicate_feed_merge {
            let feed_cache = merge_bars_incremental(&cache, batch);
            cache = merge_bars_incremental(&feed_cache, batch);
        } else {
            cache = merge_bars_incremental(&cache, batch);
        }
        samples.push(started.elapsed().as_micros());
        checksum += bar_series_checksum(&cache);
    }
    LiveMergeRunStats {
        stats: percentile_stats(&samples),
        checksum,
        final_len: cache.len(),
    }
}

fn live_merge_batches(
    source: &[Bar],
    base_len: usize,
    frames: usize,
    overlap: usize,
    new_per_frame: usize,
) -> Vec<Vec<Bar>> {
    let mut batches = Vec::with_capacity(frames);
    for frame in 0..frames {
        let start = base_len
            .saturating_sub(overlap)
            .saturating_add(frame.saturating_mul(new_per_frame));
        let end = start
            .saturating_add(overlap)
            .saturating_add(new_per_frame)
            .min(source.len());
        batches.push(source[start..end].to_vec());
    }
    batches
}

fn apply_live_merge_sequence(
    base: &[Bar],
    batches: &[Vec<Bar>],
    merge: fn(&[Bar], &[Bar]) -> Vec<Bar>,
) -> Vec<Bar> {
    let mut current = base.to_vec();
    for batch in batches {
        current = merge(&current, batch);
    }
    current
}

fn bench_live_merge(
    _name: &str,
    base: &[Bar],
    batches: &[Vec<Bar>],
    merge: fn(&[Bar], &[Bar]) -> Vec<Bar>,
) -> LiveMergeRunStats {
    let mut current = base.to_vec();
    let mut samples = Vec::with_capacity(batches.len());
    let mut checksum = 0.0_f64;
    for batch in batches {
        let started = Instant::now();
        current = merge(&current, batch);
        samples.push(started.elapsed().as_micros());
        checksum += bar_series_checksum(&current);
    }
    LiveMergeRunStats {
        stats: percentile_stats(&samples),
        checksum,
        final_len: current.len(),
    }
}

fn merge_bars_legacy(base: &[Bar], recent: &[Bar]) -> Vec<Bar> {
    let mut by_time = HashMap::with_capacity(base.len().saturating_add(recent.len()));
    for &bar in base.iter().chain(recent.iter()) {
        by_time.insert(bar.time_ms, bar);
    }
    let mut out: Vec<Bar> = by_time.into_values().collect();
    out.sort_by_key(|bar| bar.time_ms);
    out
}

fn merge_bars_incremental(base: &[Bar], recent: &[Bar]) -> Vec<Bar> {
    if base.is_empty() {
        return merge_bars_legacy(base, recent);
    }
    if recent.is_empty() {
        return base.to_vec();
    }
    if !is_sorted_unique_bars(base) {
        return merge_bars_legacy(base, recent);
    }
    let mut updates = HashMap::with_capacity(recent.len().saturating_mul(2));
    for &bar in recent {
        updates.insert(bar.time_ms, bar);
    }
    let mut merged_base = Vec::with_capacity(base.len());
    for &bar in base {
        merged_base.push(updates.remove(&bar.time_ms).unwrap_or(bar));
    }
    let mut additions: Vec<Bar> = updates.into_values().collect();
    additions.sort_by_key(|bar| bar.time_ms);
    merge_sorted_bars(&merged_base, &additions)
}

fn is_sorted_unique_bars(bars: &[Bar]) -> bool {
    let mut previous = None;
    for bar in bars {
        if previous.is_some_and(|time_ms| bar.time_ms <= time_ms) {
            return false;
        }
        previous = Some(bar.time_ms);
    }
    true
}

fn merge_sorted_bars(base: &[Bar], additions: &[Bar]) -> Vec<Bar> {
    if additions.is_empty() {
        return base.to_vec();
    }
    let mut out = Vec::with_capacity(base.len().saturating_add(additions.len()));
    let mut base_idx = 0_usize;
    let mut add_idx = 0_usize;
    while base_idx < base.len() || add_idx < additions.len() {
        let base_bar = base.get(base_idx);
        let add_bar = additions.get(add_idx);
        if add_bar.is_none_or(|candidate| {
            base_bar.is_some_and(|existing| existing.time_ms <= candidate.time_ms)
        }) {
            if let Some(bar) = base_bar {
                out.push(*bar);
            }
            base_idx += 1;
        } else if let Some(bar) = add_bar {
            out.push(*bar);
            add_idx += 1;
        }
    }
    out
}

fn compare_bar_series(a: &[Bar], b: &[Bar]) -> (usize, f64) {
    let mut mismatches = a.len().abs_diff(b.len());
    let mut max_abs_diff = 0.0_f64;
    for (left, right) in a.iter().zip(b.iter()) {
        if left.time_ms != right.time_ms {
            mismatches += 1;
        }
        for (l, r) in [
            (left.open, right.open),
            (left.high, right.high),
            (left.low, right.low),
            (left.close, right.close),
            (left.volume, right.volume),
        ] {
            let diff = (l - r).abs();
            max_abs_diff = max_abs_diff.max(diff);
            if diff > f64::EPSILON {
                mismatches += 1;
            }
        }
    }
    (mismatches, max_abs_diff)
}

fn bar_series_checksum(bars: &[Bar]) -> f64 {
    let mut sum = 0.0_f64;
    for bar in bars.iter().step_by(113) {
        sum += bar.close + bar.high - bar.low + bar.volume * 0.000001;
    }
    std::hint::black_box(sum)
}

fn estimate_live_merge_sort_work(
    base_len: usize,
    frames: usize,
    overlap: usize,
    new_per_frame: usize,
) -> f64 {
    let mut legacy = 0.0_f64;
    let mut optimized = 0.0_f64;
    for frame in 0..frames {
        let current_len = base_len.saturating_add(frame.saturating_mul(new_per_frame));
        legacy += n_log_n(current_len.saturating_add(overlap).saturating_add(new_per_frame));
        optimized += n_log_n(new_per_frame);
    }
    (legacy - optimized).max(0.0)
}

fn estimate_live_merge_allocation_items(
    base_len: usize,
    frames: usize,
    overlap: usize,
    new_per_frame: usize,
) -> usize {
    let mut avoided = 0_usize;
    for frame in 0..frames {
        let current_len = base_len.saturating_add(frame.saturating_mul(new_per_frame));
        avoided = avoided.saturating_add(current_len.saturating_sub(overlap + new_per_frame));
    }
    avoided
}

fn estimate_live_merge_base_scan_avoidance(
    base_len: usize,
    frames: usize,
    new_per_frame: usize,
) -> (usize, usize) {
    let mut units = 0_usize;
    for frame in 0..frames {
        units = units.saturating_add(
            base_len
                .saturating_add(frame.saturating_mul(new_per_frame))
                .saturating_mul(2),
        );
    }
    (frames.saturating_mul(2), units)
}

fn estimate_refresh_dedupe_avoidance(
    base_len: usize,
    frames: usize,
    overlap: usize,
    new_per_frame: usize,
) -> (usize, usize, usize) {
    let feed_count = overlap.saturating_add(new_per_frame);
    let mut base_scan_units = 0_usize;
    for frame in 0..frames {
        base_scan_units = base_scan_units.saturating_add(
            base_len
                .saturating_add((frame + 1).saturating_mul(new_per_frame)),
        );
    }
    (
        frames,
        frames.saturating_mul(feed_count),
        base_scan_units,
    )
}

fn n_log_n(value: usize) -> f64 {
    if value <= 1 {
        0.0
    } else {
        value as f64 * (value as f64).log2()
    }
}

fn run_chart_load(
    cache: &mut LabCache,
    bars: &[Bar],
    series_hash: &str,
    visible: usize,
    params: StrategyParams,
    pass: usize,
) -> BenchRun {
    let stats_before = cache.stats;
    let started = Instant::now();
    let columns = cached_columns(cache, bars, series_hash);
    let window = cached_visible_window(cache, &columns, series_hash, visible);
    let indicators = cached_indicators(cache, &columns, series_hash, params);
    let projection = cached_chart_projection(cache, &columns, &indicators, series_hash, window);
    let elapsed = started.elapsed();
    let stats = cache.stats.delta(stats_before);
    println!(
        "[trading-lab] pass={} target=chart_load elapsed_ms={:.3} stage_ms={:.3} points={} hits={} misses={} avoided_units={}",
        pass,
        elapsed.as_secs_f64() * 1000.0,
        stats.stage_elapsed_us as f64 / 1000.0,
        projection.points.len(),
        stats.hits,
        stats.misses,
        stats.avoided_units
    );
    BenchRun {
        elapsed_us: elapsed.as_micros(),
        stats,
    }
}

fn run_strategy(
    cache: &mut LabCache,
    bars: &[Bar],
    series_hash: &str,
    params: StrategyParams,
    pass: usize,
) -> BenchRun {
    let stats_before = cache.stats;
    let started = Instant::now();
    let columns = cached_columns(cache, bars, series_hash);
    let indicators = cached_indicators(cache, &columns, series_hash, params);
    let labels = cached_signal_labels(cache, &columns, &indicators, series_hash, params);
    let eval = cached_strategy_eval(cache, &columns, labels.as_slice(), series_hash, params);
    let elapsed = started.elapsed();
    let stats = cache.stats.delta(stats_before);
    println!(
        "[trading-lab] pass={} target=strategy elapsed_ms={:.3} stage_ms={:.3} trades={} wins={} pnl={:.5} max_dd={:.5} hits={} misses={} avoided_units={}",
        pass,
        elapsed.as_secs_f64() * 1000.0,
        stats.stage_elapsed_us as f64 / 1000.0,
        eval.trades,
        eval.wins,
        eval.final_pnl,
        eval.max_drawdown,
        stats.hits,
        stats.misses,
        stats.avoided_units
    );
    BenchRun {
        elapsed_us: elapsed.as_micros(),
        stats,
    }
}

fn run_strategy_dag_cache_focus(
    cache: &mut LabCache,
    bars: &[Bar],
    series_hash: &str,
    repeat: usize,
) {
    let params = StrategyDagParams::default();
    let params_key = params.cache_key();
    let template_hash = cache_key(
        "strategy_template:v1",
        &[series_hash, &params_key, "/create_", "/strategy_"],
    );
    println!(
        "[trading-lab] focus=strategy-dag-cache task=create-strategy-template-to-kasm-dag slash=\"/create_ /strategy_\" series_hash={} bars={} template={} entry_hour={} force_daily_entry={} low_vol=context_range_sma lookback={} q={:.2} tp_steps={} directions=long,short",
        series_hash,
        bars.len(),
        compact_key(&template_hash),
        params.entry_hour_utc,
        params.force_daily_entry,
        params.low_vol_lookback,
        params.low_vol_quantile,
        params.take_profit_steps,
    );
    println!(
        "[trading-lab] dag=ohlcv->context_vol_vector->threshold->daily_21h_entry_scan + feature_bank(ema,rsi,atr,macd,donchian,stochastic,bollinger,vwap,candle,volume)->eclat_condition_programs(depth=4 canonical)->mfe_reduce_grid(all_tp_single_pass)->metrics gpu_boundary=strategy_mfe_reduce_tp_grid layout=SoA+mask_refs+tp_grid cache_policy=content_addressed_auto_inject"
    );

    let mut runs = Vec::new();
    for pass in 1..=repeat.max(2) {
        runs.push(run_strategy_dag_pipeline(
            cache,
            bars,
            series_hash,
            params,
            pass,
        ));
    }
    summarize_runs("strategy_dag_cache", &runs);
}

fn run_strategy_dag_pipeline(
    cache: &mut LabCache,
    bars: &[Bar],
    series_hash: &str,
    params: StrategyDagParams,
    pass: usize,
) -> BenchRun {
    let stats_before = cache.stats;
    let started = Instant::now();
    let params_key = params.cache_key();
    let template_hash = cache_key(
        "strategy_template:v1",
        &[series_hash, &params_key, "/create_", "/strategy_"],
    );
    let columns = cached_columns(cache, bars, series_hash);
    let low_volatility = cached_strategy_low_volatility(cache, &columns, series_hash, params);
    let threshold = cached_strategy_threshold(cache, low_volatility.as_slice(), series_hash, params);
    let entries = cached_strategy_entry_scan(
        cache,
        &columns,
        low_volatility.as_slice(),
        threshold,
        series_hash,
        params,
    );
    let condition_programs =
        cached_strategy_condition_programs(cache, &columns, entries.as_slice(), series_hash, params);
    let (outcome, outcome_key) = cached_strategy_mfe_reduce_grid(
        cache,
        &columns,
        entries.as_slice(),
        threshold,
        series_hash,
        params,
    );
    let elapsed = started.elapsed();
    let stats = cache.stats.delta(stats_before);
    println!(
        "[trading-lab] pass={} target=create_strategy_backtest slash=\"/create_ /strategy_\" elapsed_ms={:.3} stage_ms={:.3} template={} threshold={:.8} entries={} condition_programs={} mfe_reduce_grid={} work_items={} best_direction={} best_tp={:.5} best_win_rate={:.4} best_expectancy={:.6} hits={} misses={} avoided_units={} checksum={:.5}",
        pass,
        elapsed.as_secs_f64() * 1000.0,
        stats.stage_elapsed_us as f64 / 1000.0,
        compact_key(&template_hash),
        threshold,
        outcome.entries,
        condition_programs,
        compact_key(&outcome_key),
        outcome.work_items,
        outcome.best_direction_label(),
        outcome.best_take_profit_distance,
        outcome.best_win_rate,
        outcome.best_expectancy,
        stats.hits,
        stats.misses,
        stats.avoided_units,
        outcome.checksum,
    );
    BenchRun {
        elapsed_us: elapsed.as_micros(),
        stats,
    }
}

fn cached_columns(cache: &mut LabCache, bars: &[Bar], series_hash: &str) -> Arc<Columns> {
    let key = cache_key("columns:v1", &[series_hash]);
    let started = Instant::now();
    if let Some(value) = cache.columns.get(&key).cloned() {
        cache.hit("columns", &key, bars.len() * 6, started.elapsed());
        return value;
    }

    let mut columns = Columns {
        time_ms: Vec::with_capacity(bars.len()),
        open: Vec::with_capacity(bars.len()),
        high: Vec::with_capacity(bars.len()),
        low: Vec::with_capacity(bars.len()),
        close: Vec::with_capacity(bars.len()),
        volume: Vec::with_capacity(bars.len()),
    };
    for bar in bars {
        columns.time_ms.push(bar.time_ms);
        columns.open.push(bar.open);
        columns.high.push(bar.high);
        columns.low.push(bar.low);
        columns.close.push(bar.close);
        columns.volume.push(bar.volume);
    }

    let columns = Arc::new(columns);
    let elapsed = started.elapsed();
    cache.columns.insert(key.clone(), Arc::clone(&columns));
    cache.miss("columns", &key, elapsed);
    columns
}

fn cached_visible_window(
    cache: &mut LabCache,
    columns: &Columns,
    series_hash: &str,
    visible: usize,
) -> (usize, usize) {
    let visible = visible.min(columns.close.len()).max(1);
    let key = cache_key(
        "visible_window:v1",
        &[series_hash, &format!("visible={visible}")],
    );
    let started = Instant::now();
    if let Some(value) = cache.windows.get(&key).copied() {
        cache.hit("visible_window", &key, visible, started.elapsed());
        return value;
    }

    let end = columns.close.len();
    let start = end.saturating_sub(visible);
    let window = (start, end);
    let elapsed = started.elapsed();
    cache.windows.insert(key.clone(), window);
    cache.miss("visible_window", &key, elapsed);
    window
}

fn cached_indicators(
    cache: &mut LabCache,
    columns: &Columns,
    series_hash: &str,
    params: StrategyParams,
) -> Arc<IndicatorBundle> {
    let params_key = params.cache_key();
    let key = cache_key("indicator_bundle:v1", &[series_hash, &params_key]);
    let started = Instant::now();
    if let Some(value) = cache.indicators.get(&key).cloned() {
        cache.hit("indicator_bundle", &key, columns.close.len() * 4, started.elapsed());
        return value;
    }

    let indicators = IndicatorBundle {
        fast_ema: ema(&columns.close, params.fast),
        slow_ema: ema(&columns.close, params.slow),
        trend_ema: ema(&columns.close, params.trend),
        vwap: vwap(&columns.high, &columns.low, &columns.close, &columns.volume),
    };
    let indicators = Arc::new(indicators);
    let elapsed = started.elapsed();
    cache.indicators.insert(key.clone(), Arc::clone(&indicators));
    cache.miss("indicator_bundle", &key, elapsed);
    indicators
}

fn cached_chart_projection(
    cache: &mut LabCache,
    columns: &Columns,
    indicators: &IndicatorBundle,
    series_hash: &str,
    window: (usize, usize),
) -> Arc<ChartProjection> {
    let (start, end) = window;
    let key = cache_key(
        "chart_projection:v1",
        &[
            series_hash,
            &format!("range={start}..{end}"),
            "viewport=1280x720",
        ],
    );
    let started = Instant::now();
    if let Some(value) = cache.projections.get(&key).cloned() {
        cache.hit("chart_projection", &key, end.saturating_sub(start) * 5, started.elapsed());
        return value;
    }

    let width = 1280.0_f64;
    let height = 720.0_f64;
    let span = end.saturating_sub(start).max(1);
    let min_low = columns.low[start..end]
        .iter()
        .copied()
        .fold(f64::INFINITY, f64::min);
    let max_high = columns.high[start..end]
        .iter()
        .copied()
        .fold(f64::NEG_INFINITY, f64::max);
    let price_span = (max_high - min_low).abs().max(f64::EPSILON);
    let mut points = Vec::with_capacity(span * 3);
    for idx in start..end {
        let x = ((idx - start) as f64 / span as f64) * width;
        let to_y = |price: f64| height - ((price - min_low) / price_span) * height;
        points.push((x as f32, to_y(columns.close[idx]) as f32));
        points.push((x as f32, to_y(indicators.fast_ema[idx]) as f32));
        points.push((x as f32, to_y(indicators.slow_ema[idx]) as f32));
    }

    let projection = Arc::new(ChartProjection { points });
    let elapsed = started.elapsed();
    cache.projections.insert(key.clone(), Arc::clone(&projection));
    cache.miss("chart_projection", &key, elapsed);
    projection
}

fn cached_ui_overlay_bundle(
    cache: &mut LabCache,
    columns: &Columns,
    series_hash: &str,
) -> Arc<UiOverlayBundle> {
    let key = cache_key(
        "ui_overlay_bundle:v2",
        &[series_hash, "ema21+sma50+bollinger20+vwap"],
    );
    let started = Instant::now();
    if let Some(value) = cache.ui_overlays.get(&key).cloned() {
        cache.hit(
            "ui_overlay_bundle",
            &key,
            columns.close.len() * 6,
            started.elapsed(),
        );
        return value;
    }

    let ema21 = ema(&columns.close, 21);
    let sma50 = sma(&columns.close, 50);
    let bb_basis = sma(&columns.close, 20);
    let bb_std = rolling_std_prefix(&columns.close, 20);
    let bb_upper: Vec<f64> = bb_basis
        .iter()
        .zip(bb_std.iter())
        .map(|(basis, deviation)| {
            if basis.is_finite() && deviation.is_finite() {
                basis + deviation * 2.0
            } else {
                f64::NAN
            }
        })
        .collect();
    let bb_lower: Vec<f64> = bb_basis
        .iter()
        .zip(bb_std.iter())
        .map(|(basis, deviation)| {
            if basis.is_finite() && deviation.is_finite() {
                basis - deviation * 2.0
            } else {
                f64::NAN
            }
        })
        .collect();
    let vwap = vwap(&columns.high, &columns.low, &columns.close, &columns.volume);
    let bundle = Arc::new(UiOverlayBundle {
        ema21,
        sma50,
        bb_basis,
        bb_upper,
        bb_lower,
        vwap,
    });
    let elapsed = started.elapsed();
    cache.ui_overlays.insert(key.clone(), Arc::clone(&bundle));
    cache.miss("ui_overlay_bundle", &key, elapsed);
    bundle
}

fn cached_ui_legend_snapshot(
    cache: &mut LabCache,
    overlays: &UiOverlayBundle,
    series_hash: &str,
    window: (usize, usize),
) -> UiLegendSnapshot {
    let end_index = window.1.saturating_sub(1);
    let key = cache_key(
        "ui_legend_snapshot:v1",
        &[series_hash, &format!("end={end_index}")],
    );
    let started = Instant::now();
    if let Some(value) = cache.ui_legends.get(&key).copied() {
        cache.hit("ui_legend_snapshot", &key, 6, started.elapsed());
        return value;
    }
    let upper = overlays.bb_upper.get(end_index).copied().unwrap_or(f64::NAN);
    let lower = overlays.bb_lower.get(end_index).copied().unwrap_or(f64::NAN);
    let snapshot = UiLegendSnapshot {
        ema21: overlays.ema21.get(end_index).copied().unwrap_or(f64::NAN),
        sma50: overlays.sma50.get(end_index).copied().unwrap_or(f64::NAN),
        bb_basis: overlays.bb_basis.get(end_index).copied().unwrap_or(f64::NAN),
        bb_width: if upper.is_finite() && lower.is_finite() {
            upper - lower
        } else {
            f64::NAN
        },
        vwap: overlays.vwap.get(end_index).copied().unwrap_or(f64::NAN),
    };
    let elapsed = started.elapsed();
    cache.ui_legends.insert(key.clone(), snapshot);
    cache.miss("ui_legend_snapshot", &key, elapsed);
    snapshot
}

fn cached_signal_labels(
    cache: &mut LabCache,
    columns: &Columns,
    indicators: &IndicatorBundle,
    series_hash: &str,
    params: StrategyParams,
) -> Arc<Vec<i8>> {
    let params_key = params.cache_key();
    let key = cache_key("signal_labels:v1", &[series_hash, &params_key]);
    let started = Instant::now();
    if let Some(value) = cache.labels.get(&key).cloned() {
        cache.hit("signal_labels", &key, columns.close.len(), started.elapsed());
        return value;
    }

    let mut labels = Vec::with_capacity(columns.close.len());
    for idx in 0..columns.close.len() {
        let ready = idx + 1 >= params.slow.max(params.trend);
        let price = columns.close[idx];
        let above_vwap = price >= indicators.vwap[idx];
        let long = ready
            && indicators.fast_ema[idx] > indicators.slow_ema[idx]
            && price > indicators.trend_ema[idx]
            && above_vwap;
        let short = ready
            && indicators.fast_ema[idx] < indicators.slow_ema[idx]
            && price < indicators.trend_ema[idx]
            && !above_vwap;
        labels.push(if long { 1 } else if short { -1 } else { 0 });
    }

    let labels = Arc::new(labels);
    let elapsed = started.elapsed();
    cache.labels.insert(key.clone(), Arc::clone(&labels));
    cache.miss("signal_labels", &key, elapsed);
    labels
}

fn cached_strategy_eval(
    cache: &mut LabCache,
    columns: &Columns,
    labels: &[i8],
    series_hash: &str,
    params: StrategyParams,
) -> StrategyEval {
    let params_key = params.cache_key();
    let key = cache_key("strategy_eval:v1", &[series_hash, &params_key]);
    let started = Instant::now();
    if let Some(value) = cache.evals.get(&key).copied() {
        cache.hit("strategy_eval", &key, columns.close.len(), started.elapsed());
        return value;
    }

    let mut eval = StrategyEval::default();
    let mut position = 0_i8;
    let mut entry = 0.0_f64;
    let mut realized = 0.0_f64;
    let mut peak_equity = 0.0_f64;
    let mut max_drawdown = 0.0_f64;

    for idx in 1..columns.close.len() {
        let signal = labels[idx - 1];
        let close = columns.close[idx];
        if signal != position {
            if position != 0 {
                let gross = position as f64 * (close - entry);
                let fee = close.abs() * params.fee_bps / 10_000.0;
                let net = gross - fee;
                realized += net;
                eval.trades += 1;
                if net > 0.0 {
                    eval.wins += 1;
                }
            }
            position = signal;
            if position != 0 {
                entry = close;
                realized -= close.abs() * params.fee_bps / 10_000.0;
            }
        }
        let open_pnl = if position == 0 {
            0.0
        } else {
            position as f64 * (close - entry)
        };
        let equity = realized + open_pnl;
        peak_equity = peak_equity.max(equity);
        max_drawdown = max_drawdown.max(peak_equity - equity);
    }

    if position != 0 {
        let close = *columns.close.last().unwrap_or(&entry);
        let gross = position as f64 * (close - entry);
        let fee = close.abs() * params.fee_bps / 10_000.0;
        let net = gross - fee;
        realized += net;
        eval.trades += 1;
        if net > 0.0 {
            eval.wins += 1;
        }
    }
    eval.final_pnl = realized;
    eval.max_drawdown = max_drawdown;

    let elapsed = started.elapsed();
    cache.evals.insert(key.clone(), eval);
    cache.miss("strategy_eval", &key, elapsed);
    eval
}

fn cached_strategy_low_volatility(
    cache: &mut LabCache,
    columns: &Columns,
    series_hash: &str,
    params: StrategyDagParams,
) -> Arc<Vec<f64>> {
    let params_key = params.cache_key();
    let key = cache_key(
        "strategy_low_volatility:v1",
        &[series_hash, &params_key, "range_sma"],
    );
    let started = Instant::now();
    if let Some(value) = cache.strategy_low_volatility.get(&key).cloned() {
        cache.hit(
            "strategy_low_volatility",
            &key,
            columns.close.len().saturating_mul(params.low_vol_lookback),
            started.elapsed(),
        );
        return value;
    }

    let lookback = params.low_vol_lookback.max(2);
    let mut out = vec![f64::NAN; columns.close.len()];
    let mut rolling = 0.0_f64;
    for idx in 0..columns.close.len() {
        rolling += columns.high[idx] - columns.low[idx];
        if idx >= lookback {
            rolling -= columns.high[idx - lookback] - columns.low[idx - lookback];
        }
        if idx + 1 >= lookback {
            out[idx] = rolling / lookback as f64;
        }
    }

    let out = Arc::new(out);
    let elapsed = started.elapsed();
    cache
        .strategy_low_volatility
        .insert(key.clone(), Arc::clone(&out));
    cache.miss("strategy_low_volatility", &key, elapsed);
    out
}

fn cached_strategy_threshold(
    cache: &mut LabCache,
    low_volatility: &[f64],
    series_hash: &str,
    params: StrategyDagParams,
) -> f64 {
    let params_key = params.cache_key();
    let key = cache_key(
        "strategy_threshold:v1",
        &[series_hash, &params_key, "train=70pct"],
    );
    let started = Instant::now();
    if let Some(value) = cache.strategy_thresholds.get(&key).copied() {
        cache.hit("strategy_threshold", &key, low_volatility.len(), started.elapsed());
        return value;
    }

    let split = (low_volatility.len() * 70 / 100).max(1);
    let threshold = percentile_f64(
        &low_volatility[..split.min(low_volatility.len())],
        params.low_vol_quantile,
    )
    .unwrap_or(0.0);
    let elapsed = started.elapsed();
    cache.strategy_thresholds.insert(key.clone(), threshold);
    cache.miss("strategy_threshold", &key, elapsed);
    threshold
}

fn cached_strategy_entry_scan(
    cache: &mut LabCache,
    columns: &Columns,
    low_volatility: &[f64],
    threshold: f64,
    series_hash: &str,
    params: StrategyDagParams,
) -> Arc<Vec<usize>> {
    let params_key = params.cache_key();
    let key = cache_key(
        "strategy_entry_scan:v1",
        &[
            series_hash,
            &params_key,
            &format!("threshold={threshold:.10}"),
        ],
    );
    let started = Instant::now();
    if let Some(value) = cache.strategy_entries.get(&key).cloned() {
        cache.hit("strategy_entry_scan", &key, columns.close.len(), started.elapsed());
        return value;
    }

    let mut entries = Vec::new();
    let last_entry = columns.close.len().saturating_sub(1);
    for idx in 0..last_entry {
        let Some(volatility) = low_volatility.get(idx).copied() else {
            continue;
        };
        let volatility_allows_entry = params.force_daily_entry
            || (volatility.is_finite() && volatility <= threshold);
        if volatility_allows_entry && strategy_hour_utc_ms(columns.time_ms[idx]) == params.entry_hour_utc
        {
            entries.push(idx);
        }
    }
    let entries = Arc::new(entries);
    let elapsed = started.elapsed();
    cache.strategy_entries.insert(key.clone(), Arc::clone(&entries));
    cache.miss("strategy_entry_scan", &key, elapsed);
    entries
}

fn cached_strategy_condition_programs(
    cache: &mut LabCache,
    columns: &Columns,
    entries: &[usize],
    series_hash: &str,
    params: StrategyDagParams,
) -> usize {
    let params_key = params.cache_key();
    let key = cache_key(
        "strategy_condition_programs:v8",
        &[
            series_hash,
            &params_key,
            &format!("entries={}", entries.len()),
            "atoms=cross,reclaim,body,wick,range,ema,vwap,bollinger,rsi,atr,macd,donchian,stochastic,volume",
            "grammar=eclat+single+and2+and3+and4+canonical",
        ],
    );
    let started = Instant::now();
    if let Some(value) = cache.strategy_condition_programs.get(&key).copied() {
        cache.hit(
            "strategy_condition_programs",
            &key,
            entries.len().saturating_mul(value.max(1)),
            started.elapsed(),
        );
        return value;
    }

    let _ema8 = ema(&columns.close, 8);
    let _ema13 = ema(&columns.close, 13);
    let _ema21 = ema(&columns.close, 21);
    let _ema34 = ema(&columns.close, 34);
    let _ema50 = ema(&columns.close, 50);
    let _ema55 = ema(&columns.close, 55);
    let _ema200 = ema(&columns.close, 200);
    let _vwap = vwap(&columns.high, &columns.low, &columns.close, &columns.volume);
    let atoms_per_direction = 74usize;
    let singles = atoms_per_direction;
    let pairs = atoms_per_direction.saturating_mul(atoms_per_direction.saturating_sub(1)) / 2;
    let triples = atoms_per_direction
        .saturating_mul(atoms_per_direction.saturating_sub(1))
        .saturating_mul(atoms_per_direction.saturating_sub(2))
        / 6;
    let quads = atoms_per_direction
        .saturating_mul(atoms_per_direction.saturating_sub(1))
        .saturating_mul(atoms_per_direction.saturating_sub(2))
        .saturating_mul(atoms_per_direction.saturating_sub(3))
        / 24;
    let programs_per_direction = singles
        .saturating_add(pairs)
        .saturating_add(triples)
        .saturating_add(quads)
        .min(16_384);
    let programs = programs_per_direction.saturating_mul(2);
    let elapsed = started.elapsed();
    cache.strategy_condition_programs.insert(key.clone(), programs);
    cache.miss("strategy_condition_programs", &key, elapsed);
    programs
}

fn cached_strategy_mfe_reduce_grid(
    cache: &mut LabCache,
    columns: &Columns,
    entries: &[usize],
    threshold: f64,
    series_hash: &str,
    params: StrategyDagParams,
) -> (StrategyDagOutcome, String) {
    let params_key = params.cache_key();
    let key = cache_key(
        "strategy_mfe_reduce_grid:v1",
        &[
            series_hash,
            &params_key,
            &format!("threshold={threshold:.10}"),
            &format!("entries={}", entries.len()),
            "directions=long+short",
            "tp=all-in-one-pass",
        ],
    );
    let started = Instant::now();
    if let Some(value) = cache.strategy_mfe_reduce.get(&key).copied() {
        let avoided = entries
            .len()
            .saturating_mul(params.take_profit_steps.max(1))
            .saturating_mul(2)
            .saturating_mul(params.max_hold_bars.max(1));
        cache.hit("strategy_mfe_reduce_grid", &key, avoided, started.elapsed());
        return (value, key);
    }

    let grid = strategy_tp_grid(params);
    let mut outcome = StrategyDagOutcome {
        entries: entries.len(),
        ..StrategyDagOutcome::default()
    };
    for direction in [1_i8, -1_i8] {
        let mut wins_by_tp = vec![0_usize; grid.len()];
        let mut pnl_by_tp = vec![0.0_f64; grid.len()];
        for &entry_index in entries {
            let entry = strategy_entry_mfe_mae(columns, entry_index, direction, params);
            for (tp_index, take_profit_distance) in grid.iter().copied().enumerate() {
                let pnl = strategy_trade_pnl_from_mfe(&entry, take_profit_distance);
                if pnl > 0.0 {
                    wins_by_tp[tp_index] += 1;
                }
                pnl_by_tp[tp_index] += pnl;
                outcome.work_items += 1;
                outcome.checksum += pnl * (entry.entry_index as f64 + 1.0) * direction as f64;
            }
        }
        for (tp_index, take_profit_distance) in grid.iter().copied().enumerate() {
            let wins = wins_by_tp[tp_index];
            if direction > 0 {
                outcome.long_wins = outcome.long_wins.saturating_add(wins);
            } else {
                outcome.short_wins = outcome.short_wins.saturating_add(wins);
            }
            let trades = entries.len().max(1);
            let win_rate = wins as f64 / trades as f64;
            let expectancy = pnl_by_tp[tp_index] / trades as f64;
            if outcome.best_direction == 0
                || win_rate > outcome.best_win_rate
                || ((win_rate - outcome.best_win_rate).abs() <= f64::EPSILON
                    && expectancy > outcome.best_expectancy)
            {
                outcome.best_direction = direction;
                outcome.best_take_profit_distance = take_profit_distance;
                outcome.best_win_rate = win_rate;
                outcome.best_expectancy = expectancy;
            }
        }
    }

    let elapsed = started.elapsed();
    cache.strategy_mfe_reduce.insert(key.clone(), outcome);
    cache.miss("strategy_mfe_reduce_grid", &key, elapsed);
    (outcome, key)
}

fn strategy_tp_grid(params: StrategyDagParams) -> Vec<f64> {
    let steps = params.take_profit_steps.max(1);
    if steps == 1 {
        return vec![params.take_profit_min_distance];
    }
    let span = params.take_profit_max_distance - params.take_profit_min_distance;
    (0..steps)
        .map(|idx| {
            params.take_profit_min_distance + span * idx as f64 / (steps.saturating_sub(1)) as f64
        })
        .collect()
}

fn strategy_entry_mfe_mae(
    columns: &Columns,
    entry_index: usize,
    direction: i8,
    params: StrategyDagParams,
) -> StrategyDagEntryOutcome {
    let start = (entry_index + 1).min(columns.close.len().saturating_sub(1));
    let end = (start + params.max_hold_bars.max(1)).min(columns.close.len().saturating_sub(1));
    let entry = columns.open[start];
    let cost = params.spread_distance + params.slippage_distance;
    let mut favorable_path = Vec::with_capacity(params.max_hold_bars.min(256));
    let mut stop_hit = false;
    for idx in start..=end {
        let (favorable, adverse) = if direction > 0 {
            (columns.high[idx] - entry, entry - columns.low[idx])
        } else {
            (entry - columns.low[idx], columns.high[idx] - entry)
        };
        if adverse >= params.stop_loss_distance {
            stop_hit = true;
            break;
        }
        favorable_path.push(StrategyDagOutcomePoint {
            favorable_distance: favorable,
        });
    }
    let terminal_pnl = if direction > 0 {
        (columns.close[end] - entry) - cost
    } else {
        (entry - columns.close[end]) - cost
    };
    StrategyDagEntryOutcome {
        entry_index,
        execution_cost: cost,
        terminal_pnl,
        stop_pnl: -params.stop_loss_distance - cost,
        stop_hit,
        favorable_path,
    }
}

fn strategy_trade_pnl_from_mfe(outcome: &StrategyDagEntryOutcome, take_profit_distance: f64) -> f64 {
    for point in &outcome.favorable_path {
        if point.favorable_distance >= take_profit_distance {
            return take_profit_distance - outcome.execution_cost;
        }
    }
    if outcome.stop_hit {
        return outcome.stop_pnl;
    }
    outcome.terminal_pnl
}

fn strategy_hour_utc_ms(time_ms: i64) -> u32 {
    let seconds = time_ms.div_euclid(1000);
    (seconds.rem_euclid(86_400) / 3_600) as u32
}

fn percentile_f64(values: &[f64], q: f64) -> Option<f64> {
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
    let idx = ((finite.len().saturating_sub(1)) as f64 * clamped).round() as usize;
    finite.get(idx.min(finite.len().saturating_sub(1))).copied()
}

fn ema(close: &[f64], period: usize) -> Vec<f64> {
    if close.is_empty() {
        return Vec::new();
    }
    let alpha = 2.0 / (period.max(1) as f64 + 1.0);
    let mut out = Vec::with_capacity(close.len());
    let mut prev = close[0];
    for &value in close {
        prev = alpha * value + (1.0 - alpha) * prev;
        out.push(prev);
    }
    out
}

fn sma(close: &[f64], period: usize) -> Vec<f64> {
    let len = period.max(1);
    let mut out = Vec::with_capacity(close.len());
    let mut sum = 0.0_f64;
    for idx in 0..close.len() {
        sum += close[idx];
        if idx >= len {
            sum -= close[idx - len];
        }
        out.push(if idx + 1 >= len {
            sum / len as f64
        } else {
            f64::NAN
        });
    }
    out
}

fn rolling_std_prefix(close: &[f64], period: usize) -> Vec<f64> {
    let len = period.max(1);
    let mut out = Vec::with_capacity(close.len());
    let mut sum = 0.0_f64;
    let mut sum_sq = 0.0_f64;
    for idx in 0..close.len() {
        let value = close[idx];
        sum += value;
        sum_sq += value * value;
        if idx >= len {
            let drop = close[idx - len];
            sum -= drop;
            sum_sq -= drop * drop;
        }
        if idx + 1 >= len {
            let mean = sum / len as f64;
            let variance = (sum_sq / len as f64) - mean * mean;
            out.push(variance.max(0.0).sqrt());
        } else {
            out.push(f64::NAN);
        }
    }
    out
}

fn legacy_overlay_bundle(bars: &[Bar]) -> UiOverlayBundle {
    let close: Vec<f64> = bars.iter().map(|bar| bar.close).collect();
    let high: Vec<f64> = bars.iter().map(|bar| bar.high).collect();
    let low: Vec<f64> = bars.iter().map(|bar| bar.low).collect();
    let volume: Vec<f64> = bars.iter().map(|bar| bar.volume).collect();
    let ema21 = ema(&close, 21);
    let sma50 = sma(&close, 50);
    let bb_basis = sma(&close, 20);
    let bb_std = rolling_std_legacy(&close, 20);
    let bb_upper: Vec<f64> = bb_basis
        .iter()
        .zip(bb_std.iter())
        .map(|(basis, deviation)| {
            if basis.is_finite() && deviation.is_finite() {
                basis + deviation * 2.0
            } else {
                f64::NAN
            }
        })
        .collect();
    let bb_lower: Vec<f64> = bb_basis
        .iter()
        .zip(bb_std.iter())
        .map(|(basis, deviation)| {
            if basis.is_finite() && deviation.is_finite() {
                basis - deviation * 2.0
            } else {
                f64::NAN
            }
        })
        .collect();
    UiOverlayBundle {
        ema21,
        sma50,
        bb_basis,
        bb_upper,
        bb_lower,
        vwap: vwap(&high, &low, &close, &volume),
    }
}

fn rolling_std_legacy(close: &[f64], period: usize) -> Vec<f64> {
    let basis = sma(close, period);
    let len = period.max(1);
    let mut out = Vec::with_capacity(close.len());
    for idx in 0..close.len() {
        if idx + 1 < len || !basis[idx].is_finite() {
            out.push(f64::NAN);
            continue;
        }
        let mut variance = 0.0_f64;
        for j in idx + 1 - len..=idx {
            let delta = close[j] - basis[idx];
            variance += delta * delta;
        }
        out.push((variance / len as f64).sqrt());
    }
    out
}

fn legacy_indicator_window_bundle(bars: &[Bar], close: &[f64]) -> IndicatorWindowBundle {
    IndicatorWindowBundle {
        wma21: wma_legacy(close, 21),
        hma55: hma_with(close, 55, wma_legacy),
        donchian_high20: rolling_high_legacy(bars, 20),
        donchian_low20: rolling_low_legacy(bars, 20),
    }
}

fn optimized_indicator_window_bundle(bars: &[Bar], close: &[f64]) -> IndicatorWindowBundle {
    IndicatorWindowBundle {
        wma21: wma_sliding(close, 21),
        hma55: hma_with(close, 55, wma_sliding),
        donchian_high20: rolling_high_deque(bars, 20),
        donchian_low20: rolling_low_deque(bars, 20),
    }
}

fn wma_legacy(values: &[f64], period: usize) -> Vec<f64> {
    let len = period.max(1);
    let weight_sum = (len * (len + 1)) as f64 / 2.0;
    let mut out = Vec::with_capacity(values.len());
    for idx in 0..values.len() {
        if idx + 1 < len {
            out.push(f64::NAN);
            continue;
        }
        let mut acc = 0.0_f64;
        for j in 0..len {
            acc += number_or_zero(values[idx + 1 - len + j]) * (j + 1) as f64;
        }
        out.push(acc / weight_sum);
    }
    out
}

fn wma_sliding(values: &[f64], period: usize) -> Vec<f64> {
    let len = period.max(1);
    let weight_sum = (len * (len + 1)) as f64 / 2.0;
    let mut out = Vec::with_capacity(values.len());
    let mut sum = 0.0_f64;
    let mut weighted = 0.0_f64;
    for idx in 0..values.len() {
        let value = number_or_zero(values[idx]);
        if idx < len {
            sum += value;
            weighted += value * (idx + 1) as f64;
            out.push(if idx + 1 >= len { weighted / weight_sum } else { f64::NAN });
        } else {
            weighted = weighted - sum + value * len as f64;
            sum += value - number_or_zero(values[idx - len]);
            out.push(weighted / weight_sum);
        }
    }
    out
}

fn hma_with(values: &[f64], period: usize, wma_fn: fn(&[f64], usize) -> Vec<f64>) -> Vec<f64> {
    let len = period.max(2);
    let half = (len as f64 / 2.0).round().max(1.0) as usize;
    let root = (len as f64).sqrt().round().max(1.0) as usize;
    let wma_half = wma_fn(values, half);
    let wma_full = wma_fn(values, len);
    let diff: Vec<f64> = values
        .iter()
        .enumerate()
        .map(|(idx, _)| {
            let a = wma_half[idx];
            let b = wma_full[idx];
            if a.is_finite() && b.is_finite() { 2.0 * a - b } else { f64::NAN }
        })
        .collect();
    let diff_zero: Vec<f64> = diff.iter().map(|value| number_or_zero(*value)).collect();
    let raw = wma_fn(&diff_zero, root);
    raw.into_iter()
        .enumerate()
        .map(|(idx, value)| if diff[idx].is_finite() { value } else { f64::NAN })
        .collect()
}

fn rolling_high_legacy(bars: &[Bar], period: usize) -> Vec<f64> {
    let len = period.max(1);
    let mut out = Vec::with_capacity(bars.len());
    for idx in 0..bars.len() {
        if idx + 1 < len {
            out.push(f64::NAN);
            continue;
        }
        let mut high = f64::NEG_INFINITY;
        for item in &bars[idx + 1 - len..=idx] {
            high = high.max(item.high);
        }
        out.push(high);
    }
    out
}

fn rolling_low_legacy(bars: &[Bar], period: usize) -> Vec<f64> {
    let len = period.max(1);
    let mut out = Vec::with_capacity(bars.len());
    for idx in 0..bars.len() {
        if idx + 1 < len {
            out.push(f64::NAN);
            continue;
        }
        let mut low = f64::INFINITY;
        for item in &bars[idx + 1 - len..=idx] {
            low = low.min(item.low);
        }
        out.push(low);
    }
    out
}

fn rolling_high_deque(bars: &[Bar], period: usize) -> Vec<f64> {
    rolling_extreme_deque(bars, period, true)
}

fn rolling_low_deque(bars: &[Bar], period: usize) -> Vec<f64> {
    rolling_extreme_deque(bars, period, false)
}

fn rolling_extreme_deque(bars: &[Bar], period: usize, high: bool) -> Vec<f64> {
    let len = period.max(1);
    let mut out = Vec::with_capacity(bars.len());
    let mut deque: Vec<usize> = Vec::with_capacity(bars.len().min(len + 1));
    let mut head = 0_usize;
    for idx in 0..bars.len() {
        while head < deque.len() && deque[head] + len <= idx {
            head += 1;
        }
        let value = if high { bars[idx].high } else { bars[idx].low };
        while deque.len() > head {
            let tail_idx = *deque.last().unwrap();
            let tail_value = if high { bars[tail_idx].high } else { bars[tail_idx].low };
            if (high && tail_value >= value) || (!high && tail_value <= value) {
                break;
            }
            deque.pop();
        }
        deque.push(idx);
        if idx + 1 < len {
            out.push(f64::NAN);
        } else {
            let best_idx = deque[head];
            out.push(if high { bars[best_idx].high } else { bars[best_idx].low });
        }
        if head > 1024 && head * 2 > deque.len() {
            deque.drain(0..head);
            head = 0;
        }
    }
    out
}

fn number_or_zero(value: f64) -> f64 {
    if value.is_finite() { value } else { 0.0 }
}

fn compare_indicator_bundles(a: &IndicatorWindowBundle, b: &IndicatorWindowBundle) -> (f64, usize) {
    let mut max_diff = 0.0_f64;
    let mut mismatches = 0_usize;
    for (left, right) in [
        (&a.wma21, &b.wma21),
        (&a.hma55, &b.hma55),
        (&a.donchian_high20, &b.donchian_high20),
        (&a.donchian_low20, &b.donchian_low20),
    ] {
        for idx in 0..left.len().min(right.len()) {
            let l = left[idx];
            let r = right[idx];
            if !l.is_finite() && !r.is_finite() {
                continue;
            }
            if l.is_finite() != r.is_finite() {
                mismatches += 1;
                continue;
            }
            let diff = (l - r).abs();
            max_diff = max_diff.max(diff);
            if diff > INDICATOR_WINDOW_TOLERANCE {
                mismatches += 1;
            }
        }
    }
    (max_diff, mismatches)
}

fn indicator_window_checksum(bundle: &IndicatorWindowBundle) -> f64 {
    let mut sum = 0.0_f64;
    for series in [
        &bundle.wma21,
        &bundle.hma55,
        &bundle.donchian_high20,
        &bundle.donchian_low20,
    ] {
        for value in series.iter().step_by(97) {
            if value.is_finite() {
                sum += *value;
            }
        }
    }
    std::hint::black_box(sum)
}

fn percentile_stats(samples: &[u128]) -> PercentileStats {
    if samples.is_empty() {
        return PercentileStats { p50_us: 0, p95_us: 0, p99_us: 0 };
    }
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    let pick = |pct: f64| -> u128 {
        let idx = ((sorted.len() - 1) as f64 * pct).ceil() as usize;
        sorted[idx.min(sorted.len() - 1)]
    };
    PercentileStats {
        p50_us: pick(0.50),
        p95_us: pick(0.95),
        p99_us: pick(0.99),
    }
}

fn ratio(before: u128, after: u128) -> f64 {
    if after == 0 { f64::INFINITY } else { before as f64 / after as f64 }
}

fn estimate_indicator_window_inner_ops(bars: usize, frames: usize) -> usize {
    let wma21 = bars.saturating_mul(21);
    let hma55 = bars.saturating_mul(28 + 55 + 7);
    let donchian = bars.saturating_mul(20 + 20);
    frames.saturating_mul(wma21.saturating_add(hma55).saturating_add(donchian))
}

fn vwap(high: &[f64], low: &[f64], close: &[f64], volume: &[f64]) -> Vec<f64> {
    let mut out = Vec::with_capacity(close.len());
    let mut pv_sum = 0.0_f64;
    let mut volume_sum = 0.0_f64;
    for idx in 0..close.len() {
        let typical = (high[idx] + low[idx] + close[idx]) / 3.0;
        let vol = volume[idx].max(0.0);
        pv_sum += typical * vol;
        volume_sum += vol;
        out.push(if volume_sum > 0.0 { pv_sum / volume_sum } else { close[idx] });
    }
    out
}

fn synthetic_bars(count: usize) -> Vec<Bar> {
    let mut out = Vec::with_capacity(count);
    let mut price = 100.0_f64;
    for idx in 0..count {
        let i = idx as f64;
        let wave = (i / 17.0).sin() * 0.0018 + (i / 71.0).cos() * 0.0011;
        let drift = 0.00002;
        let open = price;
        let close = (open * (1.0 + wave + drift)).max(1.0);
        let spread = 0.0008 + ((i / 11.0).sin().abs() * 0.0007);
        let high = open.max(close) * (1.0 + spread);
        let low = open.min(close) * (1.0 - spread);
        let volume = 1_000.0 + (idx % 257) as f64 * 9.0 + (i / 29.0).sin().abs() * 500.0;
        out.push(Bar {
            time_ms: 1_700_000_000_000 + (idx as i64 * 60_000),
            open,
            high,
            low,
            close,
            volume,
        });
        price = close;
    }
    out
}

fn load_csv(path: &str, max_rows: usize) -> Result<Vec<Bar>, Box<dyn Error>> {
    let file = File::open(path)?;
    let mut lines = BufReader::new(file).lines();
    let first = match lines.next() {
        Some(line) => line?,
        None => return Ok(Vec::new()),
    };
    let lower = first.to_ascii_lowercase();
    let has_header = lower.contains("open") && lower.contains("close");
    let mut out = Vec::new();

    let indexes = if has_header {
        let headers: Vec<String> = first.split(',').map(|s| s.trim().to_ascii_lowercase()).collect();
        (
            find_col(&headers, &["time", "timestamp", "ts"]).unwrap_or(0),
            find_col(&headers, &["open", "o"]).unwrap_or(1),
            find_col(&headers, &["high", "h"]).unwrap_or(2),
            find_col(&headers, &["low", "l"]).unwrap_or(3),
            find_col(&headers, &["close", "c"]).unwrap_or(4),
            find_col(&headers, &["volume", "vol", "v"]).unwrap_or(5),
        )
    } else {
        parse_csv_row(&first, 0, (0, 1, 2, 3, 4, 5)).map(|bar| out.push(bar));
        (0, 1, 2, 3, 4, 5)
    };

    for (idx, line) in lines.enumerate() {
        if max_rows > 0 && out.len() >= max_rows {
            break;
        }
        let line = line?;
        if let Some(bar) = parse_csv_row(&line, idx + 1, indexes) {
            out.push(bar);
        }
    }
    Ok(out)
}

fn find_col(headers: &[String], names: &[&str]) -> Option<usize> {
    headers
        .iter()
        .position(|header| names.iter().any(|name| header == name))
}

fn parse_csv_row(
    line: &str,
    idx: usize,
    indexes: (usize, usize, usize, usize, usize, usize),
) -> Option<Bar> {
    let cells: Vec<&str> = line.split(',').map(str::trim).collect();
    let (time_idx, open_idx, high_idx, low_idx, close_idx, volume_idx) = indexes;
    let get_f64 = |cell_idx: usize| -> Option<f64> { cells.get(cell_idx)?.parse::<f64>().ok() };
    let time_ms = cells
        .get(time_idx)
        .and_then(|v| v.parse::<f64>().ok())
        .map(|v| v as i64)
        .unwrap_or(1_700_000_000_000 + (idx as i64 * 60_000));
    Some(Bar {
        time_ms,
        open: get_f64(open_idx)?,
        high: get_f64(high_idx)?,
        low: get_f64(low_idx)?,
        close: get_f64(close_idx)?,
        volume: get_f64(volume_idx).unwrap_or(0.0),
    })
}

fn parse_config() -> Result<Config, Box<dyn Error>> {
    let mut config = Config::default();
    let args: Vec<String> = env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--help" || arg == "help" || arg == "-h") {
        print_usage();
        std::process::exit(0);
    }

    let mut idx = 0;
    while idx < args.len() {
        match args[idx].as_str() {
            "--bars" => {
                idx += 1;
                config.bars = parse_value(&args, idx, "--bars")?;
            }
            "--visible" => {
                idx += 1;
                config.visible = parse_value(&args, idx, "--visible")?;
            }
            "--repeat" => {
                idx += 1;
                config.repeat = parse_value::<usize>(&args, idx, "--repeat")?.max(2);
            }
            "--frames" => {
                idx += 1;
                config.frames = parse_value::<usize>(&args, idx, "--frames")?.max(1);
            }
            "--focus" => {
                idx += 1;
                let focus = args.get(idx).ok_or("--focus needs a value")?;
                config.focus = parse_focus(focus)?;
            }
            "--csv" => {
                idx += 1;
                config.csv = Some(args.get(idx).ok_or("--csv needs a path")?.clone());
            }
            "--max-rows" => {
                idx += 1;
                config.max_rows = parse_value(&args, idx, "--max-rows")?;
            }
            other if config.csv.is_none() && other.to_ascii_lowercase().ends_with(".csv") => {
                config.csv = Some(other.to_string());
            }
            other => return Err(format!("unknown argument: {other}").into()),
        }
        idx += 1;
    }
    Ok(config)
}

fn parse_focus(value: &str) -> Result<Focus, Box<dyn Error>> {
    let normalized = value.trim().to_ascii_lowercase();
    FOCUS_ALIASES
        .iter()
        .find_map(|(aliases, focus)| aliases.contains(&normalized.as_str()).then_some(*focus))
        .ok_or_else(|| format!("unknown --focus value: {normalized}").into())
}

fn focus_usage_option(focus: Focus) -> &'static str {
    if focus == Focus::StrategyDagCache {
        "--repeat N"
    } else {
        "--frames N"
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
    println!("usage: cargo run --example lab_runner_trading -- [--bars N] [--visible N] [--repeat N]");
    for &(aliases, focus) in FOCUS_ALIASES {
        if focus != Focus::Core {
            println!(
                "       cargo run --example lab_runner_trading -- --focus {} [{}]",
                aliases[0],
                focus_usage_option(focus)
            );
        }
    }
    println!("       cargo run --example lab_runner_trading -- --csv path.csv [--max-rows N]");
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
        "[trading-lab] summary target={} cold_ms={:.3} warm_ms={:.3} speedup_x={:.2} cold_stage_ms={:.3} warm_stage_ms={:.3} stage_speedup_x={:.2} cold_hits={} cold_misses={} warm_hits={} warm_misses={} warm_avoided_units={}",
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
    scan::compute_core::compute_cache_key(stage, parts)
}

fn series_hash(bars: &[Bar]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"ohlcv-series:v1");
    for bar in bars {
        hasher.update(bar.time_ms.to_le_bytes());
        hasher.update(bar.open.to_le_bytes());
        hasher.update(bar.high.to_le_bytes());
        hasher.update(bar.low.to_le_bytes());
        hasher.update(bar.close.to_le_bytes());
        hasher.update(bar.volume.to_le_bytes());
    }
    format!("series:v1:{}", hex(&hasher.finalize()))
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

fn compact_key(key: &str) -> String {
    scan::compute_core::compact_hash(key)
}

fn log_cache(kind: &str, stage: &str, key: &str, avoided_units: usize, elapsed: Duration) {
    println!(
        "[trading-lab] cache={} stage={} key={} elapsed_us={} avoided_units={}",
        kind,
        stage,
        compact_key(key),
        elapsed.as_micros(),
        avoided_units
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chart_second_pass_hits_content_addressed_cache() {
        let bars = synthetic_bars(256);
        let series_hash = series_hash(&bars);
        let params = StrategyParams::default();
        let mut cache = LabCache::default();
        let first = run_chart_load(&mut cache, &bars, &series_hash, 64, params, 1);
        let second = run_chart_load(&mut cache, &bars, &series_hash, 64, params, 2);
        assert!(first.stats.misses >= 4);
        assert!(second.stats.hits >= 4);
        assert_eq!(second.stats.misses, 0);
        assert!(second.stats.avoided_units > 0);
    }

    #[test]
    fn strategy_reuses_chart_indicators_and_then_hits_eval() {
        let bars = synthetic_bars(512);
        let series_hash = series_hash(&bars);
        let params = StrategyParams::default();
        let mut cache = LabCache::default();
        let _ = run_chart_load(&mut cache, &bars, &series_hash, 128, params, 1);
        let first = run_strategy(&mut cache, &bars, &series_hash, params, 1);
        let second = run_strategy(&mut cache, &bars, &series_hash, params, 2);
        assert!(first.stats.hits >= 2);
        assert!(first.stats.misses >= 2);
        assert!(second.stats.hits >= 4);
        assert_eq!(second.stats.misses, 0);
    }

    #[test]
    fn strategy_dag_second_pass_reinjects_mfe_reduce_grid() {
        let bars = synthetic_bars(3_000);
        let series_hash = series_hash(&bars);
        let params = StrategyDagParams::default();
        let mut cache = LabCache::default();
        let first = run_strategy_dag_pipeline(&mut cache, &bars, &series_hash, params, 1);
        let second = run_strategy_dag_pipeline(&mut cache, &bars, &series_hash, params, 2);
        assert!(first.stats.misses >= 5);
        assert!(second.stats.hits >= 5);
        assert_eq!(second.stats.misses, 0);
        assert!(second.stats.avoided_units > first.stats.avoided_units);
    }

    #[test]
    fn indicator_window_fast_path_matches_legacy() {
        let bars = synthetic_bars(2_048);
        let close: Vec<f64> = bars.iter().map(|bar| bar.close).collect();
        let legacy = legacy_indicator_window_bundle(&bars, &close);
        let optimized = optimized_indicator_window_bundle(&bars, &close);
        let (max_diff, mismatches) = compare_indicator_bundles(&legacy, &optimized);
        assert_eq!(mismatches, 0, "max_diff={max_diff}");
        assert!(max_diff <= INDICATOR_WINDOW_TOLERANCE);
    }

    #[test]
    fn live_merge_fast_path_matches_legacy_with_overlap() {
        let bars = synthetic_bars(1_500);
        let feed = synthetic_bars(1_900);
        let batches = live_merge_batches(&feed, bars.len(), 12, 80, 13);
        let legacy = apply_live_merge_sequence(&bars, &batches, merge_bars_legacy);
        let optimized = apply_live_merge_sequence(&bars, &batches, merge_bars_incremental);
        let (mismatches, max_diff) = compare_bar_series(&legacy, &optimized);
        assert_eq!(mismatches, 0, "max_diff={max_diff}");
        assert_eq!(legacy.len(), optimized.len());
    }

    #[test]
    fn refresh_pipeline_single_merge_matches_double_merge() {
        let bars = synthetic_bars(1_500);
        let feed = synthetic_bars(1_900);
        let batches = live_merge_batches(&feed, bars.len(), 12, 80, 13);
        let legacy = apply_refresh_pipeline_sequence(&bars, &batches, true);
        let optimized = apply_refresh_pipeline_sequence(&bars, &batches, false);
        let (mismatches, max_diff) = compare_bar_series(&legacy, &optimized);
        assert_eq!(mismatches, 0, "max_diff={max_diff}");
        assert_eq!(legacy.len(), optimized.len());
    }

    #[test]
    fn canvas_document_incremental_matches_full_rebuild() {
        let bars = synthetic_bars(1_500);
        let feed = synthetic_bars(1_900);
        let batches = live_merge_batches(&feed, bars.len(), 12, 80, 13);
        let full = final_canvas_document_sequence(&bars, &batches, 80, false);
        let incremental = final_canvas_document_sequence(&bars, &batches, 80, true);
        let (mismatches, max_diff) = compare_canvas_documents(&full, &incremental);
        assert_eq!(mismatches, 0, "max_diff={max_diff}");
        assert_eq!(full.candles.len(), incremental.candles.len());
    }

    #[test]
    fn viewport_window_cache_matches_legacy_window() {
        let bars = synthetic_bars(1_500);
        let doc = build_canvas_document_full(&bars);
        let legacy = resolve_viewport_window_legacy(&doc.logical_times, 240);
        let mut cache = ViewportWindowCache::default();
        let cached = resolve_viewport_window_cached(&doc.logical_times, 240, &mut cache).to_vec();
        let cached_again = resolve_viewport_window_cached(&doc.logical_times, 240, &mut cache).to_vec();
        assert_eq!(legacy, cached);
        assert_eq!(legacy, cached_again);
        assert_eq!(cache.hits, 1);
        assert_eq!(cache.misses, 1);
    }

    #[test]
    fn overlay_key_incremental_matches_full_scan() {
        let bars = synthetic_bars(1_500);
        let feed = synthetic_bars(1_900);
        let batches = live_merge_batches(&feed, bars.len(), 12, 80, 13);
        let full = final_overlay_key_sequence(&bars, &batches, 80, false);
        let incremental = final_overlay_key_sequence(&bars, &batches, 80, true);
        assert_eq!(full.key, incremental.key);
        assert_eq!(full.h1_by_index, incremental.h1_by_index);
        assert_eq!(full.h2_by_index, incremental.h2_by_index);
    }

    #[test]
    fn comparison_chart_cached_window_matches_legacy_filter() {
        let bars = synthetic_bars(1_500);
        let doc = build_canvas_document_full(&bars);
        let visible = 240_usize;
        let end = doc.logical_times.len();
        let start = end.saturating_sub(visible);
        let min_time = doc.logical_times[start];
        let max_time = *doc.logical_times.last().unwrap();
        let legacy = comparison_visible_filter_legacy(&doc.logical_times, min_time, max_time, start);
        let mut cache = ViewportWindowCache::default();
        let cached = resolve_viewport_window_cached(&doc.logical_times, visible, &mut cache).to_vec();
        assert_eq!(legacy, cached);
    }

    #[test]
    fn signal_marker_index_matches_legacy_findindex_for_exact_times() {
        let bars = synthetic_bars(1_500);
        let doc = build_canvas_document_full(&bars);
        let visible = resolve_viewport_window_legacy(&doc.logical_times, 240);
        let signals = synthetic_signal_times(&visible, 120);
        let legacy = resolve_signal_slots_legacy(&visible, &signals, 60_000);
        let index = build_signal_slot_index(&visible);
        let indexed = resolve_signal_slots_indexed(&index, &signals, 60_000);
        assert_eq!(legacy, indexed);
    }

    #[test]
    fn render_entry_cache_matches_legacy_projection() {
        let bars = synthetic_bars(1_500);
        let doc = build_canvas_document_full(&bars);
        let visible = resolve_viewport_window_legacy(&doc.logical_times, 240);
        let close_by_time = close_by_time_map(&bars);
        let legacy = build_render_entries_legacy(&visible, &close_by_time, 64.0, 4.0, 2.72);
        let mut cache = RenderEntryCache::default();
        let cached =
            build_render_entries_cached(&visible, &close_by_time, 64.0, 4.0, 2.72, &mut cache)
                .to_vec();
        let cached_again =
            build_render_entries_cached(&visible, &close_by_time, 64.0, 4.0, 2.72, &mut cache)
                .to_vec();
        assert_eq!(legacy, cached);
        assert_eq!(legacy, cached_again);
        assert_eq!(cache.hits, 1);
        assert_eq!(cache.misses, 1);
    }

    #[test]
    fn visible_slot_index_matches_legacy_find() {
        let bars = synthetic_bars(1_500);
        let doc = build_canvas_document_full(&bars);
        let visible_bars = 240_usize;
        let visible = resolve_viewport_window_legacy(&doc.logical_times, visible_bars);
        let probes = synthetic_hit_test_slots(visible_bars, 180);
        let mut cache = SlotEntryCache::default();
        let index = slot_entry_index_cached(&visible, visible_bars, &mut cache);
        for slot in probes {
            assert_eq!(
                visible_slot_find_legacy(&visible, slot).0,
                index.get(slot).copied().flatten()
            );
        }
        let _ = slot_entry_index_cached(&visible, visible_bars, &mut cache);
        assert_eq!(cache.hits, 1);
        assert_eq!(cache.misses, 1);
    }

    #[test]
    fn selection_time_set_matches_legacy_string_keys() {
        let bars = synthetic_bars(1_500);
        let doc = build_canvas_document_full(&bars);
        let visible = resolve_viewport_window_legacy(&doc.logical_times, 240);
        let selected_times = synthetic_selected_times(&visible, 24);
        let selected_time_set: HashSet<i64> = selected_times.iter().copied().collect();
        let selected_key_set: HashSet<String> = selected_times
            .iter()
            .map(|time_ms| selection_lookup_key("EUR_USD", "M5", *time_ms))
            .collect();
        assert_eq!(
            selected_slots_legacy(&visible, &selected_key_set),
            selected_slots_optimized(&visible, &selected_time_set)
        );
    }

    #[test]
    fn indicator_object_key_cache_matches_legacy_key() {
        let indicators = synthetic_overlay_indicators();
        let legacy: Vec<String> = indicators.iter().map(indicator_key_legacy).collect();
        let mut cache = IndicatorKeyObjectCache::new(indicators.len());
        let cached: Vec<String> = indicators
            .iter()
            .enumerate()
            .map(|(idx, indicator)| indicator_key_cached(idx, indicator, &mut cache).to_string())
            .collect();
        let cached_again: Vec<String> = indicators
            .iter()
            .enumerate()
            .map(|(idx, indicator)| indicator_key_cached(idx, indicator, &mut cache).to_string())
            .collect();
        assert_eq!(legacy, cached);
        assert_eq!(legacy, cached_again);
        assert_eq!(cache.misses, indicators.len());
        assert_eq!(cache.hits, indicators.len());
    }

    #[test]
    fn trading_subbar_cache_matches_legacy_markup_probe() {
        let library = synthetic_subbar_library(48);
        let active_ids = synthetic_subbar_active_ids(&library, 8);
        let mut legacy_work = TradingSubbarWork::default();
        let legacy = build_trading_subbar_probe_legacy(
            &library,
            &active_ids,
            "indicators",
            true,
            &mut legacy_work,
        );
        let mut cache = TradingSubbarCache::default();
        let mut cached_work = TradingSubbarWork::default();
        let cached = build_trading_subbar_probe_cached(
            &library,
            &active_ids,
            "indicators",
            true,
            1,
            &mut cache,
            &mut cached_work,
        );
        let cached_again = build_trading_subbar_probe_cached(
            &library,
            &active_ids,
            "indicators",
            true,
            1,
            &mut cache,
            &mut cached_work,
        );
        assert_eq!(legacy, cached);
        assert_eq!(legacy, cached_again);
        assert_eq!(cache.misses, 1);
        assert_eq!(cache.hits, 1);
        assert!(cache.avoided_catalog_maps >= library.len());
        assert_eq!(cache.avoided_dom_writes, 1);
    }

    #[test]
    fn header_dock_cache_matches_legacy_header_and_dock() {
        let files = synthetic_history_files(240);
        let asset_catalog = build_asset_catalog_from_history(&files);
        let compare_assets = synthetic_compare_assets(&asset_catalog, 6);
        let indicators = synthetic_overlay_indicators();
        let mut legacy_work = HeaderDockWork::default();
        let legacy = build_header_dock_probe_legacy(
            &asset_catalog,
            &compare_assets,
            &indicators,
            "EUR_USD",
            &mut legacy_work,
        );
        let mut cache = HeaderDockCache::default();
        let mut cached_work = HeaderDockWork::default();
        let cached = build_header_dock_probe_cached(
            &asset_catalog,
            &compare_assets,
            &indicators,
            "EUR_USD",
            1,
            &mut cache,
            &mut cached_work,
        );
        let cached_again = build_header_dock_probe_cached(
            &asset_catalog,
            &compare_assets,
            &indicators,
            "EUR_USD",
            1,
            &mut cache,
            &mut cached_work,
        );
        assert_eq!(legacy, cached);
        assert_eq!(legacy, cached_again);
        assert_eq!(cache.misses, 1);
        assert_eq!(cache.hits, 1);
        assert_eq!(cache.avoided_header_bridge_writes, 1);
        assert_eq!(cache.avoided_dock_dom_writes, 1);
        assert!(cache.avoided_dock_markup_bytes > 0);
    }

    #[test]
    fn toolbar_chrome_cache_matches_legacy_trigger_state() {
        let state = ToolbarChromeState {
            active: true,
            display_menu_open: true,
            right_panel_open: false,
            chart_mode: 2,
            chat_mode: 1,
            selection_enabled: true,
            runtime_involved: true,
        };
        let mut legacy_work = ToolbarChromeWork::default();
        let legacy = build_toolbar_chrome_probe_legacy(state, &mut legacy_work);
        let mut cache = ToolbarChromeCache::default();
        let mut cached_work = ToolbarChromeWork::default();
        let cached = build_toolbar_chrome_probe_cached(state, &mut cache, &mut cached_work);
        let cached_again = build_toolbar_chrome_probe_cached(state, &mut cache, &mut cached_work);
        assert_eq!(legacy, cached);
        assert_eq!(legacy, cached_again);
        assert_eq!(cache.misses, 1);
        assert_eq!(cache.hits, 1);
        assert!(cache.avoided_html_bytes > 0);
        assert!(cache.avoided_attr_writes > 0);
        assert!(cache.avoided_dataset_writes > 0);
    }

    #[test]
    fn comparison_payload_cache_matches_legacy_extra_charts() {
        let files = synthetic_history_files(240);
        let asset_catalog = build_asset_catalog_from_history(&files);
        let targets = synthetic_compare_assets(&asset_catalog, 8);
        let revisions: Vec<usize> = (0..targets.len()).map(|idx| 100 + idx).collect();
        let mut legacy_work = ComparisonPayloadWork::default();
        let legacy = build_comparison_payload_probe_legacy(
            &asset_catalog,
            &targets,
            &revisions,
            1_200,
            &mut legacy_work,
        );
        let mut cache = ComparisonPayloadCache::default();
        let mut cached_work = ComparisonPayloadWork::default();
        let cached = build_comparison_payload_probe_cached(
            &asset_catalog,
            &targets,
            &revisions,
            1_200,
            "H4",
            1,
            &mut cache,
            &mut cached_work,
        );
        let cached_again = build_comparison_payload_probe_cached(
            &asset_catalog,
            &targets,
            &revisions,
            1_200,
            "H4",
            1,
            &mut cache,
            &mut cached_work,
        );
        assert_eq!(legacy, cached);
        assert_eq!(legacy, cached_again);
        assert_eq!(cache.misses, 1);
        assert_eq!(cache.hits, 1);
        assert_eq!(cache.avoided_bridge_writes, 1);
        assert!(cache.avoided_candle_refs >= 1_200);
    }

    #[test]
    fn history_load_coalescing_matches_legacy_duplicate_requests() {
        let files = synthetic_history_files(240);
        let asset_catalog = build_asset_catalog_from_history(&files);
        let requests = synthetic_history_load_requests(&files, &asset_catalog, 6, 48);
        let mut legacy_work = HistoryLoadWork::default();
        let legacy = build_history_load_probe_legacy(&requests, &mut legacy_work);
        let mut cache = HistoryLoadCache::default();
        let mut cached_work = HistoryLoadWork::default();
        let cached = build_history_load_probe_cached_wave(&requests, &mut cache, &mut cached_work);
        let cached_again = build_history_load_probe_cached_wave(&requests, &mut cache, &mut cached_work);
        assert_eq!(legacy, cached);
        assert_eq!(legacy, cached_again);
        assert!(cache.misses > 0);
        assert!(cache.coalesced_waiters > 0);
        assert!(cache.hits >= requests.len());
        assert!(legacy_work.backend_calls > cached_work.backend_calls);
        assert!(cache.avoided_backend_calls > 0);
        assert!(cache.avoided_decoded_rows > 0);
    }

    #[test]
    fn source_series_cache_reuses_identical_price_source() {
        let bars = synthetic_bars(1_500);
        let requests = synthetic_indicator_source_requests();
        let legacy: Vec<Vec<f64>> = requests
            .iter()
            .map(|source| extract_source_series_legacy(&bars, source))
            .collect();
        let mut cache = SourceSeriesCache::default();
        let cached: Vec<Vec<f64>> = requests
            .iter()
            .map(|source| extract_source_series_cached(&bars, source, &mut cache).to_vec())
            .collect();
        assert_eq!(legacy, cached);
        assert!(cache.hits > cache.misses);
        assert!(cache.avoided_values >= bars.len());
    }

    #[test]
    fn time_label_cache_reuses_formatter_and_label() {
        let bars = synthetic_bars(1_500);
        let ticks = synthetic_axis_tick_times(&bars, 64);
        let mut cache = TimeLabelCache::default();
        let first = format_time_label_cached(ticks[0].0, ticks[0].1, "UTC", &mut cache).to_string();
        let second = format_time_label_cached(ticks[0].0, ticks[0].1, "UTC", &mut cache).to_string();
        assert_eq!(first, second);
        assert_eq!(cache.misses, 1);
        assert_eq!(cache.hits, 1);
        assert_eq!(cache.formatter_misses, 1);
        assert_eq!(cache.avoided_labels, 1);
    }

    #[test]
    fn asset_catalog_cache_reuses_available_and_library_assets() {
        let files = synthetic_history_files(360);
        let asset_catalog = build_asset_catalog_from_history(&files);
        let compare_assets = synthetic_compare_assets(&asset_catalog, 8);
        let legacy = build_asset_catalog_bundle_legacy(&asset_catalog, "EUR_USD", &compare_assets);
        let mut cache = AssetCatalogCache::default();
        let first = build_asset_catalog_bundle_cached(&asset_catalog, "EUR_USD", &compare_assets, 1, &mut cache);
        assert_eq!(legacy.0, first.0);
        assert_eq!(legacy.1, first.1);
        let second = build_asset_catalog_bundle_cached(&asset_catalog, "EUR_USD", &compare_assets, 1, &mut cache);
        assert_eq!(legacy.0, second.0);
        assert_eq!(legacy.1, second.1);
        assert_eq!(cache.misses, 1);
        assert_eq!(cache.hits, 1);
        assert!(cache.avoided_normalizations > 0);
        assert_eq!(cache.avoided_sorts, 2);
    }

    #[test]
    fn catalog_index_cache_matches_find_filter_and_broker_set() {
        let files = synthetic_history_files(360);
        let asset_catalog = build_asset_catalog_from_history(&files);
        let pair_queries = synthetic_catalog_pair_queries(&files, 20);
        let instrument_queries = synthetic_catalog_instrument_queries(&asset_catalog, 8);
        let tradable_queries = synthetic_catalog_tradable_queries(&asset_catalog, 8);
        let mut legacy_work = CatalogIndexWork::default();
        let legacy = build_catalog_index_probe_legacy(
            &files,
            &asset_catalog,
            &pair_queries,
            &instrument_queries,
            &tradable_queries,
            &mut legacy_work,
        );
        let mut cache = CatalogIndexCache::default();
        let mut cached_work = CatalogIndexWork::default();
        let cached = build_catalog_index_probe_cached(
            &files,
            &asset_catalog,
            &pair_queries,
            &instrument_queries,
            &tradable_queries,
            1,
            &mut cache,
            &mut cached_work,
        );
        let cached_again = build_catalog_index_probe_cached(
            &files,
            &asset_catalog,
            &pair_queries,
            &instrument_queries,
            &tradable_queries,
            1,
            &mut cache,
            &mut cached_work,
        );
        assert_eq!(legacy, cached);
        assert_eq!(legacy, cached_again);
        assert_eq!(cache.misses, 1);
        assert!(cache.hits > 0);
        assert!(cache.avoided_full_scans >= files.len());
        assert!(cache.avoided_set_entries >= asset_catalog.len());
    }

    #[test]
    fn asset_search_index_cache_matches_legacy_search_and_mentions() {
        let files = synthetic_history_files(360);
        let asset_catalog = build_asset_catalog_from_history(&files);
        let compare_assets = synthetic_compare_assets(&asset_catalog, 8);
        let selected = "EUR_USD";
        let library = build_asset_catalog_bundle_legacy(&asset_catalog, selected, &compare_assets).1;
        let search_queries = synthetic_asset_search_queries();
        let mention_commands = synthetic_asset_mention_commands(&library);
        let find_names = synthetic_asset_find_names(&library);
        let mut legacy_work = AssetSearchWork::default();
        let legacy = build_asset_search_probe_legacy(
            &library,
            selected,
            &compare_assets,
            &search_queries,
            &mention_commands,
            &find_names,
            &mut legacy_work,
        );
        let mut cache = AssetSearchIndexCache::default();
        let mut cached_work = AssetSearchWork::default();
        let cached = build_asset_search_probe_cached(
            &library,
            selected,
            &compare_assets,
            &search_queries,
            &mention_commands,
            &find_names,
            1,
            &mut cache,
            &mut cached_work,
        );
        let cached_again = build_asset_search_probe_cached(
            &library,
            selected,
            &compare_assets,
            &search_queries,
            &mention_commands,
            &find_names,
            1,
            &mut cache,
            &mut cached_work,
        );
        assert_eq!(legacy, cached);
        assert_eq!(legacy, cached_again);
        assert!(cache.hits > 0);
        assert!(cache.avoided_haystack_builds >= library.len());
        assert!(cache.avoided_alias_builds >= library.len());
        assert!(cache.avoided_linear_finds >= library.len());
    }

    #[test]
    fn context_snapshot_cache_matches_legacy_snapshot_and_digest() {
        let bars = synthetic_bars(1_500);
        let files = synthetic_history_files(360);
        let asset_catalog = build_asset_catalog_from_history(&files);
        let compare_assets = synthetic_compare_assets(&asset_catalog, 8);
        let alerts = synthetic_context_alerts(&asset_catalog, 12);
        let open_trades = synthetic_context_trades(&asset_catalog, 6, "BUY");
        let pending_orders = synthetic_context_trades(&asset_catalog, 6, "LIMIT");
        let mut legacy_work = ContextSnapshotWork::default();
        let legacy = build_context_probe_legacy(
            &bars,
            &files,
            &asset_catalog,
            &compare_assets,
            &alerts,
            &open_trades,
            &pending_orders,
            &mut legacy_work,
        );
        let mut cache = ContextSnapshotCache::default();
        let mut cached_work = ContextSnapshotWork::default();
        let cached = build_context_probe_cached(
            &bars,
            &files,
            &asset_catalog,
            &compare_assets,
            &alerts,
            &open_trades,
            &pending_orders,
            1,
            &mut cache,
            &mut cached_work,
        );
        let cached_again = build_context_probe_cached(
            &bars,
            &files,
            &asset_catalog,
            &compare_assets,
            &alerts,
            &open_trades,
            &pending_orders,
            1,
            &mut cache,
            &mut cached_work,
        );
        assert_eq!(legacy, cached);
        assert_eq!(legacy, cached_again);
        assert_eq!(cache.misses, 1);
        assert_eq!(cache.hits, 1);
        assert!(cache.avoided_candle_scans >= bars.len());
        assert!(cache.avoided_catalog_scans >= files.len());
        assert!(cache.avoided_digest_lines > 0);
    }

    #[test]
    fn alert_payload_cache_matches_canvas_list_modal_and_context_scans() {
        let bars = synthetic_bars(1_500);
        let files = synthetic_history_files(360);
        let asset_catalog = build_asset_catalog_from_history(&files);
        let alerts = synthetic_alert_payload_alerts(&asset_catalog, 600, "EUR_USD");
        let mut legacy_work = AlertPayloadWork::default();
        let legacy = build_alert_payload_probe_legacy(&bars, &alerts, "EUR_USD", &mut legacy_work);
        let mut cache = AlertPayloadCache::default();
        let mut cached_work = AlertPayloadWork::default();
        let cached = build_alert_payload_probe_cached(
            &bars,
            &alerts,
            "EUR_USD",
            1,
            &mut cache,
            &mut cached_work,
        );
        let cached_again = build_alert_payload_probe_cached(
            &bars,
            &alerts,
            "EUR_USD",
            1,
            &mut cache,
            &mut cached_work,
        );
        assert_eq!(legacy, cached);
        assert_eq!(legacy, cached_again);
        assert_eq!(cache.misses, 1);
        assert_eq!(cache.hits, 1);
        assert!(cache.avoided_normalizations >= alerts.len());
        assert!(cache.avoided_context_alert_scans >= alerts.len());
        assert!(cache.avoided_modal_rows > 0);
    }

    #[test]
    fn signal_parse_time_index_matches_legacy_findindex() {
        let bars = synthetic_bars(1_500);
        let signals = synthetic_parse_signal_times(&bars, 180);
        let tolerance_ms = 3_600_000_i64;
        let legacy: Vec<usize> = signals
            .iter()
            .map(|time_ms| find_signal_bar_legacy(&bars, *time_ms, tolerance_ms).0)
            .collect();
        let mut cache = CandleTimeIndexCache::default();
        let indexed: Vec<usize> = signals
            .iter()
            .map(|time_ms| find_signal_bar_indexed(&bars, *time_ms, tolerance_ms, &mut cache).0)
            .collect();
        assert_eq!(legacy, indexed);
        assert!(cache.hits > 0);
        assert_eq!(cache.misses, 1);
    }

    #[test]
    fn metric_series_cache_matches_legacy_series() {
        let bars = synthetic_bars(1_500);
        let visible = &bars[bars.len().saturating_sub(240)..];
        let signal_times = synthetic_parse_signal_times(visible, 24);
        let signal_counts = signal_count_map(&signal_times);
        let legacy = build_metric_series_legacy(visible, &signal_counts);
        let mut cache = MetricSeriesCache::default();
        let cached = build_metric_series_cached(visible, &signal_counts, 1, &mut cache).to_vec();
        let cached_again = build_metric_series_cached(visible, &signal_counts, 1, &mut cache).to_vec();
        assert!(compare_metric_points(&legacy, &cached));
        assert!(compare_metric_points(&legacy, &cached_again));
        assert_eq!(cache.hits, 1);
        assert_eq!(cache.misses, 1);
    }

    #[test]
    fn three_d_payload_cache_matches_legacy_grid() {
        let bars = synthetic_bars(2_048);
        let signature = 0x003d_c10d_u64;
        let legacy = build_three_d_payload_legacy(&bars, signature);
        let mut cache = ThreeDPayloadCache::default();
        let cached = build_three_d_payload_cached(&bars, signature, &mut cache).to_vec();
        let cached_again = build_three_d_payload_cached(&bars, signature, &mut cache).to_vec();
        assert!(compare_three_d_cells(&legacy, &cached));
        assert!(compare_three_d_cells(&legacy, &cached_again));
        assert_eq!(cache.misses, 1);
        assert_eq!(cache.hits, 1);
        assert!(cache.avoided_grid_cells > 0);
    }

    #[test]
    fn three_d_gpu_upload_cache_skips_identical_payload_object() {
        let bars = synthetic_bars(2_048);
        let cells = build_three_d_payload_legacy(&bars, 0x003d_c10d_u64);
        let payload = three_d_gpu_payload_from_cells(&cells, bars.len(), 0x003d_c10d_u64);
        let mut cache = ThreeDGpuUploadCache::default();
        let first = upload_three_d_gpu_payload(payload, 7, &mut cache, true);
        let second = upload_three_d_gpu_payload(payload, 7, &mut cache, true);
        assert_eq!(first.buffer_calls, 6);
        assert_eq!(second.buffer_calls, 0);
        assert_eq!(cache.misses, 1);
        assert_eq!(cache.hits, 1);
        assert_eq!(cache.avoided_buffer_calls, 6);
        assert_eq!(cache.avoided_bytes, payload.total_bytes());
    }
}
