//! Π.24 (Wave 12, 2026-05-02) — VWAP/TWAP execution simulator.
//!
//! **Origine** : ITG algorithmic execution literature, Almgren-Chriss
//! market impact model. Idée centrale : un ordre institutionnel (e.g.
//! 1M actions) est trop gros pour fill instantanément sans market
//! impact. On le slice en N petits chunks distribués dans le temps :
//!
//!   - **TWAP** (Time-Weighted Average Price) : N chunks équidistants
//!     sur la fenêtre, chacun = qty/N.
//!   - **VWAP** (Volume-Weighted Average Price) : chunks proportionnels
//!     au volume de chaque bar (suit le rythme de market activity).
//!
//! ## Pourquoi pour Forge ?
//!
//! Backtest réaliste = mesurer slippage. Une stratégie qui paraît
//! profitable à 0 slippage peut perdre tout son edge avec slippage
//! réaliste. VWAP/TWAP simulator donne une borne supérieure réaliste
//! du slippage attendu.
//!
//! Avec Wave 11 fixed-point Q31.32 + OHLCV + timestamp, on a tout pour
//! simuler bit-exact :
//!
//! ```text
//!   pour chaque chunk:
//!     fill_price = bar.close * (1 + market_impact_bps × chunk_size/avg_volume)
//!     total_value += fill_price × chunk_size
//!
//!   avg_fill = total_value / total_qty
//!   slippage = avg_fill - first_bar_close  (vs benchmark "instant fill at start")
//! ```
//!
//! ## Architecture Wave 12 minimal viable
//!
//! - `Side::Buy / Side::Sell` enum (signe du market impact).
//! - `MarketImpactModel { bps_per_pct_volume }` linear simple.
//! - `vwap_slice(target_qty, bars[start..end], side, impact)` →
//!   Vec<Fill> + avg_price + slippage.
//! - `twap_slice(target_qty, bars[start..end], side, impact)` →
//!   Vec<Fill> + avg_price + slippage.
//!
//! ## Limitations Wave 12 minimal
//!
//! - Linear market impact (Wave 13+ peut ajouter Almgren-Chriss
//!   square-root impact).
//! - Pas de latency simulator (assume execution at close of bar).
//! - Single-symbol per execution.
//! - Pas de dark pool / hidden liquidity (Wave 13+).

use crate::kasm::fixed::Q3132;
use crate::kasm::ohlcv::{OhlcvError, OhlcvStore};
use crate::kasm::order_book::Fill;

/// Direction du trade.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Buy,
    Sell,
}

/// Modèle de market impact linéaire.
/// `slippage_q3132 = base_price × (chunk_size / avg_volume) × bps_per_pct_volume / 10000`.
#[derive(Debug, Clone, Copy)]
pub struct MarketImpactModel {
    /// Basis points (1 bp = 0.01%) de slippage par % du volume moyen.
    pub bps_per_pct_volume: i64,
}

impl MarketImpactModel {
    pub const NONE: MarketImpactModel = MarketImpactModel { bps_per_pct_volume: 0 };
    pub const SMALL: MarketImpactModel = MarketImpactModel { bps_per_pct_volume: 5 };
    pub const MODERATE: MarketImpactModel = MarketImpactModel { bps_per_pct_volume: 20 };
    pub const LARGE: MarketImpactModel = MarketImpactModel { bps_per_pct_volume: 100 };

    /// Compute slippage en Q3132 raw, signed selon le side.
    pub fn slippage(
        &self,
        base_price: Q3132,
        chunk_size: i64,
        avg_volume: i64,
        side: Side,
    ) -> Q3132 {
        if avg_volume <= 0 || self.bps_per_pct_volume == 0 {
            return Q3132::ZERO;
        }
        // chunk_pct = chunk_size / avg_volume × 100 (en Q3132)
        // slippage_pct = chunk_pct × bps_per_pct_volume / 10000
        // slippage = base_price × slippage_pct
        let chunk_size_q = Q3132::from_int(chunk_size as i32);
        let avg_volume_q = Q3132::from_int(avg_volume as i32);
        let pct_volume = chunk_size_q.checked_div(avg_volume_q);
        let bps_factor = Q3132::from_rational(self.bps_per_pct_volume, 10_000);
        let pct_slippage = pct_volume.saturating_mul(bps_factor);
        let signed_slip = base_price.saturating_mul(pct_slippage);
        match side {
            Side::Buy => signed_slip,             // pay more
            Side::Sell => signed_slip.saturating_neg(),  // receive less
        }
    }
}

/// Resultat d'une execution slice.
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    pub fills: Vec<Fill>,
    pub avg_fill_price: Q3132,
    pub total_qty: i64,
    pub slippage_vs_first_close: Q3132,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionError {
    EmptyRange,
    BadRange { start: usize, end: usize, len: usize },
    Ohlcv(OhlcvError),
    /// Pas assez de volume sur la fenêtre pour VWAP.
    InsufficientVolume { required: i64, available: i64 },
}

impl From<OhlcvError> for ExecutionError {
    fn from(e: OhlcvError) -> Self { ExecutionError::Ohlcv(e) }
}

/// TWAP slice : `target_qty` divisé en N chunks équidistants sur les
/// bars [start, end). Chaque chunk fill au close du bar, avec slippage
/// linéaire selon l'impact model.
pub fn twap_slice(
    store: &OhlcvStore,
    start: usize,
    end: usize,
    target_qty: i64,
    side: Side,
    impact: MarketImpactModel,
) -> Result<ExecutionResult, ExecutionError> {
    if start >= end {
        return Err(ExecutionError::EmptyRange);
    }
    if end > store.len() {
        return Err(ExecutionError::BadRange { start, end, len: store.len() });
    }
    let n_bars = end - start;
    // Distribute target_qty equally across bars. Last bar absorbs
    // remainder pour conservation exacte du total.
    let chunk_size = target_qty / (n_bars as i64);
    let remainder = target_qty - chunk_size * (n_bars as i64);

    let mut fills = Vec::with_capacity(n_bars);
    let mut total_value: i64 = 0;
    let avg_volume = store.volume_column()[start..end].iter().sum::<i64>() / n_bars as i64;

    for i in 0..n_bars {
        let bar = store.bar(start + i)?;
        let q = if i == n_bars - 1 { chunk_size + remainder } else { chunk_size };
        if q == 0 {
            continue;
        }
        let slippage = impact.slippage(bar.close, q, avg_volume.max(1), side);
        let exec_price = bar.close.saturating_add(slippage);
        let value = exec_price.raw().saturating_mul(q);
        total_value = total_value.saturating_add(value);
        fills.push(Fill { price: exec_price.raw(), size: q });
    }

    let avg_fill = if target_qty != 0 {
        Q3132::from_raw(total_value / target_qty)
    } else {
        Q3132::ZERO
    };
    let first_close = store.bar(start)?.close;
    let slippage_total = avg_fill.saturating_sub(first_close);

    Ok(ExecutionResult {
        fills,
        avg_fill_price: avg_fill,
        total_qty: target_qty,
        slippage_vs_first_close: slippage_total,
    })
}

/// VWAP slice : `target_qty` distribué proportionnellement au volume
/// de chaque bar. Bars avec plus de volume reçoivent plus de qty.
pub fn vwap_slice(
    store: &OhlcvStore,
    start: usize,
    end: usize,
    target_qty: i64,
    side: Side,
    impact: MarketImpactModel,
) -> Result<ExecutionResult, ExecutionError> {
    if start >= end {
        return Err(ExecutionError::EmptyRange);
    }
    if end > store.len() {
        return Err(ExecutionError::BadRange { start, end, len: store.len() });
    }
    let total_volume: i64 = store.volume_column()[start..end].iter().sum();
    if total_volume <= 0 {
        return Err(ExecutionError::InsufficientVolume {
            required: 1, available: total_volume,
        });
    }

    let mut fills = Vec::with_capacity(end - start);
    let mut total_value: i64 = 0;
    let mut allocated: i64 = 0;
    let n = end - start;
    let avg_volume = total_volume / n as i64;

    for i in 0..n {
        let bar = store.bar(start + i)?;
        let pct = (bar.volume as i128 * target_qty as i128) / (total_volume as i128);
        let q = if i == n - 1 {
            target_qty - allocated
        } else {
            pct as i64
        };
        if q == 0 {
            continue;
        }
        let slippage = impact.slippage(bar.close, q, avg_volume.max(1), side);
        let exec_price = bar.close.saturating_add(slippage);
        let value = exec_price.raw().saturating_mul(q);
        total_value = total_value.saturating_add(value);
        fills.push(Fill { price: exec_price.raw(), size: q });
        allocated += q;
    }

    let avg_fill = if target_qty != 0 {
        Q3132::from_raw(total_value / target_qty)
    } else {
        Q3132::ZERO
    };
    let first_close = store.bar(start)?.close;
    let slippage_total = avg_fill.saturating_sub(first_close);

    Ok(ExecutionResult {
        fills,
        avg_fill_price: avg_fill,
        total_qty: target_qty,
        slippage_vs_first_close: slippage_total,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kasm::timestamp::Timestamp;

    fn build_store(bars: &[(i32, i64)]) -> OhlcvStore {
        // (close, volume) tuples.
        let mut store = OhlcvStore::new();
        for (i, &(close, vol)) in bars.iter().enumerate() {
            let q = Q3132::from_int(close);
            store.push_bar(
                Timestamp::from_seconds(i as i64 * 60),
                q, q, q, q, vol,
            ).unwrap();
        }
        store
    }

    #[test]
    fn twap_slice_equal_chunks() {
        // 4 bars closes = [100, 101, 102, 103], qty=40 → 10 par bar (no slip).
        let store = build_store(&[(100, 1000), (101, 1000), (102, 1000), (103, 1000)]);
        let result = twap_slice(
            &store, 0, 4, 40, Side::Buy, MarketImpactModel::NONE,
        ).unwrap();
        assert_eq!(result.fills.len(), 4);
        for f in &result.fills {
            assert_eq!(f.size, 10);
        }
        // avg_fill = (100+101+102+103)/4 = 101.5.
        assert_eq!(result.avg_fill_price, Q3132::from_rational(101*2 + 1, 2));
    }

    #[test]
    fn twap_slice_remainder_to_last() {
        // qty=10, n=3 bars → chunks 3, 3, 4 (le dernier absorbe le reste).
        let store = build_store(&[(100, 1000), (101, 1000), (102, 1000)]);
        let result = twap_slice(
            &store, 0, 3, 10, Side::Buy, MarketImpactModel::NONE,
        ).unwrap();
        assert_eq!(result.fills[0].size, 3);
        assert_eq!(result.fills[1].size, 3);
        assert_eq!(result.fills[2].size, 4);
    }

    #[test]
    fn vwap_slice_proportional_to_volume() {
        // bars : volume = [100, 200, 100, 600], total = 1000.
        // qty=100 → fills proportional : 10, 20, 10, 60.
        let store = build_store(&[(100, 100), (101, 200), (102, 100), (103, 600)]);
        let result = vwap_slice(
            &store, 0, 4, 100, Side::Buy, MarketImpactModel::NONE,
        ).unwrap();
        assert_eq!(result.fills[0].size, 10);
        assert_eq!(result.fills[1].size, 20);
        assert_eq!(result.fills[2].size, 10);
        assert_eq!(result.fills[3].size, 60);
        assert_eq!(result.total_qty, 100);
    }

    #[test]
    fn buy_slippage_increases_price() {
        let store = build_store(&[(100, 1000)]);
        // 100 units sur bar avec volume 1000 = 10% volume.
        // Impact moderate = 20 bps/pct → slippage 200 bps = 2% → 2.0 sur prix 100.
        let result = twap_slice(
            &store, 0, 1, 100, Side::Buy, MarketImpactModel::MODERATE,
        ).unwrap();
        // Avg fill price doit être > 100 (slippage positive on buy).
        assert!(result.avg_fill_price > Q3132::from_int(100));
    }

    #[test]
    fn sell_slippage_decreases_price() {
        let store = build_store(&[(100, 1000)]);
        let result = twap_slice(
            &store, 0, 1, 100, Side::Sell, MarketImpactModel::MODERATE,
        ).unwrap();
        // Avg fill price doit être < 100 (slippage negative on sell).
        assert!(result.avg_fill_price < Q3132::from_int(100));
    }

    #[test]
    fn execution_empty_range_errors() {
        let store = build_store(&[(100, 1000)]);
        let err = twap_slice(
            &store, 0, 0, 100, Side::Buy, MarketImpactModel::NONE,
        ).unwrap_err();
        assert!(matches!(err, ExecutionError::EmptyRange));
    }

    #[test]
    fn execution_bad_range_errors() {
        let store = build_store(&[(100, 1000)]);
        let err = twap_slice(
            &store, 0, 99, 100, Side::Buy, MarketImpactModel::NONE,
        ).unwrap_err();
        assert!(matches!(err, ExecutionError::BadRange { .. }));
    }

    #[test]
    fn vwap_zero_volume_errors() {
        let store = build_store(&[(100, 0), (101, 0)]);
        let err = vwap_slice(
            &store, 0, 2, 100, Side::Buy, MarketImpactModel::NONE,
        ).unwrap_err();
        assert!(matches!(err, ExecutionError::InsufficientVolume { .. }));
    }

    #[test]
    fn slippage_vs_first_close_computed() {
        let store = build_store(&[(100, 1000), (105, 1000)]);
        let result = twap_slice(
            &store, 0, 2, 20, Side::Buy, MarketImpactModel::NONE,
        ).unwrap();
        // first_close = 100, avg_fill = 102.5, slippage = 2.5.
        let expected_slippage = Q3132::from_rational(25, 10);
        assert_eq!(result.slippage_vs_first_close, expected_slippage);
    }

    #[test]
    fn impact_model_no_impact_zero_slippage() {
        let model = MarketImpactModel::NONE;
        let slip = model.slippage(Q3132::from_int(100), 50, 1000, Side::Buy);
        assert_eq!(slip, Q3132::ZERO);
    }

    #[test]
    fn impact_model_buy_positive_sell_negative() {
        let model = MarketImpactModel::MODERATE;
        let buy_slip = model.slippage(Q3132::from_int(100), 100, 1000, Side::Buy);
        let sell_slip = model.slippage(Q3132::from_int(100), 100, 1000, Side::Sell);
        assert!(buy_slip > Q3132::ZERO);
        assert!(sell_slip < Q3132::ZERO);
        // Symétrique : magnitude égale.
        assert_eq!(buy_slip, sell_slip.saturating_neg());
    }

    #[test]
    fn vwap_total_qty_conserved_with_remainder() {
        // 7 unités sur 3 bars (chacun 1/3 = 2.33 → 2, 2, 3). Le dernier
        // bar absorbe le reste pour conservation exacte.
        let store = build_store(&[(100, 100), (101, 100), (102, 100)]);
        let result = vwap_slice(
            &store, 0, 3, 7, Side::Buy, MarketImpactModel::NONE,
        ).unwrap();
        let total: i64 = result.fills.iter().map(|f| f.size).sum();
        assert_eq!(total, 7);
        assert_eq!(result.total_qty, 7);
    }
}
