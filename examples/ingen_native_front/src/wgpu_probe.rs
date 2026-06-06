use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WgpuProbe {
    pub available: bool,
    pub adapter_name: String,
    pub backend: String,
    pub device_type: String,
    pub texture_probe: bool,
    pub error: Option<String>,
}

impl WgpuProbe {
    pub fn summary(&self) -> String {
        if self.available {
            format!(
                "wgpu {} on {} ({}) texture_probe={}",
                self.backend, self.adapter_name, self.device_type, self.texture_probe
            )
        } else {
            format!("wgpu unavailable: {}", self.error.as_deref().unwrap_or("unknown error"))
        }
    }

    pub fn synthetic_available() -> Self {
        Self {
            available: true,
            adapter_name: "synthetic adapter".to_string(),
            backend: "Vulkan".to_string(),
            device_type: "DiscreteGpu".to_string(),
            texture_probe: true,
            error: None,
        }
    }
}

pub fn run_wgpu_probe() -> WgpuProbe {
    match pollster::block_on(run_wgpu_probe_inner()) {
        Ok(probe) => probe,
        Err(error) => WgpuProbe {
            available: false,
            adapter_name: "unavailable".to_string(),
            backend: "unavailable".to_string(),
            device_type: "unavailable".to_string(),
            texture_probe: false,
            error: Some(error),
        },
    }
}

async fn run_wgpu_probe_inner() -> Result<WgpuProbe, String> {
    let instance = wgpu::Instance::default();
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        })
        .await
        .map_err(|error| format!("request_adapter failed: {error:?}"))?;

    let info = adapter.get_info();
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("ingen-native-front-stage0-device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::downlevel_defaults(),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: wgpu::MemoryHints::MemoryUsage,
            trace: wgpu::Trace::Off,
        })
        .await
        .map_err(|error| format!("request_device failed: {error:?}"))?;

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("ingen-native-front-stage0-texture"),
        size: wgpu::Extent3d {
            width: 64,
            height: 64,
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
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("ingen-native-front-stage0-clear"),
    });
    {
        let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("ingen-native-front-stage0-clear-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.02,
                        g: 0.04,
                        b: 0.07,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
    }
    queue.submit(Some(encoder.finish()));
    let _ = device.poll(wgpu::PollType::wait_indefinitely());

    Ok(WgpuProbe {
        available: true,
        adapter_name: info.name,
        backend: format!("{:?}", info.backend),
        device_type: format!("{:?}", info.device_type),
        texture_probe: true,
        error: None,
    })
}
