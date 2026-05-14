use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION};
use serde::{Deserialize, Serialize};
use std::env;
use std::time::{Duration, Instant};

const PRACTICE_REST_BASE: &str = "https://api-fxpractice.oanda.com";
const LIVE_REST_BASE: &str = "https://api-fxtrade.oanda.com";
const PRACTICE_STREAM_BASE: &str = "https://stream-fxpractice.oanda.com";
const LIVE_STREAM_BASE: &str = "https://stream-fxtrade.oanda.com";
const DEFAULT_INSTRUMENT: &str = "EUR_USD";
const DEFAULT_GRANULARITY: &str = "H4";
const DEFAULT_CANDLE_COUNT: usize = 72;
const DEFAULT_STREAM_SAMPLE_SECONDS: u64 = 4;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TradingPressure3dRequest {
    pub instrument: Option<String>,
    pub granularity: Option<String>,
    pub candle_count: Option<usize>,
    pub stream_sample_seconds: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TradingPressure3dPoint {
    pub time: String,
    pub complete: bool,
    pub tick_volume: f64,
    pub volume_threshold: f64,
    pub directional_tick_score: f64,
    pub liquidity_imbalance_score: f64,
    pub spread_stress_score: f64,
    pub pressure_score: f64,
    pub signal: String,
    pub signal_text: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TradingPressure3dPanel {
    pub instrument: String,
    pub timeframe: String,
    pub current_candle: String,
    pub tick_volume: f64,
    pub volume_threshold: f64,
    pub pressure_score: f64,
    pub directional_tick_score: f64,
    pub liquidity_imbalance_score: f64,
    pub spread_stress_score: f64,
    pub signal: String,
    pub signal_text: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TradingPressure3dResponse {
    pub source: String,
    pub instrument: String,
    pub granularity: String,
    pub pressure_note: String,
    pub points: Vec<TradingPressure3dPoint>,
    pub panel: TradingPressure3dPanel,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone)]
struct CandleCore {
    time: String,
    complete: bool,
    tick_volume: f64,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    close_spread: f64,
}

#[derive(Debug, Clone, Default)]
struct StreamSampleAggregate {
    up_ticks: u64,
    down_ticks: u64,
    imbalance_sum: f64,
    imbalance_count: u64,
    spread_sum: f64,
    spread_count: u64,
}

#[derive(Debug, Clone)]
struct RawPointMetrics {
    time: String,
    complete: bool,
    tick_volume: f64,
    directional_tick_score: f64,
    liquidity_imbalance_score: f64,
    avg_spread: f64,
}

#[derive(Debug, Clone)]
struct OandaEnvConfig {
    api_token: String,
    account_id: String,
    instrument: String,
    granularity: String,
    rest_base: String,
    stream_base: String,
}

#[derive(Debug, Deserialize)]
struct OandaCandleEnvelope {
    candles: Vec<OandaCandleRecord>,
}

#[derive(Debug, Deserialize)]
struct OandaCandleRecord {
    time: String,
    #[serde(default)]
    complete: bool,
    #[serde(default)]
    volume: u64,
    bid: Option<OandaOhlc>,
    ask: Option<OandaOhlc>,
    mid: Option<OandaOhlc>,
}

#[derive(Debug, Deserialize, Clone)]
struct OandaOhlc {
    o: String,
    h: String,
    l: String,
    c: String,
}

#[derive(Debug, Deserialize)]
struct OandaPricingStreamRecord {
    #[serde(rename = "type")]
    kind: Option<String>,
    #[allow(dead_code)]
    time: Option<String>,
    instrument: Option<String>,
    bids: Option<Vec<OandaBookLevel>>,
    asks: Option<Vec<OandaBookLevel>>,
}

#[derive(Debug, Deserialize)]
struct OandaBookLevel {
    price: String,
    liquidity: Option<f64>,
}

fn clamp_unit(value: f64) -> f64 {
    value.max(-1.0).min(1.0)
}

fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().sum::<f64>() / values.len() as f64
}

fn stddev(values: &[f64]) -> f64 {
    if values.len() < 2 {
        return 0.0;
    }
    let avg = mean(values);
    let var = values
        .iter()
        .map(|value| {
            let diff = *value - avg;
            diff * diff
        })
        .sum::<f64>()
        / values.len() as f64;
    var.sqrt()
}

fn signal_text(signal: &str) -> &'static str {
    match signal {
        "BUY_PRESSURE" => "activite anormale + pression acheteuse probable",
        "SELL_PRESSURE" => "activite anormale + pression vendeuse probable",
        "ABSORPTION_OR_TRAP" => "gros volume mais direction peu claire ou spread stresse",
        _ => "activite normale",
    }
}

fn compute_volume_threshold(volumes: &[f64]) -> f64 {
    mean(volumes) + 2.0 * stddev(volumes)
}

fn compute_directional_tick_score(up_ticks: u64, down_ticks: u64) -> f64 {
    let total = up_ticks + down_ticks;
    if total == 0 {
        return 0.0;
    }
    clamp_unit((up_ticks as f64 - down_ticks as f64) / total as f64)
}

fn compute_liquidity_imbalance_score(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    clamp_unit(mean(values))
}

fn compute_pressure_score(directional_tick_score: f64, liquidity_imbalance_score: f64) -> f64 {
    clamp_unit(0.70 * directional_tick_score + 0.30 * liquidity_imbalance_score)
}

fn compute_spread_stress_score(avg_spread: f64, history: &[f64]) -> f64 {
    if history.len() < 2 {
        return 0.0;
    }
    let baseline = mean(history);
    let sigma = stddev(history);
    if sigma <= 1e-9 {
        return 0.0;
    }
    (avg_spread - baseline) / sigma
}

fn detect_signal(tick_volume: f64, volume_threshold: f64, pressure_score: f64, spread_stress_score: f64) -> String {
    if tick_volume <= volume_threshold {
        return "NO_SIGNAL".to_string();
    }
    if spread_stress_score >= 2.5 || pressure_score.abs() <= 0.20 {
        return "ABSORPTION_OR_TRAP".to_string();
    }
    if pressure_score > 0.35 {
        return "BUY_PRESSURE".to_string();
    }
    if pressure_score < -0.35 {
        return "SELL_PRESSURE".to_string();
    }
    "NO_SIGNAL".to_string()
}

fn env_value(keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| env::var(key).ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn resolve_oanda_env(request: &TradingPressure3dRequest) -> Option<OandaEnvConfig> {
    let api_token = env_value(&["OANDA_API_TOKEN", "OANDA_API_KEY"])?;
    let account_id = env_value(&["OANDA_ACCOUNT_ID"])?;
    let env_name = env_value(&["OANDA_ENV"]).unwrap_or_else(|| "practice".to_string());
    let instrument = request
        .instrument
        .clone()
        .or_else(|| env_value(&["OANDA_INSTRUMENT"]))
        .unwrap_or_else(|| DEFAULT_INSTRUMENT.to_string())
        .trim()
        .to_string();
    let granularity = request
        .granularity
        .clone()
        .or_else(|| env_value(&["OANDA_GRANULARITY"]))
        .unwrap_or_else(|| DEFAULT_GRANULARITY.to_string())
        .trim()
        .to_uppercase();
    let is_live = env_name.eq_ignore_ascii_case("live");
    Some(OandaEnvConfig {
        api_token,
        account_id,
        instrument,
        granularity,
        rest_base: if is_live { LIVE_REST_BASE } else { PRACTICE_REST_BASE }.to_string(),
        stream_base: if is_live { LIVE_STREAM_BASE } else { PRACTICE_STREAM_BASE }.to_string(),
    })
}

fn auth_headers(api_token: &str) -> Result<HeaderMap, String> {
    let mut headers = HeaderMap::new();
    headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
    let token = format!("Bearer {api_token}");
    let auth = HeaderValue::from_str(&token).map_err(|err| format!("Invalid OANDA token: {err}"))?;
    headers.insert(AUTHORIZATION, auth);
    Ok(headers)
}

fn parse_f64_str(value: &str) -> Option<f64> {
    value.trim().parse::<f64>().ok()
}

fn pick_mid_ohlc(record: &OandaCandleRecord) -> Option<(f64, f64, f64, f64)> {
    if let Some(mid) = record.mid.as_ref() {
        return Some((
            parse_f64_str(&mid.o)?,
            parse_f64_str(&mid.h)?,
            parse_f64_str(&mid.l)?,
            parse_f64_str(&mid.c)?,
        ));
    }
    let bid = record.bid.as_ref()?;
    let ask = record.ask.as_ref()?;
    Some((
        (parse_f64_str(&bid.o)? + parse_f64_str(&ask.o)?) * 0.5,
        (parse_f64_str(&bid.h)? + parse_f64_str(&ask.h)?) * 0.5,
        (parse_f64_str(&bid.l)? + parse_f64_str(&ask.l)?) * 0.5,
        (parse_f64_str(&bid.c)? + parse_f64_str(&ask.c)?) * 0.5,
    ))
}

fn candle_close_spread(record: &OandaCandleRecord) -> f64 {
    let Some(bid) = record.bid.as_ref() else { return 0.0; };
    let Some(ask) = record.ask.as_ref() else { return 0.0; };
    let Some(bid_close) = parse_f64_str(&bid.c) else { return 0.0; };
    let Some(ask_close) = parse_f64_str(&ask.c) else { return 0.0; };
    (ask_close - bid_close).max(0.0)
}

fn candle_to_core(record: OandaCandleRecord) -> Option<CandleCore> {
    let (open, high, low, close) = pick_mid_ohlc(&record)?;
    let close_spread = candle_close_spread(&record);
    Some(CandleCore {
        time: record.time,
        complete: record.complete,
        tick_volume: record.volume as f64,
        open,
        high,
        low,
        close,
        close_spread,
    })
}

async fn get_candles(
    client: &reqwest::Client,
    cfg: &OandaEnvConfig,
    candle_count: usize,
) -> Result<Vec<CandleCore>, String> {
    let headers = auth_headers(&cfg.api_token)?;
    let url = format!(
        "{}/v3/instruments/{}/candles?price=BAM&granularity={}&count={}",
        cfg.rest_base, cfg.instrument, cfg.granularity, candle_count
    );
    let response = client
        .get(url)
        .headers(headers)
        .send()
        .await
        .map_err(|err| format!("OANDA candles request failed: {err}"))?;
    if !response.status().is_success() {
        return Err(format!("OANDA candles request failed with status {}", response.status()));
    }
    let payload: OandaCandleEnvelope = response
        .json()
        .await
        .map_err(|err| format!("OANDA candles decode failed: {err}"))?;
    Ok(payload.candles.into_iter().filter_map(candle_to_core).collect())
}

async fn sample_pricing_stream(
    client: &reqwest::Client,
    cfg: &OandaEnvConfig,
    sample_seconds: u64,
) -> Result<StreamSampleAggregate, String> {
    let headers = auth_headers(&cfg.api_token)?;
    let url = format!(
        "{}/v3/accounts/{}/pricing/stream?instruments={}",
        cfg.stream_base, cfg.account_id, cfg.instrument
    );
    let mut response = client
        .get(url)
        .headers(headers)
        .send()
        .await
        .map_err(|err| format!("OANDA pricing stream failed: {err}"))?;
    if !response.status().is_success() {
        return Err(format!("OANDA pricing stream failed with status {}", response.status()));
    }

    let deadline = Instant::now() + Duration::from_secs(sample_seconds.max(1));
    let mut buffer = String::new();
    let mut last_mid: Option<f64> = None;
    let mut aggregate = StreamSampleAggregate::default();

    while Instant::now() < deadline {
        let Some(chunk) = response
            .chunk()
            .await
            .map_err(|err| format!("OANDA pricing stream read failed: {err}"))?
        else {
            break;
        };
        buffer.push_str(&String::from_utf8_lossy(&chunk));
        while let Some(pos) = buffer.find('\n') {
            let line = buffer[..pos].trim().to_string();
            buffer = buffer[pos + 1..].to_string();
            if line.is_empty() {
                continue;
            }
            let Ok(record) = serde_json::from_str::<OandaPricingStreamRecord>(&line) else {
                continue;
            };
            if record.kind.as_deref() == Some("HEARTBEAT") {
                continue;
            }
            if record.instrument.as_deref() != Some(cfg.instrument.as_str()) {
                continue;
            }
            let bids = record.bids.unwrap_or_default();
            let asks = record.asks.unwrap_or_default();
            let Some(best_bid) = bids.first().and_then(|level| parse_f64_str(&level.price)) else {
                continue;
            };
            let Some(best_ask) = asks.first().and_then(|level| parse_f64_str(&level.price)) else {
                continue;
            };
            let mid = (best_bid + best_ask) * 0.5;
            if let Some(previous_mid) = last_mid {
                if mid > previous_mid {
                    aggregate.up_ticks += 1;
                } else if mid < previous_mid {
                    aggregate.down_ticks += 1;
                }
            }
            last_mid = Some(mid);
            let bid_liq: f64 = bids.iter().map(|level| level.liquidity.unwrap_or(0.0).max(0.0)).sum();
            let ask_liq: f64 = asks.iter().map(|level| level.liquidity.unwrap_or(0.0).max(0.0)).sum();
            let denom = (bid_liq + ask_liq).max(1.0);
            aggregate.imbalance_sum += (bid_liq - ask_liq) / denom;
            aggregate.imbalance_count += 1;
            aggregate.spread_sum += (best_ask - best_bid).max(0.0);
            aggregate.spread_count += 1;
        }
    }
    Ok(aggregate)
}

fn historical_direction_proxy(candle: &CandleCore) -> f64 {
    let range = (candle.high - candle.low).abs().max(1e-9);
    clamp_unit((candle.close - candle.open) / range)
}

fn build_raw_metrics(candles: &[CandleCore]) -> Vec<RawPointMetrics> {
    candles
        .iter()
        .map(|candle| RawPointMetrics {
            time: candle.time.clone(),
            complete: candle.complete,
            tick_volume: candle.tick_volume.max(0.0),
            directional_tick_score: historical_direction_proxy(candle),
            liquidity_imbalance_score: 0.0,
            avg_spread: candle.close_spread.max(0.0),
        })
        .collect()
}

fn merge_live_sample(points: &mut [RawPointMetrics], sample: &StreamSampleAggregate) {
    let Some(last) = points.last_mut() else { return; };
    let imbalance_values = if sample.imbalance_count > 0 {
        vec![sample.imbalance_sum / sample.imbalance_count as f64]
    } else {
        vec![]
    };
    last.directional_tick_score = compute_directional_tick_score(sample.up_ticks, sample.down_ticks);
    last.liquidity_imbalance_score = compute_liquidity_imbalance_score(&imbalance_values);
    if sample.spread_count > 0 {
        last.avg_spread = sample.spread_sum / sample.spread_count as f64;
    }
    last.complete = false;
}

fn finalize_points(raw: Vec<RawPointMetrics>) -> Vec<TradingPressure3dPoint> {
    let mut out = Vec::with_capacity(raw.len());
    let mut volume_window: Vec<f64> = Vec::new();
    let mut spread_window: Vec<f64> = Vec::new();
    for point in raw {
        volume_window.push(point.tick_volume);
        if volume_window.len() > 20 {
            volume_window.remove(0);
        }
        spread_window.push(point.avg_spread);
        if spread_window.len() > 20 {
            spread_window.remove(0);
        }
        let volume_threshold = compute_volume_threshold(&volume_window);
        let spread_stress_score = compute_spread_stress_score(point.avg_spread, &spread_window);
        let pressure_score = compute_pressure_score(point.directional_tick_score, point.liquidity_imbalance_score);
        let signal = detect_signal(
            point.tick_volume,
            volume_threshold,
            pressure_score,
            spread_stress_score,
        );
        out.push(TradingPressure3dPoint {
            time: point.time,
            complete: point.complete,
            tick_volume: point.tick_volume,
            volume_threshold,
            directional_tick_score: point.directional_tick_score,
            liquidity_imbalance_score: point.liquidity_imbalance_score,
            spread_stress_score,
            pressure_score,
            signal_text: signal_text(&signal).to_string(),
            signal,
        });
    }
    out
}

fn build_panel(instrument: &str, granularity: &str, points: &[TradingPressure3dPoint]) -> TradingPressure3dPanel {
    let last = points.last().cloned().unwrap_or(TradingPressure3dPoint {
        time: "n/a".to_string(),
        complete: false,
        tick_volume: 0.0,
        volume_threshold: 0.0,
        directional_tick_score: 0.0,
        liquidity_imbalance_score: 0.0,
        spread_stress_score: 0.0,
        pressure_score: 0.0,
        signal: "NO_SIGNAL".to_string(),
        signal_text: signal_text("NO_SIGNAL").to_string(),
    });
    TradingPressure3dPanel {
        instrument: instrument.to_string(),
        timeframe: granularity.to_string(),
        current_candle: last.time.clone(),
        tick_volume: last.tick_volume,
        volume_threshold: last.volume_threshold,
        pressure_score: last.pressure_score,
        directional_tick_score: last.directional_tick_score,
        liquidity_imbalance_score: last.liquidity_imbalance_score,
        spread_stress_score: last.spread_stress_score,
        signal: last.signal.clone(),
        signal_text: last.signal_text.clone(),
    }
}

fn mock_response(request: &TradingPressure3dRequest) -> TradingPressure3dResponse {
    let instrument = request
        .instrument
        .clone()
        .unwrap_or_else(|| DEFAULT_INSTRUMENT.to_string());
    let granularity = request
        .granularity
        .clone()
        .unwrap_or_else(|| DEFAULT_GRANULARITY.to_string())
        .to_uppercase();
    let mut raw = Vec::new();
    for i in 0..36usize {
        let time = format!("2026-05-{:02}T{:02}:00:00Z", 1 + (i / 6), (i % 6) * 4);
        let mut tick_volume = 80.0 + ((i % 7) as f64 * 9.0);
        let mut directional = ((i % 5) as f64 - 2.0) * 0.08;
        let mut liquidity = ((i % 4) as f64 - 1.5) * 0.06;
        let mut spread = 0.00016 + (i % 3) as f64 * 0.00002;
        if i == 10 {
            tick_volume = 250.0;
            directional = 0.62;
            liquidity = 0.42;
        } else if i == 18 {
            tick_volume = 265.0;
            directional = -0.66;
            liquidity = -0.35;
        } else if i == 27 {
            tick_volume = 278.0;
            directional = 0.04;
            liquidity = -0.03;
        } else if i == 31 {
            tick_volume = 290.0;
            directional = 0.11;
            liquidity = 0.08;
            spread = 0.00065;
        }
        raw.push(RawPointMetrics {
            time,
            complete: i < 35,
            tick_volume,
            directional_tick_score: clamp_unit(directional),
            liquidity_imbalance_score: clamp_unit(liquidity),
            avg_spread: spread,
        });
    }
    let points = finalize_points(raw);
    let panel = build_panel(&instrument, &granularity, &points);
    TradingPressure3dResponse {
        source: "mock".to_string(),
        instrument,
        granularity,
        pressure_note: "tick volume OANDA + pression approximee depuis donnees mock de validation".to_string(),
        points,
        panel,
        warnings: vec![
            "Mock mode active: no OANDA environment detected.".to_string(),
            "Pressure is an approximation, not centralized order flow.".to_string(),
        ],
    }
}

#[tauri::command]
pub async fn trading_oanda_pressure_3d(
    request: Option<TradingPressure3dRequest>,
) -> Result<TradingPressure3dResponse, String> {
    let request = request.unwrap_or(TradingPressure3dRequest {
        instrument: None,
        granularity: None,
        candle_count: None,
        stream_sample_seconds: None,
    });
    let Some(cfg) = resolve_oanda_env(&request) else {
        return Ok(mock_response(&request));
    };
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|err| format!("OANDA client init failed: {err}"))?;
    let candle_count = request.candle_count.unwrap_or(DEFAULT_CANDLE_COUNT).max(24).min(160);
    let sample_seconds = request
        .stream_sample_seconds
        .unwrap_or(DEFAULT_STREAM_SAMPLE_SECONDS)
        .max(1)
        .min(10);

    let candles = get_candles(&client, &cfg, candle_count).await?;
    if candles.is_empty() {
        return Ok(mock_response(&request));
    }
    let mut raw_points = build_raw_metrics(&candles);
    let mut warnings = vec![
        "Pressure is approximate and uses OANDA tick volume, not true exchange order flow.".to_string(),
        "Historical candles are approximated from candle structure; live pressure uses the pricing stream on the current candle.".to_string(),
    ];

    match sample_pricing_stream(&client, &cfg, sample_seconds).await {
        Ok(sample) => merge_live_sample(&mut raw_points, &sample),
        Err(err) => warnings.push(format!("Live pricing stream unavailable: {err}")),
    }

    let points = finalize_points(raw_points);
    let panel = build_panel(&cfg.instrument, &cfg.granularity, &points);
    Ok(TradingPressure3dResponse {
        source: "oanda".to_string(),
        instrument: cfg.instrument.clone(),
        granularity: cfg.granularity.clone(),
        pressure_note: "tick volume OANDA + pression approximee via ticks directionnels et desiquilibre de liquidite".to_string(),
        points,
        panel,
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn volume_threshold_uses_mean_plus_two_std() {
        let values = [100.0, 110.0, 90.0, 95.0, 105.0];
        let threshold = compute_volume_threshold(&values);
        assert!(threshold > mean(&values));
    }

    #[test]
    fn directional_tick_score_is_signed_and_bounded() {
        assert!((compute_directional_tick_score(9, 1) - 0.8).abs() < 1e-9);
        assert!((compute_directional_tick_score(1, 9) + 0.8).abs() < 1e-9);
        assert_eq!(compute_directional_tick_score(0, 0), 0.0);
    }

    #[test]
    fn liquidity_imbalance_score_is_bounded() {
        let score = compute_liquidity_imbalance_score(&[0.9, 0.8, 1.4]);
        assert!(score <= 1.0);
        assert!(score >= -1.0);
    }

    #[test]
    fn pressure_score_is_bounded_between_minus_one_and_one() {
        let score = compute_pressure_score(1.0, 1.0);
        assert!(score <= 1.0);
        let negative = compute_pressure_score(-1.0, -1.0);
        assert!(negative >= -1.0);
    }

    #[test]
    fn detects_buy_pressure() {
        let signal = detect_signal(240.0, 120.0, 0.51, 1.0);
        assert_eq!(signal, "BUY_PRESSURE");
    }

    #[test]
    fn detects_sell_pressure() {
        let signal = detect_signal(260.0, 130.0, -0.61, 0.8);
        assert_eq!(signal, "SELL_PRESSURE");
    }

    #[test]
    fn detects_absorption_on_flat_pressure() {
        let signal = detect_signal(250.0, 140.0, 0.08, 1.1);
        assert_eq!(signal, "ABSORPTION_OR_TRAP");
    }

    #[test]
    fn detects_absorption_on_stressed_spread() {
        let signal = detect_signal(250.0, 140.0, 0.58, 2.8);
        assert_eq!(signal, "ABSORPTION_OR_TRAP");
    }

    #[test]
    fn detects_no_signal_under_veil() {
        let signal = detect_signal(110.0, 140.0, 0.7, 0.1);
        assert_eq!(signal, "NO_SIGNAL");
    }
}
