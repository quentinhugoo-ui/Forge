//! Mesh → SDF bridge — INGEN COMPUTE §11 ("Le Pont : Conversion Mesh
//! vers SDF").
//!
//! Parses a binary STL, voxelizes it into a signed-distance grid, and
//! returns the voxel buffer + padded bounds to the UI for upload as a
//! 3D texture. The fragment shader samples it via opcode 20
//! (`OP_SAMPLED_SDF`) inside the same stack machine that already drives
//! sphere/box/torus/etc — keeping a single render pass and a single
//! source of truth for the SDF semantics.
//!
//! Algorithm : for each voxel, brute-force min(point_to_triangle_dist)
//! across all triangles ; inside/outside via parity of +X ray crossings
//! (Möller-Trumbore). O(V × T) — fine for V ≤ 128³ and T ≤ ~50k.

use base64::Engine as _;
use serde::{Deserialize, Serialize};

type Vec3 = [f32; 3];
type Tri = [Vec3; 3];

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoxelizeMeshRequest {
    pub bytes_b64: String,
    pub grid_size: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoxelizeMeshResponse {
    pub ok: bool,
    pub error: Option<String>,
    pub grid_size: u32,
    pub voxels_b64: String,
    pub bounds_min: Vec3,
    pub bounds_max: Vec3,
    pub triangle_count: usize,
    pub elapsed_ms: u64,
}

fn read_v3(b: &[u8]) -> Vec3 {
    [
        f32::from_le_bytes([b[0], b[1], b[2], b[3]]),
        f32::from_le_bytes([b[4], b[5], b[6], b[7]]),
        f32::from_le_bytes([b[8], b[9], b[10], b[11]]),
    ]
}

fn parse_stl_binary(bytes: &[u8]) -> Result<Vec<Tri>, String> {
    if bytes.len() < 84 {
        return Err("STL too short (need 84-byte header)".into());
    }
    let count = u32::from_le_bytes([bytes[80], bytes[81], bytes[82], bytes[83]]) as usize;
    let expected = 84 + count * 50;
    if bytes.len() < expected {
        return Err(format!("STL truncated: have {} bytes, need {}", bytes.len(), expected));
    }
    let mut tris = Vec::with_capacity(count);
    let mut off = 84;
    for _ in 0..count {
        off += 12; // skip face normal — recompute on demand
        let a = read_v3(&bytes[off..]); off += 12;
        let b = read_v3(&bytes[off..]); off += 12;
        let c = read_v3(&bytes[off..]); off += 12;
        off += 2; // attribute byte count (always 0 for binary STL)
        tris.push([a, b, c]);
    }
    Ok(tris)
}

fn bounds_of(tris: &[Tri]) -> (Vec3, Vec3) {
    let mut mn = [f32::INFINITY; 3];
    let mut mx = [f32::NEG_INFINITY; 3];
    for t in tris {
        for v in t {
            for i in 0..3 {
                if v[i] < mn[i] { mn[i] = v[i]; }
                if v[i] > mx[i] { mx[i] = v[i]; }
            }
        }
    }
    (mn, mx)
}

#[inline(always)] fn sub(a: Vec3, b: Vec3) -> Vec3 { [a[0]-b[0], a[1]-b[1], a[2]-b[2]] }
#[inline(always)] fn add(a: Vec3, b: Vec3) -> Vec3 { [a[0]+b[0], a[1]+b[1], a[2]+b[2]] }
#[inline(always)] fn scl(a: Vec3, s: f32) -> Vec3 { [a[0]*s, a[1]*s, a[2]*s] }
#[inline(always)] fn dot(a: Vec3, b: Vec3) -> f32 { a[0]*b[0] + a[1]*b[1] + a[2]*b[2] }
#[inline(always)] fn cross(a: Vec3, b: Vec3) -> Vec3 { [a[1]*b[2]-a[2]*b[1], a[2]*b[0]-a[0]*b[2], a[0]*b[1]-a[1]*b[0]] }

/// Squared distance from point `p` to triangle `t`.
/// Implementation from Real-Time Collision Detection, ch 5.1.5.
fn point_tri_dist_sq(p: Vec3, t: &Tri) -> f32 {
    let ab = sub(t[1], t[0]);
    let ac = sub(t[2], t[0]);
    let ap = sub(p, t[0]);
    let d1 = dot(ab, ap);
    let d2 = dot(ac, ap);
    if d1 <= 0.0 && d2 <= 0.0 { return dot(ap, ap); }

    let bp = sub(p, t[1]);
    let d3 = dot(ab, bp);
    let d4 = dot(ac, bp);
    if d3 >= 0.0 && d4 <= d3 { return dot(bp, bp); }

    let cp = sub(p, t[2]);
    let d5 = dot(ab, cp);
    let d6 = dot(ac, cp);
    if d6 >= 0.0 && d5 <= d6 { return dot(cp, cp); }

    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        let v = d1 / (d1 - d3);
        let q = add(t[0], scl(ab, v));
        return dot(sub(p, q), sub(p, q));
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        let w = d2 / (d2 - d6);
        let q = add(t[0], scl(ac, w));
        return dot(sub(p, q), sub(p, q));
    }
    let va = d3 * d6 - d5 * d4;
    let ed1 = d4 - d3;
    let ed2 = d5 - d6;
    if va <= 0.0 && ed1 >= 0.0 && ed2 >= 0.0 {
        let w = ed1 / (ed1 + ed2);
        let q = add(t[1], scl(sub(t[2], t[1]), w));
        return dot(sub(p, q), sub(p, q));
    }

    let denom = 1.0 / (va + vb + vc);
    let v = vb * denom;
    let w = vc * denom;
    let q = add(t[0], add(scl(ab, v), scl(ac, w)));
    dot(sub(p, q), sub(p, q))
}

/// Möller–Trumbore intersection : does the +X ray from `origin` hit `tri` at t > 0 ?
fn ray_hit_tri_x(origin: Vec3, tri: &Tri) -> bool {
    let dir = [1.0_f32, 0.0, 0.0];
    let e1 = sub(tri[1], tri[0]);
    let e2 = sub(tri[2], tri[0]);
    let h = cross(dir, e2);
    let a = dot(e1, h);
    if a.abs() < 1e-7 { return false; }
    let f = 1.0 / a;
    let s = sub(origin, tri[0]);
    let u = f * dot(s, h);
    if u < 0.0 || u > 1.0 { return false; }
    let q = cross(s, e1);
    let v = f * dot(dir, q);
    if v < 0.0 || u + v > 1.0 { return false; }
    let t = f * dot(e2, q);
    t > 1e-6
}

fn is_inside(p: Vec3, tris: &[Tri]) -> bool {
    let mut crossings = 0u32;
    for t in tris {
        if ray_hit_tri_x(p, t) { crossings += 1; }
    }
    crossings & 1 == 1
}

fn voxelize(tris: &[Tri], grid: u32, bounds_min: Vec3, bounds_max: Vec3) -> Vec<f32> {
    let n = grid as usize;
    let span = [
        bounds_max[0] - bounds_min[0],
        bounds_max[1] - bounds_min[1],
        bounds_max[2] - bounds_min[2],
    ];
    let inv_n = 1.0 / n as f32;
    let mut voxels = vec![0.0_f32; n * n * n];
    // Note : iteration order matches the WebGL2 sampler3D layout
    // (innermost = X = u, then Y = v, then Z = w).
    for k in 0..n {
        let pz = bounds_min[2] + span[2] * (k as f32 + 0.5) * inv_n;
        for j in 0..n {
            let py = bounds_min[1] + span[1] * (j as f32 + 0.5) * inv_n;
            for i in 0..n {
                let px = bounds_min[0] + span[0] * (i as f32 + 0.5) * inv_n;
                let p = [px, py, pz];
                let mut min_sq = f32::INFINITY;
                for tri in tris {
                    let d = point_tri_dist_sq(p, tri);
                    if d < min_sq { min_sq = d; }
                }
                let dist = min_sq.sqrt();
                let sign = if is_inside(p, tris) { -1.0 } else { 1.0 };
                voxels[k * n * n + j * n + i] = sign * dist;
            }
        }
    }
    voxels
}

#[tauri::command]
pub fn banger_voxelize_mesh(
    request: VoxelizeMeshRequest,
) -> Result<VoxelizeMeshResponse, String> {
    let started = std::time::Instant::now();
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(request.bytes_b64.as_bytes())
        .map_err(|e| format!("base64 decode failed: {e}"))?;
    let grid = request.grid_size.clamp(16, 128);
    let tris = parse_stl_binary(&bytes)?;
    if tris.is_empty() {
        return Ok(VoxelizeMeshResponse {
            ok: false,
            error: Some("STL has zero triangles".into()),
            grid_size: grid,
            voxels_b64: String::new(),
            bounds_min: [0.0; 3],
            bounds_max: [0.0; 3],
            triangle_count: 0,
            elapsed_ms: started.elapsed().as_millis() as u64,
        });
    }
    let (mut bmin, mut bmax) = bounds_of(&tris);
    // Pad bounds by 5 % of each axis span so the voxel grid leaves room
    // for trilinear normal interpolation at the boundary.
    for i in 0..3 {
        let pad = (bmax[i] - bmin[i]) * 0.05;
        bmin[i] -= pad;
        bmax[i] += pad;
    }
    let voxels = voxelize(&tris, grid, bmin, bmax);
    let mut voxel_bytes = Vec::with_capacity(voxels.len() * 4);
    for v in &voxels {
        voxel_bytes.extend_from_slice(&v.to_le_bytes());
    }
    let voxels_b64 = base64::engine::general_purpose::STANDARD.encode(&voxel_bytes);
    Ok(VoxelizeMeshResponse {
        ok: true,
        error: None,
        grid_size: grid,
        voxels_b64,
        bounds_min: bmin,
        bounds_max: bmax,
        triangle_count: tris.len(),
        elapsed_ms: started.elapsed().as_millis() as u64,
    })
}
