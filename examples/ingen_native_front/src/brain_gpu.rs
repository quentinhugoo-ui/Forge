//! GPU-rendered brain core. A WGSL fragment shader computes the amber metaball "living core"
//! entirely on the GPU; we read the offscreen texture back to RGBA bytes and hand it to Slint
//! as an image. This keeps the per-pixel work off the CPU (no lag) while sidestepping the
//! Slint <-> wgpu external-texture version mismatch (we own this device and only share bytes).
//!
//! Lazy by construction: the renderer is created on first use (when the Brain page opens) and
//! only `render()` is called while that page is visible.

pub const BRAIN_GPU_DIM: u32 = 1536;

const BRAIN_CORE_WGSL: &str = r#"
struct Uniforms { time: f32, _p0: f32, _p1: f32, _p2: f32 };
@group(0) @binding(0) var<uniform> u: Uniforms;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs(@builtin(vertex_index) vi: u32) -> VsOut {
    var p = array<vec2<f32>, 3>(vec2<f32>(-1.0, -1.0), vec2<f32>(3.0, -1.0), vec2<f32>(-1.0, 3.0));
    var out: VsOut;
    let xy = p[vi];
    out.pos = vec4<f32>(xy, 0.0, 1.0);
    out.uv = vec2<f32>(xy.x, -xy.y);
    return out;
}

fn sstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = clamp((x - e0) / (e1 - e0), 0.0, 1.0);
    return t * t * (3.0 - 2.0 * t);
}

fn mix3(a: vec3<f32>, b: vec3<f32>, t: f32) -> vec3<f32> {
    return a + (b - a) * clamp(t, 0.0, 1.0);
}

fn hue_rotate(c: vec3<f32>, deg: f32) -> vec3<f32> {
    let a = radians(deg);
    let cs = cos(a);
    let sn = sin(a);
    let m0 = vec3<f32>(0.213 + cs * 0.787 - sn * 0.213, 0.715 - cs * 0.715 - sn * 0.715, 0.072 - cs * 0.072 + sn * 0.928);
    let m1 = vec3<f32>(0.213 - cs * 0.213 + sn * 0.143, 0.715 + cs * 0.285 + sn * 0.140, 0.072 - cs * 0.072 - sn * 0.283);
    let m2 = vec3<f32>(0.213 - cs * 0.213 - sn * 0.787, 0.715 - cs * 0.715 + sn * 0.715, 0.072 + cs * 0.928 + sn * 0.072);
    return vec3<f32>(dot(c, m0), dot(c, m1), dot(c, m2));
}

const TAU: f32 = 6.2831853;

// Andrew-Manzyk "loader" reimagined as a procedural amber metaball: a round body plus seven
// triangular polygons spinning around their own pivots, fused by a contrast threshold (the
// CSS blur(7px)+contrast(15) gooey trick, but kept perfectly sharp — no blur). The spin speeds
// are mutually irrational and the threshold ("roundness") and hue ("colorize") drift on mixed
// sines, so the whole field never loops or restarts from zero.
@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {
    let time = u.time;
    // colorize: amber <-> burnt-orange hue drift, two incommensurate sines (never repeats).
    let hue = -42.0 + 36.0 * sin(time * 0.047) + 14.0 * sin(time * 0.019 + 1.7);
    // Subtle non-periodic breathing of the overall scale.
    let breathe = 1.0 + 0.040 * sin(time * 0.11) + 0.018 * sin(time * 0.043);
    let p = in.uv / breathe;

    // Seven polygons: (pivot.x, pivot.y, spin speed rad/s, phase). Index 0 is static
    // (the CSS rotate(90deg) child); the rest spin both ways at irrational speeds.
    var pv = array<vec4<f32>, 7>(
        vec4<f32>( 0.170,  0.170,  0.000, 1.5708),
        vec4<f32>( 0.000,  0.000, -0.311, 0.000),
        vec4<f32>( 0.000, -0.068,  0.307, 2.094),
        vec4<f32>(-0.068,  0.068, -0.323, 4.200),
        vec4<f32>(-0.068,  0.068, -0.297, 3.140),
        vec4<f32>( 0.068,  0.068,  0.319, 1.050),
        vec4<f32>( 0.068,  0.068,  0.289, 5.020),
    );
    // (vertex-orbit radius, lobe radius) per polygon.
    var pr = array<vec2<f32>, 7>(
        vec2<f32>(0.26, 0.20),
        vec2<f32>(0.30, 0.22),
        vec2<f32>(0.24, 0.19),
        vec2<f32>(0.20, 0.17),
        vec2<f32>(0.18, 0.16),
        vec2<f32>(0.22, 0.18),
        vec2<f32>(0.16, 0.15),
    );

    // Round body, then sum the rotating triangle vertices into the metaball field.
    var field = exp(-dot(p, p) / (0.46 * 0.46));
    for (var i = 0; i < 7; i = i + 1) {
        let a = pv[i];
        let pivot = a.xy;
        let ang = a.w + time * a.z;
        let vrad = pr[i].x;
        let lr = pr[i].y;
        let inv = 1.0 / (lr * lr);
        for (var v = 0; v < 3; v = v + 1) {
            let va = ang + f32(v) * 2.0943951; // 120 deg
            let vert = pivot + vrad * vec2<f32>(cos(va), sin(va));
            let d = p - vert;
            field = field + exp(-dot(d, d) * inv);
        }
    }

    // roundness: the threshold melts non-periodically between more-merged and more-articulated.
    let thr = 1.16 + 0.10 * sin(time * 0.19) + 0.05 * sin(time * 0.071 + 0.9);
    // Resolution-independent ~1px antialiased edge: crisp, never a soft glow halo.
    let g = max(fwidth(field), 1e-4);
    let core = smoothstep(thr - g, thr + g, field);
    if (core <= 0.002) { return vec4<f32>(0.0); }
    let rim = clamp(1.0 - abs(field - thr) / (g * 5.0), 0.0, 1.0) * core;

    // Vertical amber(top) -> burnt-orange(bottom) gradient, bright rim on the edge band.
    let grad = clamp(((p.y + 1.0) * 0.5 - 0.12) / 0.72, 0.0, 1.0);
    var base = mix3(vec3<f32>(255.0, 191.0, 72.0), vec3<f32>(190.0, 74.0, 29.0), 1.0 - grad);
    base = mix3(base, vec3<f32>(255.0, 224.0, 140.0), rim * 0.7);
    let tinted = hue_rotate(base, hue) / 255.0;
    return vec4<f32>(clamp(tinted, vec3<f32>(0.0), vec3<f32>(1.0)), core);
}
"#;

pub struct BrainGpuRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    readback: wgpu::Buffer,
    uniform: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    width: u32,
    height: u32,
}

impl BrainGpuRenderer {
    /// Build the renderer (blocks on async wgpu init). Returns None if no GPU is available,
    /// so the caller can fall back to the CPU path.
    pub fn new() -> Option<Self> {
        pollster::block_on(Self::new_inner()).ok()
    }

    async fn new_inner() -> Result<Self, String> {
        let width = BRAIN_GPU_DIM;
        let height = BRAIN_GPU_DIM;
        let instance = wgpu::Instance::default();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .map_err(|e| format!("request_adapter failed: {e:?}"))?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("ingen-brain-core-device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_defaults(),
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                memory_hints: wgpu::MemoryHints::MemoryUsage,
                trace: wgpu::Trace::Off,
            })
            .await
            .map_err(|e| format!("request_device failed: {e:?}"))?;

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("ingen-brain-core-shader"),
            source: wgpu::ShaderSource::Wgsl(BRAIN_CORE_WGSL.into()),
        });

        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ingen-brain-core-uniform"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("ingen-brain-core-bgl"),
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
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ingen-brain-core-bg"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ingen-brain-core-pl"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("ingen-brain-core-pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("ingen-brain-core-texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ingen-brain-core-readback"),
            size: (width * height * 4) as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Ok(Self {
            device,
            queue,
            pipeline,
            texture,
            view,
            readback,
            uniform,
            bind_group,
            width,
            height,
        })
    }

    /// Render one frame at continuous `time` (seconds) on the GPU and read it back as a Slint image.
    pub fn render(&self, time: f32) -> slint::Image {
        let mut u = [0u8; 16];
        u[0..4].copy_from_slice(&time.to_le_bytes());
        self.queue.write_buffer(&self.uniform, 0, &u);

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("ingen-brain-core-encoder"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("ingen-brain-core-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &self.readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(self.width * 4),
                    rows_per_image: Some(self.height),
                },
            },
            wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit(Some(encoder.finish()));

        let slice = self.readback.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        let _ = self.device.poll(wgpu::PollType::wait_indefinitely());

        let mut pixels =
            slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(self.width, self.height);
        {
            let data = slice.get_mapped_range();
            pixels.make_mut_bytes().copy_from_slice(&data);
        }
        self.readback.unmap();
        slint::Image::from_rgba8(pixels)
    }
}
