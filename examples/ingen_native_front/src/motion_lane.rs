use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{f32::consts::PI, time::Duration};

pub const WORK_MOTION_FRAME_COUNT: usize = 72;
pub const WORK_MOTION_WIDTH: u32 = 64;
pub const WORK_MOTION_HEIGHT: u32 = 64;
pub const PROFILE_BANNER_FRAME_COUNT: usize = 72;
pub const PROFILE_BANNER_WIDTH: u32 = 960;
pub const PROFILE_BANNER_HEIGHT: u32 = 160;
pub const BRAIN_CORE_WIDTH: u32 = 512;
pub const BRAIN_CORE_HEIGHT: u32 = 512;


const ARSH_WORK_SPINNER_CSS_SOURCE: &str = r#"/* From Uiverse.io by arshshaikh06 */
.spinner {
  background-image: linear-gradient(rgb(49, 200, 178) 35%, rgb(244, 183, 124));
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
                "single 72-frame Rust atlas at Uiverse-paced rotation; no browser, no CSS parser, no per-frame allocation"
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
            if radius > 1.32 {
                continue;
            }

            let angle = (ny.atan2(nx) + rotation).rem_euclid(2.0 * PI) / (2.0 * PI);
            let sweep = (1.0 - smoothstep(0.74, 1.0, angle))
                .max(smoothstep(0.0, 0.18, angle) * 0.18);
            let shimmer = 0.72 + 0.28 * ((angle * 2.0 * PI + rotation).sin() * 0.5 + 0.5);
            let glow_color = app_ring_color((angle + t * 0.58).fract());
            let glow_alpha = ((1.0 - smoothstep(0.78, 1.30, radius))
                * smoothstep(0.12, 0.62, radius)
                * sweep
                * shimmer
                * 104.0) as u8;
            if glow_alpha > 0 {
                blend_pixel(
                    &mut rgba,
                    x,
                    y,
                    [
                        glow_color[0] as u8,
                        glow_color[1] as u8,
                        glow_color[2] as u8,
                        glow_alpha,
                    ],
                );
            }

            let outer = 1.0 - smoothstep(0.95, 1.03, radius);
            let inner = smoothstep(0.55, 0.68, radius);
            let ring = outer * inner * sweep;
            if ring > 0.0 {
                let edge = 0.78 + 0.22 * ((angle * 6.0 * PI - rotation).cos() * 0.5 + 0.5);
                let alpha = (ring * edge * 242.0) as u8;
                if alpha > 0 {
                    blend_pixel(
                        &mut rgba,
                        x,
                        y,
                        [
                            glow_color[0] as u8,
                            glow_color[1] as u8,
                            glow_color[2] as u8,
                            alpha,
                        ],
                    );
                }
            }

            let center = 1.0 - smoothstep(0.53, 0.64, radius);
            if center > 0.0 {
                blend_pixel(
                    &mut rgba,
                    x,
                    y,
                    [30, 30, 29, (center * 248.0) as u8],
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

/// Real-time procedural brain core — rendered one frame at a time from continuous `time`
/// (seconds). No pre-baked atlas, so startup is instant and the motion never loops: lobe
/// orbits and hue drift on incommensurate frequencies, giving infinite non-repeating life.
pub fn render_brain_core_realtime(time: f32) -> slint::Image {
    let w = BRAIN_CORE_WIDTH;
    let h = BRAIN_CORE_HEIGHT;
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    // Slow, calm motion.
    let t = time * 0.16;
    // Continuous hue drift between amber (0) and burnt-orange (-90) — two mixed sines never repeat.
    let hue = -38.0 + 34.0 * (time * 0.05).sin() + 16.0 * (time * 0.021 + 1.7).sin();
    // Breathing — quasi-periodic so it never lands exactly the same.
    let breathe = 1.0 + 0.045 * (time * 0.12).sin() + 0.02 * (time * 0.047).sin();

    // Lobe seeds: (orbit_base, orbit_amp, orbit_freq, angle_phase, angle_speed, radius_base, radius_amp).
    // angle_speeds and freqs are mutually irrational -> the whole field is non-periodic.
    let lobes: [(f32, f32, f32, f32, f32, f32, f32); 10] = [
        (0.00, 0.00, 0.000, 0.00, 0.000, 0.50, 0.03),
        (0.30, 0.05, 0.073, 0.00, 0.131, 0.32, 0.03),
        (0.34, 0.04, 0.091, 2.10, -0.107, 0.28, 0.03),
        (0.40, 0.05, 0.057, 4.20, 0.083, 0.26, 0.02),
        (0.26, 0.04, 0.113, 1.00, -0.149, 0.30, 0.03),
        (0.44, 0.05, 0.067, 3.50, 0.127, 0.22, 0.02),
        // Fine detail lobes — crisp HD surface ripple.
        (0.50, 0.06, 0.181, 0.50, 0.233, 0.13, 0.02),
        (0.46, 0.05, 0.211, 2.80, -0.271, 0.12, 0.02),
        (0.54, 0.06, 0.157, 5.00, 0.197, 0.11, 0.02),
        (0.38, 0.05, 0.241, 3.90, -0.331, 0.14, 0.02),
    ];

    // Precompute lobe centers for this instant (cheap — 10 sin/cos pairs, not per pixel).
    let mut centers = [(0.0f32, 0.0f32, 0.0f32); 10];
    for (i, l) in lobes.iter().enumerate() {
        let (orbit_b, orbit_a, orbit_f, phase, speed, rad_b, rad_a) = *l;
        let orbit = orbit_b + orbit_a * (time * orbit_f * std::f32::consts::TAU).sin();
        let ang = phase + t * std::f32::consts::TAU * (speed * 6.2);
        let rad = rad_b + rad_a * (time * (orbit_f + 0.03) * std::f32::consts::TAU).sin();
        centers[i] = (orbit * ang.cos(), orbit * ang.sin(), rad);
    }

    for y in 0..h {
        for x in 0..w {
            let nx = ((x as f32 + 0.5) / w as f32) * 2.0 - 1.0;
            let ny = ((y as f32 + 0.5) / h as f32) * 2.0 - 1.0;
            let px = nx / breathe;
            let py = ny / breathe;
            if px * px + py * py > 1.35 {
                continue;
            }

            // Metaball field — sum of gaussian lobes at their current centers.
            let mut field = 0.0f32;
            for (cx, cy, r) in centers.iter() {
                let dx = px - cx;
                let dy = py - cy;
                let d2 = dx * dx + dy * dy;
                field += (-d2 / (r * r)).exp();
            }

            // Crisp metaball: tight edge band, no soft glow halo (no gradient into the black).
            let core = smoothstep(1.04, 1.12, field);
            let rim = (1.0 - (field - 1.10).abs() / 0.05).clamp(0.0, 1.0) * core;
            if core <= 0.003 {
                continue;
            }

            let grad = (((py + 1.0) * 0.5 - 0.3) / 0.4).clamp(0.0, 1.0);
            let mut base_rgb = mix_rgb([255.0, 191.0, 72.0], [190.0, 74.0, 29.0], grad);
            base_rgb = mix_rgb(base_rgb, [255.0, 222.0, 130.0], rim);
            let tinted = hue_rotate(base_rgb, hue);

            let alpha = (core * 235.0 + rim * 20.0).min(250.0) as u8;
            blend_brain_pixel(&mut rgba, x, y, [tinted[0] as u8, tinted[1] as u8, tinted[2] as u8, alpha]);
        }
    }

    let mut pixels = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(w, h);
    pixels.make_mut_bytes().copy_from_slice(&rgba);
    slint::Image::from_rgba8(pixels)
}

fn blend_brain_pixel(rgba: &mut [u8], x: u32, y: u32, src: [u8; 4]) {
    let idx = ((y * BRAIN_CORE_WIDTH + x) * 4) as usize;
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
        (0.16, [43.0, 217.0, 207.0]),
        (0.31, [88.0, 205.0, 190.0]),
        (0.46, [154.0, 210.0, 158.0]),
        (0.59, [225.0, 208.0, 132.0]),
        (0.73, [246.0, 185.0, 124.0]),
        (0.86, [242.0, 158.0, 112.0]),
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
