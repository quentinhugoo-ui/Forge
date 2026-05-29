//! Banger SDF raymarcher — INGEN COMPUTE §19.4 ("Visualisation Directe").
//!
//! Render-to-texture offscreen pipeline using the BangerEngine wgpu
//! device. The fragment shader raymarches a hardcoded signed distance
//! scene (smooth-union of two spheres + ground plane), producing RGBA8
//! pixels that the UI blits onto the BOOM canvas via `putImageData`.
//!
//! Frontier hypothesis: prove the end-to-end circuit
//! `intent -> WGSL SDF -> GPU pixels -> 2D canvas` without a single
//! triangle, mesh import, or vertex buffer. Once verified, the next
//! slice replaces the hardcoded `scene()` body with an opcode buffer
//! emitted by `src/sdf.rs` so the scene is data, not source code.
//!
//! Verifier: `cargo check --bin forge-ui` plus the visual smoke test
//! (open Banger from the BOOM titlebar button and confirm the canvas
//! shows a rotating smooth-blended SDF instead of the placeholder).

use std::borrow::Cow;
use std::sync::Mutex;

/// WGSL shader. Full-screen triangle in the vertex stage, raymarch in
/// the fragment stage. Uniforms: viewport size + time (seconds).
const WGSL: &str = r#"
struct Uniforms {
  resolution: vec2<f32>,
  time:       f32,
  _pad:       f32,
};

@group(0) @binding(0) var<uniform> uni: Uniforms;

@vertex
fn vs_main(@builtin(vertex_index) idx: u32) -> @builtin(position) vec4<f32> {
  // Single triangle covering the full screen; UV derived from gl_Position.
  let x = f32((idx << 1u) & 2u) * 2.0 - 1.0;
  let y = f32(idx & 2u) * 2.0 - 1.0;
  return vec4<f32>(x, y, 0.0, 1.0);
}

fn sd_sphere(p: vec3<f32>, r: f32) -> f32 {
  return length(p) - r;
}

fn sd_box(p: vec3<f32>, b: vec3<f32>) -> f32 {
  let q = abs(p) - b;
  return length(max(q, vec3<f32>(0.0))) + min(max(q.x, max(q.y, q.z)), 0.0);
}

// Smooth union via log-sum-exp softmin — mirrors src/sdf.rs::SmoothUnion.
fn smin(a: f32, b: f32, k: f32) -> f32 {
  let m = min(a, b);
  return m - log(exp(-k * (a - m)) + exp(-k * (b - m))) / k;
}

fn scene(p: vec3<f32>) -> f32 {
  let t = uni.time;
  let drift = 0.45 * sin(t);
  let s1 = sd_sphere(p - vec3<f32>(-0.7,            0.05 * cos(t * 1.4), 0.0), 0.7);
  let s2 = sd_sphere(p - vec3<f32>(0.7 + drift,    -0.05 * cos(t * 1.1), 0.0), 0.7);
  let blob = smin(s1, s2, 5.0);
  let floor = sd_box(p - vec3<f32>(0.0, -1.05, 0.0), vec3<f32>(3.5, 0.05, 3.5));
  return min(blob, floor);
}

fn calc_normal(p: vec3<f32>) -> vec3<f32> {
  let e = vec2<f32>(0.0015, 0.0);
  return normalize(vec3<f32>(
    scene(p + e.xyy) - scene(p - e.xyy),
    scene(p + e.yxy) - scene(p - e.yxy),
    scene(p + e.yyx) - scene(p - e.yyx),
  ));
}

@fragment
fn fs_main(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
  let uv = (frag.xy * 2.0 - uni.resolution) / uni.resolution.y;

  // Orbital camera around the origin.
  let theta = uni.time * 0.35;
  let ro = vec3<f32>(cos(theta) * 3.6, 1.4, sin(theta) * 3.6);
  let fwd   = normalize(-ro);
  let right = normalize(cross(fwd, vec3<f32>(0.0, 1.0, 0.0)));
  let up    = cross(right, fwd);
  let rd    = normalize(fwd + uv.x * right - uv.y * up);

  var t = 0.0;
  var hit = false;
  for (var i = 0u; i < 96u; i = i + 1u) {
    let p = ro + rd * t;
    let d = scene(p);
    if (d < 0.0015) {
      hit = true;
      break;
    }
    if (t > 60.0) {
      break;
    }
    t = t + d;
  }

  if (hit) {
    let p = ro + rd * t;
    let n = calc_normal(p);
    let l = normalize(vec3<f32>(0.55, 0.85, 0.40));
    let lambert = max(dot(n, l), 0.0);
    let rim = pow(1.0 - max(dot(n, -rd), 0.0), 2.0);
    let ambient = vec3<f32>(0.12, 0.14, 0.18);
    let diffuse = vec3<f32>(0.80, 0.74, 0.68) * lambert;
    let rim_col = vec3<f32>(0.35, 0.50, 0.75) * rim * 0.20;
    let col = ambient + diffuse + rim_col;
    return vec4<f32>(col, 1.0);
  }

  // Sky gradient — uv.y < 0 is the top of the screen.
  let bg = mix(vec3<f32>(0.05, 0.06, 0.09), vec3<f32>(0.10, 0.12, 0.16), 0.5 - uv.y * 0.5);
  return vec4<f32>(bg, 1.0);
}
"#;

/// Bytes per pixel for the RGBA8Unorm target.
const BYTES_PER_PIXEL: u32 = 4;
/// wgpu requires the buffer row stride for texture readback to be a
/// multiple of 256 bytes.
const READBACK_ROW_ALIGNMENT: u32 = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;

#[repr(C)]
#[derive(Clone, Copy)]
struct UniformBlock {
    resolution: [f32; 2],
    time: f32,
    _pad: f32,
}

struct CachedRenderer {
    width: u32,
    height: u32,
    target: wgpu::Texture,
    target_view: wgpu::TextureView,
    readback: wgpu::Buffer,
    uniform_buf: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    pipeline: wgpu::RenderPipeline,
    padded_bytes_per_row: u32,
}

/// Per-engine cache: render pipeline + bind group layout are stable across
/// frames; texture / readback buffer are rebuilt when the requested size
/// changes (open-then-resize-then-rerender is a common UI path).
pub struct BangerSdfRenderer {
    cache: Mutex<Option<CachedRenderer>>,
}

impl BangerSdfRenderer {
    pub const fn new() -> Self {
        Self { cache: Mutex::new(None) }
    }

    /// Drop every cached GPU resource. Call after the BangerEngine
    /// releases its Device so the next `render_frame` rebuilds against
    /// the fresh device.
    pub fn reset(&self) {
        if let Ok(mut guard) = self.cache.lock() {
            *guard = None;
        }
    }

    /// Render a single SDF frame at the requested size. Blocks the caller
    /// while the GPU executes and the readback buffer is mapped — this is
    /// fine for an on-demand `requestAnimationFrame` driver because the
    /// scene is intentionally cheap (~10 ms per frame at 512² on a
    /// dedicated GPU).
    pub fn render_frame(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        width: u32,
        height: u32,
        time_seconds: f32,
    ) -> Result<Vec<u8>, String> {
        let width = width.max(1);
        let height = height.max(1);

        let mut guard = self
            .cache
            .lock()
            .map_err(|e| format!("sdf renderer lock poisoned: {e}"))?;
        let needs_rebuild = guard
            .as_ref()
            .map(|c| c.width != width || c.height != height)
            .unwrap_or(true);
        if needs_rebuild {
            *guard = Some(build_cached(device, width, height));
        }
        let cache = guard.as_ref().expect("cache present after rebuild");

        let uniform = UniformBlock {
            resolution: [width as f32, height as f32],
            time: time_seconds,
            _pad: 0.0,
        };
        queue.write_buffer(&cache.uniform_buf, 0, bytemuck_of_uniform(&uniform));

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("banger-sdf-encoder"),
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("banger-sdf-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &cache.target_view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&cache.pipeline);
            pass.set_bind_group(0, &cache.bind_group, &[]);
            pass.draw(0..3, 0..1);
        }

        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &cache.target,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &cache.readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(cache.padded_bytes_per_row),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        );

        queue.submit(std::iter::once(encoder.finish()));

        let slice = cache.readback.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });
        device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|e| format!("device poll failed: {e}"))?;
        rx.recv()
            .map_err(|e| format!("readback channel closed: {e}"))?
            .map_err(|e| format!("readback map failed: {e}"))?;

        let pixel_row = (width * BYTES_PER_PIXEL) as usize;
        let mut pixels = Vec::with_capacity(pixel_row * height as usize);
        {
            let view = slice.get_mapped_range();
            for row in 0..height as usize {
                let start = row * cache.padded_bytes_per_row as usize;
                pixels.extend_from_slice(&view[start..start + pixel_row]);
            }
        }
        cache.readback.unmap();

        Ok(pixels)
    }
}

fn bytemuck_of_uniform(u: &UniformBlock) -> &[u8] {
    // Plain repr(C) struct of four f32s — safe to read as raw bytes.
    unsafe {
        std::slice::from_raw_parts(
            (u as *const UniformBlock) as *const u8,
            std::mem::size_of::<UniformBlock>(),
        )
    }
}

fn build_cached(device: &wgpu::Device, width: u32, height: u32) -> CachedRenderer {
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("banger-sdf-shader"),
        source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(WGSL)),
    });

    let bind_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("banger-sdf-bind-layout"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("banger-sdf-pipeline-layout"),
        bind_group_layouts: &[Some(&bind_layout)],
        immediate_size: 0,
    });

    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("banger-sdf-pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: Some("vs_main"),
            buffers: &[],
            compilation_options: Default::default(),
        },
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            unclipped_depth: false,
            polygon_mode: wgpu::PolygonMode::Fill,
            conservative: false,
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Rgba8Unorm,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        multiview_mask: None,
        cache: None,
    });

    let uniform_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("banger-sdf-uniform"),
        size: std::mem::size_of::<UniformBlock>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("banger-sdf-bind"),
        layout: &bind_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: uniform_buf.as_entire_binding(),
        }],
    });

    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("banger-sdf-target"),
        size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let target_view = target.create_view(&wgpu::TextureViewDescriptor::default());

    let unpadded_row = width * BYTES_PER_PIXEL;
    let padded_bytes_per_row = unpadded_row
        .div_ceil(READBACK_ROW_ALIGNMENT)
        * READBACK_ROW_ALIGNMENT;
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("banger-sdf-readback"),
        size: padded_bytes_per_row as u64 * height as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    CachedRenderer {
        width,
        height,
        target,
        target_view,
        readback,
        uniform_buf,
        bind_group,
        pipeline,
        padded_bytes_per_row,
    }
}
