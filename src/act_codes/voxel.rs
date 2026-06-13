//! Shared voxelisation of an SDF interior and its implicit graph Laplacian.
//!
//! Every field act code (modal, thermal, stress...) walks the same occupancy
//! grid and sparse operator. The Laplacian stays matrix-free: CPU uses the
//! implicit stencil directly; the optional `wgpu` path uploads the compact
//! occupancy maps once and dispatches a portable WGSL matvec kernel.

use std::sync::OnceLock;

use super::{eval_scene, scene_aabb, SdfOp};

/// Voxelised occupancy plus the index map needed for the sparse Laplacian.
pub struct Voxels {
    pub nx: usize,
    pub ny: usize,
    pub nz: usize,
    /// cell-linear-index -> dof-index, or usize::MAX if empty.
    dof_of_cell: Vec<usize>,
    /// dof-index -> (ix, iy, iz).
    pub cell_of_dof: Vec<(usize, usize, usize)>,
    /// lower AABB corner (m).
    pub lo: [f64; 3],
    /// voxel pitch (m).
    pub h: f64,
    laplacian_log_once: OnceLock<()>,
    #[cfg(feature = "wgpu")]
    laplacian_gpu: OnceLock<Option<WgpuLaplacianPlan>>,
}

impl Voxels {
    /// Sample the SDF on a centred grid sized to the AABB, marking cells
    /// whose centre is interior (`eval_scene < 0`) as degrees of freedom.
    pub fn occupy(ops: &[SdfOp], grid: u32) -> Self {
        let (lo, hi) = scene_aabb(ops);
        let span = [hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]];
        let longest = span[0].max(span[1]).max(span[2]).max(1e-6);
        let h = longest / grid as f64;
        let nx = ((span[0] / h).ceil() as usize + 1).max(1);
        let ny = ((span[1] / h).ceil() as usize + 1).max(1);
        let nz = ((span[2] / h).ceil() as usize + 1).max(1);

        let mut dof_of_cell = vec![usize::MAX; nx * ny * nz];
        let mut cell_of_dof = Vec::new();
        for iz in 0..nz {
            for iy in 0..ny {
                for ix in 0..nx {
                    let p = [
                        lo[0] + (ix as f64 + 0.5) * h,
                        lo[1] + (iy as f64 + 0.5) * h,
                        lo[2] + (iz as f64 + 0.5) * h,
                    ];
                    if eval_scene(ops, p) < 0.0 {
                        let lin = (iz * ny + iy) * nx + ix;
                        dof_of_cell[lin] = cell_of_dof.len();
                        cell_of_dof.push((ix, iy, iz));
                    }
                }
            }
        }
        Self {
            nx,
            ny,
            nz,
            dof_of_cell,
            cell_of_dof,
            lo,
            h,
            laplacian_log_once: OnceLock::new(),
            #[cfg(feature = "wgpu")]
            laplacian_gpu: OnceLock::new(),
        }
    }

    pub fn ndof(&self) -> usize { self.cell_of_dof.len() }

    /// World-space centre of a dof's voxel.
    pub fn center_of_dof(&self, d: usize) -> [f64; 3] {
        let (ix, iy, iz) = self.cell_of_dof[d];
        [
            self.lo[0] + (ix as f64 + 0.5) * self.h,
            self.lo[1] + (iy as f64 + 0.5) * self.h,
            self.lo[2] + (iz as f64 + 0.5) * self.h,
        ]
    }

    #[inline]
    pub fn dof_at(&self, ix: usize, iy: usize, iz: usize) -> Option<usize> {
        let lin = (iz * self.ny + iy) * self.nx + ix;
        let d = self.dof_of_cell[lin];
        if d == usize::MAX { None } else { Some(d) }
    }

    /// Visit each occupied neighbour of dof `d` with its dof index.
    #[inline]
    pub fn for_each_neighbour(&self, d: usize, mut f: impl FnMut(usize)) {
        let (ix, iy, iz) = self.cell_of_dof[d];
        if ix > 0 { if let Some(n) = self.dof_at(ix - 1, iy, iz) { f(n); } }
        if ix + 1 < self.nx { if let Some(n) = self.dof_at(ix + 1, iy, iz) { f(n); } }
        if iy > 0 { if let Some(n) = self.dof_at(ix, iy - 1, iz) { f(n); } }
        if iy + 1 < self.ny { if let Some(n) = self.dof_at(ix, iy + 1, iz) { f(n); } }
        if iz > 0 { if let Some(n) = self.dof_at(ix, iy, iz - 1) { f(n); } }
        if iz + 1 < self.nz { if let Some(n) = self.dof_at(ix, iy, iz + 1) { f(n); } }
    }

    /// y = L x, graph Laplacian with Neumann/free boundary: diagonal =
    /// occupied-neighbour count, off-diagonal = -1 per edge.
    pub fn laplacian_matvec(&self, x: &[f64], y: &mut [f64]) {
        debug_assert_eq!(x.len(), self.ndof());
        debug_assert_eq!(y.len(), self.ndof());
        #[cfg(feature = "wgpu")]
        {
            if self.should_dispatch_laplacian_gpu() {
                if let Some(plan) = self.laplacian_gpu_plan() {
                    if plan.run(x, y).is_ok() {
                        self.log_laplacian_path(true);
                        return;
                    }
                }
            }
        }
        self.log_laplacian_path(false);
        self.laplacian_matvec_cpu(x, y);
    }

    fn log_laplacian_path(&self, gpu_path: bool) {
        self.laplacian_log_once.get_or_init(|| {
            eprintln!("[laplacian] gpu={} n_dof={}", gpu_path, self.ndof());
        });
    }

    fn laplacian_matvec_cpu(&self, x: &[f64], y: &mut [f64]) {
        for d in 0..self.cell_of_dof.len() {
            let mut deg = 0.0;
            let mut acc = 0.0;
            self.for_each_neighbour(d, |n| { deg += 1.0; acc += x[n]; });
            y[d] = deg * x[d] - acc;
        }
    }

    #[cfg(feature = "wgpu")]
    fn should_dispatch_laplacian_gpu(&self) -> bool {
        if self.ndof() == 0 {
            return false;
        }
        match std::env::var("FORGE_LAPLACIAN_GPU") {
            Ok(v) if matches!(v.as_str(), "0" | "false" | "False" | "off" | "cpu") => false,
            Ok(v) if matches!(v.as_str(), "1" | "true" | "True" | "on" | "required") => true,
            _ => self.ndof() >= 16_384,
        }
    }

    #[cfg(feature = "wgpu")]
    fn laplacian_gpu_plan(&self) -> Option<&WgpuLaplacianPlan> {
        self.laplacian_gpu
            .get_or_init(|| WgpuLaplacianPlan::new(self).ok())
            .as_ref()
    }

    #[cfg(all(feature = "wgpu", test))]
    pub(crate) fn try_laplacian_matvec_wgpu_for_test(&self, x: &[f64]) -> Option<Vec<f64>> {
        let plan = self.laplacian_gpu
            .get_or_init(|| WgpuLaplacianPlan::new(self).ok())
            .as_ref()?;
        let mut y = vec![0.0; self.ndof()];
        plan.run(x, &mut y).ok()?;
        Some(y)
    }
}

#[cfg(feature = "wgpu")]
struct WgpuLaplacianPlan {
    device: wgpu::Device,
    queue: wgpu::Queue,
    bind_group: wgpu::BindGroup,
    pipeline: wgpu::ComputePipeline,
    input: wgpu::Buffer,
    output: wgpu::Buffer,
    readback: wgpu::Buffer,
    len: usize,
}

#[cfg(feature = "wgpu")]
impl WgpuLaplacianPlan {
    fn new(vox: &Voxels) -> std::io::Result<Self> {
        use pollster::block_on;
        use wgpu::{
            Backends, BufferDescriptor, BufferUsages, ComputePipelineDescriptor, DeviceDescriptor,
            Features, Instance, InstanceDescriptor, Limits, PipelineLayoutDescriptor,
            PowerPreference, RequestAdapterOptions, ShaderModuleDescriptor, ShaderSource,
        };

        let len = vox.ndof();
        if len == 0 {
            return Err(std::io::Error::other("empty Laplacian GPU plan"));
        }
        if vox.nx > u32::MAX as usize || vox.ny > u32::MAX as usize || vox.nz > u32::MAX as usize || len > u32::MAX as usize {
            return Err(std::io::Error::other("Laplacian grid too large for WGSL u32 indexing"));
        }

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
        .map_err(|_| std::io::Error::other("wgpu request_adapter failed"))?;
        let (device, queue) = block_on(adapter.request_device(&DeviceDescriptor {
            label: Some("forge-laplacian-device"),
            required_features: Features::empty(),
            required_limits: Limits::default(),
            experimental_features: Default::default(),
            memory_hints: Default::default(),
            trace: Default::default(),
        }))
        .map_err(|e| std::io::Error::other(format!("wgpu request_device failed: {e}")))?;

        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("forge-laplacian-matvec"),
            source: ShaderSource::Wgsl(LAPLACIAN_MATVEC_WGSL.into()),
        });

        let value_bytes = (len * 4) as u64;
        let input = device.create_buffer(&BufferDescriptor {
            label: Some("forge-laplacian-x"),
            size: value_bytes,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let output = device.create_buffer(&BufferDescriptor {
            label: Some("forge-laplacian-y"),
            size: value_bytes,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let readback = device.create_buffer(&BufferDescriptor {
            label: Some("forge-laplacian-readback"),
            size: value_bytes,
            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let dof_bytes = u32_bytes(vox.dof_of_cell.iter().map(|&d| {
            if d == usize::MAX { u32::MAX } else { d as u32 }
        }));
        let dof_map = device.create_buffer(&BufferDescriptor {
            label: Some("forge-laplacian-dof-map"),
            size: dof_bytes.len() as u64,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&dof_map, 0, &dof_bytes);

        let cell_bytes = u32_bytes(vox.cell_of_dof.iter().flat_map(|&(ix, iy, iz)| {
            [ix as u32, iy as u32, iz as u32, 0]
        }));
        let cells = device.create_buffer(&BufferDescriptor {
            label: Some("forge-laplacian-cells"),
            size: cell_bytes.len() as u64,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&cells, 0, &cell_bytes);

        let params_bytes = u32_bytes([vox.nx as u32, vox.ny as u32, vox.nz as u32, len as u32]);
        let params = device.create_buffer(&BufferDescriptor {
            label: Some("forge-laplacian-params"),
            size: params_bytes.len() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&params, 0, &params_bytes);

        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("forge-laplacian-layout"),
            entries: &[
                storage_entry(0, true),
                storage_entry(1, false),
                storage_entry(2, true),
                storage_entry(3, true),
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
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
            label: Some("forge-laplacian-bind-group"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: input.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: output.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: dof_map.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 3, resource: cells.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 4, resource: params.as_entire_binding() },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("forge-laplacian-pipeline-layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("forge-laplacian-pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });

        Ok(Self { device, queue, bind_group, pipeline, input, output, readback, len })
    }

    fn run(&self, x: &[f64], y: &mut [f64]) -> std::io::Result<()> {
        use std::sync::mpsc;
        use wgpu::{CommandEncoderDescriptor, ComputePassDescriptor, MapMode, PollType};

        if x.len() != self.len || y.len() != self.len {
            return Err(std::io::Error::other("bad Laplacian GPU vector length"));
        }
        let x_bytes = f32_bytes(x.iter().map(|&v| v as f32));
        self.queue.write_buffer(&self.input, 0, &x_bytes);

        let mut encoder = self.device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("forge-laplacian-encoder"),
        });
        {
            let mut cpass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("forge-laplacian-pass"),
                timestamp_writes: None,
            });
            cpass.set_pipeline(&self.pipeline);
            cpass.set_bind_group(0, &self.bind_group, &[]);
            cpass.dispatch_workgroups(((self.len as u32) + 127) / 128, 1, 1);
        }
        encoder.copy_buffer_to_buffer(&self.output, 0, &self.readback, 0, (self.len * 4) as u64);
        self.queue.submit([encoder.finish()]);

        let slice = self.readback.slice(..);
        let (tx, rx) = mpsc::channel();
        slice.map_async(MapMode::Read, move |res| {
            let _ = tx.send(res);
        });
        let _ = self.device.poll(PollType::wait_indefinitely());
        let map_res = rx
            .recv()
            .map_err(|_| std::io::Error::other("wgpu Laplacian readback channel failed"))?;
        map_res.map_err(|e| std::io::Error::other(format!("wgpu Laplacian map_async failed: {e}")))?;

        let view = slice.get_mapped_range();
        for (dst, chunk) in y.iter_mut().zip(view.chunks_exact(4)) {
            *dst = f32::from_le_bytes(chunk.try_into().unwrap()) as f64;
        }
        drop(view);
        self.readback.unmap();
        Ok(())
    }
}

#[cfg(feature = "wgpu")]
fn storage_entry(binding: u32, read_only: bool) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

#[cfg(feature = "wgpu")]
fn u32_bytes(values: impl IntoIterator<Item = u32>) -> Vec<u8> {
    values.into_iter().flat_map(u32::to_le_bytes).collect()
}

#[cfg(feature = "wgpu")]
fn f32_bytes(values: impl IntoIterator<Item = f32>) -> Vec<u8> {
    values.into_iter().flat_map(f32::to_le_bytes).collect()
}

#[cfg(feature = "wgpu")]
const LAPLACIAN_MATVEC_WGSL: &str = r#"
const EMPTY: u32 = 0xffffffffu;

struct Params {
  nx: u32,
  ny: u32,
  nz: u32,
  len: u32,
}

@group(0) @binding(0)
var<storage, read> x: array<f32>;

@group(0) @binding(1)
var<storage, read_write> y: array<f32>;

@group(0) @binding(2)
var<storage, read> dof_of_cell: array<u32>;

@group(0) @binding(3)
var<storage, read> cell_of_dof: array<u32>;

@group(0) @binding(4)
var<uniform> params: Params;

fn dof_at(ix: u32, iy: u32, iz: u32) -> u32 {
  let lin = (iz * params.ny + iy) * params.nx + ix;
  return dof_of_cell[lin];
}

fn add_neighbour(n: u32, acc: ptr<function, f32>, deg: ptr<function, f32>) {
  if (n != EMPTY) {
    *deg = *deg + 1.0;
    *acc = *acc + x[n];
  }
}

@compute @workgroup_size(128)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
  let d = gid.x;
  if (d >= params.len) {
    return;
  }

  let base = d * 4u;
  let ix = cell_of_dof[base];
  let iy = cell_of_dof[base + 1u];
  let iz = cell_of_dof[base + 2u];

  var deg = 0.0;
  var acc = 0.0;
  if (ix > 0u) {
    add_neighbour(dof_at(ix - 1u, iy, iz), &acc, &deg);
  }
  if (ix + 1u < params.nx) {
    add_neighbour(dof_at(ix + 1u, iy, iz), &acc, &deg);
  }
  if (iy > 0u) {
    add_neighbour(dof_at(ix, iy - 1u, iz), &acc, &deg);
  }
  if (iy + 1u < params.ny) {
    add_neighbour(dof_at(ix, iy + 1u, iz), &acc, &deg);
  }
  if (iz > 0u) {
    add_neighbour(dof_at(ix, iy, iz - 1u), &acc, &deg);
  }
  if (iz + 1u < params.nz) {
    add_neighbour(dof_at(ix, iy, iz + 1u), &acc, &deg);
  }

  y[d] = deg * x[d] - acc;
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    fn cube(side: f64) -> Vec<SdfOp> {
        vec![SdfOp::Box { center: [0.0; 3], half_extents: [side * 0.5; 3] }]
    }

    #[test]
    fn laplacian_constant_field_is_zero() {
        let vox = Voxels::occupy(&cube(0.1), 12);
        let x = vec![3.25; vox.ndof()];
        let mut y = vec![0.0; vox.ndof()];
        vox.laplacian_matvec(&x, &mut y);
        let max_abs = y.iter().fold(0.0f64, |m, v| m.max(v.abs()));
        assert!(max_abs < 1e-12, "constant field should be in Laplacian nullspace: {max_abs}");
    }

    #[cfg(feature = "wgpu")]
    #[test]
    fn laplacian_wgpu_matches_cpu_when_adapter_is_available() {
        let vox = Voxels::occupy(&cube(0.1), 16);
        let x: Vec<f64> = (0..vox.ndof())
            .map(|i| ((i as f64) * 0.013).sin() + ((i as f64) * 0.007).cos())
            .collect();
        let mut cpu = vec![0.0; vox.ndof()];
        vox.laplacian_matvec_cpu(&x, &mut cpu);

        let Some(gpu) = vox.try_laplacian_matvec_wgpu_for_test(&x) else {
            eprintln!("skipping Laplacian WGPU differential: no usable adapter");
            return;
        };
        let max_abs = cpu
            .iter()
            .zip(&gpu)
            .map(|(a, b)| (a - b).abs())
            .fold(0.0f64, f64::max);
        assert!(max_abs < 2e-5, "CPU/GPU Laplacian mismatch: {max_abs}");
    }
}
