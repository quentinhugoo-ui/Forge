//! ÃŽÂ¦.ÃŽÂ½.7g Ã¢â‚¬â€ Reverse Strategy Synthesis on OHLC time series.
//!
//! Module pure-Rust + std-only (aucune dÃƒÂ©pendance externe) qui :
//!   1. Parse un CSV de bougies OHLC (`time,open,high,low,close,volume`).
//!   2. Extrait des features par-bougie packÃƒÂ©es en i64 (hour, dow, RSI,
//!      MA-delta, vol bucket, lag returns).
//!   3. Simule un trade LONG ou SHORT depuis une bougie d'entrÃƒÂ©e donnÃƒÂ©e
//!      avec contraintes (SL en points, horizon max).
//!   4. Construit des examples `(features_i64, label_i64)` consommables
//!      par `MonsterNode::evolve_i64_program` pour la synthÃƒÂ¨se Forge.
//!
//! Le critÃƒÂ¨re de succÃƒÂ¨s final (% jours profitables Ã¢â€°Â¥ 85 %, cumul Ã¢â€°Â¥ +7p
//! par jour) est ÃƒÂ©valuÃƒÂ© **post-hoc** sur le programme synthÃƒÂ©tisÃƒÂ© via
//! `eval_strategy_per_day` Ã¢â‚¬â€ la synth elle-mÃƒÂªme optimise un proxy
//! par-bougie (label Ã‚Â±1/0).
//!
//! ## Encoding des features (i64 = 64 bits)
//!
//! Layout little-endian, packÃƒÂ© pour fit dans un seul i64 (input KASM
//! standard). Bits : [hour:3][dow:3][rsi_b:4][madelta_b:4][vol_b:4]
//! [lag1_sign:1][lag2_sign:1][lag3_sign:1][lag4_sign:1][lag5_sign:1]
//! [hi_lo_b:4][reserved:34]
//!
//! Buckets discrÃƒÂ©tisÃƒÂ©s (4 bits = 16 niveaux) pour que le synth voit des
//! valeurs catÃƒÂ©gorielles plutÃƒÂ´t que continues. Forge marche mieux sur
//! des entrÃƒÂ©es finies Ã¢â‚¬â€ un programme KASM peut comparer directement
//! deux bits sans float.


/// Une bougie OHLC. `time` est en millisecondes Unix epoch.
#[derive(Debug, Clone, Copy)]
pub struct Bar {
    pub time_ms: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}

/// Erreur de parsing CSV. Pas de panique Ã¢â‚¬â€ on retourne un message clair.
#[derive(Debug)]
pub enum ParseError {
    EmptyInput,
    BadHeader(String),
    BadLine { line_no: usize, reason: String },
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyInput => write!(f, "empty CSV input"),
            Self::BadHeader(h) => write!(f, "bad CSV header (expected 'time,open,high,low,close[,volume]'): {h}"),
            Self::BadLine { line_no, reason } => {
                write!(f, "bad CSV line {line_no}: {reason}")
            }
        }
    }
}

impl std::error::Error for ParseError {}

/// Parse un CSV de bougies OHLC. Format attendu :
/// ```
/// time,open,high,low,close,volume
/// 2010-01-04T22:00:00.000000000Z,5.756,5.813,5.755,5.759,188
/// ```
/// `time` accepte ISO-8601 (avec ou sans nanos) ou epoch ms numÃƒÂ©rique.
/// `volume` est optionnel.
pub fn parse_csv(bytes: &[u8]) -> Result<Vec<Bar>, ParseError> {
    if bytes.is_empty() {
        return Err(ParseError::EmptyInput);
    }
    // Strip BOM UTF-8 si prÃƒÂ©sent (PowerShell Out-File en ajoute).
    let txt = if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        std::str::from_utf8(&bytes[3..]).map_err(|e| ParseError::BadHeader(e.to_string()))?
    } else {
        std::str::from_utf8(bytes).map_err(|e| ParseError::BadHeader(e.to_string()))?
    };

    let mut lines = txt.lines();
    let header = lines.next().ok_or(ParseError::EmptyInput)?.trim().to_lowercase();
    let header_cols: Vec<&str> = header.split(',').map(|s| s.trim()).collect();
    let expected = ["time", "open", "high", "low", "close"];
    for (i, &exp) in expected.iter().enumerate() {
        if header_cols.get(i).copied() != Some(exp) {
            return Err(ParseError::BadHeader(header));
        }
    }
    let has_volume = header_cols.get(5).copied() == Some("volume");

    let mut bars = Vec::with_capacity(8192);
    for (idx, raw_line) in lines.enumerate() {
        let line_no = idx + 2; // 1-based + header
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        let cols: Vec<&str> = line.split(',').collect();
        if cols.len() < 5 {
            return Err(ParseError::BadLine {
                line_no,
                reason: format!("expected Ã¢â€°Â¥5 columns, got {}", cols.len()),
            });
        }
        let time_ms = parse_time(cols[0]).map_err(|reason| ParseError::BadLine {
            line_no,
            reason: format!("time: {reason}"),
        })?;
        let open = parse_f64(cols[1]).map_err(|r| ParseError::BadLine { line_no, reason: format!("open: {r}") })?;
        let high = parse_f64(cols[2]).map_err(|r| ParseError::BadLine { line_no, reason: format!("high: {r}") })?;
        let low = parse_f64(cols[3]).map_err(|r| ParseError::BadLine { line_no, reason: format!("low: {r}") })?;
        let close = parse_f64(cols[4]).map_err(|r| ParseError::BadLine { line_no, reason: format!("close: {r}") })?;
        let volume = if has_volume && cols.len() > 5 {
            parse_f64(cols[5]).unwrap_or(0.0)
        } else {
            0.0
        };
        bars.push(Bar { time_ms, open, high, low, close, volume });
    }
    Ok(bars)
}

fn parse_f64(s: &str) -> Result<f64, String> {
    s.trim().parse::<f64>().map_err(|e| e.to_string())
}

/// Parse un timestamp : ISO-8601 (ex: `2010-01-04T22:00:00.000000000Z`)
/// ou epoch ms numÃƒÂ©rique. Retourne ms depuis epoch (i64).
///
/// Note : on n'utilise PAS chrono (dÃƒÂ©pendance externe interdite par la
/// doctrine). Parser ISO-8601 minimaliste qui couvre le format OANDA.
fn parse_time(s: &str) -> Result<i64, String> {
    let s = s.trim();
    // Cas 1 : numÃƒÂ©rique Ã¢â€ â€™ epoch ms direct
    if s.bytes().all(|b| b.is_ascii_digit() || b == b'-') {
        return s.parse::<i64>().map_err(|e| e.to_string());
    }
    // Cas 2 : ISO-8601 avec ou sans nanos. Format OANDA :
    // YYYY-MM-DDTHH:MM:SS[.fffffffff]Z
    if s.len() < 19 {
        return Err(format!("ISO-8601 too short: {s}"));
    }
    let year: i32 = s[0..4].parse().map_err(|_| "bad year".to_string())?;
    let month: u32 = s[5..7].parse().map_err(|_| "bad month".to_string())?;
    let day: u32 = s[8..10].parse().map_err(|_| "bad day".to_string())?;
    let hour: u32 = s[11..13].parse().map_err(|_| "bad hour".to_string())?;
    let minute: u32 = s[14..16].parse().map_err(|_| "bad minute".to_string())?;
    let second: u32 = s[17..19].parse().map_err(|_| "bad second".to_string())?;
    Ok(iso_to_epoch_ms(year, month, day, hour, minute, second))
}

/// Convertit (Y,M,D,H,M,S) UTC en ms depuis epoch Unix. ImplÃƒÂ©mentation
/// minimaliste : days_from_civil de Howard Hinnant (proven correct,
/// ~10 lignes, public domain).
fn iso_to_epoch_ms(y: i32, m: u32, d: u32, h: u32, mi: u32, s: u32) -> i64 {
    // days_from_civil Ã¢â‚¬â€ cf. http://howardhinnant.github.io/date_algorithms.html
    let y = y - if m <= 2 { 1 } else { 0 };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u32; // [0, 399]
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1; // [0, 365]
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // [0, 146096]
    let days = era as i64 * 146097 + doe as i64 - 719468; // depuis 1970-01-01
    let total_s = days * 86400 + (h as i64) * 3600 + (mi as i64) * 60 + (s as i64);
    total_s * 1000
}

// Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬ Features extraction Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬

/// Number of historical bars needed before features can be computed.
/// 200 = max requirement (MA200). En dessous, certaines features ne
/// peuvent pas ÃƒÂªtre calculÃƒÂ©es et `extract_features` retourne `None`.
pub const MIN_HISTORY: usize = 200;

/// ÃŽÂ¦.ÃŽÂ½.7g Ã¢â‚¬â€ FeatureMask : permet d'activer/dÃƒÂ©sactiver les features
/// individuellement depuis le panel UI Alpha. Une feature dÃƒÂ©sactivÃƒÂ©e
/// est packÃƒÂ©e ÃƒÂ  0 dans le i64 Ã¢â€ â€™ le synth Forge voit toujours 0 Ã¢â€ â€™
/// ne peut pas s'en servir Ã¢â€ â€™ search space rÃƒÂ©duit (synth converge
/// plus vite, moins de risque overfit sur features inutiles).
///
/// Les features TOUJOURS actives (non mappÃƒÂ©es au panel) :
///   - hour, dow (timing) Ã¢â‚¬â€ fondamentaux pour intraday
///   - RSI(14) Ã¢â‚¬â€ oscillator standard
///   - lag returns sign (Ãƒâ€”5) Ã¢â‚¬â€ momentum bas niveau
///   - hi-lo range bucket Ã¢â‚¬â€ volatilitÃƒÂ© intra-bougie
///   - ADX(14) Ã¢â‚¬â€ force du trend
///
/// Les features mappables (toggle UI) :
///   - "ema8"  Ã¢â€ â€™ MA20 delta (mappÃƒÂ© approximativement)
///   - "ema21" Ã¢â€ â€™ MA50 delta
///   - "ema50" Ã¢â€ â€™ MA200 delta
///   - "vwap"  Ã¢â€ â€™ VWAP6 delta
///   - "atr14" Ã¢â€ â€™ ATR(14) bucket
#[derive(Debug, Clone, Copy)]
pub struct FeatureMask {
    pub use_ma_short: bool,
    pub use_ma_mid: bool,
    pub use_ma_long: bool,
    pub use_vwap: bool,
    pub use_atr: bool,
}

impl Default for FeatureMask {
    fn default() -> Self {
        Self::all()
    }
}

impl FeatureMask {
    pub fn all() -> Self {
        Self {
            use_ma_short: true,
            use_ma_mid: true,
            use_ma_long: true,
            use_vwap: true,
            use_atr: true,
        }
    }

}

/// Bit layout des features dans un i64. Total = 46 bits utilisÃƒÂ©s (sur 64).
/// Layout pensÃƒÂ© pour que les features de mÃƒÂªme "famille" soient contiguÃƒÂ«s
/// (un programme KASM peut masquer un sous-ensemble en une seule shift+and).
pub mod feature_bits {
    // Timing (6 bits) Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬
    pub const HOUR_SHIFT: u32 = 0;       // 3 bits : 0..5 (H4 = {0,4,8,12,16,20})
    pub const HOUR_MASK: i64 = 0b111;
    pub const DOW_SHIFT: u32 = 3;        // 3 bits : 0..6
    pub const DOW_MASK: i64 = 0b111;
    // Mean reversion / oscillators (8 bits) Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬
    pub const RSI_SHIFT: u32 = 6;        // 4 bits : RSI/16 = bucket 0..15
    pub const RSI_MASK: i64 = 0b1111;
    pub const MADELTA_SHIFT: u32 = 10;   // 4 bits : (close-MA20)/MA20 bps signed -8..+7
    pub const MADELTA_MASK: i64 = 0b1111;
    // Volatility (4 bits) Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬
    pub const VOL_SHIFT: u32 = 14;       // 4 bits : ATR(14)/close bucket 0..15
    pub const VOL_MASK: i64 = 0b1111;
    // Momentum lag returns (5 bits, signe seulement) Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬
    pub const LAG1_SHIFT: u32 = 18;
    pub const LAG2_SHIFT: u32 = 19;
    pub const LAG3_SHIFT: u32 = 20;
    pub const LAG4_SHIFT: u32 = 21;
    pub const LAG5_SHIFT: u32 = 22;
    // Range (4 bits) Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬
    pub const HILO_SHIFT: u32 = 23;      // 4 bits : (high-low)/close bucket
    pub const HILO_MASK: i64 = 0b1111;
    // Trend multi-ÃƒÂ©chelles (12 bits) Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬
    pub const MA50_SHIFT: u32 = 27;      // 4 bits : (close-MA50)/MA50 bps signed
    pub const MA50_MASK: i64 = 0b1111;
    pub const MA200_SHIFT: u32 = 31;     // 4 bits : (close-MA200)/MA200 bps signed
    pub const MA200_MASK: i64 = 0b1111;
    pub const ADX_SHIFT: u32 = 35;       // 4 bits : ADX(14)/100 bucket
    pub const ADX_MASK: i64 = 0b1111;
    // VWAP relative (4 bits) Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬
    pub const VWAP_SHIFT: u32 = 39;      // 4 bits : (close-VWAP6)/VWAP6 bps signed
    pub const VWAP_MASK: i64 = 0b1111;
    // Reserved : bits 43..63 (21 bits libres pour iteration future)
}

/// Fast path : utilise un FeatureCache prÃƒÂ©-calculÃƒÂ©. O(1) par feature
/// aprÃƒÂ¨s un build O(N). Ãƒâ‚¬ utiliser sur tout range de bars (la voie
/// canonique ; les anciennes `compute_*` boucles O(K) ont ÃƒÂ©tÃƒÂ©
/// supprimÃƒÂ©es avec la migration M1.3 Ã¢â‚¬â€ `git log` pour l'historique).
pub fn extract_features_with_cache(
    bars: &[Bar],
    i: usize,
    mask: &FeatureMask,
    cache: &FeatureCache,
) -> Option<i64> {
    use feature_bits::*;
    if i < MIN_HISTORY || i >= bars.len() {
        return None;
    }
    let bar = bars[i];
    if bar.close <= 0.0 || !bar.close.is_finite() {
        return None;
    }

    let secs = bar.time_ms / 1000;
    let hour = ((secs / 3600).rem_euclid(24) / 4) as i64 & HOUR_MASK;
    let days = secs.div_euclid(86400);
    let dow = (days + 4).rem_euclid(7) as i64 & DOW_MASK;

    let rsi = cache.rsi(i, 14)?;
    let rsi_b = ((rsi * 16.0).clamp(0.0, 15.99)) as i64 & RSI_MASK;

    let ma = cache.sma(i, 20)?;
    let madelta_bps = ((bar.close - ma) / ma * 10_000.0).round() as i64;
    let madelta_b = madelta_bps.clamp(-8 * 25, 7 * 25) / 25;
    let madelta_packed = (madelta_b & 0xF) & MADELTA_MASK;

    let atr = cache.atr(i, 14)?;
    let atr_bps = ((atr / bar.close) * 10_000.0).round() as i64;
    let vol_b = (atr_bps / 10).clamp(0, 15) & VOL_MASK;

    let mut lag_bits: i64 = 0;
    for k in 1..=5 {
        if i < k { return None; }
        let prev = bars[i - k].close;
        let curr = if k == 1 { bar.close } else { bars[i - k + 1].close };
        if prev > 0.0 && curr > prev { lag_bits |= 1 << (k - 1); }
    }

    let hilo_bps = (((bar.high - bar.low) / bar.close) * 10_000.0).round() as i64;
    let hilo_b = (hilo_bps / 10).clamp(0, 15) & HILO_MASK;

    let ma50 = cache.sma(i, 50)?;
    let ma50_bps = ((bar.close - ma50) / ma50 * 10_000.0).round() as i64;
    let ma50_b = ma50_bps.clamp(-8 * 25, 7 * 25) / 25;
    let ma50_packed = (ma50_b & 0xF) & MA50_MASK;

    let ma200 = cache.sma(i, 200)?;
    let ma200_bps = ((bar.close - ma200) / ma200 * 10_000.0).round() as i64;
    let ma200_b = ma200_bps.clamp(-8 * 50, 7 * 50) / 50;
    let ma200_packed = (ma200_b & 0xF) & MA200_MASK;

    let adx = cache.adx(i, 14)?;
    let adx_b = ((adx / 100.0 * 16.0).clamp(0.0, 15.99)) as i64 & ADX_MASK;

    let vwap6 = cache.vwap(i, 6)?;
    let vwap_bps = ((bar.close - vwap6) / vwap6 * 10_000.0).round() as i64;
    let vwap_b = vwap_bps.clamp(-8 * 25, 7 * 25) / 25;
    let vwap_packed = (vwap_b & 0xF) & VWAP_MASK;

    let mut packed: i64 = 0;
    packed |= hour << HOUR_SHIFT;
    packed |= dow << DOW_SHIFT;
    packed |= rsi_b << RSI_SHIFT;
    packed |= ((lag_bits & 1) << LAG1_SHIFT)
        | (((lag_bits >> 1) & 1) << LAG2_SHIFT)
        | (((lag_bits >> 2) & 1) << LAG3_SHIFT)
        | (((lag_bits >> 3) & 1) << LAG4_SHIFT)
        | (((lag_bits >> 4) & 1) << LAG5_SHIFT);
    packed |= hilo_b << HILO_SHIFT;
    packed |= adx_b << ADX_SHIFT;
    if mask.use_ma_short { packed |= madelta_packed << MADELTA_SHIFT; }
    if mask.use_atr      { packed |= vol_b << VOL_SHIFT; }
    if mask.use_ma_mid   { packed |= ma50_packed << MA50_SHIFT; }
    if mask.use_ma_long  { packed |= ma200_packed << MA200_SHIFT; }
    if mask.use_vwap     { packed |= vwap_packed << VWAP_SHIFT; }

    Some(packed)
}

// Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬ FeatureCache : sliding-window via prefix sums Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬
//
// Build once O(N) sur le full bar set, puis chaque feature en O(1) par bougie.
// Remplace les compute_* boucles O(K) qui re-itÃƒÂ¨rent la window ÃƒÂ  chaque appel.
//
// Sur 25 381 bars (NATGAS H4) la mesure analytique annonÃƒÂ§ait ~96% redondance
// (10.30M Ã¢â€ â€™ 380.7k ops). Cette implÃƒÂ©mentation matÃƒÂ©rialise ce gain.
//
// Tous les prefix sums sont length `n_bars + 1` avec prefix[0] = 0 pour que
// `window_sum(start..=end) = prefix[end+1] - prefix[start]`.
pub struct FeatureCache {
    n_bars: usize,
    prefix_open: Vec<f64>,
    prefix_close: Vec<f64>,
    prefix_high: Vec<f64>,
    prefix_low: Vec<f64>,
    /// ÃŽÂ£ typical_j (pour VWAP fallback) avec typical = (h+l+c)/3
    prefix_typical: Vec<f64>,
    prefix_open_pv: Vec<f64>,
    prefix_high_pv: Vec<f64>,
    prefix_low_pv: Vec<f64>,
    prefix_close_pv: Vec<f64>,
    /// ÃŽÂ£ typical_j Ãƒâ€” volume_j
    prefix_typical_pv: Vec<f64>,
    prefix_volume: Vec<f64>,
    /// ÃŽÂ£ gain_j pour j Ã¢â€°Â¥ 1, gain_j = max(0, close_j - close_{j-1})
    prefix_gains: Vec<f64>,
    prefix_losses: Vec<f64>,
    /// ÃŽÂ£ true_range_j pour j Ã¢â€°Â¥ 0 (j=0 utilise bars[0].open comme prev_close)
    prefix_tr: Vec<f64>,
    /// ÃŽÂ£ +DM_j et ÃŽÂ£ -DM_j pour j Ã¢â€°Â¥ 1
    prefix_plus_dm: Vec<f64>,
    prefix_minus_dm: Vec<f64>,
    prefix_open_sq_pv: Vec<f64>,
    prefix_high_sq_pv: Vec<f64>,
    prefix_low_sq_pv: Vec<f64>,
    prefix_close_sq_pv: Vec<f64>,
    /// ÃŽÂ£(typicalÃ‚Â² Ãƒâ€” volume) Ã¢â‚¬â€ for VWAP standard deviation (extensions 1/2).
    prefix_typical_sq_pv: Vec<f64>,
    opens: Vec<f64>,
    closes: Vec<f64>,
    highs: Vec<f64>,
    lows: Vec<f64>,
    /// Bar indices of confirmed swing highs (local maxima, lookback=12 bars).
    swing_highs: Vec<usize>,
    /// Bar indices of confirmed swing lows (local minima, lookback=12 bars).
    swing_lows: Vec<usize>,
}

#[derive(Debug, Clone, Copy)]
pub enum VwapSource {
    Open,
    High,
    Low,
    Close,
}

impl FeatureCache {
    /// PrÃƒÂ©calcule TOUS les prefix sums en un seul pass O(N) sur les bars.
    /// Single allocation par vecteur, pas de re-walk aprÃƒÂ¨s build.
    pub fn build(bars: &[Bar]) -> Self {
        let n = bars.len();
        let mut prefix_open = Vec::with_capacity(n + 1);
        let mut prefix_close = Vec::with_capacity(n + 1);
        let mut prefix_high = Vec::with_capacity(n + 1);
        let mut prefix_low = Vec::with_capacity(n + 1);
        let mut prefix_typical = Vec::with_capacity(n + 1);
        let mut prefix_open_pv = Vec::with_capacity(n + 1);
        let mut prefix_high_pv = Vec::with_capacity(n + 1);
        let mut prefix_low_pv = Vec::with_capacity(n + 1);
        let mut prefix_close_pv = Vec::with_capacity(n + 1);
        let mut prefix_typical_pv = Vec::with_capacity(n + 1);
        let mut prefix_volume = Vec::with_capacity(n + 1);
        let mut prefix_gains = Vec::with_capacity(n + 1);
        let mut prefix_losses = Vec::with_capacity(n + 1);
        let mut prefix_tr = Vec::with_capacity(n + 1);
        let mut prefix_plus_dm = Vec::with_capacity(n + 1);
        let mut prefix_minus_dm = Vec::with_capacity(n + 1);
        let mut prefix_open_sq_pv = Vec::with_capacity(n + 1);
        let mut prefix_high_sq_pv = Vec::with_capacity(n + 1);
        let mut prefix_low_sq_pv = Vec::with_capacity(n + 1);
        let mut prefix_close_sq_pv = Vec::with_capacity(n + 1);
        let mut prefix_typical_sq_pv = Vec::with_capacity(n + 1);

        prefix_open.push(0.0);
        prefix_close.push(0.0);
        prefix_high.push(0.0);
        prefix_low.push(0.0);
        prefix_typical.push(0.0);
        prefix_open_pv.push(0.0);
        prefix_high_pv.push(0.0);
        prefix_low_pv.push(0.0);
        prefix_close_pv.push(0.0);
        prefix_typical_pv.push(0.0);
        prefix_volume.push(0.0);
        prefix_gains.push(0.0);
        prefix_losses.push(0.0);
        prefix_tr.push(0.0);
        prefix_plus_dm.push(0.0);
        prefix_minus_dm.push(0.0);
        prefix_open_sq_pv.push(0.0);
        prefix_high_sq_pv.push(0.0);
        prefix_low_sq_pv.push(0.0);
        prefix_close_sq_pv.push(0.0);
        prefix_typical_sq_pv.push(0.0);

        let mut opens = Vec::with_capacity(n);
        let mut closes = Vec::with_capacity(n);
        let mut highs = Vec::with_capacity(n);
        let mut lows = Vec::with_capacity(n);

        for j in 0..n {
            let bar = bars[j];
            let typical = (bar.high + bar.low + bar.close) / 3.0;
            prefix_open.push(prefix_open[j] + bar.open);
            prefix_close.push(prefix_close[j] + bar.close);
            prefix_high.push(prefix_high[j] + bar.high);
            prefix_low.push(prefix_low[j] + bar.low);
            prefix_typical.push(prefix_typical[j] + typical);
            prefix_open_pv.push(prefix_open_pv[j] + bar.open * bar.volume);
            prefix_high_pv.push(prefix_high_pv[j] + bar.high * bar.volume);
            prefix_low_pv.push(prefix_low_pv[j] + bar.low * bar.volume);
            prefix_close_pv.push(prefix_close_pv[j] + bar.close * bar.volume);
            prefix_typical_pv.push(prefix_typical_pv[j] + typical * bar.volume);
            prefix_open_sq_pv.push(prefix_open_sq_pv[j] + bar.open * bar.open * bar.volume);
            prefix_high_sq_pv.push(prefix_high_sq_pv[j] + bar.high * bar.high * bar.volume);
            prefix_low_sq_pv.push(prefix_low_sq_pv[j] + bar.low * bar.low * bar.volume);
            prefix_close_sq_pv.push(prefix_close_sq_pv[j] + bar.close * bar.close * bar.volume);
            prefix_typical_sq_pv.push(prefix_typical_sq_pv[j] + typical * typical * bar.volume);
            prefix_volume.push(prefix_volume[j] + bar.volume);
            opens.push(bar.open);
            closes.push(bar.close);
            highs.push(bar.high);
            lows.push(bar.low);

            let prev_close = if j == 0 { bars[0].open } else { bars[j - 1].close };
            let tr1 = bar.high - bar.low;
            let tr2 = (bar.high - prev_close).abs();
            let tr3 = (bar.low - prev_close).abs();
            prefix_tr.push(prefix_tr[j] + tr1.max(tr2).max(tr3));

            if j == 0 {
                prefix_gains.push(0.0);
                prefix_losses.push(0.0);
                prefix_plus_dm.push(0.0);
                prefix_minus_dm.push(0.0);
            } else {
                let prev = bars[j - 1];
                let dc = bar.close - prev.close;
                let gain = if dc > 0.0 { dc } else { 0.0 };
                let loss = if dc < 0.0 { -dc } else { 0.0 };
                prefix_gains.push(prefix_gains[j] + gain);
                prefix_losses.push(prefix_losses[j] + loss);

                let up_move = bar.high - prev.high;
                let down_move = prev.low - bar.low;
                let plus_dm = if up_move > down_move && up_move > 0.0 { up_move } else { 0.0 };
                let minus_dm = if down_move > up_move && down_move > 0.0 { down_move } else { 0.0 };
                prefix_plus_dm.push(prefix_plus_dm[j] + plus_dm);
                prefix_minus_dm.push(prefix_minus_dm[j] + minus_dm);
            }
        }

        // Detect swing highs/lows (lookback/lookforward = 12 bars Ã¢â€°Ë† 2 days H4).
        // Used for anchored VWAPs. O(N Ãƒâ€” 24) Ã¢â‚¬â€ trivial on 25k bars.
        const SWING_LB: usize = 12;
        let mut swing_highs = Vec::new();
        let mut swing_lows = Vec::new();
        for i in SWING_LB..n.saturating_sub(SWING_LB) {
            let mut is_high = true;
            let mut is_low = true;
            for j in i.saturating_sub(SWING_LB)..i {
                if bars[j].high > bars[i].high { is_high = false; }
                if bars[j].low < bars[i].low { is_low = false; }
            }
            if is_high || is_low {
                for j in (i + 1)..=(i + SWING_LB).min(n - 1) {
                    if bars[j].high > bars[i].high { is_high = false; }
                    if bars[j].low < bars[i].low { is_low = false; }
                    if !is_high && !is_low { break; }
                }
            }
            if is_high { swing_highs.push(i); }
            if is_low { swing_lows.push(i); }
        }

        Self {
            n_bars: n,
            prefix_open,
            prefix_close,
            prefix_high,
            prefix_low,
            prefix_typical,
            prefix_open_pv,
            prefix_high_pv,
            prefix_low_pv,
            prefix_close_pv,
            prefix_typical_pv,
            prefix_volume,
            prefix_gains,
            prefix_losses,
            prefix_tr,
            prefix_plus_dm,
            prefix_minus_dm,
            prefix_open_sq_pv,
            prefix_high_sq_pv,
            prefix_low_sq_pv,
            prefix_close_sq_pv,
            prefix_typical_sq_pv,
            opens,
            closes,
            highs,
            lows,
            swing_highs,
            swing_lows,
        }
    }

    /// SMA(n) ÃƒÂ  la bougie i. O(1).
    pub fn sma(&self, i: usize, n: usize) -> Option<f64> {
        if i + 1 < n || i >= self.n_bars { return None; }
        let sum = self.prefix_close[i + 1] - self.prefix_close[i + 1 - n];
        Some(sum / n as f64)
    }

    /// RSI(n) ÃƒÂ  i, sur la fenÃƒÂªtre [i+1-n .. i] (besoin des deltas, donc
    /// i+1 >= n+1 pour avoir n deltas valides).
    pub fn rsi(&self, i: usize, n: usize) -> Option<f64> {
        if i + 1 < n + 1 || i >= self.n_bars { return None; }
        let gains = self.prefix_gains[i + 1] - self.prefix_gains[i + 1 - n];
        let losses = self.prefix_losses[i + 1] - self.prefix_losses[i + 1 - n];
        if gains + losses <= 0.0 {
            return Some(0.5);
        }
        Some(gains / (gains + losses))
    }

    /// ATR(n) ÃƒÂ  i. O(1).
    pub fn atr(&self, i: usize, n: usize) -> Option<f64> {
        if i + 1 < n + 1 || i >= self.n_bars { return None; }
        let sum = self.prefix_tr[i + 1] - self.prefix_tr[i + 1 - n];
        Some(sum / n as f64)
    }

    /// VWAP(n) ÃƒÂ  i. Fallback sur SMA(typical) si volume total nul.
    pub fn vwap(&self, i: usize, n: usize) -> Option<f64> {
        if i + 1 < n || i >= self.n_bars { return None; }
        let pv = self.prefix_typical_pv[i + 1] - self.prefix_typical_pv[i + 1 - n];
        let v = self.prefix_volume[i + 1] - self.prefix_volume[i + 1 - n];
        if v <= 0.0 {
            let typical = self.prefix_typical[i + 1] - self.prefix_typical[i + 1 - n];
            return Some(typical / n as f64);
        }
        Some(pv / v)
    }

    /// ADX(n) Ã¢â€°Ë† DX (cf. compute_adx Ã¢â‚¬â€ approximation sans le smoothing 14-period).
    pub fn adx(&self, i: usize, n: usize) -> Option<f64> {
        if i + 1 < n + 1 || i >= self.n_bars { return None; }
        let sum_plus = self.prefix_plus_dm[i + 1] - self.prefix_plus_dm[i + 1 - n];
        let sum_minus = self.prefix_minus_dm[i + 1] - self.prefix_minus_dm[i + 1 - n];
        let sum_tr = self.prefix_tr[i + 1] - self.prefix_tr[i + 1 - n];
        if sum_tr <= 0.0 { return Some(0.0); }
        let plus_di = 100.0 * sum_plus / sum_tr;
        let minus_di = 100.0 * sum_minus / sum_tr;
        let denom = plus_di + minus_di;
        if denom <= 0.0 { return Some(0.0); }
        Some(100.0 * (plus_di - minus_di).abs() / denom)
    }

    /// VWAP(n) + standard deviation ÃÆ’ at bar i. O(1).
    /// Extension 1 = vwap Ã‚Â± ÃÆ’, Extension 2 = vwap Ã‚Â± 2ÃÆ’.
    /// `sigma_units = (close - vwap) / ÃÆ’` tells you how many ÃÆ’ from VWAP.
    pub fn vwap_sigma(&self, i: usize, n: usize) -> Option<(f64, f64)> {
        if i + 1 < n || i >= self.n_bars { return None; }
        let pv = self.prefix_typical_pv[i + 1] - self.prefix_typical_pv[i + 1 - n];
        let v = self.prefix_volume[i + 1] - self.prefix_volume[i + 1 - n];
        let vwap = if v > 0.0 {
            pv / v
        } else {
            let typical = self.prefix_typical[i + 1] - self.prefix_typical[i + 1 - n];
            typical / n as f64
        };
        let sq_pv = self.prefix_typical_sq_pv[i + 1] - self.prefix_typical_sq_pv[i + 1 - n];
        let variance = if v > 0.0 {
            (sq_pv / v) - vwap * vwap
        } else {
            0.0
        };
        let sigma = if variance > 0.0 { variance.sqrt() } else { 0.0 };
        Some((vwap, sigma))
    }

    fn source_prefixes(&self, source: VwapSource) -> (&[f64], &[f64], &[f64]) {
        match source {
            VwapSource::Open => (&self.prefix_open, &self.prefix_open_pv, &self.prefix_open_sq_pv),
            VwapSource::High => (&self.prefix_high, &self.prefix_high_pv, &self.prefix_high_sq_pv),
            VwapSource::Low => (&self.prefix_low, &self.prefix_low_pv, &self.prefix_low_sq_pv),
            VwapSource::Close => (&self.prefix_close, &self.prefix_close_pv, &self.prefix_close_sq_pv),
        }
    }

    /// Anchored VWAP from bar `from` to bar `to` (inclusive). O(1).
    /// Returns (avwap, ÃÆ’). Used for VWAP anchored at swing points.
    pub fn anchored_vwap_source(&self, from: usize, to: usize, source: VwapSource) -> Option<(f64, f64)> {
        if from > to || to >= self.n_bars { return None; }
        if to + 1 - from < 2 { return None; }
        let (source_prefix, source_pv, source_sq_pv) = self.source_prefixes(source);
        let pv = source_pv[to + 1] - source_pv[from];
        let v = self.prefix_volume[to + 1] - self.prefix_volume[from];
        let n_f = (to + 1 - from) as f64;
        let avwap = if v > 0.0 {
            pv / v
        } else {
            let source_sum = source_prefix[to + 1] - source_prefix[from];
            source_sum / n_f
        };
        let sq_pv = source_sq_pv[to + 1] - source_sq_pv[from];
        let variance = if v > 0.0 {
            (sq_pv / v) - avwap * avwap
        } else {
            0.0
        };
        let sigma = if variance > 0.0 { variance.sqrt() } else { 0.0 };
        Some((avwap, sigma))
    }

    /// Last confirmed swing high BEFORE bar `i`.
    /// A swing at bar j is confirmed when i >= j + 12 (lookforward cleared).
    pub fn last_swing_high_before(&self, i: usize) -> Option<usize> {
        const SWING_CONFIRM: usize = 12;
        let cutoff = i.checked_sub(SWING_CONFIRM)?;
        match self.swing_highs.partition_point(|&x| x <= cutoff) {
            0 => None,
            pos => Some(self.swing_highs[pos - 1]),
        }
    }

    /// Last confirmed swing low BEFORE bar `i`.
    pub fn last_swing_low_before(&self, i: usize) -> Option<usize> {
        const SWING_CONFIRM: usize = 12;
        let cutoff = i.checked_sub(SWING_CONFIRM)?;
        match self.swing_lows.partition_point(|&x| x <= cutoff) {
            0 => None,
            pos => Some(self.swing_lows[pos - 1]),
        }
    }

    pub fn major_swing_high_before(&self, i: usize, lookback_bars: usize) -> Option<usize> {
        const SWING_CONFIRM: usize = 12;
        let cutoff = i.checked_sub(SWING_CONFIRM)?;
        let window_start = cutoff.saturating_sub(lookback_bars);
        self.swing_highs
            .iter()
            .copied()
            .filter(|&idx| idx >= window_start && idx <= cutoff)
            .max_by(|&a, &b| self.highs[a].total_cmp(&self.highs[b]).then(a.cmp(&b)))
    }

    pub fn major_swing_low_before(&self, i: usize, lookback_bars: usize) -> Option<usize> {
        const SWING_CONFIRM: usize = 12;
        let cutoff = i.checked_sub(SWING_CONFIRM)?;
        let window_start = cutoff.saturating_sub(lookback_bars);
        self.swing_lows
            .iter()
            .copied()
            .filter(|&idx| idx >= window_start && idx <= cutoff)
            .min_by(|&a, &b| self.lows[a].total_cmp(&self.lows[b]).then(a.cmp(&b)))
    }

    pub fn strongest_bullish_bar_before(&self, i: usize, lookback_bars: usize) -> Option<usize> {
        let cutoff = i.checked_sub(1)?;
        let window_start = cutoff.saturating_sub(lookback_bars);
        (window_start..=cutoff)
            .filter(|&idx| self.closes[idx] > self.opens[idx])
            .max_by(|&a, &b| {
                let body_a = self.closes[a] - self.opens[a];
                let body_b = self.closes[b] - self.opens[b];
                body_a.total_cmp(&body_b).then(a.cmp(&b))
            })
    }

    pub fn strongest_bearish_bar_before(&self, i: usize, lookback_bars: usize) -> Option<usize> {
        let cutoff = i.checked_sub(1)?;
        let window_start = cutoff.saturating_sub(lookback_bars);
        (window_start..=cutoff)
            .filter(|&idx| self.closes[idx] < self.opens[idx])
            .max_by(|&a, &b| {
                let body_a = self.opens[a] - self.closes[a];
                let body_b = self.opens[b] - self.closes[b];
                body_a.total_cmp(&body_b).then(a.cmp(&b))
            })
    }

    pub fn stochastic_k(&self, i: usize, n: usize) -> Option<f64> {
        if i + 1 < n || i >= self.n_bars { return None; }
        let start = i + 1 - n;
        let mut hh = f64::NEG_INFINITY;
        let mut ll = f64::INFINITY;
        for j in start..=i {
            hh = hh.max(self.highs[j]);
            ll = ll.min(self.lows[j]);
        }
        let denom = hh - ll;
        if denom <= 1e-12 {
            return Some(0.5);
        }
        Some(((self.closes[i] - ll) / denom).clamp(0.0, 1.0))
    }

    pub fn bars_since_vwap_cross(&self, i: usize, n: usize, max_lookback: usize) -> Option<i64> {
        let (vwap_now, _) = self.vwap_sigma(i, n)?;
        let curr_sign = (self.closes[i] - vwap_now).signum() as i32;
        if curr_sign == 0 {
            return Some(0);
        }
        let start = i.saturating_sub(max_lookback);
        let mut age = 0i64;
        for j in (start..=i).rev() {
            let (vwap_j, _) = self.vwap_sigma(j, n)?;
            let sign_j = (self.closes[j] - vwap_j).signum() as i32;
            if sign_j == 0 || sign_j != curr_sign {
                break;
            }
            age += 1;
        }
        Some(age * curr_sign as i64)
    }

    pub fn bars_outside_sigma1(&self, i: usize, n: usize, max_lookback: usize) -> Option<i64> {
        let (vwap_now, sig_now) = self.vwap_sigma(i, n)?;
        if sig_now <= 1e-12 {
            return Some(0);
        }
        let sigma_now = (self.closes[i] - vwap_now) / sig_now;
        let curr_sign = if sigma_now >= 1.0 {
            1i64
        } else if sigma_now <= -1.0 {
            -1i64
        } else {
            0
        };
        if curr_sign == 0 {
            return Some(0);
        }
        let start = i.saturating_sub(max_lookback);
        let mut age = 0i64;
        for j in (start..=i).rev() {
            let (vwap_j, sig_j) = self.vwap_sigma(j, n)?;
            if sig_j <= 1e-12 {
                break;
            }
            let sigma_j = (self.closes[j] - vwap_j) / sig_j;
            let sign_j = if sigma_j >= 1.0 {
                1i64
            } else if sigma_j <= -1.0 {
                -1i64
            } else {
                0
            };
            if sign_j != curr_sign {
                break;
            }
            age += 1;
        }
        Some(age * curr_sign)
    }
}

// Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬ Trade simulation Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬

/// Direction d'un trade simulÃƒÂ©.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Long,
    Short,
}

/// RÃƒÂ©sultat d'une simulation single-trade : P&L en POINTS de prix.
/// Sorties possibles : TP touchÃƒÂ© (+tp_pts), SL touchÃƒÂ© (-sl_pts),
/// horizon atteint (floating P&L), ou impossible (pas de donnÃƒÂ©es).
#[derive(Debug, Clone, Copy)]
pub struct TradeResult {
    pub pnl_points: f64,
    pub bars_held: usize,
    pub exit_reason: ExitReason,
}

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)] // used by forge_mcp bin
pub struct StraddleResult {
    pub long: TradeResult,
    pub short: TradeResult,
    pub pnl_points: f64,
    pub bars_held: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitReason {
    TakeProfit,
    StopLoss,
    Horizon,
    NotPossible,
}

/// Simule un trade avec SL ET TP fixes. PrioritÃƒÂ© intra-bougie :
/// SL > TP (convention pessimiste Ã¢â‚¬â€ si les deux sont dans la range,
/// on assume SL touchÃƒÂ© en premier). Exit horizon si ni SL ni TP.
pub fn simulate_trade(
    bars: &[Bar],
    entry_idx: usize,
    direction: Direction,
    sl_points: f64,
    tp_points: f64,
    spread_points: f64,
    max_horizon: usize,
) -> TradeResult {
    if entry_idx >= bars.len() {
        return TradeResult { pnl_points: 0.0, bars_held: 0, exit_reason: ExitReason::NotPossible };
    }
    let entry_price = bars[entry_idx].close;
    let (sl_level, tp_level) = match direction {
        Direction::Long => (entry_price - sl_points, entry_price + tp_points),
        Direction::Short => (entry_price + sl_points, entry_price - tp_points),
    };

    let end = (entry_idx + max_horizon).min(bars.len() - 1);
    if end <= entry_idx {
        return TradeResult { pnl_points: 0.0, bars_held: 0, exit_reason: ExitReason::NotPossible };
    }

    for h in 1..=(end - entry_idx) {
        let bar = bars[entry_idx + h];
        let sl_hit = match direction {
            Direction::Long => bar.low <= sl_level,
            Direction::Short => bar.high >= sl_level,
        };
        if sl_hit {
            return TradeResult {
                pnl_points: -sl_points - spread_points,
                bars_held: h,
                exit_reason: ExitReason::StopLoss,
            };
        }
        let tp_hit = match direction {
            Direction::Long => bar.high >= tp_level,
            Direction::Short => bar.low <= tp_level,
        };
        if tp_hit {
            return TradeResult {
                pnl_points: tp_points - spread_points,
                bars_held: h,
                exit_reason: ExitReason::TakeProfit,
            };
        }
    }
    let exit_price = bars[end].close;
    let pnl = match direction {
        Direction::Long => exit_price - entry_price,
        Direction::Short => entry_price - exit_price,
    } - spread_points;
    TradeResult {
        pnl_points: pnl,
        bars_held: end - entry_idx,
        exit_reason: ExitReason::Horizon,
    }
}

/// Atlas-backed wrapper around the trade simulator. Checks the atlas
/// for a previously-computed `(file_hash, bar, direction, sl, horizon)`
/// outcome before recomputing.
///
/// On miss, the trade is computed via the **KASM trade simulator** when
/// the requested horizon matches the current Alpha H4 specialization
/// (`TRADE_HORIZON = 6`). Boundary checks and OHLC access stay in Rust;
/// the first-event selection, SL/TP priority, spread-adjusted PnL,
/// bars-held and exit reason are content-addressed KASM programs.
pub fn simulate_trade_with_atlas(
    bars: &[Bar],
    entry_idx: usize,
    direction: Direction,
    sl_points: f64,
    tp_points: f64,
    spread_points: f64,
    max_horizon: usize,
    atlas: &scan::atlas::Atlas,
    file_hash: u64,
) -> TradeResult {
    use scan::atlas::{kind, Atlas};
    let dir_byte = match direction {
        Direction::Long => 1,
        Direction::Short => 2,
    };
    let key = Atlas::trade_key(
        file_hash,
        entry_idx as u32,
        dir_byte,
        sl_points,
        tp_points,
        spread_points,
        max_horizon as u16,
    );
    if let Some(packed) = atlas.lookup_with_value(kind::RESULT, &key) {
        let (pnl, exit_byte, bars_held) = Atlas::unpack_trade(&packed);
        let exit_reason = match exit_byte {
            1 => ExitReason::StopLoss,
            2 => ExitReason::Horizon,
            3 => ExitReason::TakeProfit,
            _ => ExitReason::NotPossible,
        };
        return TradeResult {
            pnl_points: pnl,
            bars_held: bars_held as usize,
            exit_reason,
        };
    }

    let result = if max_horizon == crate::kasm_indicators::TRADE_HORIZON {
        compute_trade_with_kasm(
            bars,
            entry_idx,
            direction,
            sl_points,
            tp_points,
            spread_points,
            max_horizon,
        )
    } else {
        simulate_trade(
            bars,
            entry_idx,
            direction,
            sl_points,
            tp_points,
            spread_points,
            max_horizon,
        )
    };

    let exit_byte = match result.exit_reason {
        ExitReason::TakeProfit => 3u8,
        ExitReason::StopLoss => 1u8,
        ExitReason::Horizon => 2u8,
        ExitReason::NotPossible => 0u8,
    };
    let packed = Atlas::pack_trade(
        result.pnl_points,
        exit_byte,
        result.bars_held.min(255) as u8,
    );
    let _ = atlas.record_with_value(kind::RESULT, &key, &packed);
    result
}

fn compute_trade_with_kasm(
    bars: &[Bar],
    entry_idx: usize,
    direction: Direction,
    sl_points: f64,
    tp_points: f64,
    spread_points: f64,
    max_horizon: usize,
) -> TradeResult {
    if entry_idx >= bars.len() {
        return TradeResult { pnl_points: 0.0, bars_held: 0, exit_reason: ExitReason::NotPossible };
    }
    let end = (entry_idx + max_horizon).min(bars.len() - 1);
    if end <= entry_idx {
        return TradeResult { pnl_points: 0.0, bars_held: 0, exit_reason: ExitReason::NotPossible };
    }

    let entry_price = bars[entry_idx].close;
    let (sl_level, tp_level) = match direction {
        Direction::Long => (entry_price - sl_points, entry_price + tp_points),
        Direction::Short => (entry_price + sl_points, entry_price - tp_points),
    };

    let mut sl_hits = [0i64; crate::kasm_indicators::TRADE_HORIZON];
    let mut tp_hits = [0i64; crate::kasm_indicators::TRADE_HORIZON];
    for h in 1..=(end - entry_idx) {
        let bar = bars[entry_idx + h];
        let sl_hit = match direction {
            Direction::Long => bar.low <= sl_level,
            Direction::Short => bar.high >= sl_level,
        };
        let tp_hit = match direction {
            Direction::Long => bar.high >= tp_level,
            Direction::Short => bar.low <= tp_level,
        };
        sl_hits[h - 1] = sl_hit as i64;
        tp_hits[h - 1] = tp_hit as i64;
    }

    let exit_price = bars[end].close;
    let pnl_horizon = match direction {
        Direction::Long => exit_price - entry_price,
        Direction::Short => entry_price - exit_price,
    };
    let kasm = crate::kasm_indicators::compute_trade_kasm(
        sl_hits,
        tp_hits,
        pnl_horizon,
        sl_points,
        tp_points,
        spread_points,
    );

    let exit_reason = match kasm.exit_reason {
        1 => ExitReason::StopLoss,
        2 => ExitReason::Horizon,
        3 => ExitReason::TakeProfit,
        _ => ExitReason::NotPossible,
    };
    let bars_held = if exit_reason == ExitReason::Horizon && kasm.bars_held == 0 {
        end - entry_idx
    } else {
        kasm.bars_held.max(0) as usize
    };

    TradeResult {
        pnl_points: kasm.pnl_points,
        bars_held,
        exit_reason,
    }
}

// Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬ Decision-time filter Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬

/// Entry is allowed on daytime H4 closes used by the Alpha straddle workflow.
/// Feeds differ on their H4 grid alignment and DST handling, so we accept
/// close hours from 05:00 through 23:00 UTC and still skip overnight windows.
pub fn is_decision_hour(bar: &Bar) -> bool {
    let secs = bar.time_ms / 1000;
    let open_hour = ((secs / 3600).rem_euclid(24)) as u32;
    let close_hour = (open_hour + 4) % 24;
    matches!(close_hour, 5..=23)
}

// Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬ Examples builder pour Forge synthesis Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬

/// Configuration de la synthÃƒÂ¨se reverse.
#[derive(Debug, Clone, Copy)]
pub struct SynthConfig {
    /// Stop loss en points de prix (NATGAS : 0.09 = 9 cents).
    pub sl_points: f64,
    /// Take profit en points de prix (ex: 0.07 = 7 cents).
    pub tp_points: f64,
    /// Coût fixe de spread round-trip déduit de chaque trade complété.
    pub spread_points: f64,
    /// Horizon max d'un trade en bougies (au-delÃƒÂ , exit forcÃƒÂ©).
    pub max_horizon_bars: usize,
    /// Cumul P&L cible par jour (ex: 0.07 = "+7 points par jour").
    pub target_pnl_per_day: f64,
    /// Fraction de bougies utilisÃƒÂ©es pour train (le reste = holdout).
    pub train_split: f64,
}

impl Default for SynthConfig {
    fn default() -> Self {
        Self {
            sl_points: 0.09,
            tp_points: 0.02,
            spread_points: 0.008,
            max_horizon_bars: 6,
            target_pnl_per_day: 0.07,
            train_split: 0.7,
        }
    }
}

/// Open both legs on the same H4 close with identical SL/TP. The result is
/// the net P&L of LONG + SHORT, including spread on both legs.
#[allow(dead_code)] // used by forge_mcp bin
pub fn simulate_straddle(
    bars: &[Bar],
    entry_idx: usize,
    sl_points: f64,
    tp_points: f64,
    spread_points: f64,
    max_horizon: usize,
) -> StraddleResult {
    let long = simulate_trade(
        bars,
        entry_idx,
        Direction::Long,
        sl_points,
        tp_points,
        spread_points,
        max_horizon,
    );
    let short = simulate_trade(
        bars,
        entry_idx,
        Direction::Short,
        sl_points,
        tp_points,
        spread_points,
        max_horizon,
    );
    StraddleResult {
        long,
        short,
        pnl_points: long.pnl_points + short.pnl_points,
        bars_held: long.bars_held.max(short.bars_held),
    }
}

pub type BinaryOpportunityLabels = (i64, i64);

#[derive(Debug, Clone, Copy, Default)]
pub struct GridSearchStats {
    pub evaluated_pairs: usize,
    pub long_take_profit_hits: usize,
    pub short_take_profit_hits: usize,
    pub long_stop_loss_hits: usize,
    pub short_stop_loss_hits: usize,
    pub long_bars_held_sum: usize,
    pub short_bars_held_sum: usize,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct AtlasCacheStats {
    pub atlas_hits: usize,
    pub computed_rows: usize,
    pub persisted_values: usize,
    pub grid_evaluated_pairs: usize,
    pub long_take_profit_hits: usize,
    pub short_take_profit_hits: usize,
    pub long_stop_loss_hits: usize,
    pub short_stop_loss_hits: usize,
    pub long_bars_held_sum: usize,
    pub short_bars_held_sum: usize,
}

pub fn implicit_point_size(cfg: SynthConfig) -> f64 {
    (cfg.sl_points.abs() / 9.0).max(1e-6)
}

#[allow(dead_code)] // used by forge_mcp bin via select_best_straddle_grid_config
pub fn sl_grid_points(cfg: SynthConfig) -> Vec<f64> {
    let point = implicit_point_size(cfg);
    let max_sl = cfg.sl_points.abs().max(point);
    let mut out = (2..=9)
        .map(|pts| (pts as f64 * point).min(max_sl).max(point))
        .collect::<Vec<_>>();
    out.push(max_sl);
    out.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    out.dedup_by(|a, b| (*a - *b).abs() <= 1e-9);
    out
}

pub fn tp_grid_points(cfg: SynthConfig) -> Vec<f64> {
    let point = implicit_point_size(cfg);
    let min_tp = cfg.tp_points.abs().max(2.0 * point);
    let mut out = [2.0_f64, 3.0, 4.0, 5.0, 7.0, 9.0, 12.0, 16.0, 24.0, 32.0]
        .into_iter()
        .map(|pts| (pts * point).max(min_tp).max(cfg.spread_points * 2.0).max(1e-6))
    .collect::<Vec<_>>();
    out.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    out.dedup_by(|a, b| (*a - *b).abs() <= 1e-9);
    out
}

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)] // used by forge_mcp bin
pub struct StraddleGridSelection {
    pub cfg: SynthConfig,
    pub combinations: usize,
    pub decision_rows: usize,
    pub target_hit_pct: f64,
    pub total_pnl_points: f64,
    pub avg_expiry_bars: f64,
}

#[allow(dead_code)] // used by forge_mcp bin
pub fn select_best_straddle_grid_config(
    bars: &[Bar],
    range: std::ops::Range<usize>,
    cfg: SynthConfig,
) -> Option<StraddleGridSelection> {
    let sl_grid = sl_grid_points(cfg);
    let tp_grid = tp_grid_points(cfg);
    let start = range.start.max(MIN_HISTORY);
    let end = range.end.min(bars.len().saturating_sub(cfg.max_horizon_bars));
    let mut best: Option<StraddleGridSelection> = None;

    for sl_points in sl_grid.iter().copied() {
        for tp_points in tp_grid.iter().copied() {
            let mut candidate_cfg = cfg;
            candidate_cfg.sl_points = sl_points;
            candidate_cfg.tp_points = tp_points;

            let mut days = 0usize;
            let mut days_target = 0usize;
            let mut total_pnl = 0.0_f64;
            let mut day_pnl = 0.0_f64;
            let mut day_had_trade = false;
            let mut current_day_ms: i64 = -1;
            let mut decision_rows = 0usize;
            let mut expiry_sum = 0usize;

            for i in start..end {
                let bar = bars[i];
                let day_ms = bar.time_ms.div_euclid(86_400_000) * 86_400_000;
                if day_ms != current_day_ms {
                    if current_day_ms >= 0 && day_had_trade {
                        days += 1;
                        if day_pnl >= cfg.target_pnl_per_day {
                            days_target += 1;
                        }
                    }
                    current_day_ms = day_ms;
                    day_pnl = 0.0;
                    day_had_trade = false;
                }
                if !is_decision_hour(&bar) {
                    continue;
                }
                let straddle = simulate_straddle(
                    bars,
                    i,
                    sl_points,
                    tp_points,
                    cfg.spread_points,
                    cfg.max_horizon_bars,
                );
                if straddle.long.exit_reason == ExitReason::NotPossible
                    || straddle.short.exit_reason == ExitReason::NotPossible
                {
                    continue;
                }
                decision_rows += 1;
                expiry_sum += straddle.bars_held;
                total_pnl += straddle.pnl_points;
                day_pnl += straddle.pnl_points;
                day_had_trade = true;
            }
            if current_day_ms >= 0 && day_had_trade {
                days += 1;
                if day_pnl >= cfg.target_pnl_per_day {
                    days_target += 1;
                }
            }
            if decision_rows == 0 {
                continue;
            }
            let selection = StraddleGridSelection {
                cfg: candidate_cfg,
                combinations: sl_grid.len() * tp_grid.len(),
                decision_rows,
                target_hit_pct: if days == 0 { 0.0 } else { 100.0 * days_target as f64 / days as f64 },
                total_pnl_points: total_pnl,
                avg_expiry_bars: expiry_sum as f64 / decision_rows as f64,
            };
            let replace = best
                .as_ref()
                .map(|current| {
                    selection.target_hit_pct > current.target_hit_pct + 1e-9
                        || ((selection.target_hit_pct - current.target_hit_pct).abs() <= 1e-9
                            && selection.total_pnl_points > current.total_pnl_points + 1e-9)
                })
                .unwrap_or(true);
            if replace {
                best = Some(selection);
            }
        }
    }

    best
}

fn binary_opportunity_labels_grid_with_stats(
    bars: &[Bar],
    entry_idx: usize,
    cfg: SynthConfig,
    atlas: &scan::atlas::Atlas,
    file_hash: u64,
) -> (BinaryOpportunityLabels, GridSearchStats) {
    let mut stats = GridSearchStats::default();
    let long = simulate_trade_with_atlas(
        bars,
        entry_idx,
        Direction::Long,
        cfg.sl_points,
        cfg.tp_points,
        cfg.spread_points,
        cfg.max_horizon_bars,
        atlas,
        file_hash,
    );
    let short = simulate_trade_with_atlas(
        bars,
        entry_idx,
        Direction::Short,
        cfg.sl_points,
        cfg.tp_points,
        cfg.spread_points,
        cfg.max_horizon_bars,
        atlas,
        file_hash,
    );
    stats.evaluated_pairs += 1;
    stats.long_bars_held_sum += long.bars_held;
    stats.short_bars_held_sum += short.bars_held;
    match long.exit_reason {
        ExitReason::TakeProfit => stats.long_take_profit_hits += 1,
        ExitReason::StopLoss => stats.long_stop_loss_hits += 1,
        _ => {}
    }
    match short.exit_reason {
        ExitReason::TakeProfit => stats.short_take_profit_hits += 1,
        ExitReason::StopLoss => stats.short_stop_loss_hits += 1,
        _ => {}
    }
    let net_pnl = long.pnl_points + short.pnl_points;
    let label = if net_pnl > 0.0 { 1 } else { 0 };
    ((label, label), stats)
}

/// Materialize binary LONG/SHORT opportunity labels for each eligible bar in
/// `range`. We simulate each direction once and share the resulting labels
/// across stage-1 feature examples and stage-2 confluence examples.
pub fn build_binary_label_cache_with_stats(
    bars: &[Bar],
    range: std::ops::Range<usize>,
    cfg: SynthConfig,
    atlas: &scan::atlas::Atlas,
    file_hash: u64,
) -> (Vec<Option<BinaryOpportunityLabels>>, AtlasCacheStats) {
    let start = range.start.max(MIN_HISTORY);
    let end = range.end.min(bars.len().saturating_sub(cfg.max_horizon_bars));
    let mut rows: Vec<Option<BinaryOpportunityLabels>> = vec![None; bars.len()];
    let mut stats = AtlasCacheStats::default();

    for i in start..end {
        let bar = bars[i];
        if !is_decision_hour(&bar) || bar.close <= 0.0 || !bar.close.is_finite() {
            continue;
        }
        let label_key = scan::atlas::Atlas::label_key(
            file_hash,
            i as u32,
            cfg.sl_points,
            cfg.tp_points,
            cfg.spread_points,
            cfg.max_horizon_bars as u16,
        );
        if let Some(value) = atlas.lookup_result(&label_key) {
            rows[i] = Some(scan::atlas::Atlas::unpack_binary_labels(&value));
            stats.atlas_hits += 1;
            continue;
        }
        let (labels, grid_stats) =
            binary_opportunity_labels_grid_with_stats(bars, i, cfg, atlas, file_hash);
        let packed = scan::atlas::Atlas::pack_binary_labels(labels.0, labels.1);
        let _ = atlas.record_result(&label_key, &packed);
        rows[i] = Some(labels);
        stats.computed_rows += 1;
        stats.persisted_values += 1;
        stats.grid_evaluated_pairs += grid_stats.evaluated_pairs;
        stats.long_take_profit_hits += grid_stats.long_take_profit_hits;
        stats.short_take_profit_hits += grid_stats.short_take_profit_hits;
        stats.long_stop_loss_hits += grid_stats.long_stop_loss_hits;
        stats.short_stop_loss_hits += grid_stats.short_stop_loss_hits;
        stats.long_bars_held_sum += grid_stats.long_bars_held_sum;
        stats.short_bars_held_sum += grid_stats.short_bars_held_sum;
    }

    (rows, stats)
}

/// Pour chaque bougie de dÃƒÂ©cision avec history suffisante, gÃƒÂ©nÃƒÂ¨re
/// `(features_i64, label_i64)` oÃƒÂ¹ :
///   - features = packed via `extract_features`
///   - label = direction la plus profitable simulÃƒÂ©e (LONG=+1, SHORT=-1, FLAT=0)
///
/// Convention de label : on simule LONG et SHORT ÃƒÂ  cette bougie ; le label

/// Variante qui restreint la gÃƒÂ©nÃƒÂ©ration aux bars dans `range`. UtilisÃƒÂ©
/// pour split temporel train vs holdout Ã¢â‚¬â€ Forge synth ne doit JAMAIS
/// voir les examples du holdout (sinon overfit garanti, mÃƒÂ©trique invalide).
///
/// Wrapper backward-compat sur `_masked` avec FeatureMask::all().
pub fn build_examples_in_range(
    bars: &[Bar],
    range: std::ops::Range<usize>,
    cfg: SynthConfig,
) -> Vec<(i64, i64)> {
    build_examples_in_range_masked(bars, range, cfg, &FeatureMask::all())
}

/// Atlas-backed builder. Persists every per-bar feature value (FEATURE
/// kind) and every trade simulation outcome (TRADE kind) to the atlas
/// so future sessions retrieve them without recomputing.
pub fn build_examples_in_range_masked_with_atlas(
    bars: &[Bar],
    range: std::ops::Range<usize>,
    cfg: SynthConfig,
    mask: &FeatureMask,
    atlas: &scan::atlas::Atlas,
    file_hash: u64,
) -> Vec<(i64, i64)> {
    let start = range.start.max(MIN_HISTORY);
    let end = range.end.min(bars.len().saturating_sub(cfg.max_horizon_bars));
    let cache = FeatureCache::build(bars);
    let mut examples = Vec::with_capacity(end.saturating_sub(start) / 2);
    for i in start..end {
        if !is_decision_hour(&bars[i]) {
            continue;
        }
        let features = match extract_features_with_cache(bars, i, mask, &cache) {
            Some(f) => f,
            None => continue,
        };
        let long = simulate_trade_with_atlas(
            bars,
            i,
            Direction::Long,
            cfg.sl_points,
            cfg.tp_points,
            cfg.spread_points,
            cfg.max_horizon_bars,
            atlas,
            file_hash,
        );
        let short = simulate_trade_with_atlas(
            bars,
            i,
            Direction::Short,
            cfg.sl_points,
            cfg.tp_points,
            cfg.spread_points,
            cfg.max_horizon_bars,
            atlas,
            file_hash,
        );
        let label: i64 = if long.exit_reason == ExitReason::TakeProfit && short.exit_reason != ExitReason::TakeProfit {
            1
        } else if short.exit_reason == ExitReason::TakeProfit && long.exit_reason != ExitReason::TakeProfit {
            -1
        } else if long.exit_reason == ExitReason::TakeProfit && short.exit_reason == ExitReason::TakeProfit {
            if long.bars_held <= short.bars_held { 1 } else { -1 }
        } else {
            0
        };
        examples.push((features, label));
    }
    examples
}

/// Variante avec `mask` explicite Ã¢â‚¬â€ features dÃƒÂ©sactivÃƒÂ©es ne sont pas
/// packÃƒÂ©es dans les inputs i64 (le synth les voit comme constantes 0).
///
/// ImplÃƒÂ©mentation **sliding-window** : un seul `FeatureCache::build(bars)`
/// O(N) au dÃƒÂ©but, puis chaque feature en O(1) par bougie via prefix sums.
/// Remplace l'ancien path qui re-boucait O(K) par feature Ãƒâ€” bougie
/// (~10M ops naÃƒÂ¯ves sur 25k bars NATGAS H4 Ã¢â€ â€™ ~380k rÃƒÂ©elles, mesure
/// ratio analytique 96.31%).
pub fn build_examples_in_range_masked(
    bars: &[Bar],
    range: std::ops::Range<usize>,
    cfg: SynthConfig,
    mask: &FeatureMask,
) -> Vec<(i64, i64)> {
    let start = range.start.max(MIN_HISTORY);
    let end = range.end.min(bars.len().saturating_sub(cfg.max_horizon_bars));
    let cache = FeatureCache::build(bars);
    let mut examples = Vec::with_capacity(end.saturating_sub(start) / 2);
    for i in start..end {
        if !is_decision_hour(&bars[i]) {
            continue;
        }
        let features = match extract_features_with_cache(bars, i, mask, &cache) {
            Some(f) => f,
            None => continue,
        };
        let long = simulate_trade(bars, i, Direction::Long, cfg.sl_points, cfg.tp_points, cfg.spread_points, cfg.max_horizon_bars);
        let short = simulate_trade(bars, i, Direction::Short, cfg.sl_points, cfg.tp_points, cfg.spread_points, cfg.max_horizon_bars);
        let label: i64 = if long.exit_reason == ExitReason::TakeProfit && short.exit_reason != ExitReason::TakeProfit {
            1
        } else if short.exit_reason == ExitReason::TakeProfit && long.exit_reason != ExitReason::TakeProfit {
            -1
        } else if long.exit_reason == ExitReason::TakeProfit && short.exit_reason == ExitReason::TakeProfit {
            if long.bars_held <= short.bars_held { 1 } else { -1 }
        } else {
            0
        };
        examples.push((features, label));
    }
    examples
}

// Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬ Per-feature synthesis examples Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬
//
// Instead of packing all features into one i64 bitfield (which the beam
// search can't decompose), extract each feature as a raw i64 value and
// pair it with the trade label. The beam search runs once per feature
// and can find meaningful signals like `CmpGt(RSI, 50)` or
// `CmpLt(EMA_delta, -100)`.

/// Names of the 10 VWAP-centric features for per-feature synthesis.
///
/// Features 3-8 are VWAP-based (general + anchored at swing highs/lows):
///   - vwap_delta : raw distance from 30-bar rolling VWAP (bps)
///   - vwap_sigma : normalized distance in ÃÆ’ units (ext1=Ã‚Â±100, ext2=Ã‚Â±200)
///   - avwap_hi   : distance from anchored VWAP at last swing high (bps)
///   - avwap_lo   : distance from anchored VWAP at last swing low (bps)
///   - avwap_hi_ÃÆ’ : distance from anchored VWAP high in ÃÆ’ units
///   - avwap_lo_ÃÆ’ : distance from anchored VWAP low in ÃÆ’ units
pub const FEATURE_NAMES: &[&str] = &[
    "hour", "dow", "rsi14", "vwap_delta", "vwap_sigma",
    "avwap_hi", "avwap_lo", "avwap_hi_sig", "avwap_lo_sig",
    "vwap_ext1_up", "vwap_ext1_dn", "vwap_ext2_up", "vwap_ext2_dn",
    "atr14_bps",
    "major_avwap_hi_sig", "major_avwap_lo_sig",
    "bull_open_sig", "bull_close_sig", "bear_open_sig", "bear_close_sig",
    "stoch14", "bars_since_vwap_cross", "bars_outside_sigma1", "bars_since_major_swing",
];
pub const BASE_FEATURE_COUNT: usize = 24;
pub const RSI14_IDX: usize = 2;
pub const VWAP_SIGMA_IDX: usize = 4;
pub const AVWAP_HI_SIG_IDX: usize = 7;
pub const AVWAP_LO_SIG_IDX: usize = 8;
pub const ATR14_BPS_IDX: usize = 13;
pub const MAJOR_AVWAP_HI_SIG_IDX: usize = 14;
pub const MAJOR_AVWAP_LO_SIG_IDX: usize = 15;
pub const BULL_OPEN_SIG_IDX: usize = 16;
pub const BEAR_CLOSE_SIG_IDX: usize = 19;

/// Rolling VWAP window in bars. 30 bars on H4 = 5 days Ã¢â€°Ë† 1 trading week.
const VWAP_WINDOW: usize = 30;

/// Stage-2 confluence features derived from the base LONG/SHORT detectors.
///
/// These are not raw market features; they summarize detector agreement.
/// This gives Alpha Synth a way to discover multi-feature confluence
/// without forcing the low-level DSL to decode packed bitfields.
pub const CONFLUENCE_FEATURE_NAMES: &[&str] = &[
    "long_votes",
    "short_votes",
    "net_votes",
    "vwap_long_votes",
    "vwap_short_votes",
    "vwap_net_votes",
    "sigma_long_votes",
    "sigma_short_votes",
    "anchored_long_votes",
    "anchored_short_votes",
    "conflict_votes",
    "dominance_votes",
];

pub const CONFLUENCE_FEATURE_COUNT: usize = 12;
const LONG_VOTES_IDX: usize = 0;
const SHORT_VOTES_IDX: usize = 1;
const DOMINANCE_VOTES_IDX: usize = 11;

const REGIME_MIN_ATR_BPS: i64 = 90;
const REGIME_MIN_SIGMA_ABS: i64 = 60;
const REGIME_RSI_LOW: i64 = 45;
const REGIME_RSI_HIGH: i64 = 55;

#[inline]
fn passes_base_regime_filter(feats: &[i64; BASE_FEATURE_COUNT]) -> bool {
    let atr_ok = feats[ATR14_BPS_IDX] >= REGIME_MIN_ATR_BPS;
    let displacement_ok =
        feats[VWAP_SIGMA_IDX].abs() >= REGIME_MIN_SIGMA_ABS
        || feats[AVWAP_HI_SIG_IDX].abs() >= REGIME_MIN_SIGMA_ABS
        || feats[AVWAP_LO_SIG_IDX].abs() >= REGIME_MIN_SIGMA_ABS
        || feats[MAJOR_AVWAP_HI_SIG_IDX].abs() >= REGIME_MIN_SIGMA_ABS
        || feats[MAJOR_AVWAP_LO_SIG_IDX].abs() >= REGIME_MIN_SIGMA_ABS
        || feats[BULL_OPEN_SIG_IDX].abs() >= REGIME_MIN_SIGMA_ABS
        || feats[BEAR_CLOSE_SIG_IDX].abs() >= REGIME_MIN_SIGMA_ABS;
    let directional_ok = feats[RSI14_IDX] <= REGIME_RSI_LOW || feats[RSI14_IDX] >= REGIME_RSI_HIGH;
    atr_ok && displacement_ok && directional_ok
}

#[inline]
fn passes_confluence_regime_filter(feats: &[i64; CONFLUENCE_FEATURE_COUNT]) -> bool {
    let long_votes = feats[LONG_VOTES_IDX];
    let short_votes = feats[SHORT_VOTES_IDX];
    let dominance = feats[DOMINANCE_VOTES_IDX];
    (long_votes > 0 || short_votes > 0) && dominance > 0
}

fn rebalance_binary_examples(examples: &[(i64, i64)]) -> Vec<(i64, i64)> {
    let positives = examples.iter().filter(|(_, label)| *label == 1).count();
    let negatives = examples.len().saturating_sub(positives);
    if positives == 0 || negatives == 0 {
        return examples.to_vec();
    }

    let positive_repeat = negatives.div_ceil(positives).clamp(1, 4);
    let mut balanced = Vec::with_capacity(examples.len() + positives * positive_repeat.saturating_sub(1));
    for &(value, label) in examples {
        balanced.push((value, label));
        if label == 1 {
            for _ in 1..positive_repeat {
                balanced.push((value, label));
            }
        }
    }
    balanced
}

/// Collapse the 10 per-feature LONG/SHORT detector outputs into a small
/// set of confluence signals. All inputs are expected to be binary
/// detector outputs {0,1}; any non-zero value is treated as "active".
pub fn derive_confluence_features(long_preds: &[i64], short_preds: &[i64]) -> [i64; CONFLUENCE_FEATURE_COUNT] {
    let is_on = |x: i64| if x != 0 { 1i64 } else { 0i64 };
    let count = |vals: &[i64], idxs: &[usize]| -> i64 {
        idxs.iter().map(|&i| vals.get(i).copied().unwrap_or(0)).map(is_on).sum()
    };

    // Raw feature groups:
    //   3-4   = general VWAP features
    //   5-8   = anchored VWAP features
    //   4,7,8 = sigma / extension-style features
    const VWAP_IDXS: &[usize] = &[3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 14, 15, 16, 17, 18, 19];
    const SIGMA_IDXS: &[usize] = &[4, 7, 8, 14, 15, 16, 17, 18, 19];
    const ANCHORED_IDXS: &[usize] = &[5, 6, 7, 8, 14, 15, 16, 17, 18, 19];

    let long_votes = long_preds.iter().copied().map(is_on).sum::<i64>();
    let short_votes = short_preds.iter().copied().map(is_on).sum::<i64>();
    let net_votes = long_votes - short_votes;

    let vwap_long_votes = count(long_preds, VWAP_IDXS);
    let vwap_short_votes = count(short_preds, VWAP_IDXS);
    let vwap_net_votes = vwap_long_votes - vwap_short_votes;

    let sigma_long_votes = count(long_preds, SIGMA_IDXS);
    let sigma_short_votes = count(short_preds, SIGMA_IDXS);

    let anchored_long_votes = count(long_preds, ANCHORED_IDXS);
    let anchored_short_votes = count(short_preds, ANCHORED_IDXS);

    let conflict_votes = long_votes.min(short_votes);
    let dominance_votes = net_votes.abs();

    [
        long_votes,
        short_votes,
        net_votes,
        vwap_long_votes,
        vwap_short_votes,
        vwap_net_votes,
        sigma_long_votes,
        sigma_short_votes,
        anchored_long_votes,
        anchored_short_votes,
        conflict_votes,
        dominance_votes,
    ]
}

/// Materialize the full raw VWAP-centric feature vector for each eligible bar
/// in `range`, persisting each scalar under a stable `(file_hash, feature_id,
/// bar_index)` atlas key so future sessions can rehydrate the full VWAP row
/// without rebuilding `FeatureCache`.
pub fn build_raw_feature_cache_with_atlas(
    bars: &[Bar],
    range: std::ops::Range<usize>,
    atlas: &scan::atlas::Atlas,
    file_hash: u64,
) -> (Vec<Option<[i64; BASE_FEATURE_COUNT]>>, AtlasCacheStats) {
    let start = range.start.max(MIN_HISTORY);
    let end = range.end.min(bars.len().saturating_sub(1));
    let mut rows: Vec<Option<[i64; BASE_FEATURE_COUNT]>> = vec![None; bars.len()];
    let mut stats = AtlasCacheStats::default();
    let mut cache: Option<FeatureCache> = None;

    for i in start..end {
        if !is_decision_hour(&bars[i]) {
            continue;
        }

        let mut cached_row = [0i64; BASE_FEATURE_COUNT];
        let mut row_complete = true;
        for fi in 0..BASE_FEATURE_COUNT {
            let key = scan::atlas::Atlas::feature_key(file_hash, fi as u8, i as u32);
            if let Some(value) = atlas.lookup_result(&key) {
                cached_row[fi] = scan::atlas::Atlas::unpack_i64(&value);
            } else {
                row_complete = false;
                break;
            }
        }

        if row_complete {
            rows[i] = Some(cached_row);
            stats.atlas_hits += 1;
            continue;
        }

        let feature_cache = cache.get_or_insert_with(|| FeatureCache::build(bars));
        let Some(computed_row) = extract_raw_feature_vector(bars, i, feature_cache) else {
            continue;
        };
        for fi in 0..BASE_FEATURE_COUNT {
            let key = scan::atlas::Atlas::feature_key(file_hash, fi as u8, i as u32);
            let packed = scan::atlas::Atlas::pack_i64(computed_row[fi]);
            if atlas.record_result(&key, &packed).unwrap_or(false) {
                stats.persisted_values += 1;
            }
        }
        rows[i] = Some(computed_row);
        stats.computed_rows += 1;
    }

    (rows, stats)
}

/// Atlas-backed prediction cache. Persists the per-bar detector output so a
/// later run can rehydrate the whole prediction vector without replaying the
/// detector, even if the underlying `(program,input)` results are already
/// memoized separately.
pub fn build_prediction_cache_with_atlas<F, P>(
    bars: &[Bar],
    range: std::ops::Range<usize>,
    atlas: &scan::atlas::Atlas,
    file_hash: u64,
    program_hash: &scan::Hash,
    mut feature_at: F,
    mut predict: P,
) -> (Vec<i8>, AtlasCacheStats, usize)
where
    F: FnMut(usize) -> Option<i64>,
    P: FnMut(i64) -> i64,
{
    let start = range.start.max(MIN_HISTORY);
    let end = range.end.min(bars.len());
    let mut rows = vec![0i8; bars.len()];
    let mut memo: std::collections::HashMap<i64, i8> = std::collections::HashMap::new();
    let mut stats = AtlasCacheStats::default();

    for i in start..end {
        if !is_decision_hour(&bars[i]) {
            continue;
        }
        let pred_key = scan::atlas::Atlas::prediction_key(file_hash, program_hash.as_bytes(), i as u32);
        if let Some(value) = atlas.lookup_result(&pred_key) {
            rows[i] = if scan::atlas::Atlas::unpack_i64(&value) != 0 { 1 } else { 0 };
            stats.atlas_hits += 1;
            continue;
        }

        let feat_val = match feature_at(i) {
            Some(v) => v,
            None => continue,
        };
        let pred = if let Some(&cached) = memo.get(&feat_val) {
            cached
        } else {
            let value = if predict(feat_val) != 0 { 1 } else { 0 };
            memo.insert(feat_val, value);
            value
        };
        let packed = scan::atlas::Atlas::pack_i64(pred as i64);
        if atlas.record_result(&pred_key, &packed).unwrap_or(false) {
            stats.persisted_values += 1;
        }
        rows[i] = pred;
        stats.computed_rows += 1;
    }

    (rows, stats, memo.len())
}

pub fn build_decision_cache_with_atlas(
    bars: &[Bar],
    range: std::ops::Range<usize>,
    atlas: &scan::atlas::Atlas,
    file_hash: u64,
    decision_fp: u64,
    long_predictions: &[i8],
    short_predictions: &[i8],
) -> (Vec<i8>, AtlasCacheStats) {
    let start = range.start.max(MIN_HISTORY);
    let end = range.end.min(bars.len());
    let mut rows = vec![0i8; bars.len()];
    let mut stats = AtlasCacheStats::default();

    for i in start..end {
        if !is_decision_hour(&bars[i]) {
            continue;
        }
        let key = scan::atlas::Atlas::decision_key(file_hash, decision_fp, i as u32);
        if let Some(value) = atlas.lookup_result(&key) {
            rows[i] = scan::atlas::Atlas::unpack_i64(&value) as i8;
            stats.atlas_hits += 1;
            continue;
        }
        let long_pred = long_predictions.get(i).copied().unwrap_or(0);
        let short_pred = short_predictions.get(i).copied().unwrap_or(0);
        let decision = if long_pred != 0 && short_pred == 0 {
            1
        } else if short_pred != 0 && long_pred == 0 {
            -1
        } else {
            0
        };
        rows[i] = decision;
        stats.computed_rows += 1;
        let packed = scan::atlas::Atlas::pack_i64(decision as i64);
        if atlas.record_result(&key, &packed).unwrap_or(false) {
            stats.persisted_values += 1;
        }
    }

    (rows, stats)
}

/// Build per-feature examples with BINARY labels for dual-classifier synthesis.
///
/// Returns Vec of (feature_name, long_examples, short_examples):
///   - long_examples:  (feature_val, 0 or 1) Ã¢â‚¬â€ 1 = LONG opportunity
///   - short_examples: (feature_val, 0 or 1) Ã¢â‚¬â€ 1 = SHORT opportunity
///
/// Why binary labels? The beam search ops (CmpGt, CmpLt) return {0, 1}.
/// With ternary labels {-1, 0, 1}, the beam can NEVER produce -1, so it
/// can never predict SHORT. Decomposing into two binary classifiers makes
/// both directions reachable: `CmpGt(vwap_sigma, 150)` = 1 Ã¢â€ â€™ LONG.
pub fn build_binary_feature_examples_with_caches(
    bars: &[Bar],
    range: std::ops::Range<usize>,
    raw_feature_cache: &[Option<[i64; BASE_FEATURE_COUNT]>],
    label_cache: &[Option<BinaryOpportunityLabels>],
) -> Vec<(&'static str, Vec<(i64, i64)>, Vec<(i64, i64)>)> {
    let start = range.start.max(MIN_HISTORY);
    let end = range.end.min(bars.len());

    // Collect raw feature vectors + binary labels.
    let mut rows: Vec<([i64; BASE_FEATURE_COUNT], i64, i64)> =
        Vec::with_capacity(end.saturating_sub(start));
    for i in start..end {
        let bar = bars[i];
        if bar.close <= 0.0 || !bar.close.is_finite() { continue; }
        let feats = match raw_feature_cache.get(i).copied().flatten() {
            Some(v) => v,
            None => continue,
        };
        if !passes_base_regime_filter(&feats) {
            continue;
        }
        let (long_label, short_label) = match label_cache.get(i).copied().flatten() {
            Some(v) => v,
            None => continue,
        };

        rows.push((feats, long_label, short_label));
    }

    let mut result = Vec::with_capacity(FEATURE_NAMES.len());
    for (fi, &name) in FEATURE_NAMES.iter().enumerate() {
        let long_ex: Vec<(i64, i64)> = rows.iter()
            .map(|(feats, ll, _)| (feats[fi], *ll))
            .collect();
        let short_ex: Vec<(i64, i64)> = rows.iter()
            .map(|(feats, _, sl)| (feats[fi], *sl))
            .collect();
        result.push((name, rebalance_binary_examples(&long_ex), rebalance_binary_examples(&short_ex)));
    }
    result
}

/// Build stage-2 confluence examples from already-trained base detectors.
///
/// The caller provides a closure that returns the confluence feature vector
/// for a bar. We keep the same binary LONG/SHORT labels so Alpha Synth can
/// learn thresholds like "at least 2 VWAP-aligned long votes and zero short
/// conflict votes".
pub fn build_confluence_feature_examples_from_labels<G>(
    bars: &[Bar],
    range: std::ops::Range<usize>,
    label_cache: &[Option<BinaryOpportunityLabels>],
    mut confluence_at_bar: G,
) -> Vec<(&'static str, Vec<(i64, i64)>, Vec<(i64, i64)>)>
where
    G: FnMut(usize) -> Option<[i64; CONFLUENCE_FEATURE_COUNT]>,
{
    let start = range.start.max(MIN_HISTORY);
    let end = range.end.min(bars.len());
    let mut rows: Vec<([i64; CONFLUENCE_FEATURE_COUNT], i64, i64)> =
        Vec::with_capacity(end.saturating_sub(start));

    for i in start..end {
        if !is_decision_hour(&bars[i]) { continue; }
        let bar = bars[i];
        if bar.close <= 0.0 || !bar.close.is_finite() { continue; }

        let feats = match confluence_at_bar(i) {
            Some(v) => v,
            None => continue,
        };
        if !passes_confluence_regime_filter(&feats) {
            continue;
        }
        let (long_label, short_label) = match label_cache.get(i).copied().flatten() {
            Some(v) => v,
            None => continue,
        };

        rows.push((feats, long_label, short_label));
    }

    let mut result = Vec::with_capacity(CONFLUENCE_FEATURE_NAMES.len());
    for (fi, &name) in CONFLUENCE_FEATURE_NAMES.iter().enumerate() {
        let long_ex: Vec<(i64, i64)> = rows.iter()
            .map(|(feats, ll, _)| (feats[fi], *ll))
            .collect();
        let short_ex: Vec<(i64, i64)> = rows.iter()
            .map(|(feats, _, sl)| (feats[fi], *sl))
            .collect();
        result.push((name, rebalance_binary_examples(&long_ex), rebalance_binary_examples(&short_ex)));
    }
    result
}

/// Extract one raw feature value at bar `i`. Same scaling as
/// `build_binary_feature_examples` Ã¢â‚¬â€ bit-identical results for a given
/// feature_idx (0..23, indexed by `FEATURE_NAMES`).
pub fn extract_raw_feature(
    bars: &[Bar], i: usize, feature_idx: usize, cache: &FeatureCache,
) -> Option<i64> {
    let bar = bars[i];
    if bar.close <= 0.0 || !bar.close.is_finite() { return None; }
    match feature_idx {
        0 => { // hour
            let secs = bar.time_ms / 1000;
            Some((secs / 3600).rem_euclid(24) as i64)
        }
        1 => { // dow
            let secs = bar.time_ms / 1000;
            Some(((secs.div_euclid(86400) + 4).rem_euclid(7)) as i64)
        }
        2 => cache.rsi(i, 14).map(|v| (v * 100.0).round() as i64), // rsi14
        3 => { // vwap_delta Ã¢â‚¬â€ (close - VWAP) / VWAP * 10000 bps
            cache.vwap_sigma(i, VWAP_WINDOW).map(|(vwap, _)| {
                ((bar.close - vwap) / vwap * 10_000.0).round() as i64
            })
        }
        4 => { // vwap_sigma Ã¢â‚¬â€ (close - VWAP) / ÃÆ’ * 100 (ext1=Ã‚Â±100, ext2=Ã‚Â±200)
            cache.vwap_sigma(i, VWAP_WINDOW).map(|(vwap, sig)| {
                if sig > 1e-12 { ((bar.close - vwap) / sig * 100.0).round() as i64 }
                else { 0 }
            })
        }
        5 => { // avwap_hi Ã¢â‚¬â€ (close - anchoredVWAP_swing_high) / close * 10000
            let sh = cache.last_swing_high_before(i)?;
            let (av, _) = cache.anchored_vwap_source(sh, i, VwapSource::High)?;
            Some(((bar.close - av) / bar.close * 10_000.0).round() as i64)
        }
        6 => { // avwap_lo Ã¢â‚¬â€ (close - anchoredVWAP_swing_low) / close * 10000
            let sl = cache.last_swing_low_before(i)?;
            let (av, _) = cache.anchored_vwap_source(sl, i, VwapSource::Low)?;
            Some(((bar.close - av) / bar.close * 10_000.0).round() as i64)
        }
        7 => { // avwap_hi_sig Ã¢â‚¬â€ (close - anchoredVWAP_high) / ÃÆ’ * 100
            let sh = cache.last_swing_high_before(i)?;
            let (av, sig) = cache.anchored_vwap_source(sh, i, VwapSource::High)?;
            Some(if sig > 1e-12 { ((bar.close - av) / sig * 100.0).round() as i64 } else { 0 })
        }
        8 => { // avwap_lo_sig Ã¢â‚¬â€ (close - anchoredVWAP_low) / ÃÆ’ * 100
            let sl = cache.last_swing_low_before(i)?;
            let (av, sig) = cache.anchored_vwap_source(sl, i, VwapSource::Low)?;
            Some(if sig > 1e-12 { ((bar.close - av) / sig * 100.0).round() as i64 } else { 0 })
        }
        9 => { // vwap_ext1_up — distance to VWAP + 1σ
            cache.vwap_sigma(i, VWAP_WINDOW).map(|(vwap, sig)| {
                let level = vwap + sig;
                ((bar.close - level) / bar.close * 10_000.0).round() as i64
            })
        }
        10 => { // vwap_ext1_dn — distance to VWAP - 1σ
            cache.vwap_sigma(i, VWAP_WINDOW).map(|(vwap, sig)| {
                let level = vwap - sig;
                ((bar.close - level) / bar.close * 10_000.0).round() as i64
            })
        }
        11 => { // vwap_ext2_up — distance to VWAP + 2σ
            cache.vwap_sigma(i, VWAP_WINDOW).map(|(vwap, sig)| {
                let level = vwap + 2.0 * sig;
                ((bar.close - level) / bar.close * 10_000.0).round() as i64
            })
        }
        12 => { // vwap_ext2_dn — distance to VWAP - 2σ
            cache.vwap_sigma(i, VWAP_WINDOW).map(|(vwap, sig)| {
                let level = vwap - 2.0 * sig;
                ((bar.close - level) / bar.close * 10_000.0).round() as i64
            })
        }
        13 => cache.atr(i, 14).map(|atr| { // atr14_bps
            ((atr / bar.close) * 10_000.0).round() as i64
        }),
        14 => { // major_avwap_hi_sig — major confirmed high anchor over wider window
            let sh = cache.major_swing_high_before(i, 72)?;
            let (av, sig) = cache.anchored_vwap_source(sh, i, VwapSource::High)?;
            Some(if sig > 1e-12 { ((bar.close - av) / sig * 100.0).round() as i64 } else { 0 })
        }
        15 => { // major_avwap_lo_sig — major confirmed low anchor over wider window
            let sl = cache.major_swing_low_before(i, 72)?;
            let (av, sig) = cache.anchored_vwap_source(sl, i, VwapSource::Low)?;
            Some(if sig > 1e-12 { ((bar.close - av) / sig * 100.0).round() as i64 } else { 0 })
        }
        16 => { // bull_open_sig — anchored on strongest bullish H4 open
            let anchor = cache.strongest_bullish_bar_before(i, 48)?;
            let (av, sig) = cache.anchored_vwap_source(anchor, i, VwapSource::Open)?;
            Some(if sig > 1e-12 { ((bar.close - av) / sig * 100.0).round() as i64 } else { 0 })
        }
        17 => { // bull_close_sig — anchored on strongest bullish H4 close
            let anchor = cache.strongest_bullish_bar_before(i, 48)?;
            let (av, sig) = cache.anchored_vwap_source(anchor, i, VwapSource::Close)?;
            Some(if sig > 1e-12 { ((bar.close - av) / sig * 100.0).round() as i64 } else { 0 })
        }
        18 => { // bear_open_sig — anchored on strongest bearish H4 open
            let anchor = cache.strongest_bearish_bar_before(i, 48)?;
            let (av, sig) = cache.anchored_vwap_source(anchor, i, VwapSource::Open)?;
            Some(if sig > 1e-12 { ((bar.close - av) / sig * 100.0).round() as i64 } else { 0 })
        }
        19 => { // bear_close_sig — anchored on strongest bearish H4 close
            let anchor = cache.strongest_bearish_bar_before(i, 48)?;
            let (av, sig) = cache.anchored_vwap_source(anchor, i, VwapSource::Close)?;
            Some(if sig > 1e-12 { ((bar.close - av) / sig * 100.0).round() as i64 } else { 0 })
        }
        20 => cache.stochastic_k(i, 14).map(|v| (v * 100.0).round() as i64), // stoch14
        21 => cache.bars_since_vwap_cross(i, VWAP_WINDOW, 48), // signed persistence above/below vwap
        22 => cache.bars_outside_sigma1(i, VWAP_WINDOW, 48),   // signed persistence beyond +/-1sigma
        23 => { // bars since last major swing anchor
            let hi_age = cache.major_swing_high_before(i, 72).map(|idx| i.saturating_sub(idx) as i64);
            let lo_age = cache.major_swing_low_before(i, 72).map(|idx| i.saturating_sub(idx) as i64);
            match (hi_age, lo_age) {
                (Some(h), Some(l)) => Some(h.min(l)),
                (Some(h), None) => Some(h),
                (None, Some(l)) => Some(l),
                (None, None) => None,
            }
        }
        _ => None,
    }
}

/// Extract the full raw feature vector for bar `i`, using the exact same
/// scaling and semantics as `extract_raw_feature`.
pub fn extract_raw_feature_vector(
    bars: &[Bar], i: usize, cache: &FeatureCache,
) -> Option<[i64; BASE_FEATURE_COUNT]> {
    let mut out = [0i64; BASE_FEATURE_COUNT];
    for (fi, slot) in out.iter_mut().enumerate() {
        *slot = extract_raw_feature(bars, i, fi, cache)?;
    }
    Some(out)
}


pub fn eval_strategy_decision_cache<S>(
    bars: &[Bar],
    range: std::ops::Range<usize>,
    cfg: SynthConfig,
    decisions: &[i8],
    mut signal: S,
) -> StrategyEval
where
    S: FnMut(usize, &'static str, f64),
{
    let mut eval = StrategyEval::default();
    let mut current_day_ms: i64 = -1;
    let mut current_day_pnl = 0.0;
    let mut current_day_had_trade = false;
    let start = range.start.max(MIN_HISTORY);
    let end = range.end.min(bars.len().saturating_sub(cfg.max_horizon_bars));

    for i in start..end {
        let bar = bars[i];
        let day_ms = bar.time_ms.div_euclid(86_400_000) * 86_400_000;
        if day_ms != current_day_ms {
            if current_day_ms >= 0 && current_day_had_trade {
                eval.days_evaluated += 1;
                if current_day_pnl > 0.0 { eval.days_profitable += 1; }
                if current_day_pnl >= cfg.target_pnl_per_day { eval.days_target_hit += 1; }
                eval.day_pnl_distribution.push(current_day_pnl);
            }
            current_day_ms = day_ms;
            current_day_pnl = 0.0;
            current_day_had_trade = false;
        }
        if !is_decision_hour(&bar) { continue; }

        let direction = match decisions.get(i).copied().unwrap_or(0) {
            1 => Direction::Long,
            -1 => Direction::Short,
            _ => continue,
        };

        let trade = simulate_trade(
            bars, i, direction, cfg.sl_points, cfg.tp_points, cfg.spread_points, cfg.max_horizon_bars,
        );
        if trade.exit_reason == ExitReason::NotPossible { continue; }
        let dir_str: &'static str = match direction {
            Direction::Long => "LONG",
            Direction::Short => "SHORT",
        };
        signal(i, dir_str, bar.close);
        eval.total_trades += 1;
        match direction {
            Direction::Long => eval.long_trades += 1,
            Direction::Short => eval.short_trades += 1,
        }
        if trade.pnl_points > 0.0 { eval.winning_trades += 1; } else { eval.losing_trades += 1; }
        eval.total_pnl_points += trade.pnl_points;
        current_day_pnl += trade.pnl_points;
        current_day_had_trade = true;
    }
    if current_day_had_trade {
        eval.days_evaluated += 1;
        if current_day_pnl > 0.0 { eval.days_profitable += 1; }
        if current_day_pnl >= cfg.target_pnl_per_day { eval.days_target_hit += 1; }
        eval.day_pnl_distribution.push(current_day_pnl);
    }
    eval
}


// Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬ Per-day evaluation Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬

/// RÃƒÂ©sultat de l'ÃƒÂ©valuation d'une stratÃƒÂ©gie sur un dataset.
#[derive(Debug, Clone, Default)]
pub struct StrategyEval {
    pub total_trades: usize,
    pub long_trades: usize,
    pub short_trades: usize,
    pub winning_trades: usize,
    pub losing_trades: usize,
    pub total_pnl_points: f64,
    pub days_evaluated: usize,
    pub days_profitable: usize,
    pub days_target_hit: usize,
    pub day_pnl_distribution: Vec<f64>,
}

impl StrategyEval {
    pub fn pct_days_target_hit(&self) -> f64 {
        if self.days_evaluated == 0 { return 0.0; }
        100.0 * self.days_target_hit as f64 / self.days_evaluated as f64
    }
    pub fn pct_winning_trades(&self) -> f64 {
        if self.total_trades == 0 { return 0.0; }
        100.0 * self.winning_trades as f64 / self.total_trades as f64
    }
    pub fn avg_pnl_per_trade(&self) -> f64 {
        if self.total_trades == 0 { return 0.0; }
        self.total_pnl_points / self.total_trades as f64
    }

    // Ã¢â€â‚¬Ã¢â€â‚¬ ÃŽÂ¦.ÃŽÂ½.7g Ã¢â‚¬â€ MÃƒÂ©triques pro pour vendabilitÃƒÂ© hedge fund Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬
    //
    // Toutes calculÃƒÂ©es depuis `day_pnl_distribution` (P&L cumulÃƒÂ© par
    // jour de trading). `periods_per_year` = 252 pour des P&L
    // journaliers (jours ouvrÃƒÂ©s US standard).

    /// Sharpe Ratio annualisÃƒÂ©. Mesure le rendement par unitÃƒÂ© de
    /// volatilitÃƒÂ© (gains ET pertes confondus).
    /// Ã¢â€°Â¥ 1.5 = bon, Ã¢â€°Â¥ 2 = trÃƒÂ¨s bon, > 3 = suspect (overfit ?).
    pub fn sharpe_ratio(&self, periods_per_year: f64) -> f64 {
        let n = self.day_pnl_distribution.len();
        if n < 2 { return 0.0; }
        let mean = self.day_pnl_distribution.iter().sum::<f64>() / n as f64;
        let var = self.day_pnl_distribution.iter()
            .map(|x| (x - mean).powi(2))
            .sum::<f64>() / (n - 1) as f64;
        let std = var.sqrt();
        if std <= f64::EPSILON { return 0.0; }
        (mean / std) * periods_per_year.sqrt()
    }

    /// Sortino Ratio annualisÃƒÂ©. Comme Sharpe mais ne pÃƒÂ©nalise QUE la
    /// volatilitÃƒÂ© ÃƒÂ  la baisse (jours nÃƒÂ©gatifs). Meilleur indicateur
    /// pour stratÃƒÂ©gies asymÃƒÂ©triques (mean reversion, options).
    /// Ã¢â€°Â¥ 2 = excellent.
    pub fn sortino_ratio(&self, periods_per_year: f64) -> f64 {
        let n = self.day_pnl_distribution.len();
        if n < 2 { return 0.0; }
        let mean = self.day_pnl_distribution.iter().sum::<f64>() / n as f64;
        let downside_var = self.day_pnl_distribution.iter()
            .filter(|x| **x < 0.0)
            .map(|x| x.powi(2))
            .sum::<f64>() / n as f64;
        let downside_std = downside_var.sqrt();
        if downside_std <= f64::EPSILON {
            // Aucun jour nÃƒÂ©gatif : Sortino infini par dÃƒÂ©finition.
            // On retourne f64::INFINITY pour signaler clairement.
            return if mean > 0.0 { f64::INFINITY } else { 0.0 };
        }
        (mean / downside_std) * periods_per_year.sqrt()
    }

    /// Max Drawdown : la plus grosse chute du capital cumulÃƒÂ© depuis un
    /// pic local. Mesure ABSOLUE (en points), pas en %. Pour le %,
    /// diviser par le capital initial fictif (non trackÃƒÂ© ici).
    pub fn max_drawdown_points(&self) -> f64 {
        let mut peak = 0.0_f64;
        let mut cumul = 0.0_f64;
        let mut max_dd = 0.0_f64;
        for &pnl in &self.day_pnl_distribution {
            cumul += pnl;
            if cumul > peak { peak = cumul; }
            let dd = peak - cumul;
            if dd > max_dd { max_dd = dd; }
        }
        max_dd
    }

    /// Calmar Ratio annualisÃƒÂ© : rendement annuel / max drawdown.
    /// Mesure la rÃƒÂ©silience aux pertes prolongÃƒÂ©es. Ã¢â€°Â¥ 1 = tu rÃƒÂ©cupÃƒÂ¨res
    /// tes pertes en moins d'un an. Ã¢â€°Â¥ 3 = excellent.
    pub fn calmar_ratio(&self, periods_per_year: f64) -> f64 {
        let n = self.day_pnl_distribution.len();
        if n == 0 { return 0.0; }
        let total_return = self.total_pnl_points;
        let mdd = self.max_drawdown_points();
        if mdd <= f64::EPSILON {
            return if total_return > 0.0 { f64::INFINITY } else { 0.0 };
        }
        let annualized = total_return * periods_per_year / n as f64;
        annualized / mdd
    }

    /// Profit Factor : somme des gains / somme des pertes (en valeur
    /// absolue). > 1 = profitable. Ã¢â€°Â¥ 2 = trÃƒÂ¨s solide.
    pub fn profit_factor(&self) -> f64 {
        let mut gains = 0.0_f64;
        let mut losses = 0.0_f64;
        for &pnl in &self.day_pnl_distribution {
            if pnl > 0.0 { gains += pnl; } else { losses -= pnl; }
        }
        if losses <= f64::EPSILON {
            return if gains > 0.0 { f64::INFINITY } else { 0.0 };
        }
        gains / losses
    }

    /// Max consecutive losing days. Crucial pour la psychologie du
    /// trader humain et la confiance du capital allocator.
    pub fn max_consecutive_losing_days(&self) -> usize {
        let mut max_streak = 0;
        let mut current = 0;
        for &pnl in &self.day_pnl_distribution {
            if pnl < 0.0 {
                current += 1;
                if current > max_streak { max_streak = current; }
            } else {
                current = 0;
            }
        }
        max_streak
    }
}

/// Ãƒâ€°value une stratÃƒÂ©gie sur un range de bougies. Ãƒâ‚¬ chaque bougie de
/// dÃƒÂ©cision, appelle `predict_label(features)` qui doit retourner +1
/// (long), -1 (short), ou 0 (no trade). Simule le trade, agrÃƒÂ¨ge les
/// P&L par jour calendaire (UTC), et compte les jours qui atteignent
/// `cfg.target_pnl_per_day`.
///
/// Wrapper autour de `eval_strategy_full` avec `FeatureMask::all()` et
/// un progress callback pÃƒÂ©riodique `progress(bars_done, bars_total)`
/// appelÃƒÂ© tous les ~512 bars (utile pour logs live pendant que l'eval
/// tourne, sinon UI muet pendant 5-30s).
pub fn eval_strategy_with_progress<F, P>(
    bars: &[Bar],
    range: std::ops::Range<usize>,
    cfg: SynthConfig,
    predict_label: F,
    progress: P,
) -> StrategyEval
where
    F: FnMut(i64) -> i64,
    P: FnMut(usize, usize),
{
    eval_strategy_full(bars, range, cfg, &FeatureMask::all(), predict_label, progress, |_, _, _| {})
}

/// Backbone d'ÃƒÂ©valuation : factorise `eval_strategy*` pour accepter
/// callback de progress (par-512-bars) ET callback de signal (par-trade).
/// Pas exposÃƒÂ©e publiquement Ã¢â‚¬â€ wrappers ci-dessus pour ergonomie.
///
/// ÃŽÂ¦.ÃŽÂ½.7g Ã¢â‚¬â€ Le `mask` doit IDÃƒâ€°ALEMENT ÃƒÂªtre identique ÃƒÂ  celui utilisÃƒÂ©
/// pour `build_examples_in_range_masked` au moment du training : sinon
/// le programme synthÃƒÂ©tisÃƒÂ© voit des features avec des bits diffÃƒÂ©rents
/// et ses prÃƒÂ©dictions sont garbage. Wrappers `eval_strategy*`
/// (sans `_masked`) utilisent `FeatureMask::all()` par cohÃƒÂ©rence.
fn eval_strategy_full<F, P, S>(
    bars: &[Bar],
    range: std::ops::Range<usize>,
    cfg: SynthConfig,
    mask: &FeatureMask,
    mut predict_label: F,
    mut progress: P,
    mut signal: S,
) -> StrategyEval
where
    F: FnMut(i64) -> i64,
    P: FnMut(usize, usize),
    S: FnMut(usize, &'static str, f64),
{
    const PROGRESS_INTERVAL: usize = 512;
    let mut eval = StrategyEval::default();
    let mut current_day_ms: i64 = -1;
    let mut current_day_pnl = 0.0;
    let mut current_day_had_trade = false;

    let start = range.start.max(MIN_HISTORY);
    let end = range.end.min(bars.len().saturating_sub(cfg.max_horizon_bars));
    let total = end.saturating_sub(start);
    let cache = FeatureCache::build(bars);

    for i in start..end {
        let bar = bars[i];
        let done = i.saturating_sub(start);
        if done > 0 && done % PROGRESS_INTERVAL == 0 {
            progress(done, total);
        }
        let day_ms = bar.time_ms.div_euclid(86_400_000) * 86_400_000;
        // FrontiÃƒÂ¨re de jour : on flush le jour prÃƒÂ©cÃƒÂ©dent
        if day_ms != current_day_ms {
            if current_day_ms >= 0 && current_day_had_trade {
                eval.days_evaluated += 1;
                if current_day_pnl > 0.0 { eval.days_profitable += 1; }
                if current_day_pnl >= cfg.target_pnl_per_day { eval.days_target_hit += 1; }
                eval.day_pnl_distribution.push(current_day_pnl);
            }
            current_day_ms = day_ms;
            current_day_pnl = 0.0;
            current_day_had_trade = false;
        }

        if !is_decision_hour(&bar) { continue; }
        let features = match extract_features_with_cache(bars, i, mask, &cache) {
            Some(f) => f,
            None => continue,
        };
        let label = predict_label(features);
        let direction = match label.signum() {
            1 => Direction::Long,
            -1 => Direction::Short,
            _ => continue, // FLAT
        };
        let trade = simulate_trade(bars, i, direction, cfg.sl_points, cfg.tp_points, cfg.spread_points, cfg.max_horizon_bars);
        if trade.exit_reason == ExitReason::NotPossible { continue; }
        // ÃŽÂ¦.ÃŽÂ½.7g Ã¢â‚¬â€ ÃƒÂ©met le signal AVANT incrÃƒÂ©menter les compteurs pour
        // que le frontend puisse afficher le marker mÃƒÂªme si le trade
        // est ensuite exclu (ex: NotPossible). Direction = string
        // littÃƒÂ©rale parsable cÃƒÂ´tÃƒÂ© JS pour distinguer Ã¢â€“Â² vs Ã¢â€“Â¼.
        let dir_str: &'static str = match direction {
            Direction::Long => "LONG",
            Direction::Short => "SHORT",
        };
        signal(i, dir_str, bar.close);
        eval.total_trades += 1;
        match direction {
            Direction::Long => eval.long_trades += 1,
            Direction::Short => eval.short_trades += 1,
        }
        if trade.pnl_points > 0.0 { eval.winning_trades += 1; } else { eval.losing_trades += 1; }
        eval.total_pnl_points += trade.pnl_points;
        current_day_pnl += trade.pnl_points;
        current_day_had_trade = true;
    }
    // Flush du dernier jour
    if current_day_had_trade {
        eval.days_evaluated += 1;
        if current_day_pnl > 0.0 { eval.days_profitable += 1; }
        if current_day_pnl >= cfg.target_pnl_per_day { eval.days_target_hit += 1; }
        eval.day_pnl_distribution.push(current_day_pnl);
    }
    eval
}

/// Sort la borne idx du split train/holdout selon `cfg.train_split`.
pub fn train_holdout_split(n_bars: usize, cfg: SynthConfig) -> usize {
    ((n_bars as f64) * cfg.train_split).round() as usize
}

// Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬ Tests Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_csv() -> &'static [u8] {
        b"time,open,high,low,close,volume
2010-01-04T22:00:00.000000000Z,5.756,5.813,5.755,5.759,188
2010-01-05T02:00:00.000000000Z,5.759,5.823,5.722,5.812,455
2010-01-05T06:00:00.000000000Z,5.812,5.834,5.802,5.825,612
2010-01-05T10:00:00.000000000Z,5.825,5.829,5.717,5.717,892
2010-01-05T14:00:00.000000000Z,5.717,5.756,5.704,5.748,710
"
    }

    #[test]
    fn simulate_trade_with_atlas_persists_outcome() {
        use scan::atlas::{kind, Atlas};
        let bars: Vec<Bar> = (0..50)
            .map(|i| Bar {
                time_ms: 1_262_642_400_000 + (i as i64) * 14_400_000,
                open: 5.0 + (i as f64) * 0.01,
                high: 5.05 + (i as f64) * 0.01,
                low: 4.95 + (i as f64) * 0.01,
                close: 5.0 + (i as f64) * 0.01,
                volume: 100.0,
            })
            .collect();

        let mut path = std::env::temp_dir();
        path.push(format!("forge-trade-atlas-{}", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let atlas = Atlas::open(&path).unwrap();

        let r1 = simulate_trade_with_atlas(
            &bars,
            10,
            Direction::Long,
            0.5,
            1.0,
            0.008,
            5,
            &atlas,
            0xDEADBEEFu64,
        );
        let r2 = simulate_trade_with_atlas(
            &bars,
            10,
            Direction::Long,
            0.5,
            1.0,
            0.008,
            5,
            &atlas,
            0xDEADBEEFu64,
        );
        assert_eq!(r1.pnl_points, r2.pnl_points);
        assert_eq!(r1.bars_held, r2.bars_held);
        assert_eq!(r1.exit_reason, r2.exit_reason);
        assert_eq!(atlas.count_kind(kind::RESULT), 1);

        let _ = simulate_trade_with_atlas(
            &bars,
            20,
            Direction::Long,
            0.5,
            1.0,
            0.008,
            5,
            &atlas,
            0xDEADBEEFu64,
        );
        assert_eq!(atlas.count_kind(kind::RESULT), 2);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn tp_grid_points_emits_multiple_distinct_targets() {
        let cfg = SynthConfig {
            sl_points: 0.07,
            tp_points: 0.07,
            spread_points: 0.008,
            max_horizon_bars: 6,
            target_pnl_per_day: 0.07,
            train_split: 0.7,
        };
        let grid = tp_grid_points(cfg);
        assert!(grid.len() >= 5);
        assert!(grid.windows(2).all(|w| w[0] < w[1]));
        assert!(grid.contains(&0.07));
    }

    #[test]
    fn parse_csv_basic() {
        let bars = parse_csv(sample_csv()).unwrap();
        assert_eq!(bars.len(), 5);
        assert_eq!(bars[0].open, 5.756);
        assert_eq!(bars[0].close, 5.759);
        assert_eq!(bars[0].volume, 188.0);
    }

    /// Generate synthetic NATGAS-like bars for bench/equivalence tests.
    #[allow(dead_code)]
    fn synth_bars(n: usize) -> Vec<Bar> {
        let mut bars = Vec::with_capacity(n);
        let mut seed: u64 = 0x4F52_4147_4500_0001;
        let mut close = 5.0_f64;
        for j in 0..n {
            // splitmix64 step for deterministic noise
            seed = seed.wrapping_add(0x9e37_79b9_7f4a_7c15);
            let mut z = seed;
            z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
            z ^= z >> 31;
            let drift = ((z as i64 as f64) / (i64::MAX as f64)) * 0.05;
            close = (close + drift).max(0.5);
            let high = close + 0.02;
            let low = close - 0.02;
            let open = close - drift * 0.5;
            let volume = 100.0 + (j as f64).rem_euclid(50.0);
            bars.push(Bar {
                time_ms: 1_262_574_000_000 + (j as i64) * 14_400_000,
                open,
                high,
                low,
                close,
                volume,
            });
        }
        bars
    }

    #[test]
    fn parse_csv_strips_bom() {
        let mut bytes = vec![0xEF, 0xBB, 0xBF];
        bytes.extend_from_slice(sample_csv());
        let bars = parse_csv(&bytes).unwrap();
        assert_eq!(bars.len(), 5);
    }

    #[test]
    fn parse_csv_rejects_bad_header() {
        let bad = b"foo,bar,baz\n1,2,3\n";
        assert!(matches!(parse_csv(bad), Err(ParseError::BadHeader(_))));
    }

    #[test]
    fn parse_iso_time() {
        // 2010-01-04 22:00:00 UTC = 1262642400 s = 1262642400000 ms
        let ms = parse_time("2010-01-04T22:00:00.000000000Z").unwrap();
        assert_eq!(ms, 1262642400000);
    }

    #[test]
    fn parse_epoch_ms() {
        let ms = parse_time("1262642400000").unwrap();
        assert_eq!(ms, 1262642400000);
    }

    #[test]
    fn decision_hour_excludes_04h() {
        // 00:00 UTC bar Ã¢â€ â€™ close 04:00 Ã¢â€ â€™ excluded (< 05:00)
        let bar = Bar {
            time_ms: 1262563200000, // 2010-01-04 00:00 UTC
            open: 0.0, high: 0.0, low: 0.0, close: 0.0, volume: 0.0,
        };
        assert!(!is_decision_hour(&bar));
        // 20:00 UTC bar Ã¢â€ â€™ close 00:00 Ã¢â€ â€™ excluded (> 22:59)
        let bar = Bar {
            time_ms: 1262635200000, // 2010-01-04 20:00 UTC
            open: 0.0, high: 0.0, low: 0.0, close: 0.0, volume: 0.0,
        };
        assert!(!is_decision_hour(&bar));
        // 03:00 UTC bar -> close 07:00 -> included
        let bar = Bar {
            time_ms: 1262574000000, // 2010-01-04 03:00 UTC
            open: 0.0, high: 0.0, low: 0.0, close: 0.0, volume: 0.0,
        };
        assert!(is_decision_hour(&bar));
        // 07:00 UTC bar -> close 11:00 -> included
        let bar = Bar {
            time_ms: 1262588400000, // 2010-01-04 07:00 UTC
            open: 0.0, high: 0.0, low: 0.0, close: 0.0, volume: 0.0,
        };
        assert!(is_decision_hour(&bar));
        // 04:00 UTC bar -> close 08:00 -> included for canonical H4 feeds.
        let bar = Bar {
            time_ms: 1262577600000, // 2010-01-04 04:00 UTC
            open: 0.0, high: 0.0, low: 0.0, close: 0.0, volume: 0.0,
        };
        assert!(is_decision_hour(&bar));
        // 02:00 UTC bar -> close 06:00 -> included for DST-shifted H4 feeds.
        let bar = Bar {
            time_ms: 1262570400000, // 2010-01-04 02:00 UTC
            open: 0.0, high: 0.0, low: 0.0, close: 0.0, volume: 0.0,
        };
        assert!(is_decision_hour(&bar));
    }

    #[test]
    fn simulate_trade_long_horizon_win() {
        // Synthetic : entrÃƒÂ©e ÃƒÂ  5.0, prix monte jusqu'ÃƒÂ  5.5 sans toucher
        // SL ÃƒÂ  4.91. RÃƒÂ©sultat attendu : +0.5 points, exit horizon.
        let bars = vec![
            Bar { time_ms: 0, open: 5.0, high: 5.0, low: 5.0, close: 5.0, volume: 0.0 },
            Bar { time_ms: 1, open: 5.0, high: 5.2, low: 4.95, close: 5.1, volume: 0.0 },
            Bar { time_ms: 2, open: 5.1, high: 5.3, low: 5.05, close: 5.25, volume: 0.0 },
            Bar { time_ms: 3, open: 5.25, high: 5.55, low: 5.2, close: 5.5, volume: 0.0 },
        ];
        let r = simulate_trade(&bars, 0, Direction::Long, 0.09, 1.0, 0.008, 3);
        assert_eq!(r.exit_reason, ExitReason::Horizon);
        assert!((r.pnl_points - 0.492).abs() < 1e-9);
        assert_eq!(r.bars_held, 3);
    }

    #[test]
    fn simulate_trade_long_sl_hit() {
        let bars = vec![
            Bar { time_ms: 0, open: 5.0, high: 5.0, low: 5.0, close: 5.0, volume: 0.0 },
            Bar { time_ms: 1, open: 5.0, high: 5.0, low: 4.85, close: 4.9, volume: 0.0 },
            Bar { time_ms: 2, open: 4.9, high: 4.95, low: 4.85, close: 4.92, volume: 0.0 },
        ];
        let r = simulate_trade(&bars, 0, Direction::Long, 0.09, 1.0, 0.008, 3);
        assert_eq!(r.exit_reason, ExitReason::StopLoss);
        assert!((r.pnl_points - (-0.098)).abs() < 1e-9);
        assert_eq!(r.bars_held, 1);
    }

    #[test]
    fn simulate_trade_long_tp_hit() {
        let bars = vec![
            Bar { time_ms: 0, open: 5.0, high: 5.0, low: 5.0, close: 5.0, volume: 0.0 },
            Bar { time_ms: 1, open: 5.0, high: 5.08, low: 4.95, close: 5.07, volume: 0.0 },
        ];
        let r = simulate_trade(&bars, 0, Direction::Long, 0.09, 0.07, 0.008, 3);
        assert_eq!(r.exit_reason, ExitReason::TakeProfit);
        assert!((r.pnl_points - 0.062).abs() < 1e-9);
        assert_eq!(r.bars_held, 1);
    }

    #[test]
    fn simulate_trade_short_sl_hit() {
        let bars = vec![
            Bar { time_ms: 0, open: 5.0, high: 5.0, low: 5.0, close: 5.0, volume: 0.0 },
            Bar { time_ms: 1, open: 5.0, high: 5.15, low: 5.0, close: 5.12, volume: 0.0 },
        ];
        let r = simulate_trade(&bars, 0, Direction::Short, 0.09, 1.0, 0.008, 3);
        assert_eq!(r.exit_reason, ExitReason::StopLoss);
        assert!((r.pnl_points - (-0.098)).abs() < 1e-9);
    }

    #[test]
    fn extract_features_returns_none_when_history_short() {
        let bars: Vec<Bar> = (0..10).map(|i| Bar {
            time_ms: i * 14_400_000,
            open: 5.0, high: 5.1, low: 4.9, close: 5.0, volume: 100.0,
        }).collect();
        let cache = FeatureCache::build(&bars);
        assert!(extract_features_with_cache(&bars, 5, &FeatureMask::all(), &cache).is_none());
    }

    #[test]
    fn extract_features_returns_none_when_no_ma200() {
        // 199 bougies < MIN_HISTORY = 200 Ã¢â€ â€™ MA200 impossible
        let bars: Vec<Bar> = (0..199).map(|i| Bar {
            time_ms: i as i64 * 14_400_000,
            open: 5.0, high: 5.05, low: 4.95, close: 5.0, volume: 100.0,
        }).collect();
        let cache = FeatureCache::build(&bars);
        assert!(extract_features_with_cache(&bars, 198, &FeatureMask::all(), &cache).is_none());
    }

    #[test]
    fn extract_features_packs_within_43_bits() {
        let bars: Vec<Bar> = (0..250).map(|i| Bar {
            time_ms: i as i64 * 14_400_000,
            open: 5.0, high: 5.05, low: 4.95, close: 5.0, volume: 100.0,
        }).collect();
        let cache = FeatureCache::build(&bars);
        let f = extract_features_with_cache(&bars, 240, &FeatureMask::all(), &cache).unwrap();
        assert!(f >= 0);
        assert!(f < (1i64 << 43));
    }

    #[test]
    fn vwap_falls_back_to_sma_when_no_volume() {
        let bars: Vec<Bar> = (0..10).map(|_| Bar {
            time_ms: 0, open: 5.0, high: 5.1, low: 4.9, close: 5.0, volume: 0.0,
        }).collect();
        let cache = FeatureCache::build(&bars);
        let v = cache.vwap(9, 6).unwrap();
        // (h+l+c)/3 = (5.1+4.9+5.0)/3 = 5.0
        assert!((v - 5.0).abs() < 1e-9);
    }

    #[test]
    fn vwap_sigma_matches_weighted_mean_and_stddev() {
        let bars = vec![
            Bar { time_ms: 0, open: 10.0, high: 10.0, low: 10.0, close: 10.0, volume: 1.0 },
            Bar { time_ms: 1, open: 12.0, high: 12.0, low: 12.0, close: 12.0, volume: 2.0 },
            Bar { time_ms: 2, open: 14.0, high: 14.0, low: 14.0, close: 14.0, volume: 1.0 },
        ];
        let cache = FeatureCache::build(&bars);
        let (vwap, sigma) = cache.vwap_sigma(2, 3).unwrap();
        assert!((vwap - 12.0).abs() < 1e-9, "expected VWAP=12, got {}", vwap);
        assert!((sigma - 2.0_f64.sqrt()).abs() < 1e-9, "expected sigma=sqrt(2), got {}", sigma);
    }

    #[test]
    fn swing_points_are_confirmed_before_anchor_lookup() {
        let bars: Vec<Bar> = (0..50).map(|i| {
            let spike_hi = if i == 20 { 5.0 } else { 0.0 };
            let spike_lo = if i == 30 { 5.0 } else { 0.0 };
            let base = 100.0 - (i as f64) * 0.001;
            Bar {
                time_ms: i as i64 * 14_400_000,
                open: base,
                high: base + 1.0 + spike_hi,
                low: base - 1.0 - spike_lo,
                close: base,
                volume: 100.0,
            }
        }).collect();
        let cache = FeatureCache::build(&bars);
        assert_eq!(cache.last_swing_high_before(31), None, "swing high should not be confirmed yet");
        assert_eq!(cache.last_swing_high_before(32), Some(20));
        assert_eq!(cache.last_swing_low_before(41), None, "swing low should not be confirmed yet");
        assert_eq!(cache.last_swing_low_before(42), Some(30));
    }

    #[test]
    fn confluence_features_capture_votes_and_conflicts() {
        let long_preds = [1, 0, 0, 1, 1, 0, 1, 1, 0, 0];
        let short_preds = [0, 1, 0, 0, 1, 1, 0, 0, 1, 0];
        let feats = derive_confluence_features(&long_preds, &short_preds);
        assert_eq!(feats[0], 5); // long_votes
        assert_eq!(feats[1], 4); // short_votes
        assert_eq!(feats[2], 1); // net_votes
        assert_eq!(feats[3], 4); // vwap_long_votes over 3..8
        assert_eq!(feats[4], 3); // vwap_short_votes over 3..8
        assert_eq!(feats[5], 1); // vwap_net_votes
        assert_eq!(feats[10], 4); // conflict_votes = min(5,4)
        assert_eq!(feats[11], 1); // dominance_votes = abs(net)
    }

    #[test]
    fn adx_zero_on_flat_market() {
        let bars: Vec<Bar> = (0..30).map(|_| Bar {
            time_ms: 0, open: 5.0, high: 5.0, low: 5.0, close: 5.0, volume: 0.0,
        }).collect();
        let cache = FeatureCache::build(&bars);
        let adx = cache.adx(20, 14).unwrap();
        assert_eq!(adx, 0.0);
    }

    #[test]
    fn adx_high_on_pure_uptrend() {
        let bars: Vec<Bar> = (0..30).map(|i| Bar {
            time_ms: 0,
            open: 5.0 + i as f64 * 0.01,
            high: 5.05 + i as f64 * 0.01,
            low: 4.95 + i as f64 * 0.01,
            close: 5.0 + i as f64 * 0.01,
            volume: 0.0,
        }).collect();
        let cache = FeatureCache::build(&bars);
        let adx = cache.adx(20, 14).unwrap();
        // Trend pur up : DI+ Ã¢â€°Â« DI-, donc ADX devrait ÃƒÂªtre > 50
        assert!(adx > 50.0, "ADX uptrend = {} (expected > 50)", adx);
    }

    // Ã¢â€â‚¬Ã¢â€â‚¬ ÃŽÂ¦.ÃŽÂ½.7g Ã¢â‚¬â€ Tests pour mÃƒÂ©triques pro (Sharpe/Sortino/Calmar) Ã¢â€â‚¬Ã¢â€â‚¬

    fn synth_eval_with_pnl(daily: Vec<f64>) -> StrategyEval {
        StrategyEval {
            total_trades: daily.len(),
            long_trades: 0,
            short_trades: 0,
            winning_trades: daily.iter().filter(|x| **x > 0.0).count(),
            losing_trades: daily.iter().filter(|x| **x < 0.0).count(),
            total_pnl_points: daily.iter().sum(),
            days_evaluated: daily.len(),
            days_profitable: daily.iter().filter(|x| **x > 0.0).count(),
            days_target_hit: 0,
            day_pnl_distribution: daily,
        }
    }

    #[test]
    fn sharpe_zero_on_flat_returns() {
        let e = synth_eval_with_pnl(vec![0.0; 100]);
        assert_eq!(e.sharpe_ratio(252.0), 0.0);
    }

    #[test]
    fn sharpe_positive_on_steady_winner() {
        // 100 jours ÃƒÂ  +1pt Ã¢â€ â€™ mean=1, std=0 Ã¢â€ â€™ Sharpe 0 (pas de variance)
        // Mais avec un peu de variance : alternance +1.1 / +0.9
        let pnl: Vec<f64> = (0..100).map(|i| if i % 2 == 0 { 1.1 } else { 0.9 }).collect();
        let s = synth_eval_with_pnl(pnl).sharpe_ratio(252.0);
        // mean = 1.0, std ~0.1, Sharpe annualisÃƒÂ© = 10 Ãƒâ€” Ã¢Ë†Å¡252 Ã¢â€°Ë† 158
        assert!(s > 100.0, "Sharpe stable winner = {} (expected > 100)", s);
    }

    #[test]
    fn sortino_better_than_sharpe_on_asymmetric_winner() {
        // StratÃƒÂ©gie asymÃƒÂ©trique : grosses victoires +5, petites pertes -1
        let pnl: Vec<f64> = vec![5.0, -1.0, 5.0, -1.0, 5.0, -1.0, 5.0, -1.0, 5.0, -1.0];
        let e = synth_eval_with_pnl(pnl);
        let sharpe = e.sharpe_ratio(252.0);
        let sortino = e.sortino_ratio(252.0);
        // Sortino doit ÃƒÂªtre > Sharpe car la variance des gains ne compte
        // pas, seule la variance des pertes (-1) compte.
        assert!(sortino > sharpe, "Sortino={} should beat Sharpe={} on asymmetric", sortino, sharpe);
    }

    #[test]
    fn max_drawdown_correct_on_v_shape() {
        // PnL : +5, +3, -10, -2, +5, +5 Ã¢â€ â€™ cumul 5, 8, -2, -4, 1, 6
        // Peak ÃƒÂ  8 (jour 2), creux ÃƒÂ  -4 (jour 4) Ã¢â€ â€™ DD = 8 - (-4) = 12
        let e = synth_eval_with_pnl(vec![5.0, 3.0, -10.0, -2.0, 5.0, 5.0]);
        let dd = e.max_drawdown_points();
        assert!((dd - 12.0).abs() < 1e-9, "expected MDD=12, got {}", dd);
    }

    #[test]
    fn calmar_infinity_when_no_drawdown() {
        // PnL strictement positif Ã¢â€ â€™ MDD=0 Ã¢â€ â€™ Calmar = +Ã¢Ë†Å¾
        let e = synth_eval_with_pnl(vec![1.0; 100]);
        let c = e.calmar_ratio(252.0);
        assert!(c.is_infinite() && c > 0.0);
    }

    #[test]
    fn profit_factor_correct() {
        // Gains : 5+3+2 = 10. Pertes : 1+1 = 2. PF = 10/2 = 5
        let e = synth_eval_with_pnl(vec![5.0, -1.0, 3.0, -1.0, 2.0]);
        let pf = e.profit_factor();
        assert!((pf - 5.0).abs() < 1e-9, "expected PF=5, got {}", pf);
    }

    #[test]
    fn max_consecutive_losses_correct() {
        // Pertes consÃƒÂ©cutives max : 3 (jours 4-5-6)
        let e = synth_eval_with_pnl(vec![1.0, -1.0, 1.0, -1.0, -1.0, -1.0, 1.0, -1.0]);
        assert_eq!(e.max_consecutive_losing_days(), 3);
    }
}







