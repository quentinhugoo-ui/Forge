# CLAUDE.md

This is the compact agent protocol for Forge. It replaces the old long session doctrine. Keep this file short; if it grows past roughly 150 lines, compress it.

## Source Of Truth

- GitHub backup: https://github.com/quentinhugoo-ui/Forge.git
- Active docs: `AGENTS.md`, `README.md`, `ROADMAP.md`, `TOOLS&SANDBOX.md`, `CARNET.md`.
- Active code beats old notes. When docs disagree with code, inspect code and update docs.

## Protected Zones

Never delete or overwrite these without explicit user approval and a resolved absolute path check:

- `C:\Users\quent\Documents\EVE\MAP`
- `.git/` and Git refs
- user-untracked work not created by the current task
- secrets, tokens and local credentials
- large local research/data stores unless the user names the exact target

Build/cache folders may be cleaned only with narrow guards:

- `target/`, `target-msvc-tests/`
- `.vs/`
- `.codex-tmp/`
- generated Tauri targets

## Work Style

1. Read the local shape first with `rg`, `git status`, small file previews and targeted code search.
2. Prefer editing the smallest set of files that solves the request.
3. Remove duplicate paths before adding new architecture.
4. Keep docs smaller after your change, not larger.
5. Use content-addressed references, hashes and proof summaries for large artifacts.
6. Preserve unrelated user changes.
7. Verify with the narrowest meaningful command, then broaden if risk requires it.

## Forge MCP Doctrine

For large files, repeated or expensive computation, scientific/numerical/data/code/document-heavy analysis, custom metrics, or verifiable hashes/proofs, use Forge/MCP style work before raw LLM reading. Keep raw data on disk and exchange compact manifests, previews, artifacts and proofs.

If the MCP runner is unavailable, imitate the discipline locally:

- summarize with line counts, hashes and targeted `rg`,
- avoid dumping huge files into context,
- produce compact outputs that can be verified.

## Brain, Memory And Godel

The brain/memory layer must stay evidence-aware:

- semantic notes need scope, layer, trust score and evidence/proof hash when possible,
- unverified LLM memory stays marked as unverified,
- newer facts should supersede older facts by stable keys,
- Godel substitution must pass strict semantic verification before use,
- no external model/backend becomes trusted just because it produced plausible text.

Current core files:

- `src/brain.rs`
- `src/godel/**`
- `src/apply.rs`
- `src/monster/exec.rs`
- `src/monster/dispatch.rs`
- `examples/forge_tauri_ui/src-tauri/src/forge_agent_tools.rs`
- `examples/forge_tauri_ui/src-tauri/src/bin/forge_mcp.rs`

## UI Discipline

The Tauri UI is already large. Do not add a new UI path if an existing section/registry/bridge can carry the feature.

Current UI coordination files:

- `examples/forge_tauri_ui/ui/forge-section-registry.js`
- `examples/forge_tauri_ui/ui/forge-tauri-bridge.js`
- `examples/forge_tauri_ui/ui/forge-boot.js`
- `examples/forge_tauri_ui/ui/SECTION_OWNERSHIP.json`
- `examples/forge_tauri_ui/ui/SECTION_CONTRACT.md`

## Verification Commands

```powershell
cargo check --lib --tests
cargo test brain --lib
cargo check --manifest-path examples\forge_tauri_ui\src-tauri\Cargo.toml
cargo check --manifest-path examples\forge_tauri_ui\src-tauri\Cargo.toml --bin forge_mcp
```

Use the MSVC developer command prompt if Windows linking needs `link.exe`.

## Git Safety

Before risky cleanup:

```powershell
git status --short --branch
git diff --stat
git push
```

Do not push huge historical blobs. The current GitHub remote uses a clean snapshot history because the old local history had files over GitHub's 100 MB limit.
