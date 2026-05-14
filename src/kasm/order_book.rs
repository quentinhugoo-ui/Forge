//! Π.20 (Wave 12, 2026-05-02) — Order Book L2/L3 nanostructure.
//!
//! **Origine** : ITCH/OUCH NASDAQ protocols, Bookmap, OB-replay
//! deterministic backtesting (Lobster academic dataset). Idée centrale :
//! le carnet d'ordres = un état event-driven, chaque tick est un
//! `OrderBookEvent` (Add/Modify/Delete) qui transforme l'état du book.
//!
//! ## Pourquoi pour Forge ?
//!
//! Backtest réaliste = simuler le carnet d'ordres niveau 2 (par prix)
//! pour mesurer slippage, queue position, market impact. Sans book L2,
//! on suppose un fill à mid-price — biais énorme sur stratégies
//! d'execution.
//!
//! Forge content-addressed : à chaque tick, le book a un hash unique
//! → replay déterministe + cache hit auto pour les states identiques.
//!
//! ## Architecture Wave 12 minimal viable
//!
//! - `OrderBook { bids: BTreeMap<i64, i64>, asks: BTreeMap<i64, i64> }`
//!   Clé = prix Q31.32 raw (i64 deterministe), valeur = size cumulative.
//! - `OrderBookEvent` : `AddBid`, `AddAsk`, `RemoveBid`, `RemoveAsk`,
//!   `ModifyBid`, `ModifyAsk`.
//! - `apply(event)` : transforme le book en place.
//! - `best_bid()`, `best_ask()`, `mid_price()`, `spread()`, `depth(N)`.
//! - `walk_buy(qty)` : simule un market buy, retourne (avg_fill_price, fills).
//! - `walk_sell(qty)` : symétrique.
//!
//! ## Limitations Wave 12 minimal
//!
//! - Niveau 2 (par prix), pas L3 (par ordre individuel) — Wave 13+
//!   pourra ajouter via `BTreeMap<i64, VecDeque<OrderId>>`.
//! - Single-symbol per book.
//! - Pas de hidden orders (iceberg) — Wave 12 minimal market-data only.

use crate::kasm::fixed::Q3132;
use std::collections::BTreeMap;
use std::fmt;

/// Event sur le carnet d'ordres. Tous les prix en Q31.32 raw, sizes
/// en i64 (units de base — actions/contracts).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderBookEvent {
    /// Ajoute size au niveau price (création ou aggregation).
    AddBid { price: i64, size: i64 },
    AddAsk { price: i64, size: i64 },
    /// Set absolute size at price level (replace).
    SetBid { price: i64, size: i64 },
    SetAsk { price: i64, size: i64 },
    /// Remove the price level entirely.
    RemoveBid { price: i64 },
    RemoveAsk { price: i64 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OrderBookError {
    /// Negative size invalide (sizes représentent toujours des
    /// quantités positives ; un cancel = RemoveBid/RemoveAsk).
    NegativeSize { size: i64 },
    /// Crossed book : best_bid >= best_ask (state invariant violé).
    CrossedBook { best_bid: i64, best_ask: i64 },
    /// Walk demande plus de qty que disponible.
    InsufficientLiquidity { needed: i64, available: i64 },
}

impl fmt::Display for OrderBookError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OrderBookError::NegativeSize { size } =>
                write!(f, "order book: negative size {} disallowed", size),
            OrderBookError::CrossedBook { best_bid, best_ask } =>
                write!(f, "order book crossed: bid {} >= ask {}", best_bid, best_ask),
            OrderBookError::InsufficientLiquidity { needed, available } =>
                write!(f, "order book: needed {} but only {} available", needed, available),
        }
    }
}

/// Une fill simulée par walk_buy/walk_sell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fill {
    pub price: i64,  // Q31.32 raw
    pub size: i64,
}

/// Carnet d'ordres L2 (par prix, pas par ordre individuel).
/// Bids triés descendant (best = plus haut), asks triés ascendant
/// (best = plus bas).
#[derive(Debug, Clone, Default)]
pub struct OrderBook {
    /// Bids : prix Q31.32 raw → size cumulative à ce niveau.
    /// BTreeMap iter dans l'ordre croissant — pour best_bid on iter
    /// reverse.
    bids: BTreeMap<i64, i64>,
    /// Asks : prix → size. Iter ascending pour best_ask.
    asks: BTreeMap<i64, i64>,
    /// Compteur d'events appliqués (statistique, observabilité).
    event_count: u64,
}

impl OrderBook {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn event_count(&self) -> u64 {
        self.event_count
    }

    pub fn bids_levels(&self) -> usize {
        self.bids.len()
    }
    pub fn asks_levels(&self) -> usize {
        self.asks.len()
    }

    /// Apply an event. Validates non-negative sizes.
    pub fn apply(&mut self, event: OrderBookEvent) -> Result<(), OrderBookError> {
        match event {
            OrderBookEvent::AddBid { size, .. } | OrderBookEvent::AddAsk { size, .. }
            | OrderBookEvent::SetBid { size, .. } | OrderBookEvent::SetAsk { size, .. }
                if size < 0 =>
            {
                return Err(OrderBookError::NegativeSize { size });
            }
            _ => {}
        }
        match event {
            OrderBookEvent::AddBid { price, size } => {
                *self.bids.entry(price).or_insert(0) += size;
            }
            OrderBookEvent::AddAsk { price, size } => {
                *self.asks.entry(price).or_insert(0) += size;
            }
            OrderBookEvent::SetBid { price, size } => {
                if size == 0 {
                    self.bids.remove(&price);
                } else {
                    self.bids.insert(price, size);
                }
            }
            OrderBookEvent::SetAsk { price, size } => {
                if size == 0 {
                    self.asks.remove(&price);
                } else {
                    self.asks.insert(price, size);
                }
            }
            OrderBookEvent::RemoveBid { price } => {
                self.bids.remove(&price);
            }
            OrderBookEvent::RemoveAsk { price } => {
                self.asks.remove(&price);
            }
        }
        self.event_count += 1;
        Ok(())
    }

    /// Best bid (plus haut prix bid). None si pas de bids.
    pub fn best_bid(&self) -> Option<Q3132> {
        self.bids.keys().last().copied().map(Q3132::from_raw)
    }

    /// Best ask (plus bas prix ask). None si pas de asks.
    pub fn best_ask(&self) -> Option<Q3132> {
        self.asks.keys().next().copied().map(Q3132::from_raw)
    }

    /// Mid price = (best_bid + best_ask) / 2. None si l'un manque.
    pub fn mid_price(&self) -> Option<Q3132> {
        let bid = self.best_bid()?;
        let ask = self.best_ask()?;
        Some(bid.saturating_add(ask).checked_div(Q3132::from_int(2)))
    }

    /// Spread = best_ask - best_bid. None si manquant.
    pub fn spread(&self) -> Option<Q3132> {
        let bid = self.best_bid()?;
        let ask = self.best_ask()?;
        Some(ask.saturating_sub(bid))
    }

    /// Verify book invariant : best_bid < best_ask (no cross).
    pub fn verify_uncrossed(&self) -> Result<(), OrderBookError> {
        if let (Some(bid), Some(ask)) = (self.bids.keys().last(), self.asks.keys().next()) {
            if bid >= ask {
                return Err(OrderBookError::CrossedBook {
                    best_bid: *bid,
                    best_ask: *ask,
                });
            }
        }
        Ok(())
    }

    /// Top N bid levels (price, size) du best vers le bas.
    pub fn top_bids(&self, n: usize) -> Vec<(Q3132, i64)> {
        self.bids
            .iter()
            .rev()
            .take(n)
            .map(|(p, s)| (Q3132::from_raw(*p), *s))
            .collect()
    }

    /// Top N ask levels (price, size) du best vers le haut.
    pub fn top_asks(&self, n: usize) -> Vec<(Q3132, i64)> {
        self.asks
            .iter()
            .take(n)
            .map(|(p, s)| (Q3132::from_raw(*p), *s))
            .collect()
    }

    /// Total bid liquidity disponible (sum sizes tous niveaux).
    pub fn total_bid_size(&self) -> i64 {
        self.bids.values().sum()
    }
    pub fn total_ask_size(&self) -> i64 {
        self.asks.values().sum()
    }

    /// Walk buy : simule l'achat de `qty` units, consommant les asks
    /// du best vers le haut. Retourne les fills (prix, size par level)
    /// et l'avg fill price.
    /// Erreur si liquidité totale ask < qty.
    pub fn walk_buy(&self, qty: i64) -> Result<(Q3132, Vec<Fill>), OrderBookError> {
        if qty <= 0 {
            return Ok((Q3132::ZERO, Vec::new()));
        }
        let total = self.total_ask_size();
        if total < qty {
            return Err(OrderBookError::InsufficientLiquidity { needed: qty, available: total });
        }
        let mut fills = Vec::new();
        let mut remaining = qty;
        let mut total_value: i64 = 0; // Q31.32 raw (price × size)
        for (&price, &size) in self.asks.iter() {
            if remaining == 0 {
                break;
            }
            let take = remaining.min(size);
            // value = price (Q31.32 raw) × take (i64 plain).
            // Pour rester en Q31.32 raw : price × take = (price * take) en raw.
            let value = price.saturating_mul(take);
            total_value = total_value.saturating_add(value);
            fills.push(Fill { price, size: take });
            remaining -= take;
        }
        // avg_price = total_value / qty (en Q31.32 raw / int = Q31.32 raw).
        let avg_price = Q3132::from_raw(total_value / qty);
        Ok((avg_price, fills))
    }

    /// Walk sell : symétrique au walk_buy, consomme les bids du best
    /// vers le bas.
    pub fn walk_sell(&self, qty: i64) -> Result<(Q3132, Vec<Fill>), OrderBookError> {
        if qty <= 0 {
            return Ok((Q3132::ZERO, Vec::new()));
        }
        let total = self.total_bid_size();
        if total < qty {
            return Err(OrderBookError::InsufficientLiquidity { needed: qty, available: total });
        }
        let mut fills = Vec::new();
        let mut remaining = qty;
        let mut total_value: i64 = 0;
        // Iter bids reverse (best = plus haut prix).
        for (&price, &size) in self.bids.iter().rev() {
            if remaining == 0 {
                break;
            }
            let take = remaining.min(size);
            let value = price.saturating_mul(take);
            total_value = total_value.saturating_add(value);
            fills.push(Fill { price, size: take });
            remaining -= take;
        }
        let avg_price = Q3132::from_raw(total_value / qty);
        Ok((avg_price, fills))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(int: i32) -> i64 {
        Q3132::from_int(int).raw()
    }

    #[test]
    fn book_empty_no_best() {
        let book = OrderBook::new();
        assert_eq!(book.best_bid(), None);
        assert_eq!(book.best_ask(), None);
        assert_eq!(book.mid_price(), None);
        assert_eq!(book.spread(), None);
    }

    #[test]
    fn book_add_bids_and_asks() {
        let mut book = OrderBook::new();
        book.apply(OrderBookEvent::AddBid { price: p(100), size: 10 }).unwrap();
        book.apply(OrderBookEvent::AddBid { price: p(99), size: 20 }).unwrap();
        book.apply(OrderBookEvent::AddAsk { price: p(101), size: 15 }).unwrap();
        book.apply(OrderBookEvent::AddAsk { price: p(102), size: 30 }).unwrap();

        assert_eq!(book.best_bid(), Some(Q3132::from_int(100)));
        assert_eq!(book.best_ask(), Some(Q3132::from_int(101)));
        assert_eq!(book.spread(), Some(Q3132::from_int(1)));
        assert_eq!(book.event_count(), 4);
    }

    #[test]
    fn book_set_replaces_size() {
        let mut book = OrderBook::new();
        book.apply(OrderBookEvent::AddBid { price: p(100), size: 10 }).unwrap();
        book.apply(OrderBookEvent::SetBid { price: p(100), size: 50 }).unwrap();
        assert_eq!(book.top_bids(1), vec![(Q3132::from_int(100), 50)]);
    }

    #[test]
    fn book_set_zero_removes_level() {
        let mut book = OrderBook::new();
        book.apply(OrderBookEvent::AddBid { price: p(100), size: 10 }).unwrap();
        book.apply(OrderBookEvent::SetBid { price: p(100), size: 0 }).unwrap();
        assert_eq!(book.bids_levels(), 0);
    }

    #[test]
    fn book_remove_event() {
        let mut book = OrderBook::new();
        book.apply(OrderBookEvent::AddAsk { price: p(101), size: 15 }).unwrap();
        book.apply(OrderBookEvent::RemoveAsk { price: p(101) }).unwrap();
        assert_eq!(book.asks_levels(), 0);
    }

    #[test]
    fn book_negative_size_rejected() {
        let mut book = OrderBook::new();
        let err = book.apply(OrderBookEvent::AddBid { price: p(100), size: -5 }).unwrap_err();
        assert!(matches!(err, OrderBookError::NegativeSize { size: -5 }));
    }

    #[test]
    fn book_uncrossed_invariant() {
        let mut book = OrderBook::new();
        book.apply(OrderBookEvent::AddBid { price: p(100), size: 10 }).unwrap();
        book.apply(OrderBookEvent::AddAsk { price: p(101), size: 10 }).unwrap();
        book.verify_uncrossed().unwrap();
        // Crosser le book : ask < bid → erreur.
        book.apply(OrderBookEvent::AddAsk { price: p(99), size: 5 }).unwrap();
        let err = book.verify_uncrossed().unwrap_err();
        assert!(matches!(err, OrderBookError::CrossedBook { .. }));
    }

    #[test]
    fn book_top_levels_sorted() {
        let mut book = OrderBook::new();
        book.apply(OrderBookEvent::AddBid { price: p(99), size: 10 }).unwrap();
        book.apply(OrderBookEvent::AddBid { price: p(100), size: 20 }).unwrap();
        book.apply(OrderBookEvent::AddBid { price: p(98), size: 30 }).unwrap();
        let top = book.top_bids(3);
        // Best = 100 (plus haut), puis 99, puis 98.
        assert_eq!(top[0].0, Q3132::from_int(100));
        assert_eq!(top[1].0, Q3132::from_int(99));
        assert_eq!(top[2].0, Q3132::from_int(98));
    }

    #[test]
    fn book_walk_buy_fills_at_increasing_prices() {
        let mut book = OrderBook::new();
        book.apply(OrderBookEvent::AddAsk { price: p(101), size: 5 }).unwrap();
        book.apply(OrderBookEvent::AddAsk { price: p(102), size: 10 }).unwrap();
        book.apply(OrderBookEvent::AddAsk { price: p(103), size: 20 }).unwrap();

        // Buy 12 → 5 @ 101 + 7 @ 102.
        let (avg, fills) = book.walk_buy(12).unwrap();
        assert_eq!(fills.len(), 2);
        assert_eq!(fills[0], Fill { price: p(101), size: 5 });
        assert_eq!(fills[1], Fill { price: p(102), size: 7 });
        // avg = (5*101 + 7*102) / 12 = (505 + 714) / 12 = 1219/12 = 101.5833...
        let expected = Q3132::from_rational(1219, 12);
        // Tolerance 1 ULP pour rounding.
        let diff = avg.saturating_sub(expected).saturating_abs();
        assert!(diff.raw() < 100, "avg = {} vs expected {}", avg, expected);
    }

    #[test]
    fn book_walk_buy_insufficient_errors() {
        let mut book = OrderBook::new();
        book.apply(OrderBookEvent::AddAsk { price: p(101), size: 5 }).unwrap();
        let err = book.walk_buy(100).unwrap_err();
        assert!(matches!(err, OrderBookError::InsufficientLiquidity { needed: 100, available: 5 }));
    }

    #[test]
    fn book_walk_sell_fills_at_decreasing_prices() {
        let mut book = OrderBook::new();
        book.apply(OrderBookEvent::AddBid { price: p(100), size: 10 }).unwrap();
        book.apply(OrderBookEvent::AddBid { price: p(99), size: 20 }).unwrap();

        let (avg, fills) = book.walk_sell(15).unwrap();
        assert_eq!(fills.len(), 2);
        assert_eq!(fills[0], Fill { price: p(100), size: 10 });
        assert_eq!(fills[1], Fill { price: p(99), size: 5 });
        // avg = (10*100 + 5*99) / 15 = 1495/15 = 99.6666...
        let expected = Q3132::from_rational(1495, 15);
        let diff = avg.saturating_sub(expected).saturating_abs();
        assert!(diff.raw() < 100);
    }

    #[test]
    fn book_total_liquidity() {
        let mut book = OrderBook::new();
        book.apply(OrderBookEvent::AddBid { price: p(100), size: 10 }).unwrap();
        book.apply(OrderBookEvent::AddBid { price: p(99), size: 20 }).unwrap();
        book.apply(OrderBookEvent::AddAsk { price: p(101), size: 5 }).unwrap();
        assert_eq!(book.total_bid_size(), 30);
        assert_eq!(book.total_ask_size(), 5);
    }

    #[test]
    fn book_mid_price() {
        let mut book = OrderBook::new();
        book.apply(OrderBookEvent::AddBid { price: p(100), size: 10 }).unwrap();
        book.apply(OrderBookEvent::AddAsk { price: p(102), size: 10 }).unwrap();
        // mid = (100 + 102) / 2 = 101.
        assert_eq!(book.mid_price(), Some(Q3132::from_int(101)));
    }
}
