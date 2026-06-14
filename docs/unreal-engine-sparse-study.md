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
- GPU execution lesson: submit readiness still needs an explicit receipt tying timelines, present proof and nonblank-frame diagnostics together before live backend execution.
- Banger implementation: `BangerNativeGpuExecutionReceipt` records submit-ready phases, timeline diagnostics and readback policy hashes for the frame.
- D3D12/Vulkan backend lesson: swapchain images, descriptor tables, pipeline-state caches, command allocators and barrier batches must be explicit backend contracts.
- Banger implementation: `BangerNativeBackendSubmitPlan` specializes the RHI submit receipt into backend-family targets and proof hashes.
- GPUScene lesson: renderable primitives should be compacted into persistent primitive, instance, payload and material buffers with explicit upload ranges.
- Banger implementation: `BangerNativeGpuScenePacket` converts scene graph submissions into GPU scene primitive records and buffer hashes.
- Nanite second-layer lesson: streaming feedback, page residency, material bins, visibility resolve and ray-tracing proxies must be tied to the same cluster visibility result.
- Banger implementation: `BangerNativeNaniteSecondLayerPacket` derives those contracts from GPU scene primitives, meshlet visibility and resource-table residency.
- Lumen lesson: surface cache pages, screen probes, radiance cache tiles and trace policy must be scheduled as one lighting contract, not scattered post-process state.
- Banger implementation: `BangerNativeLumenLightingPacket` binds Nanite pages, radiance probe pages and render graph lighting passes into deterministic GI/reflection hashes.
- Virtual shadow map lesson: shadow page marking, persistent cache state, light-grid cells and projection tiles must be explicit before shadow depth submission.
- Banger implementation: `BangerNativeVirtualShadowPacket` derives shadow pages from Lumen/Nanite visibility and binds page table, cache, invalidation, projection and light-grid hashes.
- MegaLights/stochastic lighting lesson: direct lighting needs compact light clusters, stable sample sequences, shadow masks, denoiser tiles and resolve tiles tied to the same light-grid state.
- Banger implementation: `BangerNativeDirectLightingPacket` derives direct-lighting samples from virtual shadow pages and binds light-grid, sampling, shadow-mask, denoise and resolve hashes.
- Substrate/TSR lesson: material closures and temporal history must be first-class frame resources, not hidden shader side effects.
- Banger implementation: `BangerNativeMaterialClosurePacket` and `BangerNativeTemporalHistoryPacket` bind Substrate-style closure tiles, material texture tables, motion vectors, disocclusion, rejection, resurrection and accumulation hashes.
- Virtualized residency lesson: Nanite, virtual shadow maps and material payloads need a shared feedback compaction and physical-pool pressure model.
- Banger implementation: `BangerNativePageResidencyAllocatorPacket` v3 unifies Nanite, VSM and material residency with compacted feedback, stale-feedback detection, deferred page-table updates, aliasing candidates, pool receipts and validation hashes.
- RDG v3 lesson: a serious render graph must track pass-resource state, subresource transitions and external-access operations, then make those visible to later RHI/backend stages.
- Banger implementation: `BangerNativeRenderGraphCompilation` v3 now exposes pass resource states, subresource transitions, external access ops, lifetime hashes and validation receipts.
- RHI v3 lesson: queue packets, resource transitions, queue fence edges and present contracts must be explicit before backend specialization.
- Banger implementation: `BangerNativeRhiSubmitPacket` v3 derives queue packets, RHI transitions, fence edges and present contracts from frame submission and render graph contracts.
- Backend execution v3 lesson: descriptor bindings, command allocators and backend-specific resource barriers are execution artifacts, not incidental backend locals.
- Banger implementation: `BangerNativeBackendExecutionPacket` v3 records descriptor bindings, command allocator records, backend resource transitions, validation receipts and nonblank execution gates.
- Frame proof lesson: submit readiness is not enough; the final handoff needs one compact manifest joining graph, RHI, GPU receipt, backend execution, residency and temporal proof.
- Banger implementation: `BangerNativeFrameHandoffManifest` v2 is now the public `render_handoff_hash` authority for native tandem render handoffs.

Clean-room rule:

- Use Unreal only as architectural research.
- Do not copy Unreal source code, comments, identifiers, shader code, or file-local structure into Forge.
- For Banger, translate lessons into independent Rust/wgpu/Monster interfaces, tests, hashes and benchmarks.
