# Unreal Engine Sparse Study Manifest

Purpose: study Unreal Engine renderer architecture for Banger without copying Epic source code into Forge.

Local sparse clone:

- Path: `C:\Users\quent\Documents\GitHub\UnrealEngine-sparse`
- Remote: `https://github.com/EpicGames/UnrealEngine.git`
- Clone mode: `--filter=blob:none --sparse --depth=1`
- HEAD: `260bb2e1c5610b31c63a36206eedd289409c5f11`
- Approximate checkout size after sparse paths: 90 MB

Sparse checkout paths:

- `Engine/Source/Runtime/Renderer`
- `Engine/Source/Runtime/RenderCore`
- `Engine/Source/Runtime/RHI`
- `Engine/Source/Runtime/D3D12RHI`
- `Engine/Source/Runtime/VulkanRHI`
- `Engine/Shaders/Private`

Initial Banger study targets:

- Render graph discipline: `RenderCore/Public/RenderGraph*.h`, `RenderCore/Private/RenderGraph*.cpp`
- RHI command submission and resource contracts: `RHI/Public/RHICommandList.h`, `RHI/Public/RHIPipeline.h`
- D3D12/Vulkan viewport, swapchain, descriptor and pipeline cache organization.
- Renderer scene submission: `Renderer/Private/SceneRendering.*`, `Renderer/Public/MeshPassProcessor.*`
- Nanite-oriented lessons: `Renderer/Private/Nanite/*`
- Lumen-oriented lessons: `Renderer/Private/Lumen/*`

Clean-room extraction applied in Forge:

- RDG lesson: resources and passes must be declared before execution, then compiled into explicit lifetimes, barriers and an execution order.
- RHI lesson: command submission should consume a compact compiled plan rather than ad hoc renderer-local state.
- MeshPass lesson: scene visibility/material/resource binding is a pass-level contract, not a side effect hidden inside a draw call.
- Banger implementation: `BangerNativeRenderGraphCompilation` compiles Monster/KASM render artifacts into resource records, pass records, dependency edges and proof hashes.
- Nanite lesson: virtualized geometry needs a compact visibility packet that separates cluster/page visibility, LOD error, raster path choice and indirect draw arguments.
- Banger implementation: `BangerNativeMeshletVisibilityPacket` converts Monster/KASM scene submissions and culling manifests into meshlet cluster entries with hardware/compute raster candidates and buffer hashes.
- Raster queue lesson: a native renderer needs a compact work queue between visibility and backend submission so hardware mesh shading and compute fallback stay equivalent.
- Banger implementation: `BangerNativeRasterWorkQueue` converts meshlet visibility into graphics/compute raster jobs with bind table, barrier and dispatch-plan hashes.
- RHI submit lesson: the backend should consume a frame submission packet with explicit render targets, command buffers and presentable-frame proof instead of rebuilding frame state from scattered manifests.
- Banger implementation: `BangerNativeFrameSubmissionPacket` fuses render graph order, raster queue jobs and texture bridge targets into a native submission contract.
- Dynamic RHI lesson: context recording, command-list finalization, ordered submit, fences and present are a single verifiable lifecycle.
- Banger implementation: `BangerNativeRhiSubmitPacket` turns the frame submission into acquire/finalize/submit/present steps with timeline and fence hashes.

Clean-room rule:

- Use Unreal only as architectural research.
- Do not copy Unreal source code, comments, identifiers, shader code, or file-local structure into Forge.
- For Banger, translate lessons into independent Rust/wgpu/Monster interfaces, tests, hashes and benchmarks.
