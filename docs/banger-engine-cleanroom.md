# Banger Native Engine Clean-Room Map

Date: 2026-06-11

## Wall

The wall being pushed is Banger architecture quality: build a native 3D /
computational-engineering runtime at Unreal-class ambition without copying
Unreal code, creating a second app shell, or bypassing Forge/Monster proofs.

## Hypothesis

Banger should be a Rust-owned native child surface driven by content-addressed
Forge/Monster artifacts, with an ECS-like scene core, explicit render graph,
streaming asset graph and GPU compute handoff, instead of a DOM canvas or a
monolithic Unreal clone.

## Current Floor

- Epic officially exposes complete Unreal C++ source to linked Epic/GitHub
  accounts for study, customization and debugging:
  https://www.unrealengine.com/en-US/ue-on-github
- Epic's source-code documentation says access requires Epic + GitHub account
  linking, and recommends the release branch as the stable study baseline:
  https://dev.epicgames.com/documentation/en-us/unreal-engine/downloading-source-code-in-unreal-engine
- Epic explicitly distinguishes learning from copying: knowledge is free, but
  copied Unreal code brings the project under the Unreal EULA.
- Ghidra should therefore be used only for binary cartography: sections,
  imports, external symbols, module boundaries, strings, hashes and runtime
  dependency hints. It must not be used to recreate function bodies.

## Reference Engines

| Engine | Useful Lesson For Banger | Source |
| --- | --- | --- |
| Unreal Engine | AAA module scale, editor/runtime split, world partition, Nanite/Lumen/PCG/Insights-class subsystems | Epic docs/source access |
| Godot | Small comprehensible open engine, scene/node resource model, editor as engine client, permissive MIT codebase | https://github.com/godotengine/godot |
| Bevy | Rust-first ECS, pluginized schedule, data-driven systems, fast iteration | https://github.com/bevyengine/bevy |
| O3DE | AAA-oriented Apache engine, Gem modularity, Atom renderer, simulation/cinema target | https://github.com/o3de/o3de |
| Filament | Compact physically based renderer, material compiler, cross-platform backend discipline | https://github.com/google/filament |
| bgfx | Bring-your-own-engine graphics abstraction across D3D/Metal/Vulkan/OpenGL/WebGPU | https://github.com/bkaradzic/bgfx |
| wgpu | Rust-native WebGPU-style portability over Vulkan/Metal/D3D12/OpenGL/WebGPU | https://github.com/gfx-rs/wgpu |

## Recommended Banger Shape

1. Keep Electron/React as the shell and Banger as a native child surface.
2. Keep Forge/Monster as the compute/proof authority for SDF, voxel, meshlet,
   radiance, culling, streaming and export artifacts.
3. Make the Banger runtime consume compact manifests:
   `scene_hash`, `asset_graph_hash`, `render_graph_hash`, `gpu_kernel_hash`,
   `proof_hash`.
4. Use a Rust ECS/schedule core for gameplay/editor state, but keep rendering
   submission explicit and deterministic.
5. Use `wgpu` as the first native RHI unless a later benchmark proves bgfx,
   Vulkan-only or a custom backend clearly wins for Banger's workload.
6. Use Filament-like material compilation discipline: material source becomes
   a versioned artifact, never ad-hoc shader strings in UI state.
7. Use Unreal/Godot/O3DE as architecture references, not as source to copy.

## Action Plan

Date: 2026-06-14

1. Promote a scene-first editable object manifest from the existing
   `native_tandem_render` handoff: stable object ids, parent/root relation,
   representation, transforms, bounds, editable slots and proof hashes.
   Status: started in `BangerNativeEngine` as
   `forge.banger.editable_scene_manifest.v1`.
2. Route Banger `/newobject_` into the same manifest instead of creating
   renderer-local objects: edits must produce a new Forge contract before GPU
   pages are promoted.
   Status: started in `BangerNativeEngine::prepare_newobject_contract` as
   `forge.banger.newobject_prepare.v1`; GPU page promotion remains gated until
   the renderer consumes the updated editable scene manifest.
3. Add persistent native pipeline-cache blobs keyed by adapter, driver,
   shader source hash, shader reflection and render-pass ABI.
   Status: started in `BangerNativePipelineCacheManifest` as
   `forge.banger.native_pipeline_cache_manifest.v1`; deterministic seed blobs
   are persisted content-addressed now, while real driver blobs remain gated on
   the promoted `wgpu::PipelineCache` / backend-specific API path.
4. Replace the placeholder child-surface contract with a verified texture
   sharing path: same device/queue when available, explicit fallback route,
   frame hash and resize/orbit/pan/zoom proofs.
   Status: started in `BangerNativeTextureBridgeContract` as
   `forge.banger.native_texture_bridge_contract.v1`; the handoff now seals
   same-device/queue import eligibility, CPU readback fallback, frame hash,
   resize proof and orbit/pan/zoom viewport proof. Real backend external
   handle import remains gated behind the promoted RHI surface path.
5. Promote scene graph authority: parent/child transforms, local/world matrix
   propagation, object visibility and representation mix become the native
   source for viewport fit and render submission.
   Status: started in `BangerNativeSceneGraphSubmission` as
   `forge.banger.native_scene_graph_submission.v1`; local/world transform
   propagation, visibility, representation mix, viewport fit bounds and render
   submission hashes now come from the editable scene graph. Meshlet culling
   and indirect draw reuse remain the next gate.
6. Add meshlet/virtual-geometry culling manifests: cone bounds, LOD error,
   visibility result hash, indirect draw buffer hash and cache-hit reuse.
   Status: started in `BangerNativeCullingManifest` as
   `forge.banger.native_culling_manifest.v1`; meshlet/virtual-geometry entries
   now carry conservative cone bounds, LOD error buckets, visibility result
   hashes, indirect draw buffer hashes and cache reuse keys. Real GPU cull
   dispatch remains gated behind the promoted renderer path.
7. Add surfel/radiance-cache scheduling: probe pages, temporal epoch, light
   budget, invalidation hash and async-compute residency policy.
   Status: started in `BangerNativeRadianceScheduleManifest` as
   `forge.banger.native_radiance_schedule_manifest.v1`; surfel probe pages now
   carry deterministic temporal epochs, light budgets, invalidation hashes and
   async-compute residency policies. Real GI dispatch and denoised reuse remain
   behind the promoted renderer path.
8. Add Gaussian splat layer support as a hybrid representation, not a separate
   renderer: splat buckets, sort/group keys, proxy bounds and optional
   mesh/surfel conversion manifests.
   Status: started in `BangerNativeGaussianSplatLayerManifest` as
   `forge.banger.native_gaussian_splat_layer_manifest.v1`; Gaussian splats now
   live as hybrid scene layers with bucket/sort/group keys, proxy bounds and
   mesh/surfel conversion manifests. `prepare_gaussian_splat_asset` now imports
   real 3DGS PLY assets into GPU-ready position, covariance, opacity, SH and
   depth-sort buffers with bucket proof hashes. `rasterize_gaussian_splat_asset`
   now projects anisotropic splats through a pinhole camera, bins them into
   tiles, depth-sorts per tile, evaluates SH color and alpha-composites into a
   deterministic RGBA8 proof image; GPU promotion remains gated behind the
   promoted renderer path.
9. Promote Slang as the shader artifact source when available: reflection,
   WGSL/SPIR-V/HLSL/MSL targets, material ABI checks and fallback WGSL parity.
   Status: started in `BangerNativeShaderCompilerTicket` as a Slang-authority
   ticket: WGSL/SPIR-V/HLSL/MSL target artifacts are declared or compiled,
   fallback WGSL parity is hash-bound, shader reflection now carries binding
   proofs, and the Banger material ABI is attached to pipeline-cache blobs.
10. Add benchmark gates for promotion: latency, VRAM pressure, cache-hit reuse,
    proof reproducibility and visual capability versus the current path.
    Status: started in `BangerNativeBenchmarkPromotionManifest` as
    `forge.banger.benchmark_promotion_manifest.v1`; every render handoff now
    carries deterministic latency, VRAM, cache-reuse, proof-reproducibility and
    visual-capability gates before Banger GPU promotion is allowed.

## Ghidra Pipeline

Unreal Engine is not currently installed on this machine; only Epic Games
Launcher is present. After Unreal is installed, run:

```powershell
.\scripts\analyze-unreal-ghidra.ps1
```

or pass an explicit binary:

```powershell
.\scripts\analyze-unreal-ghidra.ps1 -BinaryPath "C:\Program Files\Epic Games\UE_5.x\Engine\Binaries\Win64\UnrealEditor.exe"
```

The script writes reports under `C:\tmp\unreal-ghidra\reports` and deletes the
temporary Ghidra project after export. The exported map is metadata-only by
design.

## Promotion Gate

Promote a Banger subsystem only when it beats the current path on at least one
of:

- lower latency,
- lower memory,
- smaller context surface for the LLM,
- stronger proof/hash reproducibility,
- clearer native boundary,
- better user-visible 3D capability.

Failed experiments should be deleted or reduced to a compact manifest.
