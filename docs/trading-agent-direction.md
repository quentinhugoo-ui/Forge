# Trading Agent Direction for Forge

This document explains the new direction for the `Trading` section of Forge and summarizes the last four user requests that drive this architecture.

## Why this direction exists

The Trading UI is no longer just a chart viewer. It is becoming an agent-aware operating surface where:

- Forge understands the full market/chart/broker/UI state locally.
- LLMs do not waste tokens re-reading raw candles or reverse-engineering the UI.
- The chat bar can operate in two distinct modes:
  - `LLM involved`
  - `Forge local trading command`

The core principle stays aligned with the Forge doctrine:

- do not recompute the same sub-computation twice
- keep heavy/raw market data on disk or in local runtime state
- exchange compact digests, proofs, snapshots, and structured commands instead of raw bulk payloads

## The last four requests this document covers

### 1. Add an on/off involvement control for Trading chat

Requested direction:

- the Trading chat needs an explicit `on/off` control for LLM involvement
- if LLMs are `off`, a message is not treated as a CLI/LLM request
- instead, the message becomes a local Forge trading command

Current implementation direction:

- a single Trading-only switch controls whether the LLM layer is involved
- when `off`, chat is routed to a local Trading command router
- when `on`, the selected LLM targets receive an auto-generated Trading digest

### 2. Let Codex / Gemini / Claude write code and tools inside Forge

Requested direction:

- CLI Codex, Gemini, and Claude are code-capable
- Forge should eventually let them create or extend tools, indicators, replay helpers, alerts, and UX pieces from inside the product

Architecture consequence:

- Forge needs a stable agent-facing extension layer
- this extension layer must sit on top of reusable templates, manifests, and strict conventions
- agents should not free-write arbitrary boilerplate every time

### 3. Frontend and backend must react cleanly to agent-written code

Requested direction:

- the frontend and backend must be able to respond to tools/features written by agents
- the language and integration model must be understandable for LLMs
- KASM must remain usable as a domain layer, not as a burden

Recommended stack direction:

- `Rust` for the Trading/Forge core engine
- `TypeScript/JavaScript` for the agent-friendly UI/runtime extension layer
- `KASM` as a high-level intent DSL, not the place where large repetitive code is authored

### 4. Reduce token cost with templates and Forge discipline

Requested direction:

- coding through agents can become token-expensive
- Forge should reduce this cost
- Forge should prevent repeated code writing when the pattern already exists

Architecture consequence:

- Forge must inject templates, manifests, coding contracts, and local context digests
- Forge must act as an anti-duplication layer
- agents should patch narrow extension points instead of re-authoring the same systems

## Trading-specific architecture target

The Trading section should evolve around three central building blocks.

### 1. `TradingContextSnapshot`

This is the local source of truth for the Trading surface.

It should capture, at minimum:

- selected broker
- effective broker label or `PAPERTRADING`
- selected instrument
- selected timeframe
- comparison instruments
- chart mode
- axis metric modes
- viewport state
- replay state
- indicators state
- alerts state
- open/pending positions and orders
- price state
- whether the chart is in 2D or 3D

This snapshot must be built locally and reused, not recomputed from scratch by the LLM.

### 2. `TradingContextDigest`

This is the compact summary sent to LLMs when the Trading switch is `on`.

It should contain:

- broker / instrument / timeframe
- comparison set
- chart mode and axis settings
- price summary
- candle summary
- alert summary
- order/position summary
- replay / indicators state
- view mode (2D/3D)

It should *not* dump raw full-history candles into prompt context by default.

### 3. `TradingCommandRouter`

When LLM involvement is `off`, chat input should be interpreted locally as Forge Trading intent.

Examples:

- switch timeframe
- load another asset
- add/remove compare symbols
- open alert panel
- toggle replay
- stage an order draft locally

This router is the first step toward a stronger local Trading DSL.

## Immediate implementation scope

The first implementation phase for Trading should focus on:

1. a Trading-only LLM involvement switch in the chat bar
2. a local `TradingContextSnapshot`
3. a compact `TradingContextDigest`
4. a `TradingCommandRouter`
5. automatic Trading digest injection into LLM requests

The first follow-up phase after that should focus on:

1. `ctrl + click` semantic token injection
2. wider Trading UI coverage for token injection
3. stronger local Trading command parsing
4. richer indicator / replay / alert awareness inside the digest

## Current implementation status

The first two Trading foundation steps are now active in the codebase.

### Step 1 is active

- one single `ON/OFF` master switch controls LLM involvement for the whole Trading chat
- when `OFF`, chat routes to the local Trading command router
- when `OFF`, model/provider UI noise is hidden from the Trading chat area

### Step 2 is active

`TradingContextSnapshot` is no longer a thin chart-only summary. It now carries a broader canonical Trading state, including:

- broker selection and effective broker label
- broker tradable universe awareness
- selected instrument and timeframe
- comparison instruments with tradable/library distinction
- chart display mode, axes, and viewport state
- Trading UI state like header menus, right panel, and timeframe rail visibility
- price and candle summaries
- account summary
- pending/open order summaries
- alert summaries
- catalog/history coverage information

This snapshot is also cached behind a deterministic key so Forge does not rebuild the same Trading sub-context when nothing meaningful changed.

### Step 3 is active

`TradingContextDigest` now goes beyond a plain state dump and starts acting like a local analysis digest.

Forge now enriches the LLM-facing Trading digest with:

- book depth summary
- order draft summary
- indicator panel state
- replay state
- locally synthesized signal hints from candles
- compare-strength summaries against compared instruments

This means the LLM can receive a more serious market briefing without rereading raw chart history or reconstructing basic structure and momentum from scratch.

### Step 4 is active

`TradingCommandRouter` is now substantially broader in local command mode.

It can now understand and route, locally:

- broker switching
- instrument and timeframe changes
- compare add / remove / clear
- chart display mode changes
- X axis and Y axis metric changes
- timezone changes
- session break and signal marker toggles
- scale and grid visibility toggles
- right panel routing toward `console` or `orders`
- safer local order-draft staging from plain language

### Step 5 is active

Trading requests sent while the master switch is `ON` no longer receive a raw digest block only.

Forge now injects a more disciplined Trading context packet with:

- a protocol/version header
- runtime targeting
- session awareness
- a stable digest hash
- a compact `unchanged` mode when the same runtime already received the same snapshot in the same session
- a focus summary derived from the current user request
- explicit directives telling the LLM to rely on Forge context instead of re-deriving the chart

### Step 6 is active

`Ctrl + click` semantic injection now covers a much larger portion of the Trading UI.

It now includes, beyond the first basic layer:

- compare browser group headers and asset entries
- broker / asset / display flyout menu entries
- compare search entry point
- chart-mode trigger
- right-panel trigger
- order-entry controls and order actions
- alert modal actions and alert form controls

This makes Trading chat composition much more direct because the user can capture UI semantics into structured tokens instead of retyping them manually.

## Current UX simplification

The Trading chat should use:

- one single `ON/OFF` master switch for all LLM involvement
- not one switch per provider

This keeps the UI cleaner and makes the mode boundary obvious:

- `ON` = LLM analysis mode
- `OFF` = Forge local Trading command mode

## First semantic token layer

The first token-injection layer should target the highest-value elements first:

- broker header
- current instrument header
- compare trigger
- timeframe rail
- left-panel assets
- chat-bar Trading actions like `Indicators`, `Replay`, and `Alert`

Example injected tokens:

- `<broker:OANDA>`
- `<instrument:NATGASUSD>`
- `<compare:BTCUSD,XAUUSD>`
- `<timeframe:H4>`
- `<indicators:on>`
- `<replay:on>`
- `<alert:panel>`

This phase is intentionally smaller than the full vision.

It does **not** yet mean that:

- every UI element is ctrl-click injectable
- every indicator parameter is fully serialized
- every candle is exported into prompt context
- every broker action is safely executable by free-form command

Those come later, once the foundation is stable.

## Why Forge should not send the full raw chart to the LLM

Even if the long-term goal is “the LLM knows absolutely everything”, the implementation should remain disciplined.

Forge should:

- keep raw time series local
- compute structured chart summaries locally
- compute interaction summaries locally
- compute signal summaries locally
- send only compact, high-value context to the LLM

This keeps:

- token cost under control
- behavior deterministic
- latency lower
- prompts stable across turns

## Next serious milestones after this phase

After the first Trading foundation is in place, the next robust milestones are:

### `ctrl + click` semantic injection

Every major Trading UI element should be able to inject a structured token into chat, such as:

- `<instrument:NATGASUSD>`
- `<timeframe:H4>`
- `<compare:XAUUSD>`
- `<alert:price_cross>`
- `<order:pending>`
- `<axis:x=time>`
- `<view:3d>`

### Template-driven agent extensions

Forge should give agents:

- templates
- manifests
- allowed write scopes
- reusable component registries

This avoids repeated code authoring and enforces the Forge doctrine.

### KASM intent pipeline

KASM should express Trading intent, while Forge and templates generate most of the repeatable implementation skeleton.

## Summary

The new direction for Trading is:

- Forge understands the chart locally
- LLMs receive a compact synthesized digest instead of raw chart bulk
- a Trading-only switch decides between `LLM analysis mode` and `Forge local command mode`
- this foundation prepares the way for agent-written indicators, replay tools, alerts, and richer semantic UI injection

This is the cleanest route to make Trading serious, fast, and scalable without violating the Forge anti-duplication doctrine.
