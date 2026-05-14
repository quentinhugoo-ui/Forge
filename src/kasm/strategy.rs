//! Π.22 (Wave 12, 2026-05-02) — Strategy graph DSL.
//!
//! **Origine** : QuantConnect Lean, Backtrader, vectorbt, zipline.
//! Idée centrale : une stratégie de trading = combinaison de signaux
//! indicateurs (SMA, RSI, etc.) couplés à des actions (Buy/Sell/Hold).
//! En la représentant comme un AST déclaratif (DSL), on obtient :
//!
//!   1. **Composabilité** : 2 stratégies partageant 50% des signaux
//!      → cache hit auto Forge content-addressed.
//!   2. **Backtesting déterministe** : un Strategy AST a un hash
//!      content-addressed unique → replay identique.
//!   3. **Optimization** : remplacer un signal par un autre = changer
//!      un node du DAG, pas réécrire le code.
//!
//! ## Pourquoi pour Forge ?
//!
//! Wave 11 a livré `OhlcvStore` (Π.18) avec SMA/ATR/drawdown. Wave 12
//! ajoute la couche supérieure : un DSL qui combine ces indicateurs
//! en signal logique → action de trading.
//!
//! Wave 9 `Proven<_, Deterministic>` peut ensuite valider qu'une
//! stratégie utilise UNIQUEMENT des indicateurs déterministes.
//!
//! ## Architecture Wave 12 minimal viable
//!
//! - `Indicator` enum : SmaCrossover, RsiBelow, AtrAbove, PriceAbove,
//!   Constant, And, Or, Not (composition booléenne).
//! - `Action` enum : Buy(qty), Sell(qty), Hold, ClosePosition.
//! - `Strategy { signals: Vec<(Indicator, Action)>, default: Action }`
//!   évalué en order — premier indicateur true → action correspondante.
//! - `evaluate_at(idx, store) -> Action` : runtime evaluator.
//!
//! ## Limitations Wave 12 minimal
//!
//! - Indicateurs : SMA (déjà dans OhlcvStore), RSI Wilder, ATR
//!   (déjà), price comparison. Pas encore de MACD, Bollinger,
//!   Stochastic — Wave 13+ peut étendre.
//! - Action simple : Buy/Sell flat qty. Pas de position sizing
//!   complexe (Kelly criterion etc.) — Wave 13+.
//! - Pas de stop-loss / take-profit chained — gérés par le caller.

use crate::kasm::fixed::Q3132;
use crate::kasm::ohlcv::{OhlcvError, OhlcvStore};

/// Indicateur technique boolean : retourne true si la condition est
/// satisfaite à l'index donné.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Indicator {
    /// Toujours true (constant).
    AlwaysTrue,
    /// Toujours false.
    AlwaysFalse,
    /// SMA(fast) > SMA(slow) à idx → crossover bull.
    SmaBullishCross { fast_period: usize, slow_period: usize },
    /// SMA(fast) < SMA(slow) → crossover bear.
    SmaBearishCross { fast_period: usize, slow_period: usize },
    /// Close price > Q3132 raw threshold.
    PriceAbove { price_threshold: i64 },
    /// Close price < threshold.
    PriceBelow { price_threshold: i64 },
    /// ATR(period) > threshold (high volatility).
    AtrAbove { period: usize, threshold: i64 },
    /// AND deux indicateurs.
    And(Box<Indicator>, Box<Indicator>),
    /// OR deux indicateurs.
    Or(Box<Indicator>, Box<Indicator>),
    /// NOT.
    Not(Box<Indicator>),
}

impl Indicator {
    /// Évalue l'indicateur à un index du store. Retourne false si l'idx
    /// est hors range ou si les indicateurs requis (e.g. SMA) ne sont
    /// pas définis (i.e. moins de `period` bars).
    pub fn evaluate(&self, idx: usize, store: &OhlcvStore) -> bool {
        match self {
            Indicator::AlwaysTrue => true,
            Indicator::AlwaysFalse => false,
            Indicator::SmaBullishCross { fast_period, slow_period } => {
                let fast = match store.sma_close(*fast_period) {
                    Ok(v) => v,
                    Err(_) => return false,
                };
                let slow = match store.sma_close(*slow_period) {
                    Ok(v) => v,
                    Err(_) => return false,
                };
                // Index dans les SMA arrays = idx - max(period) + 1.
                // Mais le caller peut passer idx absolu — on convertit.
                let max_period = (*fast_period).max(*slow_period);
                if idx + 1 < max_period {
                    return false;
                }
                let fast_idx = idx + 1 - *fast_period;
                let slow_idx = idx + 1 - *slow_period;
                match (fast.get(fast_idx), slow.get(slow_idx)) {
                    (Some(f), Some(s)) => f > s,
                    _ => false,
                }
            }
            Indicator::SmaBearishCross { fast_period, slow_period } => {
                let inverse = Indicator::SmaBullishCross {
                    fast_period: *fast_period, slow_period: *slow_period,
                };
                !inverse.evaluate(idx, store)
                    && Indicator::AlwaysTrue.evaluate(idx, store)
                    && idx + 1 >= (*fast_period).max(*slow_period)
            }
            Indicator::PriceAbove { price_threshold } => {
                store.bar(idx).map(|b| b.close.raw() > *price_threshold).unwrap_or(false)
            }
            Indicator::PriceBelow { price_threshold } => {
                store.bar(idx).map(|b| b.close.raw() < *price_threshold).unwrap_or(false)
            }
            Indicator::AtrAbove { period, threshold } => {
                let atr = match store.atr(*period) {
                    Ok(v) => v,
                    Err(_) => return false,
                };
                if idx + 1 < *period {
                    return false;
                }
                let atr_idx = idx + 1 - period;
                atr.get(atr_idx).map(|a| a.raw() > *threshold).unwrap_or(false)
            }
            Indicator::And(a, b) => a.evaluate(idx, store) && b.evaluate(idx, store),
            Indicator::Or(a, b) => a.evaluate(idx, store) || b.evaluate(idx, store),
            Indicator::Not(inner) => !inner.evaluate(idx, store),
        }
    }
}

/// Action de trading. Quantités en i64 (units de base).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Buy(i64),
    Sell(i64),
    Hold,
    ClosePosition,
}

/// Stratégie complète : liste ordonnée (Indicator, Action).
/// `evaluate_at` retourne l'action du premier indicateur qui matche.
/// `default` retourné si aucun ne matche.
#[derive(Debug, Clone)]
pub struct Strategy {
    rules: Vec<(Indicator, Action)>,
    default: Action,
}

impl Strategy {
    pub fn new(default: Action) -> Self {
        Self { rules: Vec::new(), default }
    }

    /// Ajoute une règle. Order-sensitive : la première qui matche gagne.
    pub fn add_rule(mut self, indicator: Indicator, action: Action) -> Self {
        self.rules.push((indicator, action));
        self
    }

    /// Setter pour le default action si aucune règle ne matche.
    pub fn with_default(mut self, default: Action) -> Self {
        self.default = default;
        self
    }

    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }
    pub fn default_action(&self) -> Action {
        self.default
    }

    /// Évalue la stratégie au bar idx.
    pub fn evaluate_at(&self, idx: usize, store: &OhlcvStore) -> Action {
        for (indicator, action) in &self.rules {
            if indicator.evaluate(idx, store) {
                return *action;
            }
        }
        self.default
    }

    /// Évalue la stratégie sur tous les bars du store, retourne le
    /// vecteur d'actions (1 par bar).
    pub fn evaluate_all(&self, store: &OhlcvStore) -> Vec<Action> {
        (0..store.len()).map(|i| self.evaluate_at(i, store)).collect()
    }
}

/// Backtest summary : count actions, P&L estimé naïf (entry/exit at close).
#[derive(Debug, Clone, Copy, Default)]
pub struct BacktestSummary {
    pub buys: u32,
    pub sells: u32,
    pub holds: u32,
    pub closes: u32,
    pub final_pnl: Q3132,
    pub final_position: i64,
}

impl BacktestSummary {
    /// Naïf P&L : execution au close de chaque bar, no commissions.
    /// Position tracking simple (long-only ou short-only selon les
    /// actions retournées par la stratégie).
    pub fn from_strategy(strategy: &Strategy, store: &OhlcvStore) -> Result<Self, OhlcvError> {
        let actions = strategy.evaluate_all(store);
        let mut summary = BacktestSummary::default();
        let mut position: i64 = 0;
        let mut entry_price = Q3132::ZERO;
        let mut realized_pnl = Q3132::ZERO;

        for (i, action) in actions.iter().enumerate() {
            let bar = store.bar(i)?;
            let close = bar.close;
            match action {
                Action::Buy(qty) => {
                    summary.buys += 1;
                    if position == 0 {
                        entry_price = close;
                    }
                    position += qty;
                }
                Action::Sell(qty) => {
                    summary.sells += 1;
                    if position > 0 {
                        // Realize partial PnL sur la sortie.
                        let exit_qty = (*qty).min(position);
                        let pnl_per_unit = close.saturating_sub(entry_price);
                        let pnl = pnl_per_unit.saturating_mul(Q3132::from_int(exit_qty as i32));
                        realized_pnl = realized_pnl.saturating_add(pnl);
                        position -= exit_qty;
                    }
                }
                Action::Hold => {
                    summary.holds += 1;
                }
                Action::ClosePosition => {
                    summary.closes += 1;
                    if position > 0 {
                        let pnl_per_unit = close.saturating_sub(entry_price);
                        let pnl = pnl_per_unit.saturating_mul(Q3132::from_int(position as i32));
                        realized_pnl = realized_pnl.saturating_add(pnl);
                        position = 0;
                    }
                }
            }
        }
        summary.final_pnl = realized_pnl;
        summary.final_position = position;
        Ok(summary)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kasm::timestamp::Timestamp;

    fn build_store(closes: &[i32]) -> OhlcvStore {
        let mut store = OhlcvStore::new();
        for (i, c) in closes.iter().enumerate() {
            let q = Q3132::from_int(*c);
            store.push_bar(
                Timestamp::from_seconds(i as i64 * 60),
                q, q, q, q, 1000,
            ).unwrap();
        }
        store
    }

    #[test]
    fn indicator_always_true_false() {
        let store = build_store(&[100]);
        assert!(Indicator::AlwaysTrue.evaluate(0, &store));
        assert!(!Indicator::AlwaysFalse.evaluate(0, &store));
    }

    #[test]
    fn indicator_price_above_below() {
        let store = build_store(&[100]);
        let above = Indicator::PriceAbove {
            price_threshold: Q3132::from_int(50).raw(),
        };
        let below = Indicator::PriceBelow {
            price_threshold: Q3132::from_int(150).raw(),
        };
        assert!(above.evaluate(0, &store));
        assert!(below.evaluate(0, &store));
    }

    #[test]
    fn indicator_sma_bullish_cross() {
        // Trend ascendant : 100, 102, 104, 106, 108 → SMA(2) > SMA(4)
        // pour idx >= 3.
        let store = build_store(&[100, 102, 104, 106, 108]);
        let cross = Indicator::SmaBullishCross {
            fast_period: 2, slow_period: 4,
        };
        // À idx=3 : fast SMA(2) = (104+106)/2 = 105, slow SMA(4) = (100+102+104+106)/4 = 103.
        assert!(cross.evaluate(3, &store));
        // À idx=4 : fast SMA(2) = (106+108)/2 = 107, slow SMA(4) = (102+104+106+108)/4 = 105.
        assert!(cross.evaluate(4, &store));
    }

    #[test]
    fn indicator_and_combinator() {
        let store = build_store(&[100]);
        let and = Indicator::And(
            Box::new(Indicator::AlwaysTrue),
            Box::new(Indicator::PriceAbove {
                price_threshold: Q3132::from_int(50).raw(),
            }),
        );
        assert!(and.evaluate(0, &store));

        let and_false = Indicator::And(
            Box::new(Indicator::AlwaysTrue),
            Box::new(Indicator::AlwaysFalse),
        );
        assert!(!and_false.evaluate(0, &store));
    }

    #[test]
    fn indicator_or_combinator() {
        let store = build_store(&[100]);
        let or = Indicator::Or(
            Box::new(Indicator::AlwaysFalse),
            Box::new(Indicator::AlwaysTrue),
        );
        assert!(or.evaluate(0, &store));
    }

    #[test]
    fn indicator_not_combinator() {
        let store = build_store(&[100]);
        let not = Indicator::Not(Box::new(Indicator::AlwaysTrue));
        assert!(!not.evaluate(0, &store));
    }

    #[test]
    fn strategy_first_match_wins() {
        let store = build_store(&[100]);
        let strat = Strategy::new(Action::Hold)
            .add_rule(Indicator::AlwaysTrue, Action::Buy(10))
            .add_rule(Indicator::AlwaysTrue, Action::Sell(5));
        // Premier match → Buy(10).
        assert_eq!(strat.evaluate_at(0, &store), Action::Buy(10));
    }

    #[test]
    fn strategy_default_when_no_match() {
        let store = build_store(&[100]);
        let strat = Strategy::new(Action::Hold)
            .add_rule(Indicator::AlwaysFalse, Action::Buy(10));
        assert_eq!(strat.evaluate_at(0, &store), Action::Hold);
    }

    #[test]
    fn strategy_evaluate_all() {
        let store = build_store(&[100, 102, 104]);
        let strat = Strategy::new(Action::Hold)
            .add_rule(
                Indicator::PriceAbove {
                    price_threshold: Q3132::from_int(101).raw(),
                },
                Action::Buy(1),
            );
        let actions = strat.evaluate_all(&store);
        // bar 0 close = 100 → no match → Hold.
        // bar 1 close = 102 → match → Buy(1).
        // bar 2 close = 104 → match → Buy(1).
        assert_eq!(actions, vec![Action::Hold, Action::Buy(1), Action::Buy(1)]);
    }

    #[test]
    fn backtest_summary_counts_actions() {
        let store = build_store(&[100, 102, 104, 106, 108]);
        let strat = Strategy::new(Action::Hold)
            .add_rule(
                Indicator::PriceAbove {
                    price_threshold: Q3132::from_int(101).raw(),
                },
                Action::Buy(1),
            );
        let summary = BacktestSummary::from_strategy(&strat, &store).unwrap();
        // bar 0 = Hold, bars 1-4 = Buy → 4 buys, 1 hold.
        assert_eq!(summary.buys, 4);
        assert_eq!(summary.holds, 1);
        assert_eq!(summary.final_position, 4);
    }

    #[test]
    fn backtest_summary_realizes_pnl_on_sell() {
        // Stratégie : buy 1 unit at bar 0, sell 1 unit at bar 4.
        // close[0] = 100, close[4] = 108 → PnL = 8 × 1 = 8.
        let store = build_store(&[100, 102, 104, 106, 108]);
        let strat = Strategy::new(Action::Hold)
            .add_rule(
                Indicator::PriceBelow {
                    price_threshold: Q3132::from_int(101).raw(),
                },
                Action::Buy(1),
            )
            .add_rule(
                Indicator::PriceAbove {
                    price_threshold: Q3132::from_int(107).raw(),
                },
                Action::Sell(1),
            );
        let summary = BacktestSummary::from_strategy(&strat, &store).unwrap();
        // 1 buy au bar 0 (close=100), 1 sell au bar 4 (close=108) → PnL = 8.
        assert_eq!(summary.buys, 1);
        assert_eq!(summary.sells, 1);
        assert_eq!(summary.final_pnl, Q3132::from_int(8));
        assert_eq!(summary.final_position, 0);
    }

    #[test]
    fn backtest_close_position_realizes_remaining() {
        let store = build_store(&[100, 110]);
        let strat = Strategy::new(Action::Hold)
            .add_rule(Indicator::PriceBelow {
                price_threshold: Q3132::from_int(105).raw(),
            }, Action::Buy(2))
            .add_rule(Indicator::PriceAbove {
                price_threshold: Q3132::from_int(105).raw(),
            }, Action::ClosePosition);
        let summary = BacktestSummary::from_strategy(&strat, &store).unwrap();
        // Bar 0: buy 2 @ 100. Bar 1: close position @ 110 → PnL = 10 × 2 = 20.
        assert_eq!(summary.final_pnl, Q3132::from_int(20));
        assert_eq!(summary.final_position, 0);
        assert_eq!(summary.closes, 1);
    }

    #[test]
    fn strategy_composable_via_and_or() {
        let store = build_store(&[100, 105, 110]);
        // Buy si price > 100 ET price < 108.
        let strat = Strategy::new(Action::Hold)
            .add_rule(
                Indicator::And(
                    Box::new(Indicator::PriceAbove {
                        price_threshold: Q3132::from_int(100).raw(),
                    }),
                    Box::new(Indicator::PriceBelow {
                        price_threshold: Q3132::from_int(108).raw(),
                    }),
                ),
                Action::Buy(1),
            );
        let actions = strat.evaluate_all(&store);
        // bar 0 (100) : 100 > 100 ? false → no match.
        // bar 1 (105) : 105 > 100 && 105 < 108 → match → Buy.
        // bar 2 (110) : 110 > 100 && 110 < 108 false → no match.
        assert_eq!(actions, vec![Action::Hold, Action::Buy(1), Action::Hold]);
    }
}
