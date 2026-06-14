# Banger Unreal Local Extraction Plan

Date: 2026-06-14

This document records the clean-room analysis of the local Unreal Engine sparse clone for Banger, Forge and Monster. It is based only on the local clone at:

`C:\Users\quent\Documents\GitHub\UnrealEngine-sparse`

Clone facts used as source floor:

- Remote: `https://github.com/EpicGames/UnrealEngine.git`
- Commit: `260bb2e1c5610b31c63a36206eedd289409c5f11`
- Sparse checkout:
  - `Engine/Shaders/Private`
  - `Engine/Source/Runtime/D3D12RHI`
  - `Engine/Source/Runtime/RHI`
  - `Engine/Source/Runtime/RenderCore`
  - `Engine/Source/Runtime/Renderer`
  - `Engine/Source/Runtime/VulkanRHI`

Clean-room boundary:

- Use Unreal as an architecture reference only.
- Do not copy Unreal source, shader bodies, comments, local file structure, or private identifiers into Forge.
- Recode Banger concepts as Rust-native, hash-addressed, Monster-verifiable packets.
- Prefer short, testable interfaces over recreating Unreal subsystems wholesale.

## Doctrine

Wall being pushed: latency, memory, proof quality and 3D capability. Banger already has many native manifests, but most are still proof and preview oriented. The wall is turning those manifests into live GPU-owned data paths without losing Forge reproducibility.

Frontier hypothesis: the Unreal sparse clone can be used as a clean-room map for promoting Banger from "hashes of intended render work" to "hash-addressed GPU packets with explicit lifetimes, barriers, caches, residency and validation receipts".

Promotion rule: every Unreal-inspired idea must enter Banger behind a narrow Rust manifest/schema version, deterministic proof hash, focused test, and later a benchmark or GPU receipt. If it does not improve clarity, speed, capability or verifiability, delete it.

## Local Unreal Map

| Unreal area inspected | Local files and folders | Clean-room lesson | Banger target |
| --- | --- | --- | --- |
| Render graph | `RenderCore/Public/RenderGraphBuilder.h`, `RenderCore/Private/RenderGraphBuilder.cpp`, `RenderGraphResources.*`, `RenderGraphValidation.*` | A render graph is useful only when pass parameters define resource lifetime, access mode, barrier scheduling, external resource gates, upload/extraction queues and validation. | Upgrade `BangerNativeRenderGraphCompilation` from pass/resource hashes to a resource lifetime and barrier compiler. |
| GPU scene | `Renderer/Private/GPUScene.cpp`, `Renderer/Public/GPUScene.h` | Scene data is an upload allocator with primitive ranges, instance ranges, payload flags, update buffers and validation, not just a primitive list. | Promote `BangerNativeGpuScenePacket` into a live upload packet with typed ranges and payload flags. |
| Nanite-like visibility and raster | `Renderer/Private/Nanite/*`, especially `NaniteCullRaster.cpp` | The important pattern is candidate clusters, visible clusters, indirect args, HZB input, streaming requests, raster path selection and time budgets. | Extend `BangerNativeCullingManifest`, `BangerNativeMeshletVisibilityPacket` and `BangerNativeRasterWorkQueue`. |
| Scene culling | `Renderer/Private/SceneCulling/*` | Static spatial data should be precomputed and cached; dynamic or large objects need a separate policy. Validation readback is valuable. | Add a Forge-hashed spatial hash grid packet for Banger scene culling. |
| Radiance cache | `Renderer/Private/Lumen/LumenRadianceCache.cpp`, related `Lumen/*` files | Radiance is a clipmap/probe atlas problem with stable indirection textures, trace-tile generation, indirect args, sorting and retention policy. | Upgrade `BangerNativeRadianceScheduleManifest` and `BangerNativeLumenLightingPacket`. |
| Virtual shadow maps | `Renderer/Private/VirtualShadowMaps/*` | Shadow quality depends on page cache age, invalidation reasons, light/clipmap movement, HZB-aware invalidation and page pool pressure. | Extend `BangerNativeVirtualShadowPacket` with page cache metadata and invalidation proofs. |
| Virtual texture and residency | `Renderer/Private/VT/VirtualTextureSystem.cpp` | The key loop is GPU feedback, unique page extraction, prioritized requests, page table update and residency accounting. | Reuse this shape for Banger virtual texture, geospatial tiles and splat residency. |
| Pipeline cache | `RHI/Private/PipelineFileCache.cpp`, `RHI/Private/PipelineStateCache.cpp` | A useful PSO cache stores descriptor ABI, shader hashes, usage history, stage support and prewarm order. | Enrich `BangerNativePipelineCacheManifest` and benchmark promotion gates. |
| RHI command lists | `RHI/Private/RHICommandList.cpp`, `RHI/Public/RHICommandList.h` | Submission needs a state machine: recording, finalized, submitted, fenced, presented, plus breadcrumbs and parallel translation budgets. | Extend `BangerNativeRhiSubmitPacket`, `BangerNativeGpuExecutionReceipt` and `BangerNativeBackendSubmitPlan`. |
| Backend contracts | `D3D12RHI/Private/*`, `VulkanRHI/Private/*` | D3D12 and Vulkan both make descriptor heaps/sets, bindless binding, barriers, command buffers, residency and presentation explicit. | Keep Banger backend-neutral, but model descriptors, barriers and submit receipts explicitly. |

## Extraction Priorities

### 1. Render graph resource lifetime compiler

Current Banger state:

- `BangerNativeRenderGraphCompilation`
- `BangerNativeRenderGraphResource`
- `BangerNativeRenderGraphEdge`
- `BangerNativeRenderGraphCompiledPass`

Clean-room extraction:

- Add resource access declarations: read, write, read-write, render-target, depth, copy source, copy destination, compute, async compute.
- Add pass flags: raster, compute, async compute, copy, present.
- Add graph compile output: culled passes, merged passes, barrier batches, transient allocations, external resource imports, external exports.
- Add validation output: missing producer, illegal simultaneous write, invalid external import, lifetime overlap, pass dependency cycle.
- Add proof counters: pass count, culled pass count, barrier count, transient alias count, external resource count.

Why first-class: this is the coordination layer that lets Monster produce render work without knowing backend details.

### 2. GPU scene upload packet

Current Banger state:

- `BangerNativeGpuScenePacket`
- `BangerNativeGpuScenePrimitive`

Clean-room extraction:

- Split primitive, instance and payload ranges.
- Add stable primitive ids and instance ids derived from Forge content hashes.
- Add update ranges so Banger can upload only changed scene sections.
- Add payload flags for transforms, previous transforms, material payload, custom data, local bounds, skinning and editor/debug data.
- Add validation receipts that compare declared flags with actual buffer ranges.

Why it should be the first coding slice: GPU scene is upstream of culling, meshlet visibility, shadows, lighting and splats. It is smaller and less risky than trying to implement a full Nanite or Lumen path first.

### 3. Meshlet visibility and raster work

Current Banger state:

- `BangerNativeCullingManifest`
- `BangerNativeMeshletVisibilityPacket`
- `BangerNativeNaniteSecondLayerPacket`
- `BangerNativeRasterWorkQueue`

Clean-room extraction:

- Candidate cluster buffer.
- Visible cluster buffer.
- Dual indirect argument buffers for alternating cull/raster phases.
- Streaming request buffer.
- HZB input resource id and resolution.
- Raster path selection: software/compute, hardware mesh, vertex fallback.
- Time budget field so Banger can degrade gracefully.
- Proof fields for visible cluster count, rejected cluster count, requested page count and selected raster path.

This should remain meshlet-oriented and Rust-native. It should not copy Nanite terminology or shader organization internally.

### 4. Forge-hashed scene culling grid

Clean-room extraction:

- Static scene cells cached by content hash.
- Dynamic objects handled by a separate uncached lane.
- Compressed cell chunks for large worlds.
- Optional readback validation for debug builds.
- Stable culling manifest tied to Banger scene graph and Monster proof hashes.

This fits Banger better than a monolithic Unreal-style world model because Forge already wants compact manifests and reproducible reuse.

### 5. Radiance schedule and cache

Current Banger state:

- `BangerNativeRadianceScheduleManifest`
- `BangerNativeRadianceProbePage`
- `BangerNativeLumenLightingPacket`

Clean-room extraction:

- Clipmap key.
- Probe atlas page id.
- Indirection table id.
- Trace tile list.
- Optional sorted trace tile list.
- Indirect dispatch args.
- Retention age and invalidation reason.
- Separate paths for screen trace, world trace, SDF trace and hardware ray trace, selected by backend capability.

This maps well to Forge because radiance pages are cacheable proof artifacts. Monster can generate the high-level schedule while Banger owns GPU execution.

### 6. Virtual shadow page cache

Current Banger state:

- `BangerNativeVirtualShadowPacket`
- `BangerNativeVirtualShadowEntry`

Clean-room extraction:

- Per-light page table id.
- Per-clipmap page ranges.
- Page age.
- Invalidated-by reason: light move, receiver move, caster move, WPO/deformation, page pool pressure, depth range shift.
- HZB invalidation input.
- Page pool pressure score.
- Proof counters for cached, invalidated, requested and rendered pages.

This should be implemented as a cache policy packet, not as a full renderer rewrite.

### 7. Virtual texture, geospatial and splat residency

Current Banger state:

- `BangerNativeTextureBridgeContract`
- Gaussian splat layer and conversion manifests.
- Google Photorealistic 3D Tiles objectives in Banger docs.

Clean-room extraction:

- GPU feedback buffer.
- Unique requested page list.
- Priority-sorted request list.
- Page table update list.
- Residency receipt.
- Physical pool pressure.
- Dropped-page proof reason.

The same residency core can serve virtual textures, Cesium/Google tile pages and splat chunks.

### 8. Pipeline cache and benchmark promotion

Current Banger state:

- `BangerNativePipelineCacheManifest`
- `BangerNativePipelineCacheEntry`
- `BangerNativeBenchmarkPromotionManifest`
- `BangerNativeBenchmarkGate`

Clean-room extraction:

- Descriptor ABI hash.
- Shader stage hashes.
- Render target and depth format tuple.
- Vertex or mesh input layout hash.
- Backend capability bits.
- Usage count and last-used frame.
- Prewarm priority.
- Cache eviction reason.
- Benchmark gate result tied to a proof hash.

This is where Banger can be more Forge-native than Unreal: every promoted PSO should have a deterministic proof path and measured local value.

### 9. RHI submit lifecycle

Current Banger state:

- `BangerNativeFrameSubmissionPacket`
- `BangerNativeRhiSubmitPacket`
- `BangerNativeRhiSubmitStep`
- `BangerNativeGpuExecutionReceipt`
- `BangerNativeGpuExecutionPhaseReceipt`
- `BangerNativeBackendSubmitPlan`

Clean-room extraction:

- Explicit command list state: recording, finalized, translated, submitted, fenced, presented.
- Fence scope id.
- Queue id and queue family.
- Parallel translation budget.
- Breadcrumb path.
- Crash/debug proof path.
- Present receipt and frame id.

This gives Forge/Monster a verifiable handoff without exposing backend implementation details to KASM.

## What Not To Extract

- Do not clone Unreal source files or shader code.
- Do not reproduce Unreal class names as Forge public APIs.
- Do not import the Unreal C++ object lifecycle.
- Do not copy the editor model before Banger has a stable native render core.
- Do not build a Blueprint clone. Forge/KASM and CodeAct are the authoring language.
- Do not add a second renderer path in Electron. Banger remains the native Rust child surface.

## Immediate Coding Plan

### Gate A: GPU scene packet v2

Files likely touched:

- `examples/ingen_native_services/src/banger_native_engine.rs`
- `src/monster.rs`
- `src/kasm.rs`

Add:

- `BangerNativeGpuSceneUploadRange`
- `BangerNativeGpuSceneInstanceRange`
- `BangerNativeGpuScenePayloadFlags`
- `BangerNativeGpuSceneValidationReceipt`

Verification:

- Deterministic hash test for stable scene upload packets.
- Monster route test proving `/newobject_` or native tandem render produces the same packet hash for identical scene input.

### Gate B: render graph compile v2

Files likely touched:

- `examples/ingen_native_services/src/banger_native_engine.rs`
- `src/monster.rs`

Add:

- resource access enum,
- pass queue kind,
- barrier batch,
- transient alias plan,
- external import/export lists,
- validation receipt.

Verification:

- Unit test for pass culling and barrier count.
- Proof hash stability test.

### Gate C: meshlet visibility proof packet

Files likely touched:

- `examples/ingen_native_services/src/banger_native_engine.rs`
- `src/monster.rs`
- `src/kasm.rs`

Add:

- candidate and visible cluster counts,
- indirect args ids,
- HZB resource id,
- streaming request count,
- raster path enum,
- time budget.

Verification:

- Deterministic mock scene test.
- Later GPU benchmark gate.

### Gate D: cache policy packets

Files likely touched:

- `examples/ingen_native_services/src/banger_native_engine.rs`

Add:

- virtual shadow invalidation reasons,
- radiance retention age,
- residency pressure receipt,
- page request priority.

Verification:

- Cache invalidation proof tests.

### Gate E: pipeline cache telemetry

Files likely touched:

- `examples/ingen_native_services/src/banger_native_engine.rs`

Add:

- descriptor ABI hash,
- shader stage hash set,
- backend capability bits,
- usage count,
- prewarm order,
- eviction reason.

Verification:

- Benchmark promotion manifest test.

## Recommended First Slice

Start with GPU scene packet v2.

Reason:

- It is already represented in Banger.
- It feeds every later renderer system.
- It is small enough to verify deterministically.
- It gives Monster a concrete bridge from Forge object commands to GPU-owned scene data.
- It avoids prematurely recoding a full Nanite, Lumen or virtual shadow system.

Acceptance criteria for the first slice:

- Banger can represent primitive, instance and payload upload ranges.
- The packet hash is stable across identical inputs.
- A validation receipt detects mismatched flags and ranges.
- Monster can attach the packet to the native tandem render artifact.
- No Electron DOM or WebGL path becomes authoritative.

## Current Crosswalk

| Current Forge/Banger concept | Missing clean-room field or behavior |
| --- | --- |
| `BangerNativeRenderGraphCompilation` | Resource access modes, barrier batches, transient aliasing, external import/export receipts. |
| `BangerNativeGpuScenePacket` | Upload allocator, instance ranges, payload flags, partial update ranges, validation receipt. |
| `BangerNativeCullingManifest` | Static spatial hash grid, dynamic lane, compressed cells, optional readback proof. |
| `BangerNativeMeshletVisibilityPacket` | Candidate/visible buffers, indirect args, HZB id, streaming request buffer, raster path. |
| `BangerNativeRadianceScheduleManifest` | Clipmap keys, probe atlas, trace tiles, indirect args, retention and invalidation. |
| `BangerNativeVirtualShadowPacket` | Page age, invalidation reason, page pool pressure, HZB invalidation. |
| `BangerNativeTextureBridgeContract` | Feedback buffer, unique page list, priority requests, residency receipt. |
| `BangerNativePipelineCacheManifest` | Descriptor ABI, usage history, prewarm order, eviction and benchmark result. |
| `BangerNativeRhiSubmitPacket` | Command lifecycle, fence scope, queue id, breadcrumbs, present receipt. |

## Verification Status

This document is an analysis artifact. No code was changed and no build was required for this slice.

Local source inspection covered the clone areas listed above, plus the current Banger/Monster/KASM code paths in:

- `examples/ingen_native_services/src/banger_native_engine.rs`
- `examples/ingen_native_services/src/bin/ingen_electron_backend_bridge.rs`
- `examples/ingen_native_services/src/lib.rs`
- `src/monster.rs`
- `src/kasm.rs`
- `src/brain.rs`
- `src/act_codes/mod.rs`

Next action should be implementation of Gate A.
