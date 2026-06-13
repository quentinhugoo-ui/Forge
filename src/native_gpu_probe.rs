#[derive(Clone, Debug)]
pub struct NativeGpuProbe {
    pub status: &'static str,
    pub preferred_vendor: &'static str,
    pub selected: Option<NativeGpuAdapter>,
    pub adapters: Vec<NativeGpuAdapter>,
}

#[derive(Clone, Debug)]
pub struct NativeGpuAdapter {
    pub name: String,
    pub vendor_id: u32,
    pub device_id: u32,
    pub backend: String,
    pub device_type: String,
    pub driver: String,
    pub driver_info: String,
    pub selected: bool,
    pub score: u8,
}

#[cfg(feature = "wgpu")]
pub fn native_gpu_probe() -> NativeGpuProbe {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::all(),
        flags: Default::default(),
        backend_options: Default::default(),
        display: Default::default(),
        memory_budget_thresholds: Default::default(),
    });
    let mut adapters = pollster::block_on(instance.enumerate_adapters(wgpu::Backends::all()))
        .into_iter()
        .map(|adapter| {
            let info = adapter.get_info();
            NativeGpuAdapter {
                score: adapter_score(&info),
                name: info.name,
                vendor_id: info.vendor,
                device_id: info.device,
                backend: format!("{:?}", info.backend),
                device_type: format!("{:?}", info.device_type),
                driver: info.driver,
                driver_info: info.driver_info,
                selected: false,
            }
        })
        .collect::<Vec<_>>();
    adapters.sort_by(|a, b| a.score.cmp(&b.score).then_with(|| a.name.cmp(&b.name)));
    if let Some(first) = adapters.first_mut() {
        first.selected = true;
    }

    NativeGpuProbe {
        status: if adapters.is_empty() { "unavailable" } else { "ready" },
        preferred_vendor: "nvidia",
        selected: adapters.first().cloned(),
        adapters,
    }
}

#[cfg(not(feature = "wgpu"))]
pub fn native_gpu_probe() -> NativeGpuProbe {
    NativeGpuProbe {
        status: "wgpu_feature_disabled",
        preferred_vendor: "nvidia",
        selected: None,
        adapters: Vec::new(),
    }
}

#[cfg(feature = "wgpu")]
fn adapter_score(info: &wgpu::AdapterInfo) -> u8 {
    let name = info.name.to_ascii_lowercase();
    if info.vendor == 0x10de
        || name.contains("nvidia")
        || name.contains("geforce")
        || name.contains("rtx")
        || name.contains("quadro")
    {
        return 0;
    }
    match info.device_type {
        wgpu::DeviceType::DiscreteGpu => 1,
        wgpu::DeviceType::IntegratedGpu => 2,
        wgpu::DeviceType::VirtualGpu => 3,
        wgpu::DeviceType::Other => 4,
        wgpu::DeviceType::Cpu => 5,
    }
}
