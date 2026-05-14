# Forge Tools and Sandboxes

This document is the working base for harmonizing Forge tools, slash commands,
LLM actions, MCP routing, and sandbox-based tool creation.

## Core Principle

Forge should expose one shared action language everywhere:

- `/program_` for reusable executable Instruments.
- `/metric` for reusable metric or semantic Nodes.
- `/create_` for creating a new reusable Instrument or Lens.
- `/visualprogram_` for runnable reusable visual Lenses.
- `/geo` and `/minigeo` for spatial anchors.
- `/transcript_` for live transcript capture and transcript-backed context.
- `/strategy_` for reusable trading strategy programs.
- `/indicator` for reusable indicator metrics or overlays.
- `/alert_` for reusable alert and monitor definitions.

Naming convention:

- Canonical commands ending in `_` are runnable programs/actions.
- Commands without a trailing `_` are metrics, nodes, anchors, or composable
  entities used inside programs.
- The rule is about the final trailing underscore only. Internal underscores are
  allowed inside metric tokens, for example `/candleh4_11am`.
- `/geo` and `/minigeo` are special spatial metric nodes. They are meaningful
  inside Planet/visual programs, not as standalone runnable actions.
- `/connect_` for secure account or provider connection flows.
- `/gmail_` for Gmail actions.

Internal names such as `forge_run_program` or MCP `run` can remain inside the
backend, but they should not be the language shown to users or used in visible
agent reasoning. The canonical product language is the slash-command language.

## LLM vs Forge Responsibilities

The LLM is the semantic interpreter.

It understands natural language, metaphors, ambiguity, and user intent. For
example, "clean up Sophie's emails" may mean archive, delete, summarize, filter,
or find important messages.

Forge is the action system.

Forge does not guess subtle natural language. Forge receives structured intent,
validates it, applies safety rules, executes local tools, hashes artifacts,
updates Atlas, and enforces confirmation for risky actions.

The ideal flow is:

1. User writes natural language with a slash command.
2. LLM translates the request into a strict `ForgeIntent`.
3. Forge validates the intent schema.
4. Forge blocks, clarifies, plans, or executes according to risk.
5. Results are returned as compact refs, candidates, hashes, artifacts, and
   proof summaries, not raw user data.

Example structured intent:

```json
{
  "command": "/gmail_",
  "intent": "delete_messages",
  "confidence": 0.93,
  "filters": {
    "from_label": "Sophie",
    "contains": "reunion annulee"
  },
  "risk": "destructive",
  "requires_confirmation": true
}
```

Forge must not execute destructive or external-impact actions directly from a
single natural-language interpretation. It must first show a plan and require
explicit confirmation.

## Content-Addressed OAuth Proofs

Provider connection flows such as `/connect_` must save a compact
content-addressed proof ledger. The ledger stores `kasm://sha256/...` hashes for
the OAuth channel, requested scope set, callback state, profile identity, and
success/failure transitions.

The ledger must never store credentials, passwords, OAuth authorization codes,
PKCE verifiers, access tokens, refresh tokens, full auth URLs, email addresses,
or raw profile pictures. Those values live only in the browser/keyring path
required for the provider flow. Forge can prove the flow happened and avoid
duplicating identical channel/setup work by comparing saved hashes, without
making the LLM or UI able to read the secrets.

## Global KASM Ledger Bus

Every heavy or verifiable app feature should emit compact actions through the
global KASM ledger bus:

```text
feature event -> canonical JSON -> payload hash -> cache/index lookup
             -> append proof entry only on miss -> return compact hashes
```

The first shared bus is the UI proof action stream. Features that already call
`appendForgeProofAction(...)` automatically get persisted hashes under the
`ui-proof` namespace. This covers WebExplorer navigation, DOM commits, auth
state changes, analysis snapshots, webview presentation, and future `/program_`
or `/visualprogram_` actions that report through the same proof stream.

Rules for new features:

- hash canonical inputs before expensive work,
- check the saved hash/index before recomputing,
- save compact summaries and hashes, not raw user data,
- include validity context such as account hash, URL/domain hash, timeframe,
  scope set, program hash, indicator params, and provider permissions,
- keep UI animation/hover/transient state out of the ledger unless it changes
  a real action or artifact.

## `/program_` Cache Contract

`/program_` runs are cacheable when the canonical run inputs are identical:

- program hash or inline program hash,
- title hash when the title changes the job identity,
- input file content hashes and roles,
- dry-run / plan-only flags.

The cache stores the compact tool response and result hashes under the
`program-run` namespace. It must not store raw source files in the ledger.
Returning a cached job/result is valid because the content hash proves the same
program was run on the same input bytes.

## `/gmail_` Cache Contract

`/gmail_` must use the same content-addressed pattern, but with stricter privacy
rules. It may cache:

- account hash,
- OAuth scope hash,
- natural-language intent hash,
- Gmail query/filter hash,
- thread/message id hashes,
- label hashes,
- result count,
- action plan hash,
- confirmation hash for destructive actions.

It must not cache raw email addresses, message bodies, subjects, attachments,
contacts, password material, OAuth tokens, or full Gmail URLs. For previews, use
short snippets only when the user explicitly asks to view them in the UI; the
ledger receives only their hashes.

Destructive operations such as delete, archive, send, label mutation, or filter
creation must always create a `requires_confirmation=true` plan first. The
confirmed execution receives a second hash entry linked to the plan hash.

`/gmail_` has two execution rails, not one:

- Google API rail: preferred for structured, permissioned operations when API
  scope and validation are clear.
- Gmail WebView rail: preferred when the user wants the real Gmail interface, or
  when the action depends on visual context, account-specific UI state, prompts,
  verification screens, buttons, tabs, rows, menus, or browser-only flows.

The WebView rail turns the current Gmail DOM memory into runnable targets. Every
clickable row, button, tab, textbox, checkbox, menu item, contact, message row,
thread, snippet, and visible action receives content-addressed refs:

- `surface_hash` for the whole current Gmail page/action surface,
- `dom_hash` or `tree_hash` for the captured DOM state,
- `target_hash` for a runnable UI target,
- `label_hash` for visible text/name/value,
- `selector_hash` for the selector hint,
- `href_hash` for links,
- optional bounds and role metadata.

The LLM should not receive raw mailbox content by default. It receives the
intent, the available target hashes, compact semantic kinds, and proof refs. If
the user asks to inspect mail content, Forge can show it in the UI, but the
ledger still stores only hashes and small counts. The agent can then request
actions such as `click target_hash`, `type into target_hash`, `open thread
target_hash`, or `confirm destructive plan_hash`.

This keeps the real human-like Gmail navigation path while still making every
step auditable, repeatable, and cacheable.

## Command Registry

Forge should have a single central command registry used by:

- chatbar tokens and suggestions,
- My Atlas,
- WebExplorer subbars,
- MCP/dynamic tool descriptions,
- LLM prompts,
- tool events,
- logs and proof summaries.

Example registry entry:

```json
{
  "token": "/program_vwap_stress",
  "kind": "program",
  "label": "VWAP Stress",
  "internal_tool": "forge_run_program",
  "internal_route": "mcp.run",
  "args": {
    "program_hash": "..."
  },
  "risk": "compute",
  "requires_user_launch_approval": true
}
```

The registry maps public slash commands to internal execution routes. The slash
token is the public identity. The internal tool name is plumbing.

## Temporary Context Tokens

Forge also has temporary slash tokens generated by an active UI surface.

These are not permanent Atlas tools. They are session-scoped context metrics
created by direct selection, brushing, hovering, or clicking inside a view. In
trading, selecting H4 candles near the lower Bollinger band can create tokens
such as:

- `/candleh4_11am`
- `/bollinger_h4_20_2_close_lower_band`

The same rule applies to every chart timeframe Forge supports:

- `w1`
- `d1`
- `h4`
- `h1`
- `m30`
- `m15`
- `m5`
- `m1`
- `s30`
- `s10`

Indicator context tokens include the indicator, timeframe, relevant parameters,
and the displayed plot metric. The current trading set covers:

- `/vwap_{tf}_{anchor}_{source}`
- `/ema_{tf}_{length}_{source}`
- `/sma_{tf}_{length}_{source}`
- `/wma_{tf}_{length}_{source}`
- `/hma_{tf}_{length}_{source}`
- `/vwma_{tf}_{length}_{source}`
- `/bollinger_{tf}_{length}_{deviation}_{source}_{basis|upper_band|lower_band|cloud}`
- `/donchian_{tf}_{length}_{upper_band|lower_band|cloud}`
- `/keltner_{tf}_{length}_{multiplier}_{source}_{basis|upper_band|lower_band|cloud}`
- `/supertrend_{tf}_{atrLength}_{multiplier}_{bull|bear}`
- `/ichimoku_{tf}_{conversion}_{base}_{spanB}_{displacement}_{tenkan|kijun|span_a|span_b|cloud}`
- `/psar_{tf}_{step}_{max}_sar`

The visible label may use natural spacing, for example "Bollinger lower band",
but the token itself should stay machine-safe and space-free. Temporary tokens
expire with the current chart/session/timeframe unless the user or agent
promotes them through `/create_`.

Temporary token payloads should stay compact:

- token,
- kind and family,
- source surface,
- chart or view id,
- timeframe,
- selected candle ids or time range,
- indicator id and plot name,
- proximity rule or selection bounds,
- proof refs or screenshot bounds,
- small numeric preview.

Raw rows, full candles, and heavy datasets stay local. The LLM receives the
token plus a compact context packet, then asks Forge to resolve or analyze that
token when needed. This lets the agent reason case by case without paying tokens
for the full chart state.

## MCP Harmonization

MCP should speak Forge's slash-command language at the visible layer.

Good visible language:

- `/program_vwap_stress`
- `/create_`
- `/metric_volatility`
- `/visualprogram_market_cloud_`
- `/gmail_`

Avoid visible language:

- `forge_run_program`
- `forge_create_program`
- `mcp.run`
- `tools/call`

The technical MCP tool names may remain stable internally for compatibility, but
every result, event, prompt, and UI label should include the canonical command:

```json
{
  "display_command": "/program_vwap_stress",
  "canonical_action": "/program_",
  "internal_tool": "forge_run_program",
  "status": "running"
}
```

Long-term, Forge can add a universal command entrypoint:

```json
{
  "tool": "forge_command",
  "command": "/program_vwap_stress",
  "mode": "plan | run | inspect | repair",
  "args": {}
}
```

This lets LLMs reason in the same language as the user, while Forge still routes
to MCP, Atlas, local executors, and proof systems internally.

## Normal Tool Flow vs Sandbox Flow

There are two different action modes.

### Stable Slash Tools

Use existing slash tools when the capability already exists.

Examples:

- `/gmail_` searches Gmail.
- `/program_vwap_stress` runs an existing Instrument.
- `/metric_volatility` refers to an existing Node.
- `/visualprogram_3d_market_map_` materializes an existing Lens.

This is the fastest, cheapest, and most reliable path. The LLM receives compact
candidate results or tool summaries, not raw files or raw mailbox content.

### Sandbox Creation

Use sandboxes only when Forge needs to create or modify a reusable capability.

Examples:

- The user asks for a new Gmail workflow that does not exist yet.
- The user wants a custom parser, metric, simulator, classifier, or visual Lens.
- Existing tools are insufficient and a reusable Instrument should be created.

The sandbox is the workshop. `/program_` is the forged tool.

## Prefilled Sandboxes

Sandbox windows should not ask the LLM to write everything from scratch.

They should open with locked, tested, typed templates. The LLM fills only the
empty slots.

Example TypeScript-style template:

```ts
export const manifest = {
  command: "/program_{{name}}",
  base: "/gmail_",
  inputs: ["gmail.messages"],
  outputs: ["summary", "artifact"],
  permissions: ["gmail.readonly"],
  network: false,
  destructive: false
}

export async function run(ctx) {
  const messages = await ctx.gmail.search({
    // AGENT_FILL: query
  })

  const extracted = messages.map((message) => {
    // AGENT_FILL: extraction_logic
  })

  return ctx.artifact("result.json", extracted)
}
```

Locked sections:

- permissions,
- secret handling,
- input loading,
- artifact writing,
- cache/hash/proof plumbing,
- logging limits,
- destructive-action gates.

Editable sections:

- query logic,
- metric formula,
- extraction logic,
- classification labels,
- visual mapping recipe,
- validation checks.

This reduces token cost, improves code quality, and keeps security boundaries
inside Forge instead of relying on the LLM to rebuild them correctly.

## Gmail Flow

`/gmail_` should not open a sandbox for normal Gmail tasks.

### Step 1: Command Activation

As soon as `/gmail_` is committed in the chatbar, Forge activates Gmail mode
before the user finishes typing the prompt.

If no Google account is connected:

1. Forge blocks `/gmail_`.
2. Forge explains that the user is not connected.
3. Forge injects `/connect_`.
4. The secure connection flow starts.

If the account is connected:

1. Forge opens or prewarms Gmail / the Gmail connector.
2. Forge switches to read-only presearch mode.
3. The user continues typing the prompt.

### Step 2: Incremental Presearch

While the user types, Forge can extract candidate entities and topics without
calling the LLM for every keystroke.

Example prompt:

`/gmail_ | retrouve moi le mail de Sophie ou elle parle de vacances au Portugal`

Candidate chips:

- `/person_sophie`
- `/topic_vacances`
- `/topic_portugal`

Important privacy rule: Forge must not store Sophie's email address in Atlas or
in persistent cleartext. It may store a local display label, an opaque Gmail id,
a local hash, or encrypted cache metadata where appropriate.

When the user confirms chips, Forge recomputes search combinations before send:

- Sophie + vacances
- Sophie + Portugal
- vacances + Portugal
- Sophie + vacances + Portugal

This should be debounced, cached, and bounded.

### Step 3: Prompt Send

When the user sends the prompt, the LLM receives a compact Gmail action packet,
not a sandbox and not raw mailbox content.

Example:

```json
{
  "command": "/gmail_",
  "draft": "retrouve moi le mail de Sophie ou elle parle de vacances au Portugal",
  "confirmed_chips": ["/person_sophie", "/topic_vacances", "/topic_portugal"],
  "candidate_messages": [
    {
      "id": "msg_opaque_1",
      "date": "2026-05-10",
      "from_label": "Sophie",
      "snippet": "Portugal ... vacances ...",
      "score": 0.91
    }
  ],
  "allowed_actions": ["search", "read", "summarize", "open"],
  "blocked_until_confirmation": ["delete", "send", "forward"]
}
```

The LLM then chooses the most likely result, asks for clarification, or returns a
safe next action.

### Step 4: Risk Gates

Read/search/summarize can proceed when authorized.

Delete, send, forward, share, unsubscribe, or external-impact actions require:

1. plan-only execution,
2. visible candidate list,
3. exact action summary,
4. explicit user confirmation,
5. final execution by Forge.

### Step 5: Sandbox Escalation

A sandbox opens only if `/gmail_` cannot satisfy the task with existing tools.

Examples:

- build a reusable travel-mail audit workflow,
- extract invoices and detect duplicates,
- classify administrative emails into a custom taxonomy,
- create recurring cleanup rules,
- generate a reusable report pipeline.

Then Forge opens `/create_` with a prefilled Gmail template. If validated, the
new workflow becomes a reusable `/program_...` in Atlas.

## Token and Latency Strategy

The cheapest and fastest path is:

1. Slash command activates domain mode early.
2. Forge presearches and caches candidates locally.
3. User confirms chips before send.
4. LLM receives compact candidates and strict schemas.
5. Forge executes only validated structured intent.

Avoid:

- sending raw Gmail/mailbox content to the LLM,
- sending full generated code for normal actions,
- opening a sandbox for every `/gmail_` request,
- asking the LLM to repeatedly rediscover available tools,
- exposing internal tool names as the visible action language.

## Security Rules

Credentials:

- never visible to Forge,
- never sent to the LLM,
- never stored by Forge,
- entered only through secure `/connect_` flows.

Mailbox identifiers:

- no email addresses persisted in cleartext,
- prefer opaque ids, display labels, local hashes, and encrypted local cache,
- snippets must be bounded and task-relevant.

Destructive actions:

- never direct from natural language,
- always plan-only first,
- require explicit confirmation,
- final execution handled by Forge, not freeform LLM code.

Sandbox execution:

- no network by default,
- no raw secrets,
- only session refs and approved connectors,
- bounded logs,
- hash all artifacts,
- promote to Atlas only after validation.

## Sealed Web Actions

Goal: protect sensitive web actions from Forge, the LLM, logs, screenshots, and
normal DOM capture while still allowing proof that the action protocol was
followed.

The strongest practical model is not "Forge encrypts the password". It is
"Forge never receives the password".

For `/connect_`:

1. Forge opens the official provider auth page in an isolated native WebView.
2. Forge does not inject initialization scripts on provider auth hosts such as
   `accounts.google.com`.
3. Forge skips DOM capture, screenshot capture, prompt capture, and LLM routing
   on those auth hosts.
4. The user types credentials only inside the provider page.
5. Forge records a content-addressed proof event: `secret.not_collected`.
6. After the provider leaves the auth host, normal WebExplorer capture can resume
   for non-secret pages.

For future high-risk web actions, use a "sealed action channel":

- The LLM emits only a structured intent, for example `delete_messages`.
- Forge converts the intent to a deterministic action plan.
- Any sensitive user input is handled in an isolated WebView or OS credential
  prompt, never in the chatbar.
- Action payloads are committed as hashes before execution.
- The WebView executor signs each step into a hash chain.
- The Verification terminal displays only action labels, hosts, timestamps,
  bounds, consent state, and hashes.
- Secrets are represented as `secret.not_collected`, `secret.user_typed_remote`,
  or `secret.os_keychain_ref`, never as values.

Network traffic to normal HTTPS sites is already encrypted by TLS. The hard
part is local exposure: scripts, logs, screenshots, browser extensions, injected
helpers, debug probes, and compromised OS processes. Forge can reduce its own
access by isolating WebViews and avoiding capture, but it cannot prove absolute
inviolability against malware or an already-compromised operating system.

## Product Direction

Forge should become a system where agents do not merely call tools. They speak
Forge's slash language, compose capabilities, and create new reusable tools only
when needed.

The target mental model:

- The chatbar is the command line.
- Atlas is the memory of reusable tools and semantic Nodes.
- MCP is the routing layer.
- Stable slash commands are the production tools.
- Temporary slash tokens are per-session context handles created by the UI.
- Sandboxes are workshops for creating the next tools.
