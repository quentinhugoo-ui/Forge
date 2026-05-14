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
```

Prefer the unified pipeline. The individual commands are lower-level debugging tools. Live runs write hashes and metadata, and raw files stay in the harvester store.
