use ingen_native_services::banger_native_engine::{
    BangerGaussianSplatRasterizeRequest, BangerNativeEngine,
    BangerNativePresentLoopBootstrapRequest,
};
use ingen_native_services::gpu_adapter_probe::{native_gpu_adapter_probe, NativeGpuAdapterProbe};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ElectronBackendProjection {
    schema: &'static str,
    source: &'static str,
    backend: &'static str,
    generated_at_unix_ms: u128,
    active_section: &'static str,
    section_title: &'static str,
    sessions: Vec<SessionProjection>,
    transcript: Vec<TranscriptProjection>,
    native_status: NativeStatusProjection,
    proof_hash: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionProjection {
    session_id: &'static str,
    label: &'static str,
    date: &'static str,
    section: &'static str,
    pinned: bool,
    working: bool,
    automated: bool,
    archived: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TranscriptProjection {
    id: &'static str,
    role: &'static str,
    text: &'static str,
    proof_hash: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct NativeStatusProjection {
    state_owner: String,
    jobs: String,
    banger: String,
    webexplorer: String,
    monster: String,
    brain: String,
    provider: String,
    proof: String,
    gpu_probe: NativeGpuAdapterProbe,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BangerPreviewFrameProjection {
    accepted: bool,
    schema: &'static str,
    source: &'static str,
    width: u32,
    height: u32,
    frame_data_url: String,
    frame_hash: String,
    scene_hash: String,
    proof_hash: String,
    metrics: BangerPreviewFrameMetrics,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BangerPreviewFrameMetrics {
    splat_count: usize,
    projected_splat_count: usize,
    rasterized_splat_count: usize,
    shaded_pixel_count: u32,
    tile_count: u32,
    benchmark_gate_count: u32,
    promotion_allowed: bool,
    render_path: &'static str,
}

fn main() {
    if env::args().any(|argument| argument == "--banger-present-loop-bootstrap") {
        let frame = BangerNativeEngine::bootstrap_present_loop(BangerNativePresentLoopBootstrapRequest {
            parent_window_handle: env::var("FORGE_BANGER_PARENT_HWND").ok(),
            viewport_width: env::var("FORGE_BANGER_VIEWPORT_WIDTH").ok().and_then(|value| value.parse().ok()),
            viewport_height: env::var("FORGE_BANGER_VIEWPORT_HEIGHT").ok().and_then(|value| value.parse().ok()),
            target_frame_ms: env::var("FORGE_BANGER_TARGET_FRAME_MS").ok().and_then(|value| value.parse().ok()),
        })
        .expect("bootstrap banger native present loop");
        println!("{}", serde_json::to_string(&frame).expect("serialize banger present loop bootstrap"));
        return;
    }
    if env::args().any(|argument| argument == "--banger-preview-frame") {
        let frame = banger_preview_frame();
        println!("{}", serde_json::to_string(&frame).expect("serialize banger preview frame"));
        return;
    }
    let projection = projection();
    println!("{}", serde_json::to_string(&projection).expect("serialize electron projection"));
}

fn projection() -> ElectronBackendProjection {
    let generated_at_unix_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    let sessions = vec![
        SessionProjection {
            session_id: "native-front-migration",
            label: "Electron cutover",
            date: "2026-06-09",
            section: "forge",
            pinned: true,
            working: false,
            automated: false,
            archived: false,
        },
        SessionProjection {
            session_id: "test-session-example",
            label: "test session example",
            date: "2026-06-10",
            section: "forge",
            pinned: false,
            working: true,
            automated: false,
            archived: false,
        },
        SessionProjection {
            session_id: "banger-native-surface",
            label: "Banger native surface",
            date: "2026-06-09",
            section: "banger",
            pinned: false,
            working: false,
            automated: true,
            archived: false,
        },
        SessionProjection {
            session_id: "webexplorer-rust-webview",
            label: "WebExplorer Rust WebView",
            date: "2026-06-09",
            section: "webexplorer",
            pinned: false,
            working: false,
            automated: false,
            archived: false,
        },
        SessionProjection {
            session_id: "monster-compute-proof",
            label: "Monster compute proof",
            date: "2026-06-08",
            section: "forge",
            pinned: false,
            working: false,
            automated: false,
            archived: false,
        },
    ];
    let transcript = Vec::new();
    let gpu_probe = native_gpu_adapter_probe();
    let banger_status = banger_status_from_probe(&gpu_probe);
    let native_status = NativeStatusProjection {
        state_owner: "ingen_native_services::electron_backend_bridge".to_string(),
        jobs: "queued=0 running=0 done=0 failed=0".to_string(),
        banger: banger_status,
        webexplorer: "rust-owned-webview-slot=ready policy=locked".to_string(),
        monster: "local-compute=ready proof-cache=ready".to_string(),
        brain: "evidence-aware-memory=ready godel=ready".to_string(),
        provider: "provider=openai ready=true source=local-auth".to_string(),
        proof: "electron-cutover-rust-projection".to_string(),
        gpu_probe,
    };
    let mut projection = ElectronBackendProjection {
        schema: "ingen.native_services.electron_backend_projection.v1",
        source: "examples/ingen_native_services",
        backend: "rust",
        generated_at_unix_ms,
        active_section: "forge",
        section_title: "Forge",
        sessions,
        transcript,
        native_status,
        proof_hash: String::new(),
    };
    projection.proof_hash = proof_hash(&projection);
    projection
}

fn banger_status_from_probe(probe: &NativeGpuAdapterProbe) -> String {
    match &probe.selected {
        Some(adapter) => format!(
            "child-window-slot=ready renderer=rust-banger/wgpu selected={} backend={} type={} adapters={}",
            adapter.name,
            adapter.backend,
            adapter.device_type,
            probe.adapters.len()
        ),
        None => "child-window-slot=pending renderer=rust-banger/wgpu selected=unavailable adapters=0".to_string(),
    }
}

fn proof_hash<T: Serialize>(value: &T) -> String {
    let bytes = serde_json::to_vec(value).expect("projection hash serialization");
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn banger_preview_frame() -> BangerPreviewFrameProjection {
    let width = 320;
    let height = 200;
    let ply_path = preview_splat_path();
    fs::create_dir_all(ply_path.parent().expect("preview ply parent")).expect("create banger preview dir");
    fs::write(&ply_path, preview_splat_ply()).expect("write banger preview ply");

    let raster = BangerNativeEngine::rasterize_gaussian_splat_asset(BangerGaussianSplatRasterizeRequest {
        asset_id: Some("electron_header_banger_preview".to_string()),
        ply_path: ply_path.to_string_lossy().to_string(),
        width,
        height,
        camera_position: Some([0.0, 0.08, -4.25]),
        camera_target: Some([0.0, 0.02, 0.0]),
        camera_up: Some([0.0, 1.0, 0.0]),
        fov_y_degrees: Some(42.0),
        near_plane: Some(0.01),
        max_splats: None,
        tile_size: Some(16),
        background_rgba: Some([0.015, 0.018, 0.024, 1.0]),
    })
    .expect("rasterize banger preview splats");

    let bmp = rgba8_to_bmp(width, height, &raster.rgba8);
    let frame_hash = sha256_hex(&bmp);
    let scene_hash = sha256_hex(preview_splat_ply().as_bytes());
    let metrics = BangerPreviewFrameMetrics {
        splat_count: raster.splat_count,
        projected_splat_count: raster.projected_splat_count,
        rasterized_splat_count: raster.rasterized_splat_count,
        shaded_pixel_count: raster.shaded_pixel_count,
        tile_count: raster.tile_count,
        benchmark_gate_count: 5,
        promotion_allowed: raster.ok && raster.projected_splat_count > 0 && raster.shaded_pixel_count > 0,
        render_path: "rust_banger_gaussian_splat_rgba8_to_bmp_data_url",
    };
    let mut frame = BangerPreviewFrameProjection {
        accepted: true,
        schema: "forge.banger.visible_preview_frame.v1",
        source: "examples/ingen_native_services/banger_preview_frame",
        width,
        height,
        frame_data_url: format!("data:image/bmp;base64,{}", base64_encode(&bmp)),
        frame_hash,
        scene_hash,
        proof_hash: String::new(),
        metrics,
    };
    frame.proof_hash = proof_hash(&frame);
    frame
}

fn preview_splat_path() -> PathBuf {
    env::temp_dir()
        .join("forge-banger-preview-frame")
        .join("electron_header_banger_preview.ply")
}

fn preview_splat_ply() -> &'static str {
    r#"ply
format ascii 1.0
element vertex 12
property float x
property float y
property float z
property float f_dc_0
property float f_dc_1
property float f_dc_2
property float opacity
property float scale_0
property float scale_1
property float scale_2
property float rot_0
property float rot_1
property float rot_2
property float rot_3
end_header
0.00 0.00 0.00 1.55 0.18 0.10 4.4 -1.14 -1.20 -1.32 1 0 0 0
0.34 0.06 0.10 0.12 1.35 0.30 3.9 -1.33 -1.25 -1.45 1 0 0 0
-0.34 -0.07 0.04 0.14 0.38 1.42 3.8 -1.38 -1.28 -1.48 1 0 0 0
0.02 0.36 0.14 1.26 0.98 0.18 3.6 -1.42 -1.36 -1.54 1 0 0 0
0.00 -0.34 0.16 0.18 1.08 1.24 3.5 -1.44 -1.40 -1.56 1 0 0 0
0.46 0.26 0.28 1.36 0.38 0.86 3.2 -1.58 -1.50 -1.68 1 0 0 0
-0.46 0.23 0.22 0.28 1.22 0.88 3.2 -1.58 -1.50 -1.68 1 0 0 0
0.42 -0.28 0.24 1.12 0.80 0.20 3.1 -1.60 -1.55 -1.70 1 0 0 0
-0.42 -0.28 0.24 0.20 0.72 1.22 3.1 -1.60 -1.55 -1.70 1 0 0 0
0.00 0.02 0.48 1.42 1.28 1.02 2.9 -1.72 -1.64 -1.82 1 0 0 0
0.18 -0.02 -0.22 0.90 0.30 1.18 3.4 -1.50 -1.42 -1.60 1 0 0 0
-0.18 0.03 -0.20 0.30 1.10 1.04 3.4 -1.50 -1.42 -1.60 1 0 0 0
"#
}

fn rgba8_to_bmp(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    assert_eq!(rgba.len(), width as usize * height as usize * 4);
    let pixel_bytes = width as usize * height as usize * 4;
    let file_size = 54usize + pixel_bytes;
    let mut bmp = Vec::with_capacity(file_size);
    bmp.extend_from_slice(b"BM");
    bmp.extend_from_slice(&(file_size as u32).to_le_bytes());
    bmp.extend_from_slice(&[0, 0, 0, 0]);
    bmp.extend_from_slice(&54u32.to_le_bytes());
    bmp.extend_from_slice(&40u32.to_le_bytes());
    bmp.extend_from_slice(&(width as i32).to_le_bytes());
    bmp.extend_from_slice(&(height as i32).to_le_bytes());
    bmp.extend_from_slice(&1u16.to_le_bytes());
    bmp.extend_from_slice(&32u16.to_le_bytes());
    bmp.extend_from_slice(&0u32.to_le_bytes());
    bmp.extend_from_slice(&(pixel_bytes as u32).to_le_bytes());
    bmp.extend_from_slice(&0i32.to_le_bytes());
    bmp.extend_from_slice(&0i32.to_le_bytes());
    bmp.extend_from_slice(&0u32.to_le_bytes());
    bmp.extend_from_slice(&0u32.to_le_bytes());

    for y in (0..height as usize).rev() {
        let row = y * width as usize * 4;
        for x in 0..width as usize {
            let offset = row + x * 4;
            bmp.push(rgba[offset + 2]);
            bmp.push(rgba[offset + 1]);
            bmp.push(rgba[offset]);
            bmp.push(rgba[offset + 3]);
        }
    }
    bmp
}

fn base64_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut encoded = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0];
        let b1 = *chunk.get(1).unwrap_or(&0);
        let b2 = *chunk.get(2).unwrap_or(&0);
        encoded.push(TABLE[(b0 >> 2) as usize] as char);
        encoded.push(TABLE[(((b0 & 0b0000_0011) << 4) | (b1 >> 4)) as usize] as char);
        if chunk.len() > 1 {
            encoded.push(TABLE[(((b1 & 0b0000_1111) << 2) | (b2 >> 6)) as usize] as char);
        } else {
            encoded.push('=');
        }
        if chunk.len() > 2 {
            encoded.push(TABLE[(b2 & 0b0011_1111) as usize] as char);
        } else {
            encoded.push('=');
        }
    }
    encoded
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emits_banger_preview_frame_from_native_rasterizer() {
        let frame = banger_preview_frame();
        assert!(frame.accepted);
        assert_eq!(frame.schema, "forge.banger.visible_preview_frame.v1");
        assert_eq!(frame.width, 320);
        assert_eq!(frame.height, 200);
        assert!(frame.frame_data_url.starts_with("data:image/bmp;base64,Qk"));
        assert_eq!(frame.frame_hash.len(), 64);
        assert_eq!(frame.scene_hash.len(), 64);
        assert_eq!(frame.proof_hash.len(), 64);
        assert_eq!(frame.metrics.splat_count, 12);
        assert!(frame.metrics.projected_splat_count > 0);
        assert!(frame.metrics.shaded_pixel_count > 0);
        assert!(frame.metrics.promotion_allowed);
    }

    #[test]
    fn encodes_rgba8_as_browser_bmp() {
        let bmp = rgba8_to_bmp(1, 1, &[0x11, 0x22, 0x33, 0x44]);
        assert_eq!(&bmp[0..2], b"BM");
        assert_eq!(bmp.len(), 58);
        assert_eq!(&bmp[54..58], &[0x33, 0x22, 0x11, 0x44]);
        assert_eq!(base64_encode(b"BM"), "Qk0=");
    }
}
