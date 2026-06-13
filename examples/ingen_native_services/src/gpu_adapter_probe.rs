use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeGpuAdapterProbe {
    pub schema: &'static str,
    pub status: &'static str,
    pub preferred_vendor: &'static str,
    pub selected: Option<NativeGpuAdapter>,
    pub adapters: Vec<NativeGpuAdapter>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
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

pub fn native_gpu_adapter_probe() -> NativeGpuAdapterProbe {
    let probe = scan::native_gpu_probe();
    let adapters = probe
        .adapters
        .into_iter()
        .map(|adapter| NativeGpuAdapter {
            name: adapter.name,
            vendor_id: adapter.vendor_id,
            device_id: adapter.device_id,
            backend: adapter.backend,
            device_type: adapter.device_type,
            driver: adapter.driver,
            driver_info: adapter.driver_info,
            selected: adapter.selected,
            score: adapter.score,
        })
        .collect::<Vec<_>>();

    NativeGpuAdapterProbe {
        schema: "ingen.native_services.gpu_adapter_probe.v1",
        status: probe.status,
        preferred_vendor: probe.preferred_vendor,
        selected: adapters.first().cloned(),
        adapters,
    }
}
