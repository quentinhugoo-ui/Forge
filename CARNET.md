# CARNET

Compact decision log for Forge. Do not append transcripts, brainstorm dumps or long historical plans here. Keep entries short enough that an agent can read the whole file.

## 2026-05-14 - GitHub backup

- Canonical remote: https://github.com/quentinhugoo-ui/Forge.git
- GitHub `master` uses a clean snapshot history.
- Old local history with oversized blobs is preserved locally as `archive/master-large-history-before-github-20260514`.
- Reason: GitHub rejected historical `lab_findings.jsonl` blobs over 100 MB and `.pixi-home/bin/pixi.exe` warnings.

## 2026-05-14 - Data protection

- `C:\Users\quent\Documents\EVE\MAP` is explicitly protected.
- EVE Online client/cache folders removed: `C:\Users\quent\Documents\EVE\tq`, `C:\Users\quent\Documents\EVE\ResFiles`, `C:\Users\quent\AppData\Local\CCP`.
- Build/cache cleanup must use resolved path guards before recursive deletion.

## 2026-05-14 - Brain/memory/Godel

- `src/brain.rs` is the compact brain core.
- Memory records must preserve scope, layer, trust and evidence/proof anchors.
- Godel substitutions are accepted only after strict semantic verification.
- Tauri/MCP tools expose the brain to agents through `forge_agent_tools.rs` and `forge_mcp.rs`.

## 2026-05-14 - Docs compaction

- Active docs were reduced to a small set: `AGENTS.md`, `README.md`, `CLAUDE.md`, `ROADMAP.md`, `TOOLS&SANDBOX.md`.
- Historical detail stays in Git, not in the live context.
- New rule: if documentation becomes too noisy, compress it immediately instead of adding another document.

## Permanent Decisions

- Prefer circuit courts over pipelines.
- Prefer proof/artifact references over raw large data in prompts.
- Prefer one memory/Godel path over duplicated agent-specific memory stores.
- Prefer deleting obsolete docs/code paths over documenting around them.
- Before writing code beyond a purely mechanical one-line edit, research the freshest market/state-of-the-art techniques available, then implement the shortest locally verified circuit that can put Forge ahead of that state.
- Every code change should reduce complexity: fewer middlemen, fewer duplicated functions, fewer branches, fewer runtime steps.
