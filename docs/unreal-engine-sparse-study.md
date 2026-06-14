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

Clean-room rule:

- Use Unreal only as architectural research.
- Do not copy Unreal source code, comments, identifiers, shader code, or file-local structure into Forge.
- For Banger, translate lessons into independent Rust/wgpu/Monster interfaces, tests, hashes and benchmarks.
