//! GPUnode runtime:
//! - First-install GPU detection + persisted bootstrap plan
//! - Logical GPUnode deployment count = detected GPU count
//! - Batch execution helper used by Forge CPU `dispatch_batch`

use std::fmt;
use std::fs;
use std::io;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use std::collections::HashMap;

use super::dispatch::{BatchInput, PackedOutput};
use super::hotplan::{HotPlan, HotProgram};
use super::MonsterNode;
use crate::kasm::Ty;

/// Persisted first-install bootstrap plan.
pub const GPUNODE_BOOTSTRAP_PATH: &str = ".codex-tmp/gpunode-bootstrap-v1.txt";
const GPU_MIN_BATCH_SIZE: usize = 4096;
// CUDA_CHUNK_ELEMS removed — was used by the legacy cudarc Affine /
// HashChain kernels which were deleted in Phase Ω.10. cuda_min handles
// chunking internally if needed.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GpuNodeBootstrap {
    /// True when the plan was created during this process run.
    pub first_install: bool,
    /// Number of GPUs detected locally.
    pub detected_gpu_count: usize,
    /// Number of logical GPUnodes to deploy.
    pub deployed_gpunodes: usize,
    /// True if at least one NVIDIA adapter is detected.
    pub nvidia_detected: bool,
    /// True if CUDA mode is enabled for this runtime.
    pub cuda_enabled: bool,
    /// True if the NVIDIA CUDA Toolkit (nvrtc) is detected on PATH.
    /// When `nvidia_detected && cuda_enabled && !nvrtc_available`,
    /// runtime kernel compilation will fail — the user has the driver
    /// but not the Toolkit. Recomputed every bootstrap (not persisted).
    pub nvrtc_available: bool,
}

impl GpuNodeBootstrap {
    pub fn zero() -> Self {
        Self {
            first_install: false,
            detected_gpu_count: 0,
            deployed_gpunodes: 0,
            nvidia_detected: false,
            cuda_enabled: false,
            nvrtc_available: false,
        }
    }

    /// User-facing message when CUDA is "available" (driver) but the
    /// Toolkit (nvrtc) is missing. Returns None when no problem.
    /// Mentions WGPU as the no-download alternative (uses Vulkan/DX12,
    /// shipped with every modern GPU driver — no separate install).
    pub fn cuda_toolkit_warning(&self) -> Option<String> {
        if self.nvidia_detected && self.cuda_enabled && !self.nvrtc_available {
            Some(format!(
                "CUDA driver detected ({} NVIDIA GPU{}) but the CUDA Toolkit is missing — \
                 nvrtc.dll cannot be loaded, so Forge cannot JIT-compile CUDA kernels. \
                 \nTwo options to enable GPU acceleration:\n  \
                 (A) Install the NVIDIA CUDA Toolkit (~3 GB) from https://developer.nvidia.com/cuda-downloads — \
                 unlocks all CUDA kernels including custom ones.\n  \
                 (B) Switch to WGPU backend (no download needed, uses Vulkan/DX12 already on your system). \
                 Rebuild Forge with `--features wgpu` instead of `--features cuda`. \
                 WGPU works on any modern GPU (NVIDIA, AMD, Intel) and any OS.\n\
                 For now Forge runs on CPU fallback only.",
                self.detected_gpu_count,
                if self.detected_gpu_count > 1 { "s" } else { "" }
            ))
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GpuVendor {
    Nvidia,
    Amd,
    Intel,
    Apple,
    Microsoft, // Basic Display Adapter, virtual, etc.
    Other,
}

impl GpuVendor {
    pub fn from_name(name: &str) -> Self {
        let n = name.to_ascii_lowercase();
        if n.contains("nvidia") || n.contains("geforce") || n.contains("quadro") || n.contains("tesla") {
            Self::Nvidia
        } else if n.contains("amd") || n.contains("radeon") || n.contains("ati ") {
            Self::Amd
        } else if n.contains("intel") {
            Self::Intel
        } else if n.contains("apple") {
            Self::Apple
        } else if n.contains("microsoft") || n.contains("basic display") {
            Self::Microsoft
        } else {
            Self::Other
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Nvidia => "NVIDIA",
            Self::Amd => "AMD",
            Self::Intel => "Intel",
            Self::Apple => "Apple",
            Self::Microsoft => "Microsoft",
            Self::Other => "Other",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GpuBackend {
    /// Native NVIDIA path via CUDA driver + nvrtc kernel JIT.
    /// Preferred for NVIDIA hardware (15-30% faster than WGPU on same GPU).
    Cuda { device_id: u32 },
    /// Vendor-agnostic path via Vulkan/DX12/Metal — works on any modern GPU.
    /// The only path for non-NVIDIA hardware in this build.
    Wgpu { adapter_index: u32 },
    /// GPU detected but no usable backend in this Forge build (e.g. AMD with
    /// wgpu feature absent). The node is recorded but won't receive work.
    Inactive { reason: String },
}

impl GpuBackend {
    pub fn label(&self) -> String {
        match self {
            Self::Cuda { device_id } => format!("CUDA (device {device_id})"),
            Self::Wgpu { adapter_index } => format!("WGPU (adapter {adapter_index})"),
            Self::Inactive { reason } => format!("inactive — {reason}"),
        }
    }

    pub fn is_active(&self) -> bool {
        !matches!(self, Self::Inactive { .. })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GpuNode {
    pub id: usize,
    /// Hardware vendor parsed from the OS-reported adapter name.
    pub vendor: GpuVendor,
    /// Full adapter name as reported by the OS.
    pub name: String,
    /// Execution path chosen for this node based on vendor + Forge build features.
    pub backend: GpuBackend,
}

/// Per-GPU backend selection. Pure function of (vendor, system caps).
/// NVIDIA + CUDA Toolkit  → CUDA           (best perf path)
/// NVIDIA + no Toolkit    → WGPU degraded  (only if wgpu compiled in)
/// AMD/Intel/Apple        → WGPU           (only path)
/// Anything else with no usable backend → Inactive (logged, not used)
fn pick_backend(
    vendor: GpuVendor,
    adapter_index: u32,
    cuda_compiled: bool,
    cuda_toolkit_present: bool,
    wgpu_compiled: bool,
) -> GpuBackend {
    match vendor {
        GpuVendor::Nvidia => {
            if cuda_compiled && cuda_toolkit_present {
                GpuBackend::Cuda { device_id: adapter_index }
            } else if wgpu_compiled {
                GpuBackend::Wgpu { adapter_index }
            } else if cuda_compiled && !cuda_toolkit_present {
                GpuBackend::Inactive {
                    reason: "CUDA Toolkit (nvrtc) not installed".to_string(),
                }
            } else {
                GpuBackend::Inactive {
                    reason: "neither cuda nor wgpu feature compiled in".to_string(),
                }
            }
        }
        GpuVendor::Amd | GpuVendor::Intel | GpuVendor::Apple | GpuVendor::Other => {
            if wgpu_compiled {
                GpuBackend::Wgpu { adapter_index }
            } else {
                GpuBackend::Inactive {
                    reason: format!(
                        "{} GPU needs wgpu feature (rebuild with --features wgpu)",
                        vendor.label()
                    ),
                }
            }
        }
        GpuVendor::Microsoft => GpuBackend::Inactive {
            reason: "virtual / basic display adapter (not a real GPU)".to_string(),
        },
    }
}

/// Runtime deployed inside `MonsterNode`. One logical GPUnode per
/// detected adapter.
#[derive(Debug, Clone)]
pub struct GpuNodeRuntime {
    bootstrap: GpuNodeBootstrap,
    nodes: Vec<GpuNode>,
}

impl GpuNodeRuntime {
    pub fn bootstrap_best_effort() -> Self {
        let bootstrap = bootstrap_gpunodes_best_effort();
        // Re-enumerate adapters by name so we can attach per-node vendor
        // and backend info (the persisted bootstrap struct only stores
        // counts for backwards compat). If detection fails, we fall back
        // to anonymous nodes — never block startup on adapter naming.
        let names = detect_gpu_adapter_names().unwrap_or_default();
        let cuda_compiled = cfg!(feature = "cuda");
        let wgpu_compiled = cfg!(feature = "wgpu");
        let toolkit = bootstrap.nvrtc_available;

        let mut nodes: Vec<GpuNode> = Vec::with_capacity(bootstrap.deployed_gpunodes);
        for id in 0..bootstrap.deployed_gpunodes {
            let name = names
                .get(id)
                .cloned()
                .unwrap_or_else(|| format!("GPU adapter #{id}"));
            let vendor = GpuVendor::from_name(&name);
            let backend = pick_backend(
                vendor,
                id as u32,
                cuda_compiled,
                toolkit,
                wgpu_compiled,
            );
            nodes.push(GpuNode {
                id,
                vendor,
                name,
                backend,
            });
        }
        Self { bootstrap, nodes }
    }

    /// Returns nodes whose backend is active and matches the predicate.
    /// Used by the muscle dispatcher to pick a concrete execution path.
    pub fn nodes_for<F: Fn(&GpuBackend) -> bool>(&self, pred: F) -> Vec<&GpuNode> {
        self.nodes
            .iter()
            .filter(|n| n.backend.is_active() && pred(&n.backend))
            .collect()
    }

    /// First node whose backend is CUDA (NVIDIA + Toolkit). None if no NVIDIA
    /// in this system or Toolkit missing.
    pub fn first_cuda_node(&self) -> Option<&GpuNode> {
        self.nodes
            .iter()
            .find(|n| matches!(n.backend, GpuBackend::Cuda { .. }))
    }

    /// First node whose backend is WGPU. Used for AMD/Intel/Apple, or for
    /// NVIDIA in degraded mode (no CUDA Toolkit but wgpu compiled in).
    pub fn first_wgpu_node(&self) -> Option<&GpuNode> {
        self.nodes
            .iter()
            .find(|n| matches!(n.backend, GpuBackend::Wgpu { .. }))
    }

    pub fn count(&self) -> usize {
        self.nodes.len()
    }

    pub fn bootstrap(&self) -> &GpuNodeBootstrap {
        &self.bootstrap
    }

    pub fn node_ids(&self) -> &[GpuNode] {
        &self.nodes
    }

    pub fn cuda_enabled(&self) -> bool {
        self.bootstrap.cuda_enabled
    }

    /// Execute miss batch through logical GPUnodes.
    ///
    /// Today this is a deterministic CPU fallback split across logical
    /// GPUnodes (round-robin ownership), which keeps the architecture
    /// seam stable while real WGPU/CUDA/Metal kernels are integrated.
    pub fn eval_batch(
        &self,
        node: &MonsterNode,
        calls: &[BatchInput<'_>],
    ) -> io::Result<PackedOutput> {
        if calls.is_empty() {
            return Ok(PackedOutput::default());
        }
        if calls.len() < GPU_MIN_BATCH_SIZE {
            return eval_serial(node, calls);
        }

        // ────────────────────────────────────────────────────────────
        // CPU-fast bypass : si le programme est CPU-auto-routable
        // (HotPlan rule fast lane OU petit Interpret ≤ 64 nodes), le
        // CPU traite à 75-300 ns/call vs GPU à ~5-10 µs/call (50-100×
        // plus rapide). On force le path eval_serial qui appliquera
        // les fast paths v0/v1/v2 inline.
        //
        // Ce filtre est exactement la règle "GPU = Lourd uniquement"
        // demandée par le user. Les programmes simples ne quittent
        // PAS le CPU même quand le batch est gros.
        //
        // Mesures (DNA bench k=21, 100k k-mers) confirment :
        //   complement (4 nodes)  : CPU 88 ns vs GPU 10 049 ns (114×)
        //   strobemer  (7 nodes)  : CPU 130 ns vs GPU 6 974 ns (53×)
        //   minhash10  (41 nodes) : CPU 250 ns vs GPU 7 092 ns (28×)
        //
        // Crossover empirique : ~64 nodes Interpret. Au-delà, même
        // l'auto-router stack interp ralentit suffisamment pour que
        // GPU devienne compétitif (crypto_heavy 101 nodes : GPU
        // dominera sur batches très larges).
        if all_calls_cpu_routable(node, calls)? {
            return eval_serial(node, calls);
        }

        // Doctrine de routing (user-spec) :
        //   - CPU = cerveau + Léger (auto-routé v0/v1/v2 dans dispatch_impl
        //     et eval_serial). Les calculs simples NE quittent JAMAIS le CPU.
        //   - GPU = Lourd uniquement. Atteint uniquement quand le programme
        //     est suffisamment compute-intensive pour amortir le transfer
        //     overhead (~10 µs).
        //   - 1 GPU NVIDIA → CUDA exclusif
        //   - 1 GPU autre  → WGPU exclusif
        //   - 2 GPU vendors différents (NVIDIA + AMD/Intel) → split en
        //     parallèle via thread::scope, NVIDIA bouffe sa moitié via cuda,
        //     AMD/Intel bouffe l'autre moitié via wgpu. Aucune GPU idle.
        //
        // Détection : on s'appuie sur `first_cuda_node()` et
        // `first_wgpu_node()` du runtime (déjà câblés par
        // `bootstrap_best_effort` qui assigne un backend par adapter).
        //
        // Note : le path GPU bouffeur ne fire QUE quand calls.len() ≥
        // GPU_MIN_BATCH_SIZE (4096). En dessous, eval_serial CPU déjà
        // shorte tout. C'est le filtre "Lourd" demandé par l'utilisateur.

        let cuda_node = self.first_cuda_node();
        let wgpu_node = self.first_wgpu_node();
        // Two distinct GPU vendors = both nodes exist AND target different
        // hardware (CUDA path + WGPU path). On a single-NVIDIA system avec
        // CUDA Toolkit absent, first_cuda_node() = None, on retombe sur
        // wgpu seul. On a single-AMD, first_cuda_node() = None, wgpu seul.
        let dual_gpu = cuda_node.is_some() && wgpu_node.is_some();

        if dual_gpu && calls.len() >= GPU_MIN_BATCH_SIZE * 2 {
            #[cfg(all(feature = "cuda", feature = "wgpu"))]
            {
                // Split parallèle CUDA + WGPU. Ne fire QUE quand le
                // programme est handled par les deux paths (today : Affine
                // only puisque try_eval_wgpu_affine est limité). Quand un
                // wgpu universal kernel arrive, ce path s'élargit.
                if crate::cuda_min::cuda_min_available()
                    && std::env::var("FORGE_CUDA_MIN").as_deref() != Ok("0")
                {
                    if let Some(packed) = try_eval_split_cuda_wgpu(node, calls)? {
                        set_last_cuda_status(CudaStatus::SplitOk);
                        return Ok(packed);
                    }
                    // Split refusé (programme incompatible wgpu) → tomber
                    // sur single-GPU cuda dessous.
                }
            }
        }

        if self.cuda_enabled() {
            #[cfg(feature = "cuda")]
            {
                // ─── Forge cuda_min path (Phase C moonshot) ──────────────
                // Direct nvcuda.dll FFI + pre-compiled PTX. No cudarc, no
                // CUDA Toolkit at runtime, no version mismatch hell. The
                // user only needs the NVIDIA driver. Universal KASM
                // interpreter — handles ANY KASM program.
                if crate::cuda_min::cuda_min_available()
                    && std::env::var("FORGE_CUDA_MIN").as_deref() != Ok("0")
                {
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        try_eval_cuda_min(node, calls)
                    }));
                    match result {
                        Ok(Ok(Some(packed))) => {
                            set_last_cuda_status(CudaStatus::Ok);
                            return Ok(packed);
                        }
                        Ok(Ok(None)) => { /* program shape unsupported, fall through */ }
                        Ok(Err(e)) => {
                            // Detect the most common error and surface an
                            // actionable message rather than the cryptic CUDA
                            // wording. "unsupported toolchain" = our PTX is
                            // newer than the driver supports.
                            let raw = e.to_string();
                            if raw.contains("unsupported toolchain") {
                                eprintln!(
                                    "[gpunode] cuda_min: NVIDIA driver too old for the embedded PTX. \
                                     Update your NVIDIA driver to one matching the CUDA version this \
                                     Forge was built against, or rebuild Forge with an older CUDA \
                                     Toolkit. Underlying error: {}",
                                    raw
                                );
                                set_last_cuda_status(CudaStatus::DriverTooOld(io::Error::other(raw)));
                            } else {
                                eprintln!("[gpunode] cuda_min io error: {}", raw);
                                set_last_cuda_status(CudaStatus::Io(io::Error::new(e.kind(), raw)));
                            }
                        }
                        Err(panic) => {
                            let panic_msg = if let Some(s) = panic.downcast_ref::<&str>() {
                                s.to_string()
                            } else if let Some(s) = panic.downcast_ref::<String>() {
                                s.clone()
                            } else {
                                "unknown panic".to_string()
                            };
                            eprintln!("[gpunode] cuda_min panicked: {}", panic_msg);
                            set_last_cuda_status(CudaStatus::Panicked(io::Error::other(panic_msg)));
                        }
                    }
                }
            }
        }

        #[cfg(feature = "wgpu")]
        {
            // Try universal kernel first (covers ~20 i64 opcodes including
            // Hash64 / Cond / mixers — couvre la majorité des programmes
            // DNA / kmer / finance simple). Fallback à try_eval_wgpu_affine
            // qui couvre uniquement AffineI64 mais peut être plus rapide
            // sur ces patterns spécifiques (pas de loop overhead).
            if let Some(packed) = try_eval_wgpu_universal(node, calls)? {
                return Ok(packed);
            }
            if let Some(packed) = try_eval_wgpu_affine(node, calls)? {
                return Ok(packed);
            }
        }

        eval_serial(node, calls)
    }
}

// ─── Forge cuda_min bridge (Phase C moonshot) ────────────────────────
//
// Routes a dispatch_batch call through Forge's own minimal CUDA driver
// FFI (no cudarc, no CUDA Toolkit at runtime). The runtime uses pinned
// host memory + async streams internally for max PCIe throughput.
#[cfg(feature = "cuda")]
fn try_eval_cuda_min(
    node: &MonsterNode,
    calls: &[BatchInput<'_>],
) -> io::Result<Option<PackedOutput>> {
    let func = calls[0].func;
    if calls.iter().any(|c| c.func != func) {
        return Ok(None);
    }
    let hot = node.hot_program(&func)?;
    if hot.program.inputs() != 1
        || hot.program.outputs() != 1
        || hot.program.output_types() != vec![Ty::I64]
    {
        return Ok(None);
    }
    let nodes = hot.program.nodes();
    if nodes.is_empty() || nodes.len() > 256 {
        return Ok(None);
    }

    // KASM v1.0 meta-ops require runtime support the scalar kernel
    // doesn't have (atlas lookup for prog hashes, vector ops, parallel
    // dispatch). Reject programs containing them so dispatch falls back
    // to the CPU brain transparently.
    //
    // Audit §1.5 (2026-05-01) — Op::Pipeline aligned with Rust
    // interpreter Wave 6 fail-loud : added to the reject list so a
    // program embedding Op::Pipeline never reaches the CUDA kernel
    // (where it used to silently pass-through `values[a]` and produce
    // a wrong result). The canonical use of Pipeline is
    // `MonsterNode::call_pipeline(prog_a, prog_b, args)` — never an
    // embedded bytecode node.
    use crate::kasm::Op;
    if nodes.iter().any(|n| matches!(
        n.op,
        Op::Grad | Op::Vmap | Op::Pmap | Op::Fori | Op::WhileLoop
        | Op::Reduce | Op::Scan | Op::Pipeline | Op::VLenI64
        | Op::VSumI64 | Op::VAddI64 | Op::VMulI64
        | Op::VSubI64 | Op::VMaxI64 | Op::VMinI64 | Op::VRangeI64
        | Op::VConcatI64 | Op::VReverseI64 | Op::VBroadcastI64
        | Op::VEqI64 | Op::VAndI64 | Op::VOrI64 | Op::VXorI64
        | Op::VAbsI64 | Op::VNegI64 | Op::VBitFlipI64
    )) {
        return Ok(None);
    }

    // Serialize the KASM program into 8 bytes per node (op, ty, a, b, imm).
    let mut packed_program = vec![0u8; nodes.len() * 8];
    for (i, n) in nodes.iter().enumerate() {
        let off = i * 8;
        packed_program[off] = n.op as u8;
        packed_program[off + 1] = n.ty as u8;
        packed_program[off + 2..off + 4].copy_from_slice(&n.a.to_le_bytes());
        packed_program[off + 4..off + 6].copy_from_slice(&n.b.to_le_bytes());
        packed_program[off + 6..off + 8].copy_from_slice(&n.imm.to_le_bytes());
    }

    // Extract i64 inputs (one per call).
    let mut inputs: Vec<i64> = Vec::with_capacity(calls.len());
    for c in calls {
        let bytes = c.args.get(0..8).ok_or_else(|| {
            io::Error::other("cuda_min: bad KASM input length (expected 8 bytes per call)")
        })?;
        inputs.push(i64::from_le_bytes(bytes.try_into().unwrap()));
    }

    // Run via the Forge cuda_min runtime.
    let runtime = crate::cuda_min::kasm_runtime()?;
    let outputs = runtime.run_batch(&packed_program, nodes.len() as u32, &inputs)?;

    let mut packed = PackedOutput::with_capacity(calls.len(), calls.len() * 8);
    for value in outputs {
        packed.push_result(&value.to_le_bytes());
    }
    Ok(Some(packed))
}

fn eval_serial(node: &MonsterNode, calls: &[BatchInput<'_>]) -> io::Result<PackedOutput> {
    // Parallel multi-worker evaluation when batch is large enough to
    // amortize thread setup. Pour 100k k-mers ADN sur 8 cores Windows :
    // single-thread = ~500 ms, 8 workers = ~62 ms (8x speedup attendu
    // sur les workloads où chaque call est non-trivial).
    //
    // Doctrine §9 (CLAUDE.md) : "Le user a 2 GPU AMD + NVIDIA, il ne
    // faut pas qu'un seul travaille pendant que l'autre dort." Cette
    // parallélisation s'applique au CPU pour le moment ; multi-GPU
    // simultané demande wgpu universal kernel + cuda + balance, plus
    // gros chantier (cf. 619738a "Lacune architecturale").
    //
    // Threshold : batch ≥ 1024. En dessous, le coût thread::scope
    // dépasse le gain. (1024 × 5 µs = 5 ms total, négligeable seq.)
    const PARALLEL_THRESHOLD: usize = 1024;

    if calls.len() < PARALLEL_THRESHOLD {
        return eval_serial_single(node, calls);
    }

    // Detect CPU count. Sur Windows std::thread::available_parallelism
    // retourne le nombre de logical cores (HT inclus).
    let workers = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .min(16);
    if workers <= 1 {
        return eval_serial_single(node, calls);
    }

    let chunk_size = (calls.len() + workers - 1) / workers;
    // Pre-allocate output slots in original order — each worker writes
    // its chunk's PackedOutput independently, we splice them in order.
    let mut chunks: Vec<io::Result<PackedOutput>> = Vec::with_capacity(workers);
    for _ in 0..workers {
        chunks.push(Ok(PackedOutput::default()));
    }

    std::thread::scope(|scope| {
        let mut handles = Vec::with_capacity(workers);
        for (worker_idx, chunk) in calls.chunks(chunk_size).enumerate() {
            let h = scope.spawn(move || (worker_idx, eval_serial_single(node, chunk)));
            handles.push(h);
        }
        for h in handles {
            let (idx, result) = h.join().unwrap_or((0, Err(io::Error::other("worker panicked"))));
            chunks[idx] = result;
        }
    });

    // Merge ordered chunks into single PackedOutput.
    let total_results: usize = chunks.iter().map(|r| match r {
        Ok(p) => p.offsets.len().saturating_sub(1),
        Err(_) => 0,
    }).sum();
    let total_bytes: usize = chunks.iter().map(|r| match r {
        Ok(p) => p.bytes.len(),
        Err(_) => 0,
    }).sum();
    let mut merged = PackedOutput::with_capacity(total_results, total_bytes);
    for chunk_result in chunks {
        let chunk = chunk_result?;
        let n = chunk.offsets.len().saturating_sub(1);
        for i in 0..n {
            merged.push_result(chunk.slice(i));
        }
    }
    Ok(merged)
}

/// WGPU universal KASM interpreter — équivalent du cuda_min PTX en
/// WGSL. Supporte les opcodes i64-only que le user emploie sur le hot
/// path (k-mer hash, mixers, conditional). Programmes hors-spec
/// (Vec, F64, meta-ops Wave 8) → returns None, fallback à cuda_min.
///
/// Couvre : Input, ConstI64, AddI64, MulI64, SubI64, EqI64, Hash64,
/// MinI64, MaxI64, LtI64, LeI64, BitAndI64, BitOrI64, BitXorI64,
/// ShlI64, ShrI64, BitFlipI64, NegI64, Cond, Output.
///
/// Limites :
///   - 1 input i64, 1 output i64 (extensible)
///   - ≤ 64 nodes (stack size dans le shader)
///   - GPU doit supporter SHADER_INT64 (NVIDIA RTX, AMD RDNA 2+,
///     Apple M1+, Intel Arc — la plupart du hardware modern)
#[cfg(feature = "wgpu")]
fn try_eval_wgpu_universal(
    node: &MonsterNode,
    calls: &[BatchInput<'_>],
) -> io::Result<Option<PackedOutput>> {
    use crate::kasm::Op;

    let func = calls[0].func;
    if calls.iter().any(|c| c.func != func) {
        return Ok(None);
    }
    let hot = node.hot_program(&func)?;
    let prog = &hot.program;

    // Spec-check : 1 input i64, 1 output i64, ≤ 64 nodes.
    if prog.inputs() != 1 || prog.outputs() != 1 || prog.output_types() != vec![Ty::I64] {
        return Ok(None);
    }
    let nodes_v = prog.nodes();
    if nodes_v.len() > 128 || nodes_v.is_empty() {
        return Ok(None);
    }
    if prog.target().needs_external_backend() {
        return Ok(None);
    }

    // Encode program: 4 u32 per node (op, a, b, imm).
    // Vérifie aussi que tous les opcodes sont supportés par le shader ;
    // sinon return None (fallback CUDA / CPU).
    let mut prog_buf: Vec<u32> = Vec::with_capacity(nodes_v.len() * 4);
    for n in nodes_v {
        let op_id = n.op as u32;
        // Whitelist des opcodes implémentés dans le shader. Tout autre
        // → fallback.
        match n.op {
            Op::Input | Op::ConstI64 | Op::AddI64 | Op::MulI64 | Op::EqI64
            | Op::Hash64 | Op::Output | Op::SubI64 | Op::MinI64 | Op::MaxI64
            | Op::LtI64 | Op::LeI64 | Op::BitAndI64 | Op::BitOrI64
            | Op::BitXorI64 | Op::ShlI64 | Op::ShrI64 | Op::BitFlipI64
            | Op::NegI64 | Op::Cond => {}
            _ => return Ok(None),
        }
        // Bool/Vec types not supported by this shader stack (i64-only).
        if !matches!(n.ty, Ty::I64 | Ty::Bool) {
            return Ok(None);
        }
        prog_buf.push(op_id);
        prog_buf.push(n.a as u32);
        prog_buf.push(n.b as u32);
        prog_buf.push(n.imm as i32 as u32);  // signed → bitcast preserved
    }

    // Inputs : 1 i64 per call.
    let mut input_buf: Vec<i64> = Vec::with_capacity(calls.len());
    for c in calls {
        let bytes = c.args.get(0..8).ok_or_else(|| {
            io::Error::other("bad KASM input length for wgpu universal kernel")
        })?;
        input_buf.push(i64::from_le_bytes(bytes.try_into().unwrap()));
    }

    let n_nodes = nodes_v.len() as u32;
    let out = match run_wgpu_universal_i64(&prog_buf, n_nodes, &input_buf) {
        Ok(v) => v,
        Err(e) => {
            // Most common failure: GPU lacks SHADER_INT64 feature, or
            // shader compile error. Don't promote to error — fallback.
            eprintln!("[gpunode] wgpu universal kernel unavailable: {e}");
            return Ok(None);
        }
    };
    if out.len() != calls.len() {
        return Err(io::Error::other("wgpu universal kernel returned wrong output length"));
    }

    let mut packed = PackedOutput::with_capacity(calls.len(), calls.len() * 8);
    for value in out {
        packed.push_result(&value.to_le_bytes());
    }
    Ok(Some(packed))
}

/// Public test helper : run the universal WGSL kernel directly on
/// a Program + inputs, return raw i64 outputs (no Forge cache layer).
/// Used by `monster_bench gpu_correctness_strict` to validate that
/// wgpu_universal produces bit-pour-bit identical results to
/// `kasm::execute`. Returns None if the program is out of spec
/// (Vec, F64, multi-output, > 64 nodes, etc.).
#[cfg(feature = "wgpu")]
pub fn run_wgpu_universal_for_test(
    program: &crate::kasm::Program,
    inputs: &[i64],
) -> io::Result<Option<Vec<i64>>> {
    use crate::kasm::Op;
    if program.inputs() != 1 || program.outputs() != 1 || program.output_types() != vec![Ty::I64] {
        return Ok(None);
    }
    let nodes_v = program.nodes();
    if nodes_v.len() > 128 || nodes_v.is_empty() {
        return Ok(None);
    }
    if program.target().needs_external_backend() {
        return Ok(None);
    }
    let mut prog_buf: Vec<u32> = Vec::with_capacity(nodes_v.len() * 4);
    for n in nodes_v {
        match n.op {
            Op::Input | Op::ConstI64 | Op::AddI64 | Op::MulI64 | Op::EqI64
            | Op::Hash64 | Op::Output | Op::SubI64 | Op::MinI64 | Op::MaxI64
            | Op::LtI64 | Op::LeI64 | Op::BitAndI64 | Op::BitOrI64
            | Op::BitXorI64 | Op::ShlI64 | Op::ShrI64 | Op::BitFlipI64
            | Op::NegI64 | Op::Cond => {}
            _ => return Ok(None),
        }
        if !matches!(n.ty, Ty::I64 | Ty::Bool) {
            return Ok(None);
        }
        prog_buf.push(n.op as u32);
        prog_buf.push(n.a as u32);
        prog_buf.push(n.b as u32);
        prog_buf.push(n.imm as i32 as u32);
    }
    let n_nodes = nodes_v.len() as u32;
    match run_wgpu_universal_i64(&prog_buf, n_nodes, inputs) {
        Ok(v) => Ok(Some(v)),
        Err(e) => {
            eprintln!("[wgpu_universal_for_test] kernel unavailable: {e}");
            Ok(None)
        }
    }
}

/// Run the universal WGSL interpreter on a flat program buffer + inputs.
#[cfg(feature = "wgpu")]
fn run_wgpu_universal_i64(
    prog: &[u32],
    n_nodes: u32,
    inputs: &[i64],
) -> io::Result<Vec<i64>> {
    use std::sync::mpsc;

    use pollster::block_on;
    use wgpu::{
        Backends, BufferDescriptor, BufferUsages, CommandEncoderDescriptor, ComputePassDescriptor,
        ComputePipelineDescriptor, DeviceDescriptor, Features, Instance, InstanceDescriptor,
        Limits, MapMode, PipelineLayoutDescriptor, PollType, PowerPreference,
        RequestAdapterOptions, ShaderModuleDescriptor, ShaderSource,
    };

    let instance = Instance::new(InstanceDescriptor {
        backends: Backends::all(),
        flags: Default::default(),
        backend_options: Default::default(),
        display: Default::default(),
        memory_budget_thresholds: Default::default(),
    });
    // Pour le multi-GPU split, on demande le LowPower adapter ici (typically
    // AMD/Intel sur portables) pour ne pas se battre avec CUDA pour la
    // NVIDIA. Si LowPower indisponible → HighPerformance.
    let adapter = match block_on(instance.request_adapter(&RequestAdapterOptions {
        power_preference: PowerPreference::LowPower,
        force_fallback_adapter: false,
        compatible_surface: None,
    })) {
        Ok(a) => a,
        Err(_) => block_on(instance.request_adapter(&RequestAdapterOptions {
            power_preference: PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
        }))
        .map_err(|_| io::Error::other("wgpu request_adapter failed"))?,
    };

    let (device, queue) = block_on(adapter.request_device(&DeviceDescriptor {
        label: Some("forge-gpunode-universal-device"),
        required_features: Features::SHADER_INT64,
        required_limits: Limits::default(),
        experimental_features: Default::default(),
        memory_hints: Default::default(),
        trace: Default::default(),
    }))
    .map_err(|e| io::Error::other(format!("wgpu request_device failed (SHADER_INT64?): {e}")))?;

    // Universal KASM interpreter shader. Supporte les ~20 opcodes i64-only
    // les plus courants (k-mer hash, mixers, conditional).
    let shader_src = r#"
struct ProgramHeader {
    n_nodes: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
}

@group(0) @binding(0) var<storage, read>       prog_nodes: array<u32>;
@group(0) @binding(1) var<storage, read>       inputs:     array<i64>;
@group(0) @binding(2) var<storage, read_write> outputs:    array<i64>;
@group(0) @binding(3) var<uniform>             header:     ProgramHeader;

fn hash_i64(v: i64) -> i64 {
    var x: u64 = bitcast<u64>(v);
    x = x + 0x9e3779b97f4a7c15lu;
    x = (x ^ (x >> 30u)) * 0xbf58476d1ce4e5b9lu;
    x = (x ^ (x >> 27u)) * 0x94d049bb133111eblu;
    return bitcast<i64>(x ^ (x >> 31u));
}

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let call_idx: u32 = gid.x;
    if (call_idx >= arrayLength(&inputs)) { return; }

    let arg: i64 = inputs[call_idx];
    var stack: array<i64, 128>;

    var i: u32 = 0u;
    loop {
        if (i >= header.n_nodes) { break; }
        let base: u32 = i * 4u;
        let op:   u32 = prog_nodes[base];
        let a:    u32 = prog_nodes[base + 1u];
        let b:    u32 = prog_nodes[base + 2u];
        let imm:  i32 = bitcast<i32>(prog_nodes[base + 3u]);

        var v: i64 = 0li;
        switch op {
            case 0u: { v = arg; }                                        // Input
            case 1u: { v = i64(imm); }                                   // ConstI64
            case 2u: { v = stack[a] + stack[b]; }                        // AddI64
            case 3u: { v = stack[a] * stack[b]; }                        // MulI64
            case 4u: { v = select(0li, 1li, stack[a] == stack[b]); }     // EqI64
            case 5u: { v = hash_i64(stack[a]); }                         // Hash64
            case 6u: { outputs[call_idx] = stack[a]; return; }           // Output
            case 7u: { v = stack[a] - stack[b]; }                        // SubI64
            case 9u: { v = min(stack[a], stack[b]); }                    // MinI64
            case 10u: { v = max(stack[a], stack[b]); }                   // MaxI64
            case 15u: { v = select(0li, 1li, stack[a] < stack[b]); }     // LtI64
            case 16u: { v = select(0li, 1li, stack[a] <= stack[b]); }    // LeI64
            case 17u: { v = stack[a] & stack[b]; }                       // BitAndI64
            case 18u: { v = stack[a] | stack[b]; }                       // BitOrI64
            case 19u: { v = stack[a] ^ stack[b]; }                       // BitXorI64
            case 20u: {                                                  // ShlI64
                let s: u32 = u32(stack[b]) & 63u;
                v = bitcast<i64>(bitcast<u64>(stack[a]) << s);
            }
            case 21u: {                                                  // ShrI64
                let s: u32 = u32(stack[b]) & 63u;
                v = bitcast<i64>(bitcast<u64>(stack[a]) >> s);
            }
            case 28u: { v = ~stack[a]; }                                 // BitFlipI64
            case 29u: { v = -stack[a]; }                                 // NegI64
            case 37u: {                                                  // Cond (pred=a, then=b, else=imm)
                let pred: bool = stack[a] != 0li;
                let chosen_idx: u32 = select(u32(imm) & 0xFFFFu, b, pred);
                v = stack[chosen_idx];
            }
            default: { v = 0li; }
        }
        stack[i] = v;
        i = i + 1u;
    }
}
"#;
    let shader = device.create_shader_module(ShaderModuleDescriptor {
        label: Some("forge-wgpu-universal-i64"),
        source: ShaderSource::Wgsl(shader_src.into()),
    });

    let prog_bytes_len = (prog.len() * 4) as u64;
    let prog_buf_data: Vec<u8> = prog.iter().flat_map(|x| x.to_le_bytes()).collect();
    let prog_buf = device.create_buffer(&BufferDescriptor {
        label: Some("forge-univ-prog"),
        size: prog_bytes_len,
        usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&prog_buf, 0, &prog_buf_data);

    let in_bytes_len = (inputs.len() * 8) as u64;
    let in_data: Vec<u8> = inputs.iter().flat_map(|x| x.to_le_bytes()).collect();
    let in_buf = device.create_buffer(&BufferDescriptor {
        label: Some("forge-univ-input"),
        size: in_bytes_len,
        usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&in_buf, 0, &in_data);

    let out_buf = device.create_buffer(&BufferDescriptor {
        label: Some("forge-univ-output"),
        size: in_bytes_len,
        usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });

    let header_bytes = {
        let mut v = Vec::with_capacity(16);
        v.extend_from_slice(&n_nodes.to_le_bytes());
        v.extend_from_slice(&0u32.to_le_bytes());
        v.extend_from_slice(&0u32.to_le_bytes());
        v.extend_from_slice(&0u32.to_le_bytes());
        v
    };
    let header_buf = device.create_buffer(&BufferDescriptor {
        label: Some("forge-univ-header"),
        size: 16,
        usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&header_buf, 0, &header_bytes);

    let readback = device.create_buffer(&BufferDescriptor {
        label: Some("forge-univ-readback"),
        size: in_bytes_len,
        usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("forge-univ-layout"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0, visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false, min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1, visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false, min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2, visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false, min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 3, visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false, min_binding_size: None,
                },
                count: None,
            },
        ],
    });
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("forge-univ-bind-group"),
        layout: &layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: prog_buf.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: in_buf.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 2, resource: out_buf.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 3, resource: header_buf.as_entire_binding() },
        ],
    });

    let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some("forge-univ-pipeline-layout"),
        bind_group_layouts: &[Some(&layout)],
        immediate_size: 0,
    });
    let pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
        label: Some("forge-univ-pipeline"),
        layout: Some(&pipeline_layout),
        module: &shader,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: None,
    });

    let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
        label: Some("forge-univ-encoder"),
    });
    {
        let mut cpass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("forge-univ-pass"),
            timestamp_writes: None,
        });
        cpass.set_pipeline(&pipeline);
        cpass.set_bind_group(0, &bind_group, &[]);
        let wg_count = ((inputs.len() as u32) + 63) / 64;
        cpass.dispatch_workgroups(wg_count.max(1), 1, 1);
    }
    encoder.copy_buffer_to_buffer(&out_buf, 0, &readback, 0, in_bytes_len);
    queue.submit([encoder.finish()]);

    let slice = readback.slice(..);
    let (tx, rx) = mpsc::channel();
    slice.map_async(MapMode::Read, move |res| { let _ = tx.send(res); });
    let _ = device.poll(PollType::wait_indefinitely());
    let map_res = rx.recv()
        .map_err(|_| io::Error::other("wgpu readback channel failed"))?;
    map_res.map_err(|e| io::Error::other(format!("wgpu map_async failed: {e}")))?;

    let view = slice.get_mapped_range();
    let mut out = Vec::with_capacity(inputs.len());
    for chunk in view.chunks_exact(8) {
        out.push(i64::from_le_bytes(chunk.try_into().unwrap()));
    }
    drop(view);
    readback.unmap();
    Ok(out)
}

/// Détecte si TOUS les calls du batch ciblent un programme CPU-auto-
/// routable (HotPlan rule fast lane OU Interpret ≤ 64 nodes). Si oui,
/// le path CPU `eval_serial` traite à 75-300 ns/call via les bypass
/// v0/v1/v2 — beaucoup plus rapide que n'importe quel GPU pour ces
/// tailles. Le caller doit donc skip GPU et router au CPU.
/// Φ.ν.7g — Seuils pour décider GPU sur volume cumulé (ajusté post-test
/// 2026-05-03 pour activer GPU sur tous les stages alpha synth).
///
/// L'heuristique CPU-mieux-que-GPU originale (`is_cpu_auto_routable`)
/// regarde uniquement la TAILLE du programme. Mais pour la synth alpha,
/// on évalue des candidats de 16-64 nodes sur 17 567 examples chacun.
/// Volume cumulé = nodes × batch va de 281k à 1.12M.
///
/// Critère étendu : un programme considéré CPU-routable seul devient
/// HEAVY (= GPU paye) si :
///   - nodes ≥ HEAVY_NODES_THRESHOLD (16) — pas trivial Léger
///   - ET batch × nodes ≥ HEAVY_VOLUME_THRESHOLD (250 000)
///
/// Avec ces seuils :
///   - DNA splitmix1  (3 nodes × 100k = 300k)  → CPU (nodes < 16)
///   - DNA complement (4 nodes × 100k = 400k)  → CPU (nodes < 16)
///   - DNA strobemer  (7 nodes × 100k = 700k)  → CPU (nodes < 16)
///   - DNA spaced     (5 nodes × 100k = 500k)  → CPU (nodes < 16)
///   - DNA minhash10  (41 nodes × 100k = 4.1M) → GPU
///   - Alpha stage 1  (16 nodes × 17k = 281k)  → GPU ✓
///   - Alpha stage 2  (32 nodes × 17k = 562k)  → GPU ✓
///   - Alpha stages 3-4 (48-64 nodes × 17k)    → GPU ✓
const HEAVY_NODES_THRESHOLD: usize = 16;
const HEAVY_VOLUME_THRESHOLD: usize = 50_000;

fn all_calls_cpu_routable(
    node: &MonsterNode,
    calls: &[BatchInput<'_>],
) -> io::Result<bool> {
    use std::collections::HashSet;
    let mut seen: HashSet<crate::Hash> = HashSet::new();
    let mut max_program_nodes: usize = 0;
    for c in calls {
        if !seen.insert(c.func) { continue; }
        let hot = node.hot_program(&c.func)?;
        if !super::hotplan::is_cpu_auto_routable(&hot) {
            return Ok(false);
        }
        max_program_nodes = max_program_nodes.max(hot.program.nodes().len());
    }

    // Φ.ν.7g — Vérification volume cumulé. Si nodes ≥ 30 ET batch ×
    // nodes ≥ 500k, on considère que le GPU paye malgré l'auto-router
    // CPU. Permet d'activer GPU pour la synth alpha (stages 2-4).
    let cumulative_volume = max_program_nodes.saturating_mul(calls.len());
    if max_program_nodes >= HEAVY_NODES_THRESHOLD
        && cumulative_volume >= HEAVY_VOLUME_THRESHOLD
    {
        return Ok(false); // NE PAS forcer CPU → permet GPU dispatch
    }

    Ok(true)
}

/// Multi-GPU split : NVIDIA cuda_min + AMD/Intel wgpu en parallèle
/// via thread::scope. La 1ère moitié du batch va sur CUDA, la 2nde
/// sur WGPU. Les deux GPU travaillent simultanément, aucune n'attend.
///
/// Returns :
///   - Ok(Some(merged))  : split réussi, packed merged in original order
///   - Ok(None)          : un des deux paths a refusé (programme
///                         incompatible avec wgpu actuel = non-Affine)
///                         → caller fallback sur single-GPU
///   - Err(_)            : erreur fatale
///
/// Limitation actuelle : try_eval_wgpu_affine ne handle que AffineI64.
/// Pour étendre à tout programme KASM, il faut un universal WGSL kernel
/// (équivalent du cuda_min PTX). C'est un chantier séparé.
#[cfg(all(feature = "cuda", feature = "wgpu"))]
fn try_eval_split_cuda_wgpu(
    node: &MonsterNode,
    calls: &[BatchInput<'_>],
) -> io::Result<Option<PackedOutput>> {
    let half = calls.len() / 2;
    let (cuda_chunk, wgpu_chunk) = calls.split_at(half);

    let mut cuda_result: io::Result<Option<PackedOutput>> = Ok(None);
    let mut wgpu_result: io::Result<Option<PackedOutput>> = Ok(None);

    std::thread::scope(|s| {
        let h_cuda = s.spawn(|| {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                try_eval_cuda_min(node, cuda_chunk)
            }))
            .unwrap_or_else(|_| Ok(None))
        });
        // WGPU side: try universal kernel first (handles 20+ opcodes
        // including Hash64/XOR/Cond), fallback à wgpu_affine pour
        // AffineI64 spécifique.
        let h_wgpu = s.spawn(|| {
            match try_eval_wgpu_universal(node, wgpu_chunk) {
                Ok(Some(p)) => Ok(Some(p)),
                Ok(None) => try_eval_wgpu_affine(node, wgpu_chunk),
                Err(e) => Err(e),
            }
        });

        cuda_result = h_cuda.join().unwrap_or(Ok(None));
        wgpu_result = h_wgpu.join().unwrap_or(Ok(None));
    });

    let cuda_packed = match cuda_result? {
        Some(p) => p,
        None => return Ok(None),
    };
    let wgpu_packed = match wgpu_result? {
        Some(p) => p,
        None => return Ok(None),
    };

    // Merge in original order : cuda_chunk results first, then wgpu_chunk.
    let total_n = calls.len();
    let total_bytes = cuda_packed.bytes.len() + wgpu_packed.bytes.len();
    let mut merged = PackedOutput::with_capacity(total_n, total_bytes);
    let n_cuda = cuda_packed.offsets.len().saturating_sub(1);
    for i in 0..n_cuda {
        merged.push_result(cuda_packed.slice(i));
    }
    let n_wgpu = wgpu_packed.offsets.len().saturating_sub(1);
    for i in 0..n_wgpu {
        merged.push_result(wgpu_packed.slice(i));
    }
    Ok(Some(merged))
}

/// Single-threaded core for one chunk. Used by `eval_serial` directly
/// (small batches) and by each worker thread (large batches).
///
/// Applique les mêmes bypass que `call_one_i64` (auto-router v0/v1/v2)
/// pour shorter le path quand le programme est cheap (AffineI64 /
/// HashChain / small Interpret). Crucial pour le parallélisme : sans
/// bypass, chaque worker passe par dispatch_impl qui prend des locks
/// (RwLock cache, Mutex LRU, RwLock oracles) → contention serialise
/// les threads, le parallélisme ne paye pas.
///
/// Avec bypass, les workers ne touchent AUCUN état partagé écrivable
/// pour les programmes Léger → vraie concurrence linéaire.
fn eval_serial_single(node: &MonsterNode, calls: &[BatchInput<'_>]) -> io::Result<PackedOutput> {
    let mut packed = PackedOutput::with_capacity(calls.len(), calls.len() * 8);
    let mut hot_cache: HashMap<crate::Hash, Arc<HotProgram>> = HashMap::new();
    let mut last: Option<(crate::Hash, Arc<HotProgram>)> = None;
    for c in calls {
        let hot = match &last {
            Some((h, hot)) if *h == c.func => Arc::clone(hot),
            _ => {
                let h = if let Some(h) = hot_cache.get(&c.func) {
                    Arc::clone(h)
                } else {
                    let h = node.hot_program(&c.func)?;
                    hot_cache.insert(c.func, Arc::clone(&h));
                    h
                };
                last = Some((c.func, Arc::clone(&h)));
                h
            }
        };

        // Auto-router fast paths (mêmes que call_one_i64).
        // Tout cela suppose 1 input i64 / 1 output i64 (8 bytes args
        // et result), garde-fou silencieux en haut.
        if c.args.len() == 8 && hot.explicit_memos.is_empty() {
            let arg = i64::from_le_bytes(c.args.try_into().unwrap());

            // v0 : AffineI64
            if let HotPlan::AffineI64 { input_slot: 0, mul, add } = hot.plan {
                let result = arg.wrapping_mul(mul).wrapping_add(add);
                packed.push_result(&result.to_le_bytes());
                continue;
            }

            // v1 : HashChain
            if let HotPlan::HashChain { input_slot: 0, rounds } = hot.plan {
                let mut value = arg;
                for _ in 0..rounds {
                    value = crate::kasm::hash_i64(value);
                }
                packed.push_result(&value.to_le_bytes());
                continue;
            }

            // v2 : small Interpret via stack interp (no Vec alloc, no lock)
            if matches!(hot.plan, HotPlan::Interpret)
                && hot.program.nodes().len() <= 64
                && !hot.program.target().needs_external_backend()
            {
                if let Some(result) = crate::kasm::try_execute_i64_inline(&hot.program, arg) {
                    packed.push_result(&result.to_le_bytes());
                    continue;
                }
            }
        }

        // Fallback : full dispatch via call_value_bytes_hot_args.
        // Cette voie prend les locks dispatch_impl, mais on ne peut pas
        // shorter sans casser les contrats (ex : explicit_memos).
        let value = node.call_value_bytes_hot_args(&hot, c.args)?;
        packed.push_result(&value.bytes);
    }
    Ok(packed)
}


#[cfg(feature = "wgpu")]
fn try_eval_wgpu_affine(
    node: &MonsterNode,
    calls: &[BatchInput<'_>],
) -> io::Result<Option<PackedOutput>> {
    let func = calls[0].func;
    if calls.iter().any(|c| c.func != func) {
        return Ok(None);
    }

    let hot = node.hot_program(&func)?;
    if hot.program.inputs() != 1 || hot.program.outputs() != 1 || hot.program.output_types() != vec![Ty::I64] {
        return Ok(None);
    }

    let (input_slot, mul, add) = match hot.plan {
        HotPlan::AffineI64 { input_slot, mul, add } => (input_slot, mul, add),
        _ => return Ok(None),
    };
    if input_slot != 0 {
        return Ok(None);
    }

    let mut inputs = Vec::with_capacity(calls.len());
    for c in calls {
        let bytes = c
            .args
            .get(0..8)
            .ok_or_else(|| io::Error::other("bad KASM input length for GPUnode affine kernel"))?;
        inputs.push(i64::from_le_bytes(bytes.try_into().unwrap()));
    }

    let out = match run_wgpu_affine_i64(&inputs, mul, add) {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };
    if out.len() != calls.len() {
        return Err(io::Error::other("GPUnode affine kernel returned wrong output length"));
    }

    let mut packed = PackedOutput::with_capacity(calls.len(), calls.len() * 8);
    for value in out {
        packed.push_result(&value.to_le_bytes());
    }
    Ok(Some(packed))
}

#[cfg(feature = "wgpu")]
fn run_wgpu_affine_i64(inputs: &[i64], mul: i64, add: i64) -> io::Result<Vec<i64>> {
    use std::sync::mpsc;

    use pollster::block_on;
    use wgpu::{
        Backends, BufferDescriptor, BufferUsages, CommandEncoderDescriptor, ComputePassDescriptor,
        ComputePipelineDescriptor, DeviceDescriptor, Features, Instance, InstanceDescriptor,
        Limits, MapMode, PipelineLayoutDescriptor, PollType, PowerPreference,
        RequestAdapterOptions, ShaderModuleDescriptor, ShaderSource,
    };

    let instance = Instance::new(InstanceDescriptor {
        backends: Backends::all(),
        flags: Default::default(),
        backend_options: Default::default(),
        display: Default::default(),
        memory_budget_thresholds: Default::default(),
    });
    let adapter = block_on(instance.request_adapter(&RequestAdapterOptions {
        power_preference: PowerPreference::HighPerformance,
        force_fallback_adapter: false,
        compatible_surface: None,
    }))
    .map_err(|_| io::Error::other("wgpu request_adapter failed"))?;

    let (device, queue) = block_on(adapter.request_device(&DeviceDescriptor {
        label: Some("forge-gpunode-device"),
        required_features: Features::empty(),
        required_limits: Limits::default(),
        experimental_features: Default::default(),
        memory_hints: Default::default(),
        trace: Default::default(),
    }))
    .map_err(|e| io::Error::other(format!("wgpu request_device failed: {e}")))?;

    let shader = device.create_shader_module(ShaderModuleDescriptor {
        label: Some("forge-affine-i64"),
        source: ShaderSource::Wgsl(
            "
struct Params {
  mul: i64,
  add: i64,
  len: u32,
  _pad: u32,
}

@group(0) @binding(0)
var<storage, read> in_vals: array<i64>;

@group(0) @binding(1)
var<storage, read_write> out_vals: array<i64>;

@group(0) @binding(2)
var<uniform> params: Params;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
  let idx = gid.x;
  if (idx >= params.len) {
    return;
  }
  let x = in_vals[idx];
  out_vals[idx] = x * params.mul + params.add;
}
            "
            .into(),
        ),
    });

    let bytes_len = (inputs.len() * 8) as u64;
    let params_bytes = {
        let mut v = Vec::with_capacity(24);
        v.extend_from_slice(&mul.to_le_bytes());
        v.extend_from_slice(&add.to_le_bytes());
        v.extend_from_slice(&(inputs.len() as u32).to_le_bytes());
        v.extend_from_slice(&0u32.to_le_bytes());
        v
    };

    let input_bytes: Vec<u8> = inputs.iter().flat_map(|x| x.to_le_bytes()).collect();
    let input = device.create_buffer(&BufferDescriptor {
        label: Some("forge-affine-input"),
        size: bytes_len,
        usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&input, 0, &input_bytes);

    let output = device.create_buffer(&BufferDescriptor {
        label: Some("forge-affine-output"),
        size: bytes_len,
        usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });

    let params = device.create_buffer(&BufferDescriptor {
        label: Some("forge-affine-params"),
        size: params_bytes.len() as u64,
        usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&params, 0, &params_bytes);

    let readback = device.create_buffer(&BufferDescriptor {
        label: Some("forge-affine-readback"),
        size: bytes_len,
        usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("forge-affine-layout"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    });
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("forge-affine-bind-group"),
        layout: &layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: input.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: output.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: params.as_entire_binding(),
            },
        ],
    });

    let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some("forge-affine-pipeline-layout"),
        bind_group_layouts: &[Some(&layout)],
        immediate_size: 0,
    });
    let pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
        label: Some("forge-affine-pipeline"),
        layout: Some(&pipeline_layout),
        module: &shader,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: None,
    });

    let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
        label: Some("forge-affine-encoder"),
    });
    {
        let mut cpass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("forge-affine-pass"),
            timestamp_writes: None,
        });
        cpass.set_pipeline(&pipeline);
        cpass.set_bind_group(0, &bind_group, &[]);
        let wg_count = ((inputs.len() as u32) + 63) / 64;
        cpass.dispatch_workgroups(wg_count.max(1), 1, 1);
    }
    encoder.copy_buffer_to_buffer(&output, 0, &readback, 0, bytes_len);
    queue.submit([encoder.finish()]);

    let slice = readback.slice(..);
    let (tx, rx) = mpsc::channel();
    slice.map_async(MapMode::Read, move |res| {
        let _ = tx.send(res);
    });
    let _ = device.poll(PollType::wait_indefinitely());
    let map_res = rx
        .recv()
        .map_err(|_| io::Error::other("wgpu readback channel failed"))?;
    map_res.map_err(|e| io::Error::other(format!("wgpu map_async failed: {e}")))?;

    let view = slice.get_mapped_range();
    let mut out = Vec::with_capacity(inputs.len());
    for chunk in view.chunks_exact(8) {
        out.push(i64::from_le_bytes(chunk.try_into().unwrap()));
    }
    drop(view);
    readback.unmap();
    Ok(out)
}

/// Best-effort first-install bootstrap.
///
/// On success, returns persisted or newly-created plan.
/// On failure, degrades to a non-persistent in-memory detection.
pub fn bootstrap_gpunodes_best_effort() -> GpuNodeBootstrap {
    match bootstrap_gpunodes() {
        Ok(plan) => plan,
        Err(_) => {
            let adapters = detect_gpu_adapter_names().unwrap_or_default();
            let n = adapters.len();
            let nvidia = adapters
                .iter()
                .any(|name| name.to_ascii_lowercase().contains("nvidia"));
            GpuNodeBootstrap {
                first_install: false,
                detected_gpu_count: n,
                deployed_gpunodes: n,
                nvidia_detected: nvidia,
                cuda_enabled: should_enable_cuda(nvidia),
                nvrtc_available: nvrtc_present(),
            }
        }
    }
}

/// First-install bootstrap with persistence.
pub fn bootstrap_gpunodes() -> io::Result<GpuNodeBootstrap> {
    bootstrap_gpunodes_at(Path::new(GPUNODE_BOOTSTRAP_PATH))
}

fn bootstrap_gpunodes_at(path: &Path) -> io::Result<GpuNodeBootstrap> {
    if let Ok(text) = fs::read_to_string(path) {
        if let Some(mut parsed) = parse_bootstrap(&text) {
            parsed.first_install = false;
            return Ok(parsed);
        }
    }

    let adapters = detect_gpu_adapter_names().unwrap_or_default();
    let detected = adapters.len();
    let nvidia = adapters
        .iter()
        .any(|name| name.to_ascii_lowercase().contains("nvidia"));
    let plan = GpuNodeBootstrap {
        first_install: true,
        detected_gpu_count: detected,
        deployed_gpunodes: detected,
        nvidia_detected: nvidia,
        cuda_enabled: should_enable_cuda(nvidia),
        nvrtc_available: nvrtc_present(),
    };
    write_bootstrap(path, &plan)?;
    Ok(plan)
}

fn write_bootstrap(path: &Path, plan: &GpuNodeBootstrap) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let payload = format!(
        "version=1\n\
detected_gpu_count={}\n\
deployed_gpunodes={}\n\
nvidia_detected={}\n\
cuda_enabled={}\n",
        plan.detected_gpu_count,
        plan.deployed_gpunodes,
        if plan.nvidia_detected { 1 } else { 0 },
        if plan.cuda_enabled { 1 } else { 0 },
    );
    fs::write(path, payload)
}

fn parse_bootstrap(text: &str) -> Option<GpuNodeBootstrap> {
    let mut version: Option<usize> = None;
    let mut detected: Option<usize> = None;
    let mut deployed: Option<usize> = None;
    let mut nvidia_detected: Option<bool> = None;
    let mut cuda_enabled: Option<bool> = None;

    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (k, v) = line.split_once('=')?;
        let key = k.trim();
        let value = v.trim().parse::<usize>().ok()?;
        match key {
            "version" => version = Some(value),
            "detected_gpu_count" => detected = Some(value),
            "deployed_gpunodes" => deployed = Some(value),
            "nvidia_detected" => nvidia_detected = Some(value != 0),
            "cuda_enabled" => cuda_enabled = Some(value != 0),
            _ => {}
        }
    }

    if version != Some(1) {
        return None;
    }
    let detected_gpu_count = detected?;
    let deployed_gpunodes = deployed?;
    // nvrtc_available is recomputed every load — never trust the persisted
    // file for it (user may have just installed/removed the Toolkit).
    Some(GpuNodeBootstrap {
        first_install: false,
        detected_gpu_count,
        deployed_gpunodes,
        nvidia_detected: nvidia_detected.unwrap_or(false),
        cuda_enabled: cuda_enabled.unwrap_or(false),
        nvrtc_available: nvrtc_present(),
    })
}

fn should_enable_cuda(nvidia_detected: bool) -> bool {
    if let Some(forced) = cuda_forced_from_env() {
        return forced;
    }
    nvidia_detected && cuda_runtime_present()
}

fn cuda_forced_from_env() -> Option<bool> {
    match std::env::var("FORGE_CUDA").ok()?.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "on" => Some(true),
        "0" | "false" | "off" => Some(false),
        _ => None,
    }
}

fn cuda_runtime_present() -> bool {
    Command::new("nvidia-smi")
        .arg("--help")
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

/// True if the Vulkan loader is present on this system. Vulkan is the
/// primary backend used by WGPU on Linux/Windows, and is shipped with
/// modern GPU drivers (NVIDIA, AMD, Intel) — no separate install needed.
pub fn vulkan_available() -> bool {
    #[cfg(windows)]
    {
        if std::path::Path::new(r"C:\Windows\System32\vulkan-1.dll").exists() {
            return true;
        }
        if let Ok(path_var) = std::env::var("PATH") {
            for dir in path_var.split(';') {
                if dir.is_empty() {
                    continue;
                }
                if std::path::Path::new(dir).join("vulkan-1.dll").exists() {
                    return true;
                }
            }
        }
        false
    }
    #[cfg(unix)]
    {
        const CANDIDATES: &[&str] = &[
            "/usr/lib/x86_64-linux-gnu/libvulkan.so.1",
            "/usr/lib/x86_64-linux-gnu/libvulkan.so",
            "/usr/lib/libvulkan.so.1",
            "/usr/lib/libvulkan.so",
            "/usr/local/lib/libvulkan.so.1",
        ];
        CANDIDATES.iter().any(|p| std::path::Path::new(p).exists())
    }
    #[cfg(not(any(windows, unix)))]
    {
        false
    }
}

/// True if DirectX 12 runtime is present. Windows 10+ ships d3d12.dll
/// system-wide; on older Windows or non-Windows, returns false.
pub fn dx12_available() -> bool {
    #[cfg(windows)]
    {
        std::path::Path::new(r"C:\Windows\System32\d3d12.dll").exists()
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// Comprehensive GPU capability report — what acceleration paths are
/// available on this machine, per-GPU node identification, and what the
/// user can do to unlock more. Run once at install / startup so the user
/// sees the picture upfront.
pub fn gpu_capability_report() -> Vec<String> {
    let bs = bootstrap_gpunodes_best_effort();
    let vulkan = vulkan_available();
    let dx12 = dx12_available();
    let wgpu_compiled = cfg!(feature = "wgpu");
    let cuda_compiled = cfg!(feature = "cuda");
    let names = detect_gpu_adapter_names().unwrap_or_default();

    let mut lines = Vec::new();
    lines.push("=== Forge GPU capability check ===".to_string());

    // Per-node identification — which GPU is which, on which backend.
    if names.is_empty() {
        lines.push("  no GPU adapters detected".to_string());
    } else {
        for (idx, name) in names.iter().enumerate() {
            let vendor = GpuVendor::from_name(name);
            let backend = pick_backend(
                vendor,
                idx as u32,
                cuda_compiled,
                bs.nvrtc_available,
                wgpu_compiled,
            );
            lines.push(format!("  Node #{idx} — {}", name));
            lines.push(format!("    vendor : {}", vendor.label()));
            lines.push(format!("    backend: {}", backend.label()));
        }
    }

    lines.push(String::new());
    lines.push("  System capabilities:".to_string());
    lines.push(format!(
        "    CUDA driver          : {}",
        if bs.cuda_enabled { "present (nvidia-smi works)" } else { "absent" }
    ));
    lines.push(format!(
        "    CUDA Toolkit (nvrtc) : {}",
        if bs.nvrtc_available { "present (kernel JIT will work)" } else { "MISSING" }
    ));
    lines.push(format!(
        "    Vulkan loader        : {}",
        if vulkan { "present" } else { "absent" }
    ));
    lines.push(format!(
        "    DirectX 12           : {}",
        if dx12 { "present" } else { "absent" }
    ));
    lines.push(format!(
        "    Forge built w/ cuda  : {}",
        if cuda_compiled { "yes" } else { "no" }
    ));
    lines.push(format!(
        "    Forge built w/ wgpu  : {}",
        if wgpu_compiled { "yes" } else { "no" }
    ));

    let cuda_full = bs.nvidia_detected && bs.cuda_enabled && bs.nvrtc_available && cuda_compiled;
    let wgpu_ready = wgpu_compiled && (vulkan || dx12);

    if cuda_full {
        lines.push(
            "✓ CUDA fully active — kernel JIT compilation will run on NVIDIA GPU(s).".to_string(),
        );
        if cfg!(windows) {
            lines.push(
                "  Windows tip: ensure Forge has High-Performance GPU access (Settings → System".to_string(),
            );
            lines.push(
                "    → Display → Graphics → Browse → forge-ui.exe → Options → High performance)."
                    .to_string(),
            );
            lines.push(
                "    Without this, Windows may route Forge to the integrated GPU instead of NVIDIA."
                    .to_string(),
            );
        }
    } else if wgpu_ready {
        lines.push(
            "✓ WGPU active — vendor-agnostic GPU acceleration via Vulkan/DX12.".to_string(),
        );
        if cfg!(windows) {
            lines.push(
                "  Windows tip: ensure Forge has High-Performance GPU access (Settings → System".to_string(),
            );
            lines.push(
                "    → Display → Graphics → Browse → forge-ui.exe → Options → High performance)."
                    .to_string(),
            );
        }
    } else if bs.nvidia_detected && !bs.nvrtc_available {
        lines.push(
            "⚠ NVIDIA GPU detected but CUDA Toolkit is missing — kernel JIT cannot run."
                .to_string(),
        );
        lines.push("  If you JUST installed the Toolkit:".to_string());
        lines.push(
            "    Close ALL terminals + the Forge app, open a fresh terminal, then relaunch."
                .to_string(),
        );
        lines.push(
            "    Process PATH is frozen at start — running processes don't see new install paths."
                .to_string(),
        );
        lines.push("  Two ways to unlock GPU acceleration:".to_string());
        lines.push(
            "    (A) Install CUDA Toolkit (~3 GB) from https://developer.nvidia.com/cuda-downloads"
                .to_string(),
        );
        lines.push(
            "        — unlocks all CUDA features including custom runtime kernels.".to_string(),
        );
        lines.push(
            "    (B) Rebuild Forge with `--features wgpu` instead of `--features cuda`."
                .to_string(),
        );
        lines.push(
            "        — no download (uses your existing GPU driver via Vulkan/DX12)."
                .to_string(),
        );
        lines.push(
            "        — vendor-agnostic (works on NVIDIA, AMD, Intel)."
                .to_string(),
        );
    } else if !cuda_compiled && !wgpu_compiled {
        lines.push(
            "⚠ Forge was built without GPU support — CPU fallback only.".to_string(),
        );
        lines.push(
            "  Rebuild with `--features cuda` (NVIDIA only) or `--features wgpu` (any GPU)."
                .to_string(),
        );
    } else {
        lines.push("⚠ No GPU acceleration path available — CPU fallback only.".to_string());
    }

    lines.push("=== end GPU capability check ===".to_string());
    lines
}

/// True if the CUDA Toolkit (specifically nvrtc) is available — distinct
/// from the driver check `cuda_runtime_present`. Driver alone gives us
/// nvidia-smi and can run pre-compiled CUDA, but nvrtc.dll is needed to
/// JIT-compile PTX from CUDA C source at runtime, which is what cudarc
/// does. Four-step detection that survives stale process PATH after a
/// fresh Toolkit install: CUDA_PATH env, nvcc on PATH, standard install
/// dirs (Windows), then PATH DLL search.
fn nvrtc_present() -> bool {
    // 1. CUDA_PATH env var — set by the installer, points to the toolkit root.
    if let Ok(cuda_path) = std::env::var("CUDA_PATH") {
        let bin = std::path::Path::new(&cuda_path).join("bin");
        if dir_contains_nvrtc_dll(&bin) {
            return true;
        }
    }

    // 2. nvcc on PATH — works if user opened terminal AFTER install.
    if Command::new("nvcc")
        .arg("--version")
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
    {
        return true;
    }

    // 3. Windows standard install dir — survives stale PATH in current process.
    #[cfg(windows)]
    {
        const TOOLKIT_ROOT: &str = r"C:\Program Files\NVIDIA GPU Computing Toolkit\CUDA";
        if let Ok(entries) = std::fs::read_dir(TOOLKIT_ROOT) {
            for entry in entries.flatten() {
                let bin = entry.path().join("bin");
                if dir_contains_nvrtc_dll(&bin) {
                    return true;
                }
            }
        }
    }

    // 4. PATH-wide DLL search.
    if let Ok(path_var) = std::env::var("PATH") {
        let sep = if cfg!(windows) { ';' } else { ':' };
        for dir in path_var.split(sep) {
            if dir.is_empty() {
                continue;
            }
            if dir_contains_nvrtc_dll(std::path::Path::new(dir)) {
                return true;
            }
        }
    }

    false
}

use std::sync::Mutex;

#[derive(Debug)]
pub enum CudaStatus {
    /// Single-GPU CUDA only (cuda_min, NVIDIA seul ou pas de wgpu).
    Ok,
    /// Multi-GPU split firé : moitié batch sur CUDA, moitié sur WGPU,
    /// exécution parallèle via thread::scope. Aucune GPU idle.
    SplitOk,
    DriverTooOld(io::Error),
    Io(io::Error),
    Panicked(io::Error),
}

impl fmt::Display for CudaStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CudaStatus::Ok => {
                write!(f, "cuda_min: KASM interpreter executed via direct nvcuda.dll FFI")
            }
            CudaStatus::SplitOk => {
                write!(f, "split cuda_min + wgpu_universal in parallel (multi-GPU)")
            }
            CudaStatus::DriverTooOld(err) => write!(
                f,
                "cuda_min: NVIDIA driver too old for embedded PTX ({err})"
            ),
            CudaStatus::Io(err) => write!(f, "cuda_min io error: {err}"),
            CudaStatus::Panicked(err) => write!(f, "cuda_min panicked: {err}"),
        }
    }
}

static LAST_CUDA_STATUS: Mutex<Option<CudaStatus>> = Mutex::new(None);

/// Records the most recent outcome of a muscle-lane CUDA invocation
/// (kernel ran, fell back, panicked, etc.) so the caller can surface
/// it to the UI without having to check stderr.
#[cfg(feature = "cuda")]
pub(crate) fn set_last_cuda_status(status: CudaStatus) {
    if let Ok(mut g) = LAST_CUDA_STATUS.lock() {
        *g = Some(status);
    }
}

/// Returns and clears the last CUDA status set by gpunode. Call right
/// after `BulkEvaluator::eval_batch` to know what actually happened.
pub fn take_last_cuda_status() -> Option<CudaStatus> {
    LAST_CUDA_STATUS.lock().ok().and_then(|mut g| g.take())
}

/// Returns the directory containing nvrtc DLL, if findable. Same scan
/// order as nvrtc_present, but returns the actual path so callers can
/// register it with the Windows DLL loader (SetDllDirectoryW). Used by
/// `register_cuda_dll_path` to fix the stale-PATH issue: when the user
/// installs Toolkit while a shell is already open, that shell's PATH
/// is frozen and any process launched from it inherits the old PATH —
/// so cudarc's LoadLibrary fails even though the file exists on disk.
pub fn find_cuda_bin_dir() -> Option<std::path::PathBuf> {
    // Subdirs to check for nvrtc DLL — CUDA 12 used `bin/`, CUDA 13
    // moved DLLs to `bin/x64/`. Check both.
    const BIN_SUBDIRS: &[&str] = &["bin/x64", "bin"];

    if let Ok(cuda_path) = std::env::var("CUDA_PATH") {
        for sub in BIN_SUBDIRS {
            let bin = std::path::PathBuf::from(&cuda_path).join(sub);
            if dir_contains_nvrtc_dll(&bin) {
                return Some(bin);
            }
        }
    }
    #[cfg(windows)]
    {
        const TOOLKIT_ROOT: &str = r"C:\Program Files\NVIDIA GPU Computing Toolkit\CUDA";
        if let Ok(entries) = std::fs::read_dir(TOOLKIT_ROOT) {
            // Pick the highest version present (sorted lexicographically reversed).
            let mut versions: Vec<std::path::PathBuf> =
                entries.flatten().map(|e| e.path()).collect();
            versions.sort();
            for path in versions.into_iter().rev() {
                for sub in BIN_SUBDIRS {
                    let bin = path.join(sub);
                    if dir_contains_nvrtc_dll(&bin) {
                        return Some(bin);
                    }
                }
            }
        }
    }
    if let Ok(path_var) = std::env::var("PATH") {
        let sep = if cfg!(windows) { ';' } else { ':' };
        for dir in path_var.split(sep) {
            if dir.is_empty() {
                continue;
            }
            let pb = std::path::PathBuf::from(dir);
            if dir_contains_nvrtc_dll(&pb) {
                return Some(pb);
            }
        }
    }
    // Last resort: ask the OS where nvcc lives. Windows resolves nvcc via App
    // Path registry keys + a richer search than std::env::var("PATH"), so this
    // catches cases where the parent process's PATH is stale or the Toolkit
    // bin dir was registered out-of-band.
    let cmd = if cfg!(windows) { "where" } else { "which" };
    if let Ok(output) = Command::new(cmd).arg("nvcc").output() {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                let nvcc_path = std::path::Path::new(line.trim());
                if let Some(parent) = nvcc_path.parent() {
                    if dir_contains_nvrtc_dll(parent) {
                        return Some(parent.to_path_buf());
                    }
                }
            }
        }
    }
    None
}

/// On Windows, makes the CUDA bin directory's DLLs findable by cudarc
/// even when the running process inherited a stale PATH (user installed
/// Toolkit after opening their shell). Belt-and-suspenders approach:
///
/// 1. SetDllDirectoryW(bin)            — legacy DLL search path injection
/// 2. AddDllDirectory(bin) + SetDefaultDllDirectories(USER_DIRS|DEFAULT_DIRS)
///                                       — modern flag-based DLL search
/// 3. LoadLibraryW(full_path/nvrtc*.dll) — preload by full path; subsequent
///                                       LoadLibrary("nvrtc*.dll") calls
///                                       match by basename and return the
///                                       already-loaded HMODULE
///
/// The preload (#3) is the catch-all that works regardless of which flags
/// libloading/cudarc use internally. Idempotent via OnceLock.
pub fn register_cuda_dll_path() -> Option<std::path::PathBuf> {
    use std::sync::OnceLock;
    static REGISTERED: OnceLock<Option<std::path::PathBuf>> = OnceLock::new();
    REGISTERED
        .get_or_init(|| {
            let dir = find_cuda_bin_dir()?;
            #[cfg(windows)]
            {
                use std::os::windows::ffi::OsStrExt;
                #[link(name = "kernel32")]
                extern "system" {
                    fn SetDllDirectoryW(lpPathName: *const u16) -> i32;
                    fn AddDllDirectory(NewDirectory: *const u16) -> *mut std::ffi::c_void;
                    fn LoadLibraryW(lpLibFileName: *const u16) -> *mut std::ffi::c_void;
                    fn SetDefaultDllDirectories(DirectoryFlags: u32) -> i32;
                }

                const LOAD_LIBRARY_SEARCH_DEFAULT_DIRS: u32 = 0x00001000;
                const LOAD_LIBRARY_SEARCH_USER_DIRS: u32 = 0x00000400;

                let to_wide = |s: &std::path::Path| -> Vec<u16> {
                    s.as_os_str()
                        .encode_wide()
                        .chain(std::iter::once(0))
                        .collect()
                };

                let dir_wide = to_wide(&dir);

                // 1. Legacy SetDllDirectoryW.
                unsafe { SetDllDirectoryW(dir_wide.as_ptr()) };

                // 2. Modern AddDllDirectory + SetDefaultDllDirectories.
                //    These together cover libloaders that use LoadLibraryEx
                //    with LOAD_LIBRARY_SEARCH_* flags (which ignore #1).
                unsafe {
                    AddDllDirectory(dir_wide.as_ptr());
                    SetDefaultDllDirectories(
                        LOAD_LIBRARY_SEARCH_DEFAULT_DIRS | LOAD_LIBRARY_SEARCH_USER_DIRS,
                    );
                }

                // 3. Preload nvrtc and its sibling cudart with full path so
                //    cudarc's LoadLibrary("nvrtc*.dll") matches by basename.
                //    Walk every DLL in the bin dir whose name starts with
                //    "nvrtc" or "cudart" — that way we don't have to guess
                //    version suffixes that change with each Toolkit release.
                let mut preloaded = 0usize;
                if let Ok(entries) = std::fs::read_dir(&dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.extension().and_then(|s| s.to_str())
                            != Some("dll")
                        {
                            continue;
                        }
                        let name_lower = path
                            .file_name()
                            .and_then(|s| s.to_str())
                            .map(|s| s.to_ascii_lowercase())
                            .unwrap_or_default();
                        if !name_lower.starts_with("nvrtc")
                            && !name_lower.starts_with("cudart")
                            && !name_lower.starts_with("nvjitlink")
                        {
                            continue;
                        }
                        let full_wide = to_wide(&path);
                        let h = unsafe { LoadLibraryW(full_wide.as_ptr()) };
                        if !h.is_null() {
                            preloaded += 1;
                        }
                    }
                }
                eprintln!(
                    "[forge-startup] CUDA DLLs preloaded: {} from {}",
                    preloaded,
                    dir.display()
                );
            }
            Some(dir)
        })
        .clone()
}

fn dir_contains_nvrtc_dll(dir: &std::path::Path) -> bool {
    #[cfg(windows)]
    {
        const NVRTC_NAMES: &[&str] = &[
            "nvrtc64_130_0.dll",
            "nvrtc64_130.dll",
            "nvrtc64_120_0.dll",
            "nvrtc64_120.dll",
            "nvrtc64_12.dll",
            "nvrtc64_11.dll",
            "nvrtc64.dll",
            "nvrtc.dll",
        ];
        for candidate in NVRTC_NAMES {
            if dir.join(candidate).exists() {
                return true;
            }
        }
        false
    }
    #[cfg(not(windows))]
    {
        let _ = dir;
        false
    }
}

fn detect_gpu_adapter_names() -> io::Result<Vec<String>> {
    if let Some(env_count) = gpu_count_from_env() {
        return Ok((0..env_count).map(|i| format!("GPU-{i}")).collect());
    }
    detect_gpu_adapter_names_os()
}

fn gpu_count_from_env() -> Option<usize> {
    std::env::var("FORGE_GPU_COUNT")
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
}

#[cfg(target_os = "windows")]
fn detect_gpu_adapter_names_os() -> io::Result<Vec<String>> {
    let script = "Get-CimInstance Win32_VideoController | Where-Object { $_.Name -notmatch 'Microsoft Basic Display Adapter' } | Select-Object -ExpandProperty Name";
    let out = Command::new("powershell")
        .args(["-NoProfile", "-Command", script])
        .output()?;
    if !out.status.success() {
        return Err(io::Error::other("powershell GPU adapter detection failed"));
    }
    let txt = String::from_utf8_lossy(&out.stdout);
    let names: Vec<String> = txt
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .map(|l| l.to_string())
        .collect();
    Ok(names)
}

#[cfg(target_os = "linux")]
fn detect_gpu_adapter_names_os() -> io::Result<Vec<String>> {
    let cmd = "lspci | grep -E -i 'vga|3d|display'";
    let out = Command::new("sh").args(["-c", cmd]).output()?;
    if !out.status.success() {
        return Err(io::Error::other("linux GPU adapter detection command failed"));
    }
    let txt = String::from_utf8_lossy(&out.stdout);
    let names: Vec<String> = txt
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .map(|l| l.to_string())
        .collect();
    Ok(names)
}

#[cfg(target_os = "macos")]
fn detect_gpu_adapter_names_os() -> io::Result<Vec<String>> {
    let out = Command::new("system_profiler")
        .args(["SPDisplaysDataType"])
        .output()?;
    if !out.status.success() {
        return Err(io::Error::other("macOS GPU adapter detection command failed"));
    }
    let txt = String::from_utf8_lossy(&out.stdout);
    let names: Vec<String> = txt
        .lines()
        .filter_map(|l| {
            let t = l.trim_start();
            t.strip_prefix("Chipset Model:")
                .map(|s| s.trim().to_string())
        })
        .collect();
    Ok(names)
}

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
fn detect_gpu_adapter_names_os() -> io::Result<Vec<String>> {
    Ok(Vec::new())
}

#[cfg(test)]
fn first_usize_token(s: &str) -> Option<usize> {
    let mut num = String::new();
    let mut seen_digit = false;
    for ch in s.chars() {
        if ch.is_ascii_digit() {
            seen_digit = true;
            num.push(ch);
        } else if seen_digit {
            break;
        }
    }
    if num.is_empty() {
        None
    } else {
        num.parse::<usize>().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_bootstrap_ok() {
        let text = "version=1\ndetected_gpu_count=2\ndeployed_gpunodes=2\nnvidia_detected=1\ncuda_enabled=1\n";
        let p = parse_bootstrap(text).expect("must parse");
        assert_eq!(p.detected_gpu_count, 2);
        assert_eq!(p.deployed_gpunodes, 2);
        assert!(p.nvidia_detected);
        assert!(p.cuda_enabled);
        assert!(!p.first_install);
    }

    #[test]
    fn parse_bootstrap_rejects_wrong_version() {
        let text = "version=9\ndetected_gpu_count=2\ndeployed_gpunodes=2\nnvidia_detected=1\ncuda_enabled=1\n";
        assert!(parse_bootstrap(text).is_none());
    }

    #[test]
    fn first_usize_token_reads_first_number() {
        assert_eq!(first_usize_token("  \n42\n"), Some(42));
        assert_eq!(first_usize_token("count: 12 adapters"), Some(12));
        assert_eq!(first_usize_token("no number"), None);
    }

    #[test]
    fn bootstrap_file_roundtrip() {
        let unique = format!(
            "forge-gpunode-bootstrap-test-{}-{}.txt",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time before unix epoch")
                .as_nanos()
        );
        let mut p = std::env::temp_dir();
        p.push(unique);

        let first = bootstrap_gpunodes_at(&p).expect("first bootstrap");
        let second = bootstrap_gpunodes_at(&p).expect("reload bootstrap");
        assert!(first.first_install);
        assert!(!second.first_install);
        assert_eq!(first.detected_gpu_count, second.detected_gpu_count);
        assert_eq!(first.deployed_gpunodes, second.deployed_gpunodes);
        assert_eq!(first.nvidia_detected, second.nvidia_detected);
        assert_eq!(first.cuda_enabled, second.cuda_enabled);

        let _ = fs::remove_file(p);
    }
}
