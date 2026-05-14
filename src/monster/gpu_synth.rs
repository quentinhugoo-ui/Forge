//! GPU-accelerated synth scoring kernel.
//!
//! Replaces the CPU `push_binary` inner loop with a massively parallel
//! GPU kernel. Each GPU thread scores ONE (left, right, op) combination
//! across ALL examples, computing loss = Î£|left[i] OP right[i] - target[i]|.
//!
//! Supports dual-GPU split: CUDA (NVIDIA) + WGPU (AMD/Intel) in parallel.

use std::io;
#[cfg(feature = "wgpu")]
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU8, Ordering};

/// Last GPU backend used: 0=none/cpu, 1=cuda, 2=wgpu, 3=cuda+wgpu split.
static LAST_GPU_BACKEND: AtomicU8 = AtomicU8::new(0);

#[cfg(feature = "wgpu")]
struct WgpuSynthContext {
    device: wgpu::Device,
    queue: wgpu::Queue,
    layout: wgpu::BindGroupLayout,
    pipeline: wgpu::ComputePipeline,
}

#[cfg(feature = "wgpu")]
static WGPU_SYNTH_CONTEXT: OnceLock<Result<WgpuSynthContext, String>> = OnceLock::new();

/// Returns the last GPU backend that actually executed a scoring batch.
pub fn last_gpu_backend() -> &'static str {
    match LAST_GPU_BACKEND.load(Ordering::Relaxed) {
        1 => "CUDA",
        2 => "WGPU",
        3 => "CUDA+WGPU",
        _ => "CPU-fallback",
    }
}

/// A batch of candidate scoring jobs to run on GPU.
/// Each job = (left_idx, right_idx, op) â†’ compute loss over all examples.
#[derive(Clone)]
pub struct SynthGpuBatch {
    /// Flat buffer of all candidate outputs: beam_count Ã— n_examples i64 values.
    /// candidate_outputs[candidate_idx * n_examples + example_idx]
    pub candidate_outputs: Vec<i64>,
    /// Target outputs for all examples.
    pub targets: Vec<i64>,
    /// Number of examples (M).
    pub n_examples: usize,
    /// Number of candidates in the beam.
    #[allow(dead_code)] // metadata; kernel uses candidate_outputs.len() / n_examples
    pub n_candidates: usize,
    /// Valid (left_idx, right_idx) pairs to score â€” pre-filtered by CPU.
    /// Jobs refer to entries in this table via their packed pair index.
    pub pairs: Vec<(u16, u16)>,
    /// Packed scoring jobs. Layout per u32:
    ///   `[pair_idx:24][op:8]`
    /// `pair_idx` indexes `pairs`, `op` matches the OP_* constants below.
    pub jobs: Vec<u32>,
}

/// Result of a single (left, right, op) scoring.
#[derive(Clone, Copy, Debug)]
pub struct SynthGpuResult {
    pub pair_idx: u32,
    pub op: u8,
    pub loss: u128,
    pub fingerprint: u64,
}

/// Op encoding for GPU kernel (matches Expr variants in train.rs).
pub const OP_ADD: u32 = 0;
pub const OP_SUB: u32 = 1;
pub const OP_MUL: u32 = 2;
pub const OP_XOR: u32 = 3;
pub const OP_AND: u32 = 4;
pub const OP_OR: u32 = 5;
pub const OP_GT: u32 = 6;
pub const OP_LT: u32 = 7;
#[allow(dead_code)] // referenced by tests + kernel via wildcard match
pub const OP_SEL: u32 = 8;
pub const N_OPS: u32 = 9;

#[inline]
pub fn pack_job(pair_idx: u32, op: u8) -> u32 {
    (pair_idx << 8) | (op as u32)
}

#[inline]
fn unpack_job(job: u32) -> (u32, u8) {
    (job >> 8, (job & 0xFF) as u8)
}

#[inline]
fn dense_jobs_for_pairs(n_pairs: usize) -> Vec<u32> {
    let mut jobs = Vec::with_capacity(n_pairs * N_OPS as usize);
    for pair_idx in 0..n_pairs as u32 {
        for op in 0..N_OPS as u8 {
            jobs.push(pack_job(pair_idx, op));
        }
    }
    jobs
}

#[inline]
fn preferred_cuda_split_pairs(n_pairs: usize) -> usize {
    if n_pairs <= 2 {
        1
    } else {
        let preferred = (n_pairs * 2) / 3;
        preferred.clamp(1, n_pairs - 1)
    }
}

#[inline]
fn jobs_are_dense(batch: &SynthGpuBatch) -> bool {
    if batch.jobs.len() != batch.pairs.len() * N_OPS as usize {
        return false;
    }
    for (idx, &job) in batch.jobs.iter().enumerate() {
        let expected_pair = (idx / N_OPS as usize) as u32;
        let expected_op = (idx % N_OPS as usize) as u8;
        if job != pack_job(expected_pair, expected_op) {
            return false;
        }
    }
    true
}

#[inline]
fn jobs_are_dense_for_pairs(jobs: &[u32], n_pairs: usize) -> bool {
    if jobs.len() != n_pairs * N_OPS as usize {
        return false;
    }
    for (idx, &job) in jobs.iter().enumerate() {
        let expected_pair = (idx / N_OPS as usize) as u32;
        let expected_op = (idx % N_OPS as usize) as u8;
        if job != pack_job(expected_pair, expected_op) {
            return false;
        }
    }
    true
}

/// Score all pairs Ã— 6 ops on available GPUs.
/// Falls back to CPU if no GPU available or batch too small.
pub fn score_batch_gpu(batch: &SynthGpuBatch) -> io::Result<Vec<SynthGpuResult>> {
    let total_jobs = batch.jobs.len();
    if total_jobs == 0 {
        return Ok(Vec::new());
    }

    if std::env::var_os("FORGE_SYNTH_FORCE_CPU").is_some() {
        LAST_GPU_BACKEND.store(0, Ordering::Relaxed);
        return score_batch_cpu(batch);
    }

    // Minimum batch to justify GPU overhead. With large example counts
    // (20k+), even 72 jobs (8 pairs Ã— 9 ops) is 1.4M computations.
    // Real threshold = jobs Ã— examples â‰¥ 50k.
    let total_work = total_jobs.saturating_mul(batch.n_examples);
    if total_work < 50_000 {
        LAST_GPU_BACKEND.store(0, Ordering::Relaxed);
        return score_batch_cpu(batch);
    }

    // Prefer CUDA first for synth scoring. The dense pair-fused CUDA
    // kernel avoids 9x repeated pair loads; on the trading beam workload
    // it beats the CUDA+WGPU split because the slower WGPU half gates the
    // whole batch. Dual split stays available for explicit experiments.
    let dense_jobs = jobs_are_dense(batch);

    if dense_jobs
        && std::env::var_os("FORGE_SYNTH_FORCE_DUAL_GPU").is_some()
        && std::env::var_os("FORGE_SYNTH_DISABLE_DUAL_GPU").is_none()
    {
        #[cfg(all(feature = "cuda", feature = "wgpu"))]
        {
            if let Ok(results) = score_split_cuda_wgpu(batch) {
                LAST_GPU_BACKEND.store(3, Ordering::Relaxed);
                return Ok(results);
            }
        }
    }

    #[cfg(feature = "cuda")]
    {
        if let Ok(results) = score_cuda(batch) {
            LAST_GPU_BACKEND.store(1, Ordering::Relaxed);
            return Ok(results);
        }
    }

    #[cfg(feature = "wgpu")]
    {
        if let Ok(results) = score_wgpu(batch) {
            LAST_GPU_BACKEND.store(2, Ordering::Relaxed);
            return Ok(results);
        }
    }

    LAST_GPU_BACKEND.store(0, Ordering::Relaxed);
    score_batch_cpu(batch)
}

/// CPU fallback â€” same logic, single-threaded. Used when no GPU available
/// or batch too small to justify transfer overhead.
pub fn score_batch_cpu(batch: &SynthGpuBatch) -> io::Result<Vec<SynthGpuResult>> {
    score_cpu_parts(
        &batch.candidate_outputs,
        &batch.targets,
        batch.n_examples,
        &batch.pairs,
        &batch.jobs,
    )
}

fn score_cpu_parts(
    candidate_outputs: &[i64],
    targets: &[i64],
    n_examples: usize,
    pairs: &[(u16, u16)],
    jobs: &[u32],
) -> io::Result<Vec<SynthGpuResult>> {
    if std::env::var_os("FORGE_SYNTH_DISABLE_CPU_PAIR_FUSED").is_none()
        && jobs_are_dense_for_pairs(jobs, pairs.len())
    {
        return score_cpu_dense_pairs(candidate_outputs, targets, n_examples, pairs);
    }

    score_cpu_scalar_jobs(candidate_outputs, targets, n_examples, pairs, jobs)
}

fn score_cpu_scalar_jobs(
    candidate_outputs: &[i64],
    targets: &[i64],
    n_examples: usize,
    pairs: &[(u16, u16)],
    jobs: &[u32],
) -> io::Result<Vec<SynthGpuResult>> {
    let mut results = Vec::with_capacity(jobs.len());

    for &job in jobs {
        let (pair_idx, op) = unpack_job(job);
        let (left_idx, right_idx) = pairs[pair_idx as usize];
        let left_base = left_idx as usize * n_examples;
        let right_base = right_idx as usize * n_examples;
        let left_slice = &candidate_outputs[left_base..left_base + n_examples];
        let right_slice = &candidate_outputs[right_base..right_base + n_examples];
        let (loss, fp) = compute_loss_and_fp(left_slice, right_slice, targets, op as u32);
        results.push(SynthGpuResult {
            pair_idx,
            op,
            loss,
            fingerprint: fp,
        });
    }
    Ok(results)
}

fn score_cpu_dense_pairs(
    candidate_outputs: &[i64],
    targets: &[i64],
    n_examples: usize,
    pairs: &[(u16, u16)],
) -> io::Result<Vec<SynthGpuResult>> {
    let mut results = Vec::with_capacity(pairs.len() * N_OPS as usize);

    for (pair_idx, &(left_idx, right_idx)) in pairs.iter().enumerate() {
        let left_base = left_idx as usize * n_examples;
        let right_base = right_idx as usize * n_examples;
        let left = &candidate_outputs[left_base..left_base + n_examples];
        let right = &candidate_outputs[right_base..right_base + n_examples];

        let mut loss = [0u128; N_OPS as usize];
        let mut fp = [0xcbf2_9ce4_8422_2325u64; N_OPS as usize];

        for i in 0..n_examples {
            let a = left[i];
            let b = right[i];
            let t = targets[i];

            accumulate_loss_fp(a.wrapping_add(b), t, &mut loss[0], &mut fp[0]);
            accumulate_loss_fp(a.wrapping_sub(b), t, &mut loss[1], &mut fp[1]);
            accumulate_loss_fp(a.wrapping_mul(b), t, &mut loss[2], &mut fp[2]);
            accumulate_loss_fp(a ^ b, t, &mut loss[3], &mut fp[3]);
            accumulate_loss_fp(a & b, t, &mut loss[4], &mut fp[4]);
            accumulate_loss_fp(a | b, t, &mut loss[5], &mut fp[5]);
            accumulate_loss_fp((a > b) as i64, t, &mut loss[6], &mut fp[6]);
            accumulate_loss_fp((a < b) as i64, t, &mut loss[7], &mut fp[7]);
            accumulate_loss_fp(if a != 0 { b } else { 0 }, t, &mut loss[8], &mut fp[8]);
        }

        for op_idx in 0..N_OPS as usize {
            results.push(SynthGpuResult {
                pair_idx: pair_idx as u32,
                op: op_idx as u8,
                loss: loss[op_idx],
                fingerprint: fp[op_idx],
            });
        }
    }

    Ok(results)
}

#[inline(always)]
fn accumulate_loss_fp(out: i64, target: i64, loss: &mut u128, fp: &mut u64) {
    *fp ^= out as u64;
    *fp = (*fp).wrapping_mul(0x100_0000_01b3);
    *loss += ((out as i128) - (target as i128)).unsigned_abs();
}

/// Compute loss and FNV-1a fingerprint for one (left, right, op) combination.
#[inline]
fn compute_loss_and_fp(left: &[i64], right: &[i64], targets: &[i64], op: u32) -> (u128, u64) {
    let mut loss: u128 = 0;
    let mut fp: u64 = 0xcbf2_9ce4_8422_2325;
    for i in 0..left.len() {
        let out = apply_op(left[i], right[i], op);
        fp ^= out as u64;
        fp = fp.wrapping_mul(0x100_0000_01b3);
        let diff = (out as i128) - (targets[i] as i128);
        loss += diff.unsigned_abs();
    }
    (loss, fp)
}

#[inline(always)]
fn apply_op(a: i64, b: i64, op: u32) -> i64 {
    match op {
        OP_ADD => a.wrapping_add(b),
        OP_SUB => a.wrapping_sub(b),
        OP_MUL => a.wrapping_mul(b),
        OP_XOR => a ^ b,
        OP_AND => a & b,
        OP_OR => a | b,
        OP_GT => if a > b { 1 } else { 0 },
        OP_LT => if a < b { 1 } else { 0 },
        _ => if a != 0 { b } else { 0 }, // OP_SEL
    }
}

// â”€â”€â”€ WGPU kernel â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[cfg(feature = "wgpu")]
fn score_wgpu(batch: &SynthGpuBatch) -> io::Result<Vec<SynthGpuResult>> {
    score_wgpu_parts(
        &batch.candidate_outputs,
        &batch.targets,
        batch.n_examples,
        &batch.pairs,
        &batch.jobs,
    )
}

#[cfg(feature = "wgpu")]
fn wgpu_synth_context() -> io::Result<&'static WgpuSynthContext> {
    use pollster::block_on;
    use wgpu::{
        Backends, ComputePipelineDescriptor, DeviceDescriptor, Features, Instance,
        InstanceDescriptor, Limits, PipelineLayoutDescriptor, PowerPreference,
        RequestAdapterOptions, ShaderModuleDescriptor, ShaderSource,
    };

    let ctx = WGPU_SYNTH_CONTEXT.get_or_init(|| {
        let instance = Instance::new(InstanceDescriptor {
            backends: Backends::all(),
            flags: Default::default(),
            backend_options: Default::default(),
            display: Default::default(),
            memory_budget_thresholds: Default::default(),
        });
        let adapter = match block_on(instance.request_adapter(&RequestAdapterOptions {
            power_preference: PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
        })) {
            Ok(adapter) => adapter,
            Err(_) => return Err("wgpu: no adapter for synth kernel".to_string()),
        };

        let (device, queue) = match block_on(adapter.request_device(&DeviceDescriptor {
            label: Some("forge-synth-gpu"),
            required_features: Features::SHADER_INT64,
            required_limits: Limits::default(),
            experimental_features: Default::default(),
            memory_hints: Default::default(),
            trace: Default::default(),
        })) {
            Ok(pair) => pair,
            Err(e) => return Err(format!("wgpu synth device failed: {e}")),
        };

        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("forge-synth-shader"),
            source: ShaderSource::Wgsl(SYNTH_WGSL_SHADER.into()),
        });

        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("synth-layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0, visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false, min_binding_size: None,
                    }, count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1, visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false, min_binding_size: None,
                    }, count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2, visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false, min_binding_size: None,
                    }, count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3, visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false, min_binding_size: None,
                    }, count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4, visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false, min_binding_size: None,
                    }, count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 5, visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false, min_binding_size: None,
                    }, count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("synth-pipeline-layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("synth-pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });

        Ok(WgpuSynthContext {
            device,
            queue,
            layout,
            pipeline,
        })
    });

    ctx.as_ref()
        .map_err(|e| io::Error::other(e.clone()))
}

#[cfg(feature = "wgpu")]
fn score_wgpu_parts(
    candidate_outputs: &[i64],
    targets: &[i64],
    n_examples: usize,
    pairs: &[(u16, u16)],
    jobs: &[u32],
) -> io::Result<Vec<SynthGpuResult>> {
    use std::sync::mpsc;
    use wgpu::{
        BufferDescriptor, BufferUsages, CommandEncoderDescriptor, ComputePassDescriptor, MapMode,
        PollType,
    };

    let ctx = wgpu_synth_context()?;
    let device = &ctx.device;
    let queue = &ctx.queue;
    let m = n_examples as u32;
    let n_pairs = pairs.len() as u32;
    let total_jobs = jobs.len() as u32;

    let pairs_packed: Vec<u32> = pairs
        .iter()
        .map(|&(l, r)| (l as u32) | ((r as u32) << 16))
        .collect();

    let candidates_buf = device.create_buffer(&BufferDescriptor {
        label: Some("candidates"),
        size: (candidate_outputs.len() * 8) as u64,
        usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&candidates_buf, 0, bytemuck_cast_slice_i64(candidate_outputs));

    let targets_buf = device.create_buffer(&BufferDescriptor {
        label: Some("targets"),
        size: (targets.len() * 8) as u64,
        usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&targets_buf, 0, bytemuck_cast_slice_i64(targets));

    let pairs_buf = device.create_buffer(&BufferDescriptor {
        label: Some("pairs"),
        size: (pairs_packed.len() * 4) as u64,
        usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&pairs_buf, 0, bytemuck_cast_slice_u32(&pairs_packed));

    let jobs_buf = device.create_buffer(&BufferDescriptor {
        label: Some("jobs"),
        size: (jobs.len() * 4) as u64,
        usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&jobs_buf, 0, bytemuck_cast_slice_u32(jobs));

    let out_size = total_jobs as u64 * 24;
    let out_buf = device.create_buffer(&BufferDescriptor {
        label: Some("results"),
        size: out_size,
        usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let readback = device.create_buffer(&BufferDescriptor {
        label: Some("readback"),
        size: out_size,
        usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let uniform_data: [u32; 3] = [m, n_pairs, total_jobs];
    let uniform_buf = device.create_buffer(&BufferDescriptor {
        label: Some("uniform"),
        size: 12,
        usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&uniform_buf, 0, bytemuck_cast_slice_u32(&uniform_data));

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("synth-bind"),
        layout: &ctx.layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: candidates_buf.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: targets_buf.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 2, resource: pairs_buf.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 3, resource: jobs_buf.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 4, resource: out_buf.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 5, resource: uniform_buf.as_entire_binding() },
        ],
    });

    let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
        label: Some("synth-encoder"),
    });
    {
        let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("synth-pass"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&ctx.pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        let workgroups = (total_jobs + 63) / 64;
        pass.dispatch_workgroups(workgroups, 1, 1);
    }
    encoder.copy_buffer_to_buffer(&out_buf, 0, &readback, 0, out_size);
    queue.submit([encoder.finish()]);

    let slice = readback.slice(..);
    let (tx, rx) = mpsc::channel();
    slice.map_async(MapMode::Read, move |r| { let _ = tx.send(r); });
    let _ = device.poll(PollType::wait_indefinitely());
    rx.recv()
        .map_err(|_| io::Error::other("wgpu synth: map channel closed"))?
        .map_err(|e| io::Error::other(format!("wgpu synth: map failed: {e}")))?;

    let data = slice.get_mapped_range();
    let results = parse_gpu_results(&data, jobs);
    drop(data);
    readback.unmap();

    Ok(results)
}

// â”€â”€â”€ CUDA kernel â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[cfg(feature = "cuda")]
fn score_cuda(batch: &SynthGpuBatch) -> io::Result<Vec<SynthGpuResult>> {
    score_cuda_parts(
        &batch.candidate_outputs,
        &batch.targets,
        batch.n_examples,
        &batch.pairs,
        &batch.jobs,
    )
}

#[cfg(feature = "cuda")]
fn score_cuda_parts(
    candidate_outputs: &[i64],
    targets: &[i64],
    n_examples: usize,
    pairs: &[(u16, u16)],
    jobs: &[u32],
) -> io::Result<Vec<SynthGpuResult>> {
    use crate::cuda_min;

    let m = n_examples;
    let n_pairs = pairs.len();

    let pairs_packed: Vec<u32> = pairs
        .iter()
        .map(|&(l, r)| (l as u32) | ((r as u32) << 16))
        .collect();

    let pair_fused_enabled = std::env::var_os("FORGE_SYNTH_DISABLE_CUDA_PAIR_FUSED").is_none();
    let vec2_enabled = std::env::var_os("FORGE_SYNTH_FORCE_CUDA_VEC2_LOADS").is_some();
    let result_buf = if pair_fused_enabled && jobs_are_dense_for_pairs(jobs, n_pairs) {
        if vec2_enabled && m % 2 == 0 {
            cuda_min::synth_score_dense_pairs_vec2(
                candidate_outputs,
                targets,
                &pairs_packed,
                m as u32,
                n_pairs as u32,
            )?
        } else {
            cuda_min::synth_score_dense_pairs(
                candidate_outputs,
                targets,
                &pairs_packed,
                m as u32,
                n_pairs as u32,
            )?
        }
    } else {
        cuda_min::synth_score_batch(
            candidate_outputs,
            targets,
            &pairs_packed,
            jobs,
            m as u32,
            n_pairs as u32,
        )?
    };

    Ok(parse_gpu_results(&result_buf, jobs))
}

// â”€â”€â”€ Dual-GPU split â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[cfg(all(feature = "cuda", feature = "wgpu"))]
fn score_split_cuda_wgpu(batch: &SynthGpuBatch) -> io::Result<Vec<SynthGpuResult>> {
    if !jobs_are_dense(batch) {
        return Err(io::Error::other("sparse jobs not worth split"));
    }
    let n_pairs = batch.pairs.len();
    // Once the batch is dense and score_batch_gpu accepted it, dual-submit
    // is already worthwhile on medium batches. Keep the floor low so both
    // cards fire more often instead of defaulting to CUDA-only.
    let min_pairs = if batch.n_examples >= 1000 { 8 } else { 64 };
    if n_pairs < min_pairs {
        return Err(io::Error::other("batch too small for split"));
    }

    let cuda_pairs_len = preferred_cuda_split_pairs(n_pairs);
    let wgpu_pairs_len = n_pairs - cuda_pairs_len;
    if cuda_pairs_len == 0 || wgpu_pairs_len == 0 {
        return Err(io::Error::other("batch too small for balanced split"));
    }

    let cuda_pairs = batch.pairs[..cuda_pairs_len].to_vec();
    let cuda_jobs = dense_jobs_for_pairs(cuda_pairs_len);
    let wgpu_pairs = batch.pairs[cuda_pairs_len..].to_vec();
    let wgpu_jobs = dense_jobs_for_pairs(wgpu_pairs_len);

    let mut cuda_result: io::Result<Vec<SynthGpuResult>> = Err(io::Error::other("not run"));
    let mut wgpu_result: io::Result<Vec<SynthGpuResult>> = Err(io::Error::other("not run"));

    std::thread::scope(|s| {
        let candidate_outputs = &batch.candidate_outputs;
        let targets = &batch.targets;
        let n_examples = batch.n_examples;
        let cuda_pairs_ref = &cuda_pairs;
        let cuda_jobs_ref = &cuda_jobs;
        let wgpu_pairs_ref = &wgpu_pairs;
        let wgpu_jobs_ref = &wgpu_jobs;
        let h_cuda = s.spawn(move || {
            score_cuda_parts(candidate_outputs, targets, n_examples, cuda_pairs_ref, cuda_jobs_ref)
        });
        let h_wgpu = s.spawn(move || {
            score_wgpu_parts(candidate_outputs, targets, n_examples, wgpu_pairs_ref, wgpu_jobs_ref)
        });
        cuda_result = h_cuda.join().unwrap_or(Err(io::Error::other("cuda thread panic")));
        wgpu_result = h_wgpu.join().unwrap_or(Err(io::Error::other("wgpu thread panic")));
    });

    let mut results = match cuda_result {
        Ok(results) => results,
        Err(_) => score_wgpu_parts(
            &batch.candidate_outputs,
            &batch.targets,
            batch.n_examples,
            &cuda_pairs,
            &cuda_jobs,
        )
        .or_else(|_| {
            score_cpu_parts(
                &batch.candidate_outputs,
                &batch.targets,
                batch.n_examples,
                &cuda_pairs,
                &cuda_jobs,
            )
        })?,
    };
    let mut wgpu_res = match wgpu_result {
        Ok(results) => results,
        Err(_) => score_cuda_parts(
            &batch.candidate_outputs,
            &batch.targets,
            batch.n_examples,
            &wgpu_pairs,
            &wgpu_jobs,
        )
        .or_else(|_| {
            score_cpu_parts(
                &batch.candidate_outputs,
                &batch.targets,
                batch.n_examples,
                &wgpu_pairs,
                &wgpu_jobs,
            )
        })?,
    };
    // Fix pair indices for the second half (they were 0-based in the sub-batch).
    for r in &mut wgpu_res {
        r.pair_idx += cuda_pairs_len as u32;
    }
    results.reserve(wgpu_res.len());
    results.append(&mut wgpu_res);
    Ok(results)
}

// â”€â”€â”€ Shared helpers â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

fn parse_gpu_results(raw: &[u8], jobs: &[u32]) -> Vec<SynthGpuResult> {
    let total_jobs = jobs.len();
    let mut results = Vec::with_capacity(total_jobs);
    for (job_idx, &job) in jobs.iter().enumerate() {
        let base = job_idx * 24;
        if base + 24 > raw.len() { break; }
        let loss_lo = u64::from_le_bytes(raw[base..base + 8].try_into().unwrap());
        let loss_hi = u64::from_le_bytes(raw[base + 8..base + 16].try_into().unwrap());
        let fp = u64::from_le_bytes(raw[base + 16..base + 24].try_into().unwrap());
        let loss = (loss_lo as u128) | ((loss_hi as u128) << 64);
        let (pair_idx, op) = unpack_job(job);
        results.push(SynthGpuResult { pair_idx, op, loss, fingerprint: fp });
    }
    results
}

fn bytemuck_cast_slice_i64(data: &[i64]) -> &[u8] {
    unsafe { std::slice::from_raw_parts(data.as_ptr() as *const u8, data.len() * 8) }
}

fn bytemuck_cast_slice_u32(data: &[u32]) -> &[u8] {
    unsafe { std::slice::from_raw_parts(data.as_ptr() as *const u8, data.len() * 4) }
}

// â”€â”€â”€ WGSL Shader â”€â”€â”€â”€ï¿½ï¿½â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

const SYNTH_WGSL_SHADER: &str = r#"
struct Params {
    n_examples: u32,
    n_pairs: u32,
    n_jobs: u32,
}

@group(0) @binding(0) var<storage, read> candidates: array<i64>;
@group(0) @binding(1) var<storage, read> targets: array<i64>;
@group(0) @binding(2) var<storage, read> pairs: array<u32>;
@group(0) @binding(3) var<storage, read> jobs: array<u32>;
@group(0) @binding(4) var<storage, read_write> results: array<u64>;
@group(0) @binding(5) var<uniform> params: Params;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let job_idx: u32 = gid.x;
    let total_jobs: u32 = params.n_jobs;
    if (job_idx >= total_jobs) { return; }

    let job: u32 = jobs[job_idx];
    let pair_idx: u32 = job >> 8u;
    let op: u32 = job & 0xFFu;

    let packed: u32 = pairs[pair_idx];
    let left_idx: u32 = packed & 0xFFFFu;
    let right_idx: u32 = (packed >> 16u) & 0xFFFFu;

    let m: u32 = params.n_examples;
    let left_base: u32 = left_idx * m;
    let right_base: u32 = right_idx * m;

    var loss_lo: u64 = 0lu;
    var loss_hi: u64 = 0lu;
    var fp: u64 = 0xcbf29ce484222325lu;

    var i: u32 = 0u;
    loop {
        if (i >= m) { break; }

        let a: i64 = candidates[left_base + i];
        let b: i64 = candidates[right_base + i];

        var out: i64 = 0li;
        switch op {
            case 0u: { out = a + b; }
            case 1u: { out = a - b; }
            case 2u: { out = a * b; }
            case 3u: { out = a ^ b; }
            case 4u: { out = a & b; }
            case 5u: { out = a | b; }
            case 6u: { out = select(0li, 1li, a > b); }
            case 7u: { out = select(0li, 1li, a < b); }
            default: { out = select(0li, b, a != 0li); }
        }

        // FNV-1a fingerprint
        fp = fp ^ bitcast<u64>(out);
        fp = fp * 0x100000001b3lu;

        // Absolute difference â†’ accumulate into 128-bit loss.
        let t: i64 = targets[i];
        let diff: i64 = out - t;
        // abs(diff) via branchless: mask = diff >> 63, abs = (diff ^ mask) - mask
        let mask: i64 = diff >> 63u;
        let abs_diff: u64 = bitcast<u64>((diff ^ mask) - mask);

        // 128-bit addition: loss += abs_diff
        let new_lo: u64 = loss_lo + abs_diff;
        let carry: u64 = select(0lu, 1lu, new_lo < loss_lo);
        loss_lo = new_lo;
        loss_hi = loss_hi + carry;

        i = i + 1u;
    }

    // Write output: 3 Ã— u64 per job = (loss_lo, loss_hi, fingerprint)
    let out_base: u32 = job_idx * 3u;
    results[out_base] = loss_lo;
    results[out_base + 1u] = loss_hi;
    results[out_base + 2u] = fp;
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_fallback_matches_manual() {
        let left = vec![1i64, 2, 3, 4];
        let right = vec![10i64, 20, 30, 40];
        let targets = vec![11i64, 22, 33, 44];
        let batch = SynthGpuBatch {
            candidate_outputs: [left.clone(), right.clone()].concat(),
            targets: targets.clone(),
            n_examples: 4,
            n_candidates: 2,
            pairs: vec![(0, 1)],
            jobs: dense_jobs_for_pairs(1),
        };
        let results = score_batch_cpu(&batch).unwrap();
        assert_eq!(results.len(), 9);
        assert_eq!(results[0].loss, 0); // OP_ADD: perfect match
        assert_eq!(results[0].op, 0);
        assert_eq!(results[1].loss, 200); // OP_SUB: |(-9)-11|+|(-18)-22|+... = 200
    }

    #[test]
    fn fingerprint_matches_train_rs() {
        let outputs = vec![5i64, -3, 100];
        let mut fp: u64 = 0xcbf2_9ce4_8422_2325;
        for &v in &outputs {
            fp ^= v as u64;
            fp = fp.wrapping_mul(0x100_0000_01b3);
        }
        let left = vec![2i64, -1, 50];
        let right = vec![3i64, -2, 50];
        let targets = vec![0i64; 3];
        let batch = SynthGpuBatch {
            candidate_outputs: [left, right].concat(),
            targets,
            n_examples: 3,
            n_candidates: 2,
            pairs: vec![(0, 1)],
            jobs: dense_jobs_for_pairs(1),
        };
        let results = score_batch_cpu(&batch).unwrap();
        // OP_ADD: 2+3=5, -1+(-2)=-3, 50+50=100 â†’ fingerprint should match
        assert_eq!(results[0].fingerprint, fp);
    }

    #[test]
    fn cpu_fallback_supports_sparse_jobs() {
        let batch = SynthGpuBatch {
            candidate_outputs: vec![1, 2, 10, 20],
            targets: vec![0, 0],
            n_examples: 2,
            n_candidates: 2,
            pairs: vec![(0, 1)],
            jobs: vec![pack_job(0, OP_ADD as u8), pack_job(0, OP_SEL as u8)],
        };
        let results = score_batch_cpu(&batch).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].op, OP_ADD as u8);
        assert_eq!(results[1].op, OP_SEL as u8);
    }

    #[test]
    fn cpu_pair_fused_matches_scalar_dense_jobs() {
        let batch = synthetic_cpu_bench_batch(128, 257, 32);
        let scalar = score_cpu_scalar_jobs(
            &batch.candidate_outputs,
            &batch.targets,
            batch.n_examples,
            &batch.pairs,
            &batch.jobs,
        )
        .unwrap();
        let fused = score_cpu_dense_pairs(
            &batch.candidate_outputs,
            &batch.targets,
            batch.n_examples,
            &batch.pairs,
        )
        .unwrap();

        assert_eq!(fused.len(), scalar.len());
        for (left, right) in fused.iter().zip(scalar.iter()) {
            assert_eq!(left.pair_idx, right.pair_idx);
            assert_eq!(left.op, right.op);
            assert_eq!(left.loss, right.loss);
            assert_eq!(left.fingerprint, right.fingerprint);
        }
    }

    #[test]
    #[ignore = "explicit CPU micro-benchmark; run with --ignored --nocapture"]
    fn cpu_pair_fused_bench_reports_speed() {
        use std::hint::black_box;
        use std::time::Instant;

        let batch = synthetic_cpu_bench_batch(256, 1156, 4096);
        let work_items = batch.jobs.len() as f64 * batch.n_examples as f64;

        let scalar_t0 = Instant::now();
        let scalar = black_box(
            score_cpu_scalar_jobs(
                &batch.candidate_outputs,
                &batch.targets,
                batch.n_examples,
                &batch.pairs,
                &batch.jobs,
            )
            .unwrap(),
        );
        let scalar_dt = scalar_t0.elapsed();

        let fused_t0 = Instant::now();
        let fused = black_box(
            score_cpu_dense_pairs(
                &batch.candidate_outputs,
                &batch.targets,
                batch.n_examples,
                &batch.pairs,
            )
            .unwrap(),
        );
        let fused_dt = fused_t0.elapsed();

        assert_eq!(fused.len(), scalar.len());
        for (left, right) in fused.iter().zip(scalar.iter()) {
            assert_eq!(left.pair_idx, right.pair_idx);
            assert_eq!(left.op, right.op);
            assert_eq!(left.loss, right.loss);
            assert_eq!(left.fingerprint, right.fingerprint);
        }

        let scalar_ns = scalar_dt.as_nanos() as f64 / work_items;
        let fused_ns = fused_dt.as_nanos() as f64 / work_items;
        let speedup = scalar_dt.as_secs_f64() / fused_dt.as_secs_f64();
        println!(
            "cpu_pair_fused_bench: jobs={} examples={} work_items={:.0} scalar={:.3}ms ({:.2}ns/item) fused={:.3}ms ({:.2}ns/item) speedup={:.2}x",
            batch.jobs.len(),
            batch.n_examples,
            work_items,
            scalar_dt.as_secs_f64() * 1000.0,
            scalar_ns,
            fused_dt.as_secs_f64() * 1000.0,
            fused_ns,
            speedup
        );
    }

    fn synthetic_cpu_bench_batch(
        n_candidates: usize,
        n_pairs: usize,
        n_examples: usize,
    ) -> SynthGpuBatch {
        let mut candidate_outputs = Vec::with_capacity(n_candidates * n_examples);
        for candidate_idx in 0..n_candidates {
            for example_idx in 0..n_examples {
                let value =
                    ((candidate_idx as i64 * 131 + example_idx as i64 * 17) % 2048) - 1024;
                candidate_outputs.push(value);
            }
        }

        let targets = (0..n_examples)
            .map(|idx| ((idx as i64 * 29) % 2048) - 1024)
            .collect();

        let pairs = (0..n_pairs)
            .map(|idx| {
                let left = (idx % n_candidates) as u16;
                let right = ((idx * 37 + 11) % n_candidates) as u16;
                (left, right)
            })
            .collect();

        SynthGpuBatch {
            candidate_outputs,
            targets,
            n_examples,
            n_candidates,
            pairs,
            jobs: dense_jobs_for_pairs(n_pairs),
        }
    }
}
