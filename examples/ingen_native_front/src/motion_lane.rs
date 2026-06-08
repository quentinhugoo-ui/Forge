use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{f32::consts::PI, time::Duration};

pub const WORK_MOTION_FRAME_COUNT: usize = 72;
pub const WORK_MOTION_WIDTH: u32 = 96;
pub const WORK_MOTION_HEIGHT: u32 = 96;
pub const PROFILE_BANNER_FRAME_COUNT: usize = 72;
pub const PROFILE_BANNER_WIDTH: u32 = 960;
pub const PROFILE_BANNER_HEIGHT: u32 = 160;
pub const BRAIN_CORE_DIM: u32 = 512;
pub const BRAIN_CORE_FRAME_COUNT: usize = 120;


const ARSH_WORK_SPINNER_CSS_SOURCE: &str = r#"/* From Uiverse.io by arshshaikh06 */
.spinner {
  background-image: linear-gradient(in oklch, rgb(49, 200, 178) 24%, rgb(123, 220, 190) 42%, rgb(246, 185, 124) 78%);
  border-radius: 50%;
  filter: blur(1px);
  box-shadow: 0px -5px 20px #31c8b2, 0px 5px 20px #f4b77c;
  animation: spinning82341 1.7s linear infinite, hue 1s ease-in-out infinite;
}
/* InGen Motion Lane adaptation: Rust-baked 3D/runtime frames, no Slint CSS animation. */
"#;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MotionLaneManifest {
    pub schema: String,
    pub motion_id: String,
    pub source_kind: String,
    pub source_hash: String,
    pub frame_count: usize,
    pub width: u32,
    pub height: u32,
    pub runtime_route: String,
    pub energy_policy: String,
    pub proof_hash: String,
}

pub struct MotionLane {
    frames: Vec<slint::Image>,
    manifest: MotionLaneManifest,
}

impl MotionLane {
    pub fn css_work_loader() -> Self {
        Self::css_work_loader_from_rgba_frames(css_work_loader_rgba_frames())
    }

    pub fn css_work_loader_from_rgba_frames(frames_rgba: Vec<Vec<u8>>) -> Self {
        let frames = frames_rgba
            .into_iter()
            .map(|rgba| image_from_rgba(WORK_MOTION_WIDTH, WORK_MOTION_HEIGHT, &rgba))
            .collect::<Vec<_>>();
        let source_hash = sha256_hex(ARSH_WORK_SPINNER_CSS_SOURCE.as_bytes());
        let proof_hash = hash_manifest_parts(&[
            "ingen.motion_lane.v1",
            "work-arsh-turquoise-orange-spinner",
            &source_hash,
            &WORK_MOTION_FRAME_COUNT.to_string(),
            &WORK_MOTION_WIDTH.to_string(),
            &WORK_MOTION_HEIGHT.to_string(),
        ]);
        let manifest = MotionLaneManifest {
            schema: "ingen.motion_lane.v1".to_string(),
            motion_id: "work-arsh-turquoise-orange-spinner".to_string(),
            source_kind: "css-reference-baked-to-rust-frame-atlas".to_string(),
            source_hash,
            frame_count: WORK_MOTION_FRAME_COUNT,
            width: WORK_MOTION_WIDTH,
            height: WORK_MOTION_HEIGHT,
            runtime_route: "preloaded SharedPixelBuffer frames; Slint receives current image only"
                .to_string(),
            energy_policy:
                "single 72-frame Rust atlas at Uiverse-paced rotation; CSS reference is baked as a full gradient disk with radial mask and bloom"
                    .to_string(),
            proof_hash,
        };
        Self { frames, manifest }
    }

    pub fn profile_banner() -> Self {
        let frames = (0..PROFILE_BANNER_FRAME_COUNT)
            .map(|frame| render_profile_banner_frame(frame, PROFILE_BANNER_FRAME_COUNT))
            .collect::<Vec<_>>();
        let source_hash = sha256_hex(b"profile-css-js-animation-viewer-v1");
        let proof_hash = hash_manifest_parts(&[
            "ingen.motion_lane.v1",
            "profile-animation-viewer",
            &source_hash,
            &PROFILE_BANNER_FRAME_COUNT.to_string(),
            &PROFILE_BANNER_WIDTH.to_string(),
            &PROFILE_BANNER_HEIGHT.to_string(),
        ]);
        let manifest = MotionLaneManifest {
            schema: "ingen.motion_lane.v1".to_string(),
            motion_id: "profile-animation-viewer".to_string(),
            source_kind: "css-js-animation-viewer-baked-to-rust-frame-atlas".to_string(),
            source_hash,
            frame_count: PROFILE_BANNER_FRAME_COUNT,
            width: PROFILE_BANNER_WIDTH,
            height: PROFILE_BANNER_HEIGHT,
            runtime_route: "Rust motion lane renders an animation preview frame; Slint displays image only"
                .to_string(),
            energy_policy:
                "shared low-frequency timer; no Slint geometry animation or embedded browser"
                    .to_string(),
            proof_hash,
        };
        Self { frames, manifest }
    }

    pub fn frame(&self, tick: usize) -> slint::Image {
        self.frames[tick % self.frames.len()].clone()
    }

    pub fn frame_count(&self) -> usize {
        self.frames.len()
    }

    pub fn frame_duration(&self) -> Duration {
        Duration::from_millis(24)
    }

    pub fn manifest(&self) -> &MotionLaneManifest {
        &self.manifest
    }
}

pub fn css_work_loader_rgba_frames() -> Vec<Vec<u8>> {
    (0..WORK_MOTION_FRAME_COUNT)
        .map(|frame| render_devvarad_ring_frame_rgba(frame, WORK_MOTION_FRAME_COUNT))
        .collect()
}

fn render_profile_banner_frame(frame: usize, frame_count: usize) -> slint::Image {
    let mut rgba = vec![0u8; (PROFILE_BANNER_WIDTH * PROFILE_BANNER_HEIGHT * 4) as usize];
    let t = frame as f32 / frame_count as f32;
    for y in 0..PROFILE_BANNER_HEIGHT {
        for x in 0..PROFILE_BANNER_WIDTH {
            let u = x as f32 / PROFILE_BANNER_WIDTH as f32;
            let v = y as f32 / PROFILE_BANNER_HEIGHT as f32;
            let scan = ((u * 3.0 + t * 1.2 + (v * 5.0).sin() * 0.045).fract() - 0.5).abs();
            let wave = ((u * 10.0 + t * 6.0).sin() * 0.5 + 0.5) * ((v * 4.0 - t * 2.0).cos() * 0.5 + 0.5);
            let line = (1.0 - smoothstep(0.0, 0.035, scan)) * 80.0;
            let glow = (wave * (1.0 - smoothstep(0.74, 1.0, v)) * 34.0) as u8;
            let red = (18.0 + line * 0.8) as u8;
            let green = 12u8.saturating_add(glow / 2);
            let blue = 13u8.saturating_add(glow);
            let idx = ((y * PROFILE_BANNER_WIDTH + x) * 4) as usize;
            rgba[idx] = red;
            rgba[idx + 1] = green;
            rgba[idx + 2] = blue;
            rgba[idx + 3] = 255;
        }
    }

    for row in 0..5 {
        let y = 22 + row * 25;
        let offset = ((t * PROFILE_BANNER_WIDTH as f32 * (0.25 + row as f32 * 0.04)) as i32)
            % PROFILE_BANNER_WIDTH as i32;
        let width = 240 + row as i32 * 54;
        blend_banner_rect(
            &mut rgba,
            ((offset - width).rem_euclid(PROFILE_BANNER_WIDTH as i32), y, width, 1),
            [192, 57, 43, 82],
        );
        blend_banner_rect(
            &mut rgba,
            ((PROFILE_BANNER_WIDTH as i32 - offset / 2).rem_euclid(PROFILE_BANNER_WIDTH as i32), y + 8, width / 2, 1),
            [104, 208, 196, 62],
        );
    }

    let mut pixels =
        slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(PROFILE_BANNER_WIDTH, PROFILE_BANNER_HEIGHT);
    pixels.make_mut_bytes().copy_from_slice(&rgba);
    slint::Image::from_rgba8(pixels)
}

fn blend_banner_rect(rgba: &mut [u8], rect: (i32, i32, i32, i32), color: [u8; 4]) {
    let (x, y, w, h) = rect;
    for yy in y.max(0)..(y + h).min(PROFILE_BANNER_HEIGHT as i32) {
        for xx_offset in 0..w.max(0) {
            let xx = (x + xx_offset).rem_euclid(PROFILE_BANNER_WIDTH as i32) as u32;
            let idx = ((yy as u32 * PROFILE_BANNER_WIDTH + xx) * 4) as usize;
            let alpha = color[3] as f32 / 255.0;
            rgba[idx] = ((color[0] as f32 * alpha) + (rgba[idx] as f32 * (1.0 - alpha))) as u8;
            rgba[idx + 1] = ((color[1] as f32 * alpha) + (rgba[idx + 1] as f32 * (1.0 - alpha))) as u8;
            rgba[idx + 2] = ((color[2] as f32 * alpha) + (rgba[idx + 2] as f32 * (1.0 - alpha))) as u8;
        }
    }
}

fn render_devvarad_ring_frame_rgba(frame: usize, frame_count: usize) -> Vec<u8> {
    let mut rgba = vec![0u8; (WORK_MOTION_WIDTH * WORK_MOTION_HEIGHT * 4) as usize];
    let t = frame as f32 / frame_count as f32;
    let rotation = t * 2.0 * PI;

    for y in 0..WORK_MOTION_HEIGHT {
        for x in 0..WORK_MOTION_WIDTH {
            let fx = (x as f32 + 0.5) / WORK_MOTION_WIDTH as f32;
            let fy = (y as f32 + 0.5) / WORK_MOTION_HEIGHT as f32;
            let nx = fx * 2.0 - 1.0;
            let ny = fy * 2.0 - 1.0;
            let radius = (nx * nx + ny * ny).sqrt();
            if radius > 1.28 {
                continue;
            }

            let angle = (ny.atan2(nx) + rotation).rem_euclid(2.0 * PI) / (2.0 * PI);
            let diagonal = ((nx * 0.58 + ny * 0.82) + 1.0) * 0.5;
            let hue_pos = (angle * 0.62 + diagonal * 0.30 + t * 0.18).fract();
            let glow_color = app_ring_color(hue_pos);

            let ring_core = gaussian(radius, 0.68, 0.055);
            let soft_edge = gaussian(radius, 0.68, 0.105);
            let outer_bloom = gaussian(radius, 0.72, 0.205);
            let inner_bloom = gaussian(radius, 0.51, 0.180) * 0.45;
            let light_bias = 0.80 + 0.20 * ((angle * 2.0 * PI + rotation * 0.45).sin() * 0.5 + 0.5);

            let bloom_alpha = ((outer_bloom * 82.0) + (inner_bloom * 34.0)) as u8;
            if bloom_alpha > 0 {
                blend_pixel(
                    &mut rgba,
                    x,
                    y,
                    [
                        glow_color[0] as u8,
                        glow_color[1] as u8,
                        glow_color[2] as u8,
                        bloom_alpha,
                    ],
                );
            }

            let ring_alpha = ((ring_core * 214.0 + soft_edge * 70.0) * light_bias).min(245.0) as u8;
            if ring_alpha > 0 {
                blend_pixel(
                    &mut rgba,
                    x,
                    y,
                    [
                        glow_color[0] as u8,
                        glow_color[1] as u8,
                        glow_color[2] as u8,
                        ring_alpha,
                    ],
                );
            }

            let core = 1.0 - smoothstep(0.41, 0.61, radius);
            if core > 0.0 {
                let core_shadow = 25.0 + 7.0 * smoothstep(0.0, 0.55, radius);
                blend_pixel(
                    &mut rgba,
                    x,
                    y,
                    [
                        core_shadow as u8,
                        core_shadow as u8,
                        (core_shadow - 1.0) as u8,
                        (core * 246.0) as u8,
                    ],
                );
            }
        }
    }

    rgba
}

fn image_from_rgba(width: u32, height: u32, rgba: &[u8]) -> slint::Image {
    let mut pixels = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(width, height);
    pixels.make_mut_bytes().copy_from_slice(rgba);
    slint::Image::from_rgba8(pixels)
}

/// Prebaked "lava lamp" brain core — one **seamless** frame of a lit metaball blob, returned as
/// raw RGBA so it can be baked on a worker thread (`slint::Image` is not `Send`). All motion is
/// driven by integer harmonics of `phase = frame / frame_count`, so frame N wraps back to frame 0
/// with no visible jump: the lane is a true seamless loop (cycled cheaply at runtime, never
/// regenerated per frame). The field gradient gives a fake surface normal -> diffuse + specular +
/// fresnel rim, for a glossy 3D look without raymarching.
pub fn render_brain_core_lava_rgba(frame: usize, frame_count: usize) -> Vec<u8> {
    let dim = BRAIN_CORE_DIM as usize;
    let tau = std::f32::consts::TAU;
    let ph = (frame as f32 / frame_count as f32) * tau;

    // Seamless hue + breathing (integer harmonics only).
    let hue = -40.0 + 30.0 * ph.sin() + 12.0 * (2.0 * ph + 1.7).sin();
    let breathe = 1.0 + 0.05 * ph.sin() + 0.025 * (2.0 * ph).cos();

    // Blobs: (ax, kx, px, ay, ky, py, r0, r1, kr). kx/ky/kr are INTEGER harmonics of the loop so
    // every blob returns exactly to its start at frame_count -> seamless. Mixed harmonics + phases
    // keep the drift organic (lava-lamp merge/split) within the loop.
    let blobs: [(f32, f32, f32, f32, f32, f32, f32, f32, f32); 7] = [
        (0.10, 1.0, 0.0, 0.12, 1.0, 1.0, 0.34, 0.04, 1.0),
        (0.30, 1.0, 2.1, 0.22, 2.0, 0.3, 0.26, 0.05, 2.0),
        (0.24, 2.0, 4.2, 0.30, 1.0, 2.7, 0.24, 0.04, 1.0),
        (0.32, 1.0, 1.0, 0.18, 3.0, 4.0, 0.22, 0.05, 2.0),
        (0.20, 3.0, 3.1, 0.28, 1.0, 5.2, 0.20, 0.04, 3.0),
        (0.34, 2.0, 5.0, 0.16, 2.0, 1.5, 0.18, 0.04, 1.0),
        (0.14, 1.0, 0.6, 0.26, 3.0, 3.3, 0.16, 0.03, 2.0),
    ];
    let mut centers = [(0.0f32, 0.0f32, 0.0f32); 7];
    for (i, b) in blobs.iter().enumerate() {
        let (ax, kx, px, ay, ky, py, r0, r1, kr) = *b;
        let cx = ax * (kx * ph + px).sin();
        let cy = ay * (ky * ph + py).sin();
        let r = r0 + r1 * (kr * ph).sin();
        centers[i] = (cx, cy, r);
    }

    // Pass 1: gaussian metaball field into a scratch buffer.
    let inv = 1.0 / dim as f32;
    let mut field = vec![0.0f32; dim * dim];
    for y in 0..dim {
        for x in 0..dim {
            let nx = ((x as f32 + 0.5) * inv * 2.0 - 1.0) / breathe;
            let ny = ((y as f32 + 0.5) * inv * 2.0 - 1.0) / breathe;
            let mut f = 0.0f32;
            for (cx, cy, r) in centers.iter() {
                let dx = nx - cx;
                let dy = ny - cy;
                f += (-(dx * dx + dy * dy) / (r * r)).exp();
            }
            field[y * dim + x] = f;
        }
    }

    // Pass 2: shade. Surface normal from the field gradient -> diffuse + specular + fresnel rim.
    let mut rgba = vec![0u8; dim * dim * 4];
    let light = normalize3([-0.42, -0.52, 0.74]);
    let thr = 1.0f32;
    for y in 0..dim {
        for x in 0..dim {
            let f = field[y * dim + x];
            let body = smoothstep(thr - 0.10, thr + 0.16, f);
            if body <= 0.004 {
                continue;
            }
            let xl = x.saturating_sub(1);
            let xr = (x + 1).min(dim - 1);
            let yt = y.saturating_sub(1);
            let yb = (y + 1).min(dim - 1);
            let gx = field[y * dim + xr] - field[y * dim + xl];
            let gy = field[yb * dim + x] - field[yt * dim + x];
            let n = normalize3([-gx * 6.0, -gy * 6.0, 1.0]);
            let diff = (n[0] * light[0] + n[1] * light[1] + n[2] * light[2]).max(0.0);
            let spec = diff.powf(26.0);
            let fres = (1.0 - n[2]).clamp(0.0, 1.0).powf(2.0);

            let ny = (y as f32 + 0.5) * inv * 2.0 - 1.0;
            let grad = (((ny + 1.0) * 0.5 - 0.1) / 0.8).clamp(0.0, 1.0);
            let base = mix_rgb([255.0, 196.0, 80.0], [186.0, 72.0, 28.0], grad);
            let lit = 0.42 + 0.66 * diff;
            let mut col = [base[0] * lit, base[1] * lit, base[2] * lit];
            col = mix_rgb(col, [255.0, 238.0, 190.0], spec * 0.85);
            col = mix_rgb(col, [255.0, 168.0, 70.0], fres * 0.35);
            let tinted = hue_rotate(
                [col[0].min(255.0), col[1].min(255.0), col[2].min(255.0)],
                hue,
            );

            let idx = (y * dim + x) * 4;
            rgba[idx] = tinted[0] as u8;
            rgba[idx + 1] = tinted[1] as u8;
            rgba[idx + 2] = tinted[2] as u8;
            rgba[idx + 3] = (body * 244.0).min(248.0) as u8;
        }
    }
    rgba
}

fn normalize3(v: [f32; 3]) -> [f32; 3] {
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-6);
    [v[0] / len, v[1] / len, v[2] / len]
}

/// Luminance-preserving hue rotation on a 0..255 RGB triple.
fn hue_rotate(c: [f32; 3], deg: f32) -> [f32; 3] {
    let a = deg.to_radians();
    let (sin, cos) = a.sin_cos();
    let m = [
        0.213 + cos * 0.787 - sin * 0.213,
        0.715 - cos * 0.715 - sin * 0.715,
        0.072 - cos * 0.072 + sin * 0.928,
        0.213 - cos * 0.213 + sin * 0.143,
        0.715 + cos * 0.285 + sin * 0.140,
        0.072 - cos * 0.072 - sin * 0.283,
        0.213 - cos * 0.213 - sin * 0.787,
        0.715 - cos * 0.715 + sin * 0.715,
        0.072 + cos * 0.928 + sin * 0.072,
    ];
    [
        (c[0] * m[0] + c[1] * m[1] + c[2] * m[2]).clamp(0.0, 255.0),
        (c[0] * m[3] + c[1] * m[4] + c[2] * m[5]).clamp(0.0, 255.0),
        (c[0] * m[6] + c[1] * m[7] + c[2] * m[8]).clamp(0.0, 255.0),
    ]
}

fn app_ring_color(t: f32) -> [f32; 3] {
    const STOPS: &[(f32, [f32; 3])] = &[
        (0.0, [49.0, 200.0, 178.0]),
        (0.18, [54.0, 182.0, 231.0]),
        (0.36, [112.0, 97.0, 231.0]),
        (0.50, [156.0, 80.0, 196.0]),
        (0.66, [231.0, 112.0, 132.0]),
        (0.80, [246.0, 185.0, 124.0]),
        (0.92, [123.0, 213.0, 180.0]),
        (1.0, [49.0, 200.0, 178.0]),
    ];
    for pair in STOPS.windows(2) {
        let (a_t, a_color) = pair[0];
        let (b_t, b_color) = pair[1];
        if t >= a_t && t <= b_t {
            let p = smootherstep((t - a_t) / (b_t - a_t));
            return mix_rgb(a_color, b_color, p);
        }
    }
    STOPS[0].1
}

fn mix_rgb(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    let t = t.clamp(0.0, 1.0);
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

fn gaussian(x: f32, center: f32, sigma: f32) -> f32 {
    let z = (x - center) / sigma;
    (-0.5 * z * z).exp()
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn smootherstep(x: f32) -> f32 {
    let t = x.clamp(0.0, 1.0);
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

fn blend_pixel(rgba: &mut [u8], x: u32, y: u32, src: [u8; 4]) {
    let idx = ((y * WORK_MOTION_WIDTH + x) * 4) as usize;
    let src_a = src[3] as f32 / 255.0;
    let dst_a = rgba[idx + 3] as f32 / 255.0;
    let out_a = src_a + dst_a * (1.0 - src_a);
    if out_a <= 0.0 {
        return;
    }
    for channel in 0..3 {
        let src_c = src[channel] as f32 / 255.0;
        let dst_c = rgba[idx + channel] as f32 / 255.0;
        let out_c = (src_c * src_a + dst_c * dst_a * (1.0 - src_a)) / out_a;
        rgba[idx + channel] = (out_c * 255.0).round() as u8;
    }
    rgba[idx + 3] = (out_a * 255.0).round() as u8;
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn hash_manifest_parts(parts: &[&str]) -> String {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part.as_bytes());
        hasher.update([0]);
    }
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn css_loader_motion_lane_is_prebaked_and_hashed() {
        let lane = MotionLane::css_work_loader();
        assert_eq!(lane.frames.len(), WORK_MOTION_FRAME_COUNT);
        assert_eq!(lane.manifest.motion_id, "work-arsh-turquoise-orange-spinner");
        assert_eq!(lane.manifest.width, WORK_MOTION_WIDTH);
        assert_eq!(lane.manifest.height, WORK_MOTION_HEIGHT);
        assert_eq!(lane.manifest.proof_hash.len(), 64);
        assert_eq!(lane.frame_count(), WORK_MOTION_FRAME_COUNT);
    }

    #[test]
    fn profile_banner_motion_lane_is_prebaked_and_hashed() {
        let lane = MotionLane::profile_banner();
        assert_eq!(lane.frames.len(), PROFILE_BANNER_FRAME_COUNT);
        assert_eq!(lane.manifest.motion_id, "profile-animation-viewer");
        assert_eq!(lane.manifest.width, PROFILE_BANNER_WIDTH);
        assert_eq!(lane.manifest.height, PROFILE_BANNER_HEIGHT);
        assert_eq!(lane.manifest.proof_hash.len(), 64);
        assert_eq!(lane.frame_count(), PROFILE_BANNER_FRAME_COUNT);
    }
}
