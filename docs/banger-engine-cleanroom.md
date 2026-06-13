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
