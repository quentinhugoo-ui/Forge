//! Act Code `thermal.static.v1` — steady-state temperature field of an SDF
//! solid with internal heat sources and convective surface cooling.
//!
//! Finite-volume energy balance on the voxelised interior :
//!   - conduction across each face shared by two occupied cells :
//!       flux = k · h · (θ_j − θ_i)      (k = conductivity, h = pitch)
//!   - convective loss at each exposed face :
//!       loss = h_conv · h² · θ_i        (θ = T − T_ambient)
//!   - internal sources q_i (W) deposited at the nearest voxel.
//! Steady state ⇒  A θ = q   with  A = k·h·L + diag(n_exposed · h_conv · h²).
//! A is SPD (Laplacian PSD + strictly-positive convective diagonal), solved
//! by Conjugate Gradient — matvec-only, the GPU-friendly shape.
//!
//! Why it matters : the Raspberry Pi dumps ~3 W and each motor ~5 W into a
//! sealed ABS shell. If the steady-state hotspot exceeds the RPi's ~70 °C
//! throttle point the flight computer downclocks mid-flight. This act code
//! is the thermal half of the thermal×CFD coupling (the convection
//! coefficient h_conv is later fed by the downwash velocity from cfd_hover).

use super::voxel::Voxels;
use super::{ActCode, Artifact, SdfOp, Vec3};

/// A heat source : world position, power (W), and physical radius (m). Real
/// components are NOT points — a motor is ~2 cm, an RPi SoC ~1.5 cm — so the
/// power is spread over every voxel within `radius`. Depositing into a single
/// voxel instead produces an unphysical discretisation spike (the point-source
/// singularity of the diffusion equation).
#[derive(Clone, Debug)]
pub struct HeatSource {
    pub pos: Vec3,
    pub watts: f64,
    pub radius: f64,
}

#[derive(Clone, Debug)]
pub struct ThermalActCode {
    pub grid: u32,
    /// Thermal conductivity k (W/m·K). ABS ≈ 0.17, aluminium ≈ 205.
    pub conductivity: f64,
    /// Convective film coefficient h_conv (W/m²·K). Natural ≈ 10, forced ≈ 60.
    pub h_conv: f64,
    /// Ambient temperature (°C) — the field is solved as a rise above it.
    pub ambient_c: f64,
    /// Internal heat sources.
    pub sources: Vec<HeatSource>,
    /// CG iteration cap.
    pub cg_iters: u32,
}

impl Default for ThermalActCode {
    fn default() -> Self {
        Self {
            grid: 48,
            conductivity: 0.17,
            h_conv: 12.0,
            ambient_c: 25.0,
            sources: Vec::new(),
            cg_iters: 4000,
        }
    }
}

impl ThermalActCode {
    pub fn new(grid: u32, conductivity: f64, h_conv: f64, ambient_c: f64, sources: Vec<HeatSource>) -> Self {
        Self { grid, conductivity, h_conv, ambient_c, sources, cg_iters: 4000 }
    }
}

/// Number of exposed (non-occupied-neighbour) faces of each dof. A cell with
/// all 6 neighbours present is interior (0 exposed) ; surface cells lose heat.
fn exposed_faces(vox: &Voxels) -> Vec<f64> {
    let mut ex = vec![6.0f64; vox.ndof()];
    for d in 0..vox.ndof() {
        let mut deg = 0.0;
        vox.for_each_neighbour(d, |_| deg += 1.0);
        ex[d] = 6.0 - deg;
    }
    ex
}

impl ActCode for ThermalActCode {
    fn id(&self) -> &'static str { "thermal.static.v1" }

    fn params_bytes(&self) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&self.grid.to_le_bytes());
        v.extend_from_slice(&self.conductivity.to_le_bytes());
        v.extend_from_slice(&self.h_conv.to_le_bytes());
        v.extend_from_slice(&self.ambient_c.to_le_bytes());
        v.extend_from_slice(&self.cg_iters.to_le_bytes());
        v.extend_from_slice(&(self.sources.len() as u32).to_le_bytes());
        for s in &self.sources {
            for c in s.pos { v.extend_from_slice(&c.to_le_bytes()); }
            v.extend_from_slice(&s.watts.to_le_bytes());
            v.extend_from_slice(&s.radius.to_le_bytes());
        }
        v
    }

    fn run(&self, ops: &[SdfOp]) -> Artifact {
        let vox = Voxels::occupy(ops, self.grid.max(4));
        let n = vox.ndof();
        if n < 8 {
            return Artifact::Scalars { label: "thermal_C".into(), values: vec![] };
        }
        let ex = exposed_faces(&vox);
        let h = vox.h;
        let kh = self.conductivity * h;              // conduction edge weight
        let conv = self.h_conv * h * h;              // convection per exposed face

        // Right-hand side : spread each source's watts over every voxel
        // within its physical radius (finite-size component, not a point).
        // Falls back to the single nearest voxel if the radius captures none.
        let mut q = vec![0.0f64; n];
        for s in &self.sources {
            let r2 = (s.radius.max(h)).powi(2);
            let mut hits: Vec<usize> = Vec::new();
            let mut nearest = usize::MAX;
            let mut nearest_d2 = f64::INFINITY;
            for d in 0..n {
                let c = vox.center_of_dof(d);
                let d2 = (c[0]-s.pos[0]).powi(2) + (c[1]-s.pos[1]).powi(2) + (c[2]-s.pos[2]).powi(2);
                if d2 <= r2 { hits.push(d); }
                if d2 < nearest_d2 { nearest_d2 = d2; nearest = d; }
            }
            if hits.is_empty() && nearest != usize::MAX { hits.push(nearest); }
            let per = s.watts / hits.len().max(1) as f64;
            for d in hits { q[d] += per; }
        }

        // Operator A x = kh·(L x) + diag(ex·conv)·x.  SPD.
        let apply = |x: &[f64], y: &mut [f64], lbuf: &mut [f64]| {
            vox.laplacian_matvec(x, lbuf);
            for i in 0..n { y[i] = kh * lbuf[i] + ex[i] * conv * x[i]; }
        };

        // Conjugate Gradient : A θ = q.
        let mut theta = vec![0.0f64; n];
        let mut r = q.clone();              // r = q - A·0
        let mut p = r.clone();
        let mut ax = vec![0.0f64; n];
        let mut lbuf = vec![0.0f64; n];
        let mut rs_old = dot(&r, &r);
        let tol = 1e-10 * rs_old.max(1e-30);
        for _ in 0..self.cg_iters {
            apply(&p, &mut ax, &mut lbuf);
            let denom = dot(&p, &ax).max(1e-300);
            let alpha = rs_old / denom;
            for i in 0..n { theta[i] += alpha * p[i]; ax_sub(&mut r, i, alpha, &ax); }
            let rs_new = dot(&r, &r);
            if rs_new < tol { break; }
            let beta = rs_new / rs_old;
            for i in 0..n { p[i] = r[i] + beta * p[i]; }
            rs_old = rs_new;
        }

        // Reduce to (T_max, hotspot.xyz, T_mean).
        let mut t_max = f64::NEG_INFINITY;
        let mut hotspot = [0.0; 3];
        let mut sum = 0.0;
        for d in 0..n {
            let t = self.ambient_c + theta[d];
            sum += t;
            if t > t_max { t_max = t; hotspot = vox.center_of_dof(d); }
        }
        let t_mean = sum / n as f64;

        Artifact::Scalars {
            label: "thermal_C".into(),
            values: vec![t_max, hotspot[0], hotspot[1], hotspot[2], t_mean],
        }
    }
}

fn dot(a: &[f64], b: &[f64]) -> f64 { a.iter().zip(b).map(|(x, y)| x * y).sum() }
#[inline]
fn ax_sub(r: &mut [f64], i: usize, alpha: f64, ax: &[f64]) { r[i] -= alpha * ax[i]; }

#[cfg(test)]
mod tests {
    use super::*;

    fn cube(side: f64) -> Vec<SdfOp> {
        vec![SdfOp::Box { center: [0.0; 3], half_extents: [side * 0.5; 3] }]
    }

    fn run_max(code: &ThermalActCode, ops: &[SdfOp]) -> f64 {
        let Artifact::Scalars { values, .. } = code.run(ops) else { panic!() };
        values[0]
    }

    /// A heated cube : steady-state peak is above ambient, finite, and the
    /// hotspot sits near the source. Doubling the power doubles the rise.
    #[test]
    fn heated_cube_peak_scales_with_power() {
        let ops = cube(0.1);
        let amb = 25.0;
        let mk = |w: f64| ThermalActCode::new(
            32, 0.17, 12.0, amb,
            vec![HeatSource { pos: [0.0, 0.0, 0.0], watts: w, radius: 0.02 }],
        );
        let t1 = run_max(&mk(3.0), &ops);
        let t2 = run_max(&mk(6.0), &ops);
        assert!(t1 > amb, "peak {} must exceed ambient {}", t1, amb);
        let rise1 = t1 - amb;
        let rise2 = t2 - amb;
        assert!((rise2 / rise1 - 2.0).abs() < 0.05, "rise should be linear in power: {} vs {}", rise1, rise2);
    }

    /// Higher conductivity spreads heat → lower peak for the same source.
    #[test]
    fn higher_conductivity_lowers_peak() {
        let ops = cube(0.1);
        let mk = |k: f64| ThermalActCode::new(
            32, k, 12.0, 25.0,
            vec![HeatSource { pos: [0.0; 3], watts: 4.0, radius: 0.02 }],
        );
        let low_k = run_max(&mk(0.17), &ops);
        let high_k = run_max(&mk(2.0), &ops);
        assert!(high_k < low_k, "k=2 peak {} should be below k=0.17 peak {}", high_k, low_k);
    }

    /// Deterministic + hotspot tracks an off-centre source.
    #[test]
    fn hotspot_tracks_source() {
        let ops = cube(0.16);
        let code = ThermalActCode::new(
            36, 0.17, 12.0, 25.0,
            vec![HeatSource { pos: [0.05, 0.0, 0.0], watts: 5.0, radius: 0.025 }],
        );
        let a = code.run(&ops);
        let b = code.run(&ops);
        assert_eq!(a, b, "deterministic");
        let Artifact::Scalars { values, .. } = a else { panic!() };
        assert!(values[1] > 0.0, "hotspot x {} should be on the +x source side", values[1]);
    }
}
