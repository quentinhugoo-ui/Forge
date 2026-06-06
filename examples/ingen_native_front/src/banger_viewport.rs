use crate::wgpu_probe::{run_wgpu_probe, WgpuProbe};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const BANGER_VIEWPORT_WIDTH: u32 = 640;
pub const BANGER_VIEWPORT_HEIGHT: u32 = 360;
const BANGER_VIEWPORT_FORMAT: &str = "rgba8unorm";

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BangerViewportRequest {
    pub scene_id: String,
    pub width: u32,
    pub height: u32,
    pub frame_index: u32,
}

impl Default for BangerViewportRequest {
    fn default() -> Self {
        Self {
            scene_id: "stage4-native-fixture".to_string(),
            width: BANGER_VIEWPORT_WIDTH,
            height: BANGER_VIEWPORT_HEIGHT,
            frame_index: 0,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BangerSlintTextureBridgeProof {
    pub schema: String,
    pub target_shell: String,
    pub forge_wgpu_version: String,
    pub slint_wgpu_floor: String,
    pub texture_binding_ready: bool,
    pub direct_texture_import_ready: bool,
    pub fallback_route: String,
    pub blocker: String,
    pub proof_hash: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BangerViewportFrame {
    pub schema: String,
    pub scene_id: String,
    pub width: u32,
    pub height: u32,
    pub format: String,
    pub frame_index: u32,
    pub backend: String,
    pub adapter_name: String,
    pub texture_usage: Vec<String>,
    pub render_target_hash: String,
    pub frame_hash: String,
    pub telemetry_hash: String,
    pub viewport_contract_hash: String,
    pub slint_texture_bridge: BangerSlintTextureBridgeProof,
    pub rgba: Vec<u8>,
}

pub fn render_banger_viewport_frame() -> BangerViewportFrame {
    render_banger_viewport_frame_with(BangerViewportRequest::default(), run_wgpu_probe())
}

pub fn render_banger_viewport_frame_with(
    request: BangerViewportRequest,
    probe: WgpuProbe,
) -> BangerViewportFrame {
    let width = request.width.clamp(64, 2048);
    let height = request.height.clamp(64, 2048);
    let rgba = raster_fixture_scene(width, height, request.frame_index);
    let render_target_hash = sha256_hex(&rgba);
    let texture_usage = vec![
        "RENDER_ATTACHMENT".to_string(),
        "COPY_SRC".to_string(),
        "COPY_DST".to_string(),
        "TEXTURE_BINDING".to_string(),
    ];
    let bridge = slint_texture_bridge_proof(width, height, &render_target_hash, &probe);
    let telemetry_hash = hash_json(&(
        &request.scene_id,
        width,
        height,
        request.frame_index,
        &probe.backend,
        &probe.adapter_name,
        probe.texture_probe,
    ));
    let viewport_contract_hash = hash_json(&(
        "ingen.banger.native_viewport.frame_contract.v1",
        &render_target_hash,
        &telemetry_hash,
        &bridge.proof_hash,
        &texture_usage,
    ));
    let frame_hash = hash_json(&(
        "ingen.banger.native_viewport.frame.v1",
        &request.scene_id,
        width,
        height,
        request.frame_index,
        &render_target_hash,
        &viewport_contract_hash,
    ));

    BangerViewportFrame {
        schema: "ingen.banger.native_viewport.frame.v1".to_string(),
        scene_id: request.scene_id,
        width,
        height,
        format: BANGER_VIEWPORT_FORMAT.to_string(),
        frame_index: request.frame_index,
        backend: probe.backend,
        adapter_name: probe.adapter_name,
        texture_usage,
        render_target_hash,
        frame_hash,
        telemetry_hash,
        viewport_contract_hash,
        slint_texture_bridge: bridge,
        rgba,
    }
}

pub fn banger_frame_image(frame: &BangerViewportFrame) -> slint::Image {
    let mut pixels = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(frame.width, frame.height);
    pixels.make_mut_bytes().copy_from_slice(&frame.rgba);
    slint::Image::from_rgba8(pixels)
}

fn slint_texture_bridge_proof(
    width: u32,
    height: u32,
    render_target_hash: &str,
    probe: &WgpuProbe,
) -> BangerSlintTextureBridgeProof {
    let texture_binding_ready = probe.available && probe.texture_probe;
    let direct_texture_import_ready = false;
    let fallback_route = "SharedPixelBuffer image until Slint and Banger share the same wgpu major"
        .to_string();
    let blocker = "Slint 1.16 exposes a versioned unstable wgpu texture path while the native Banger probe uses wgpu 29; direct external texture import promotes only when Device/Queue/Texture types match."
        .to_string();
    let proof_hash = hash_json(&(
        "ingen.banger.slint_texture_bridge.v1",
        width,
        height,
        render_target_hash,
        &probe.backend,
        &probe.adapter_name,
        texture_binding_ready,
        direct_texture_import_ready,
        &fallback_route,
        &blocker,
    ));
    BangerSlintTextureBridgeProof {
        schema: "ingen.banger.slint_texture_bridge.v1".to_string(),
        target_shell: "slint-native-shell".to_string(),
        forge_wgpu_version: "wgpu_29".to_string(),
        slint_wgpu_floor: "slint_1_16_versioned_unstable_wgpu".to_string(),
        texture_binding_ready,
        direct_texture_import_ready,
        fallback_route,
        blocker,
        proof_hash,
    }
}

fn raster_fixture_scene(width: u32, height: u32, frame_index: u32) -> Vec<u8> {
    let mut rgba = vec![0; (width as usize) * (height as usize) * 4];
    let time = frame_index as f32 * 0.11;
    let cx = width as f32 * 0.52;
    let cy = height as f32 * 0.48;
    let radius = width.min(height) as f32 * 0.22;
    for y in 0..height {
        for x in 0..width {
            let fx = x as f32 / width.max(1) as f32;
            let fy = y as f32 / height.max(1) as f32;
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            let dist = (dx * dx + dy * dy).sqrt();
            let rim = ((radius - dist).abs() / radius.max(1.0)).clamp(0.0, 1.0);
            let grid = ((x / 32 + y / 32) % 2) as u8;
            let glow = (1.0 - (dist / (radius * 1.9)).clamp(0.0, 1.0)).powf(2.0);
            let axis = ((dx * 0.012 + time).sin() * (dy * 0.018 - time).cos()).abs();
            let inside = dist < radius;
            let idx = ((y * width + x) * 4) as usize;
            let base = if grid == 0 { 14.0 } else { 18.0 };
            rgba[idx] = (base + 34.0 * fx + 42.0 * glow + if inside { 42.0 * rim } else { 0.0 })
                .clamp(0.0, 255.0) as u8;
            rgba[idx + 1] = (base + 44.0 * fy + 92.0 * glow + if inside { 68.0 * axis } else { 0.0 })
                .clamp(0.0, 255.0) as u8;
            rgba[idx + 2] = (18.0 + 60.0 * (1.0 - fy) + 108.0 * glow + if inside { 38.0 } else { 0.0 })
                .clamp(0.0, 255.0) as u8;
            rgba[idx + 3] = 255;
        }
    }
    draw_crosshair(&mut rgba, width, height, cx as i32, cy as i32);
    draw_frame_corners(&mut rgba, width, height);
    rgba
}

fn draw_crosshair(rgba: &mut [u8], width: u32, height: u32, cx: i32, cy: i32) {
    for offset in -42..=42 {
        put_pixel(rgba, width, height, cx + offset, cy, [214, 221, 225, 210]);
        put_pixel(rgba, width, height, cx, cy + offset, [214, 221, 225, 210]);
    }
    for offset in -18..=18 {
        put_pixel(rgba, width, height, cx + offset, cy - offset, [49, 200, 178, 230]);
        put_pixel(rgba, width, height, cx + offset, cy + offset, [217, 119, 87, 230]);
    }
}

fn draw_frame_corners(rgba: &mut [u8], width: u32, height: u32) {
    let margin = 28;
    let len = 48;
    for i in 0..len {
        put_pixel(rgba, width, height, margin + i, margin, [214, 221, 225, 170]);
        put_pixel(rgba, width, height, margin, margin + i, [214, 221, 225, 170]);
        put_pixel(
            rgba,
            width,
            height,
            width as i32 - margin - i,
            margin,
            [214, 221, 225, 170],
        );
        put_pixel(
            rgba,
            width,
            height,
            width as i32 - margin,
            margin + i,
            [214, 221, 225, 170],
        );
        put_pixel(
            rgba,
            width,
            height,
            margin + i,
            height as i32 - margin,
            [214, 221, 225, 170],
        );
        put_pixel(
            rgba,
            width,
            height,
            margin,
            height as i32 - margin - i,
            [214, 221, 225, 170],
        );
        put_pixel(
            rgba,
            width,
            height,
            width as i32 - margin - i,
            height as i32 - margin,
            [214, 221, 225, 170],
        );
        put_pixel(
            rgba,
            width,
            height,
            width as i32 - margin,
            height as i32 - margin - i,
            [214, 221, 225, 170],
        );
    }
}

fn put_pixel(rgba: &mut [u8], width: u32, height: u32, x: i32, y: i32, color: [u8; 4]) {
    if x < 0 || y < 0 || x >= width as i32 || y >= height as i32 {
        return;
    }
    let idx = ((y as u32 * width + x as u32) * 4) as usize;
    rgba[idx..idx + 4].copy_from_slice(&color);
}

fn hash_json<T: Serialize>(value: &T) -> String {
    let bytes = serde_json::to_vec(value).expect("serialize banger viewport hash input");
    sha256_hex(&bytes)
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn banger_viewport_frame_is_deterministic() {
        let probe = WgpuProbe::synthetic_available();
        let request = BangerViewportRequest::default();
        let first = render_banger_viewport_frame_with(request.clone(), probe.clone());
        let second = render_banger_viewport_frame_with(request, probe);

        assert_eq!(first.frame_hash, second.frame_hash);
        assert_eq!(first.width, BANGER_VIEWPORT_WIDTH);
        assert_eq!(first.height, BANGER_VIEWPORT_HEIGHT);
        assert_eq!(first.rgba.len(), (first.width * first.height * 4) as usize);
        assert!(first.slint_texture_bridge.texture_binding_ready);
        assert!(!first.slint_texture_bridge.direct_texture_import_ready);
    }

    #[test]
    fn banger_viewport_frame_changes_with_frame_index() {
        let probe = WgpuProbe::synthetic_available();
        let first = render_banger_viewport_frame_with(
            BangerViewportRequest {
                frame_index: 0,
                ..BangerViewportRequest::default()
            },
            probe.clone(),
        );
        let second = render_banger_viewport_frame_with(
            BangerViewportRequest {
                frame_index: 1,
                ..BangerViewportRequest::default()
            },
            probe,
        );

        assert_ne!(first.frame_hash, second.frame_hash);
        assert_ne!(first.render_target_hash, second.render_target_hash);
    }
}
