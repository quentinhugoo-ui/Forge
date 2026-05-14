//! Trading components exposed as content-addressed KASM programs.
//!
//! This module keeps the trade simulator pieces available as deterministic
//! KASM bytecode. Their structural hashes can be reused by Atlas across
//! sessions and across domains whenever the same "first event in a short
//! horizon" computation appears again.

use scan::kasm::{execute, Node, Program, Target, Ty};
use std::sync::OnceLock;

#[inline]
fn push_f64(args: &mut Vec<u8>, value: f64) {
    args.extend_from_slice(&value.to_bits().to_le_bytes());
}

#[inline]
fn push_i64(args: &mut Vec<u8>, value: i64) {
    args.extend_from_slice(&value.to_le_bytes());
}

#[inline]
fn read_f64(out: &[u8]) -> f64 {
    let bits = u64::from_le_bytes(out[..8].try_into().expect("8 bytes f64 out"));
    f64::from_bits(bits)
}

/// The fixed max-horizon the trade-simulator KASM programs are specialized for.
/// It matches the default NATGAS H4 reverse-synth workflow.
pub const TRADE_HORIZON: usize = 6;

const SL_HIT_BASE: usize = 0;
const TP_HIT_BASE: usize = 6;
const PNL_HORIZON_SLOT: u8 = 12;
const SL_POINTS_SLOT: u8 = 13;
const TP_POINTS_SLOT: u8 = 14;
const SPREAD_POINTS_SLOT: u8 = 15;
const TRADE_INPUT_ARITY: usize = 16;
const TRADE_INPUT_ARITY_U8: u8 = 16;

#[derive(Debug, Clone, Copy)]
struct TradeProgramState {
    first_sl: u16,
    first_tp: u16,
    bars_held: u16,
    exit_reason: u16,
}

fn push_node(nodes: &mut Vec<Node>, node: Node) -> u16 {
    let idx = nodes.len();
    nodes.push(node);
    idx.try_into().expect("trade KASM program fits u16 slots")
}

fn trade_program_prefix() -> (Vec<Node>, [u16; 7]) {
    let mut nodes = Vec::new();
    for slot in 0..12 {
        push_node(&mut nodes, Node::input(slot));
    }
    push_node(&mut nodes, Node::input_f64(PNL_HORIZON_SLOT));
    push_node(&mut nodes, Node::input_f64(SL_POINTS_SLOT));
    push_node(&mut nodes, Node::input_f64(TP_POINTS_SLOT));
    push_node(&mut nodes, Node::input_f64(SPREAD_POINTS_SLOT));

    let mut ints = [0u16; 7];
    for value in 0..=6 {
        ints[value] = push_node(&mut nodes, Node::const_i64(value as i16));
    }

    (nodes, ints)
}

/// Appends deterministic "first event" logic:
/// - per bar priority: SL wins over TP when both are hit in the same candle
/// - across bars priority: earliest horizon wins
/// - no event: reason = Horizon, bars_held = 0 sentinel
fn append_first_event_logic(nodes: &mut Vec<Node>, ints: [u16; 7]) -> TradeProgramState {
    let zero = ints[0];
    let stop_loss = ints[1];
    let horizon = ints[2];
    let take_profit = ints[3];

    let mut first_sl = zero;
    let mut first_tp = zero;
    let mut bars_held = zero;
    let mut exit_reason = horizon;

    for h in (0..TRADE_HORIZON).rev() {
        let sl_hit = (SL_HIT_BASE + h) as u16;
        let tp_hit = (TP_HIT_BASE + h) as u16;
        let event_i64 = push_node(nodes, Node::bit_or(sl_hit, tp_hit));
        let event_bool = push_node(nodes, Node::lt(zero, event_i64));
        let sl_bool = push_node(nodes, Node::lt(zero, sl_hit));

        // TP is ignored when SL is also true on the same candle.
        let tp_after_sl_priority = push_node(nodes, Node::cond(sl_bool, zero, tp_hit));
        first_sl = push_node(nodes, Node::cond(event_bool, sl_hit, first_sl));
        first_tp = push_node(nodes, Node::cond(event_bool, tp_after_sl_priority, first_tp));
        bars_held = push_node(nodes, Node::cond(event_bool, ints[h + 1], bars_held));
        let reason_at_h = push_node(nodes, Node::cond(sl_bool, stop_loss, take_profit));
        exit_reason = push_node(nodes, Node::cond(event_bool, reason_at_h, exit_reason));
    }

    TradeProgramState {
        first_sl,
        first_tp,
        bars_held,
        exit_reason,
    }
}

/// Trade simulator PnL output for the current Alpha workflow.
///
/// Inputs:
/// - slots 0..5: direction-resolved stop-hit flags, packed as i64 0/1
/// - slots 6..11: direction-resolved TP-hit flags, packed as i64 0/1
/// - slot 12: signed horizon PnL before spread as f64
/// - slot 13: SL loss magnitude as f64
/// - slot 14: TP gain magnitude as f64
/// - slot 15: round-trip spread as f64
///
/// Output:
/// - `-sl_points - spread` on SL
/// - `tp_points - spread` on TP
/// - `pnl_horizon - spread` when no event occurs before horizon
pub fn trade_pnl_program() -> Program {
    let (mut nodes, ints) = trade_program_prefix();
    let state = append_first_event_logic(&mut nodes, ints);

    let first_sl_f = push_node(&mut nodes, Node::f64_from_i64(state.first_sl));
    let first_tp_f = push_node(&mut nodes, Node::f64_from_i64(state.first_tp));
    let any_event_i64 = push_node(&mut nodes, Node::bit_or(state.first_sl, state.first_tp));
    let any_event_f = push_node(&mut nodes, Node::f64_from_i64(any_event_i64));
    let one_f = push_node(&mut nodes, Node::const_f64(1));
    let no_event_f = push_node(&mut nodes, Node::f64_sub(one_f, any_event_f));

    let sl_plus_spread = push_node(
        &mut nodes,
        Node::f64_add(SL_POINTS_SLOT as u16, SPREAD_POINTS_SLOT as u16),
    );
    let sl_loss = push_node(&mut nodes, Node::f64_neg(sl_plus_spread));
    let tp_gain = push_node(
        &mut nodes,
        Node::f64_sub(TP_POINTS_SLOT as u16, SPREAD_POINTS_SLOT as u16),
    );
    let horizon_pnl = push_node(
        &mut nodes,
        Node::f64_sub(PNL_HORIZON_SLOT as u16, SPREAD_POINTS_SLOT as u16),
    );
    let sl_component = push_node(&mut nodes, Node::f64_mul(sl_loss, first_sl_f));
    let tp_component = push_node(&mut nodes, Node::f64_mul(tp_gain, first_tp_f));
    let horizon_component = push_node(&mut nodes, Node::f64_mul(horizon_pnl, no_event_f));
    let event_component = push_node(&mut nodes, Node::f64_add(sl_component, tp_component));
    let pnl = push_node(&mut nodes, Node::f64_add(event_component, horizon_component));
    push_node(&mut nodes, Node::output(pnl, Ty::F64));

    Program::new(
        Target::Cpu,
        TRADE_INPUT_ARITY_U8,
        1,
        160,
        nodes,
    )
    .expect("trade_pnl_program is well-formed")
}

/// Trade simulator bars-held output.
///
/// Returns the first hit index in `1..=6`, or `0` when no stop was hit.
pub fn trade_bars_held_program() -> Program {
    let (mut nodes, ints) = trade_program_prefix();
    let state = append_first_event_logic(&mut nodes, ints);
    push_node(&mut nodes, Node::output(state.bars_held, Ty::I64));

    Program::new(
        Target::Cpu,
        TRADE_INPUT_ARITY_U8,
        1,
        96,
        nodes,
    )
    .expect("trade_bars_held_program is well-formed")
}

/// Trade simulator exit-reason output.
///
/// Returns `1` for StopLoss, `2` for Horizon, or `3` for TakeProfit.
pub fn trade_exit_reason_program() -> Program {
    let (mut nodes, ints) = trade_program_prefix();
    let state = append_first_event_logic(&mut nodes, ints);
    push_node(&mut nodes, Node::output(state.exit_reason, Ty::I64));

    Program::new(
        Target::Cpu,
        TRADE_INPUT_ARITY_U8,
        1,
        96,
        nodes,
    )
    .expect("trade_exit_reason_program is well-formed")
}

fn cached_trade_pnl_program() -> &'static Program {
    static PROGRAM: OnceLock<Program> = OnceLock::new();
    PROGRAM.get_or_init(trade_pnl_program)
}

fn cached_trade_bars_held_program() -> &'static Program {
    static PROGRAM: OnceLock<Program> = OnceLock::new();
    PROGRAM.get_or_init(trade_bars_held_program)
}

fn cached_trade_exit_reason_program() -> &'static Program {
    static PROGRAM: OnceLock<Program> = OnceLock::new();
    PROGRAM.get_or_init(trade_exit_reason_program)
}

#[derive(Debug, Clone, Copy)]
pub struct TradeKasmOutput {
    pub pnl_points: f64,
    pub bars_held: i64,
    /// 1 = StopLoss, 2 = Horizon, 3 = TakeProfit, 0 = NotPossible (caller-set)
    pub exit_reason: i64,
}

fn pack_trade_args(
    sl_hit: [i64; TRADE_HORIZON],
    tp_hit: [i64; TRADE_HORIZON],
    pnl_horizon: f64,
    sl_points: f64,
    tp_points: f64,
    spread_points: f64,
) -> Vec<u8> {
    let mut args = Vec::with_capacity(TRADE_INPUT_ARITY * 8);
    for hit in sl_hit {
        push_i64(&mut args, hit);
    }
    for hit in tp_hit {
        push_i64(&mut args, hit);
    }
    push_f64(&mut args, pnl_horizon);
    push_f64(&mut args, sl_points);
    push_f64(&mut args, tp_points);
    push_f64(&mut args, spread_points);
    args
}

/// Run all three trade-simulator KASM programs and join the outcome.
pub fn compute_trade_kasm(
    sl_hit: [i64; TRADE_HORIZON],
    tp_hit: [i64; TRADE_HORIZON],
    pnl_horizon: f64,
    sl_points: f64,
    tp_points: f64,
    spread_points: f64,
) -> TradeKasmOutput {
    let args = pack_trade_args(
        sl_hit,
        tp_hit,
        pnl_horizon,
        sl_points,
        tp_points,
        spread_points,
    );

    let pnl_out = execute(cached_trade_pnl_program(), &args).expect("trade pnl exec");
    let pnl_points = read_f64(&pnl_out);

    let held_out = execute(cached_trade_bars_held_program(), &args).expect("trade bars exec");
    let bars_held = i64::from_le_bytes(held_out[..8].try_into().expect("8 bytes i64 out"));

    let reason_out = execute(cached_trade_exit_reason_program(), &args).expect("trade reason exec");
    let exit_reason = i64::from_le_bytes(reason_out[..8].try_into().expect("8 bytes i64 out"));

    TradeKasmOutput {
        pnl_points,
        bars_held,
        exit_reason,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_hits(
        bars: &[crate::synth_strategy::Bar],
        entry_idx: usize,
        direction: crate::synth_strategy::Direction,
        sl_level: f64,
        tp_level: f64,
        max_horizon: usize,
    ) -> ([i64; TRADE_HORIZON], [i64; TRADE_HORIZON], usize) {
        use crate::synth_strategy::Direction;

        assert!(max_horizon <= TRADE_HORIZON, "test fixture max_horizon=6");
        let end = (entry_idx + max_horizon).min(bars.len() - 1);
        let effective_h = end - entry_idx;
        let mut sl_hits = [0i64; TRADE_HORIZON];
        let mut tp_hits = [0i64; TRADE_HORIZON];

        for h in 1..=effective_h {
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

        (sl_hits, tp_hits, effective_h)
    }

    #[test]
    fn trade_kasm_matches_simulate_trade_with_tp_and_spread() {
        use crate::synth_strategy::{simulate_trade, Bar, Direction, ExitReason};

        let bars: Vec<Bar> = (0..60)
            .map(|i| {
                let drift = (i as f64 * 0.21).sin() * 0.4;
                let close = 5.0 + drift;
                Bar {
                    time_ms: 1_000_000 + (i as i64) * 14_400_000,
                    open: close - 0.005,
                    high: close + 0.07,
                    low: close - 0.07,
                    close,
                    volume: 100.0,
                }
            })
            .collect();

        let sl_points = 0.05_f64;
        let tp_points = 0.06_f64;
        let spread_points = 0.008_f64;

        for &dir in &[Direction::Long, Direction::Short] {
            for entry_idx in 5..bars.len() - TRADE_HORIZON - 1 {
                let entry_price = bars[entry_idx].close;
                let (sl_level, tp_level) = match dir {
                    Direction::Long => (entry_price - sl_points, entry_price + tp_points),
                    Direction::Short => (entry_price + sl_points, entry_price - tp_points),
                };
                let (sl_hits, tp_hits, effective_h) =
                    build_hits(&bars, entry_idx, dir, sl_level, tp_level, TRADE_HORIZON);
                let exit_close = bars[entry_idx + effective_h].close;
                let pnl_horizon = match dir {
                    Direction::Long => exit_close - entry_price,
                    Direction::Short => entry_price - exit_close,
                };

                let kasm = compute_trade_kasm(
                    sl_hits,
                    tp_hits,
                    pnl_horizon,
                    sl_points,
                    tp_points,
                    spread_points,
                );
                let rust = simulate_trade(
                    &bars,
                    entry_idx,
                    dir,
                    sl_points,
                    tp_points,
                    spread_points,
                    TRADE_HORIZON,
                );

                assert_eq!(kasm.pnl_points.to_bits(), rust.pnl_points.to_bits());

                let expected_bars_held = match rust.exit_reason {
                    ExitReason::StopLoss | ExitReason::TakeProfit => rust.bars_held as i64,
                    ExitReason::Horizon | ExitReason::NotPossible => 0,
                };
                assert_eq!(kasm.bars_held, expected_bars_held);

                let expected_reason = match rust.exit_reason {
                    ExitReason::StopLoss => 1,
                    ExitReason::Horizon => 2,
                    ExitReason::TakeProfit => 3,
                    ExitReason::NotPossible => 0,
                };
                assert_eq!(kasm.exit_reason, expected_reason);
            }
        }
    }

    #[test]
    fn trade_pnl_program_hash_is_stable() {
        let prog = trade_pnl_program();
        assert_eq!(prog.nodes().len(), 93, "pnl is a 93-node program");
        assert!(!prog.structural_hash_hex().is_empty());
    }

    #[test]
    fn trade_bars_held_program_hash_is_stable() {
        let prog = trade_bars_held_program();
        assert_eq!(prog.nodes().len(), 78, "bars_held is a 78-node program");
        assert!(!prog.structural_hash_hex().is_empty());
    }

    #[test]
    fn trade_exit_reason_program_hash_is_stable() {
        let prog = trade_exit_reason_program();
        assert_eq!(prog.nodes().len(), 78, "exit_reason is a 78-node program");
        assert!(!prog.structural_hash_hex().is_empty());
    }
}
