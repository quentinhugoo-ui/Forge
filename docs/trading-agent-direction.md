# Trading Agent Direction

Trading is a Forge compute surface, not a separate app.

## Goal

An agent should be able to inspect market context, request compact metrics, propose a strategy, backtest it, explain the evidence, and ask for explicit approval before any live action.

## Current Files

- Backend: `examples/forge_tauri_ui/src-tauri/src/trading.rs`
- Strategy synthesis: `examples/forge_tauri_ui/src-tauri/src/synth_strategy.rs`
- Indicators: `examples/forge_tauri_ui/src-tauri/src/kasm_indicators.rs`
- Pressure/3D view: `examples/forge_tauri_ui/src-tauri/src/trading_pressure.rs`
- UI: `examples/forge_tauri_ui/ui/trading.js`, `examples/forge_tauri_ui/ui/app.js`, `examples/forge_tauri_ui/ui/styles.css`
- Lab runner: `examples/lab_runner_trading.rs`
- Native market/news panel: Bloomberg live commands are section-gated through the shared Tauri bridge.

## Rules

- Market datasets and credentials stay outside Git.
- The LLM should receive digests, not full raw chart dumps.
- Any backtest should be reproducible from symbol, timeframe, provider, config and data hash.
- Live order placement requires explicit user approval.
- Missing data must be reported as missing, not silently filled.

## Agent Context Shape

Use compact snapshots:

```text
provider, account mode, instrument, granularity, time range,
last price, spread, candle count, missing ranges,
active strategy config, latest backtest metrics, risk flags
```

Use semantic tokens for chart state:

```text
trend, volatility regime, breakout/compression, liquidity gap,
indicator agreement, signal age, drawdown, exposure
```

## Near Work

- Keep the trading chat toggle explicit.
- Route agent-written metrics through the existing Forge/Tauri bridge.
- Cache metric outputs by content/config hash.
- Preserve chart split/overlay state through the existing UI bridge rather than a parallel store.
- Keep split-chart menus anchored through the existing topbar/menu system, not a second dropdown implementation.
- Keep proof summaries short enough to send to any LLM.
