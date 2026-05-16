# Real-Estate Public Source Registry

This folder seeds the real-estate mega harvester with official public/API sources.

Current phase:

1. discover official catalogs and datasets;
2. validate URLs, formats and parser coverage;
3. download only bounded previews for live audits;
4. later promote validated sources into continuous collectors.

Commands:

```powershell
node examples\forge_tauri_ui\scripts\real-estate-source-pipeline.mjs --pretty --store=examples\forge_tauri_ui\.tmp-source-discovery --download-limit=12 --parser-limit=20
node examples\forge_tauri_ui\scripts\real-estate-source-pipeline.mjs --live --pretty --store=C:\tmp\forge-real-estate-source-pipeline --download-limit=12 --parser-limit=20 --timeout-ms=8000 --max-bytes=1048576

node examples\forge_tauri_ui\scripts\real-estate-source-audit.mjs --pretty
node examples\forge_tauri_ui\scripts\real-estate-source-audit.mjs --live --output=C:\tmp\real-estate-source-audit-live.json --timeout-ms=8000 --max-bytes=32768
node examples\forge_tauri_ui\scripts\real-estate-source-discovery.mjs --pretty --store=C:\tmp\forge-real-estate-source-discovery
node examples\forge_tauri_ui\scripts\real-estate-source-discovery.mjs --live --pretty --store=C:\tmp\forge-real-estate-source-discovery --timeout-ms=8000 --max-bytes=524288
node examples\forge_tauri_ui\scripts\real-estate-raw-downloader.mjs --pretty --manifest=C:\tmp\forge-real-estate-source-discovery\real-estate-harvester\data\source_manifest.jsonl --limit=20
node examples\forge_tauri_ui\scripts\real-estate-raw-downloader.mjs --live --pretty --manifest=C:\tmp\forge-real-estate-source-discovery\real-estate-harvester\data\source_manifest.jsonl --store=C:\tmp\forge-real-estate-source-discovery --limit=20 --max-bytes=1048576
node examples\forge_tauri_ui\scripts\real-estate-parser-router.mjs --pretty --downloads=C:\tmp\forge-real-estate-source-discovery\real-estate-harvester\data\raw_downloads.jsonl --store=C:\tmp\forge-real-estate-source-discovery --adapters=examples\forge_tauri_ui\source-registry\real-estate-parser-adapters.json --limit=20
node examples\forge_tauri_ui\scripts\real-estate-entity-resolver.mjs --pretty --events=C:\tmp\forge-real-estate-source-discovery\real-estate-harvester\data\normalized_events.jsonl --store=C:\tmp\forge-real-estate-source-discovery --limit=20
node examples\forge_tauri_ui\scripts\real-estate-intel-pack-builder.mjs --pretty --graph=C:\tmp\forge-real-estate-source-discovery\real-estate-harvester\data\entity_graph.jsonl --events=C:\tmp\forge-real-estate-source-discovery\real-estate-harvester\data\normalized_events.jsonl --store=C:\tmp\forge-real-estate-source-discovery --limit=20
node examples\forge_tauri_ui\scripts\real-estate-kasm-seed-builder.mjs --pretty --packs=C:\tmp\forge-real-estate-source-discovery\real-estate-harvester\data\intel_packs.jsonl --store=C:\tmp\forge-real-estate-source-discovery --limit=20
node examples\forge_tauri_ui\scripts\real-estate-kasm-simulator.mjs --pretty --seeds=C:\tmp\forge-real-estate-source-discovery\real-estate-harvester\data\kasm_metric_seeds.jsonl --store=C:\tmp\forge-real-estate-source-discovery --limit=20
node examples\forge_tauri_ui\scripts\real-estate-brain-commit.mjs --pretty --ranked=C:\tmp\forge-real-estate-source-discovery\real-estate-harvester\data\ranked_actions.json --compute=C:\tmp\forge-real-estate-source-discovery\real-estate-harvester\data\kasm_rust_compute.json --packs=C:\tmp\forge-real-estate-source-discovery\real-estate-harvester\data\intel_packs.jsonl --store=C:\tmp\forge-real-estate-source-discovery --limit=20
```

Prefer the unified pipeline. The individual commands are lower-level debugging tools. Live runs write hashes and metadata, and raw files stay in the harvester store.

The parser router reads `real-estate-parser-adapters.json`, probes local SOTA adapters (`duckdb`, `tika`, `docling`, `magika`) with a short `--version` check, and falls back to the native parser while preserving `adapterPlan` in every normalized event.

Every normalized event also carries the universal ingestion envelope used by KASM/brain: dataset, distribution, raw artifact, parsed elements, entity candidates, lineage and metric seeds.

The entity resolver consumes those envelopes and emits `entity_graph.jsonl`: canonical entity nodes, evidence edges and clusters, all hash-addressed.

The intel pack builder consumes the graph and normalized events to emit `intel_packs.jsonl`: compact, LLM-safe packs with evidence refs, graph refs, usability score, data gaps and recommended actions.

The KASM seed builder consumes intel packs to emit `kasm_metric_seeds.jsonl`: numeric feature vectors, priority scores, simulation hints and proof hashes.

The KASM simulator calls the Rust `lab_runner_immo` massive compute path with `--seeds kasm_metric_seeds.jsonl`, then emits `kasm_rust_compute.json`, `kasm_simulation_results.jsonl` and `ranked_actions.json`. The JS layer stays an artifact adapter; Rust owns the seed-backed scenario matrix.

The brain commit bridge consumes `ranked_actions.json`, `kasm_rust_compute.json` and `intel_packs.jsonl` to emit `real_estate_memory_commits.jsonl`: compact semantic `brain_commit` requests anchored by action hashes, Rust proof hashes and evidence refs.
