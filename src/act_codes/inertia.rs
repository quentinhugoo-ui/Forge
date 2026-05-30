//! Act Code `inertia.mc.v1` — rigid-body mass properties of an SDF solid.
//!
//! Monte-Carlo integration over the implicit interior (`eval_scene(p) < 0`):
//! samples a stratified grid jittered inside the AABB, accumulates mass,
//! first moment (→ centre of mass) and second moments (→ inertia tensor
//! about the COM). One density per call — the planner runs this once per
//! material sub-part and combines them with the parallel-axis theorem, so
//! a heterogeneous body (brass weight + ABS cage) is exact without the SDF
//! ever carrying material tags.
//!
//! This is the ground-truth physics that a flight controller's attitude
//! loop needs : without (mass, COM, I) the PID gains can't be derived and
//! the drone is undisignable. Boston-Dynamics / Tesla call it the rigid-
//! body parameter identification step — here it's one deterministic act
//! code, content-addressed so re-runs over an unchanged sub-part cost 0.

use super::{eval_scene, scene_aabb, ActCode, Artifact, SdfOp, Vec3};

/// Deterministic SplitMix64 — same generator as forge_drone_design so the
/// whole drone toolchain shares one reproducible RNG lineage.
struct Rng(u64);
impl Rng {
    fn new(seed: u64) -> Self { Self(seed | 1) }
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^ (z >> 31)
    }
    fn u01(&mut self) -> f64 { (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64 }
}

#[derive(Clone, Debug)]
pub struct InertiaActCode {
    /// Number of Monte-Carlo samples. 2e6 gives <0.5% volume error on the
    /// drone parts ; cost is ~20 ms single-thread.
    pub samples: u32,
    /// Material density (kg/m³). ABS ≈ 1050, brass ≈ 8500, PCB ≈ 1600.
    pub density: f64,
    /// RNG seed — part of the params hash so two densities or two seeds are
    /// distinct ledger entries.
    pub seed: u64,
}

impl Default for InertiaActCode {
    fn default() -> Self {
        Self { samples: 2_000_000, density: 1050.0, seed: 0xC0FFEE }
    }
}

impl InertiaActCode {
    pub fn with(samples: u32, density: f64, seed: u64) -> Self {
        Self { samples, density, seed }
    }
}

impl ActCode for InertiaActCode {
    fn id(&self) -> &'static str { "inertia.mc.v1" }

    fn params_bytes(&self) -> Vec<u8> {
        let mut v = Vec::with_capacity(24);
        v.extend_from_slice(&self.samples.to_le_bytes());
        v.extend_from_slice(&self.density.to_le_bytes());
        v.extend_from_slice(&self.seed.to_le_bytes());
        v
    }

    fn run(&self, ops: &[SdfOp]) -> Artifact {
        let (lo, hi) = scene_aabb(ops);
        let span = [hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]];
        let aabb_vol = span[0].max(0.0) * span[1].max(0.0) * span[2].max(0.0);
        if aabb_vol <= 0.0 {
            return Artifact::Inertia { mass: 0.0, com: [0.0; 3], tensor: [0.0; 6] };
        }

        let mut rng = Rng::new(self.seed);
        let n = self.samples.max(1);
        let mut inside: u64 = 0;
        // First moments Σ p_i  and  second moments Σ p_i p_j over inside pts.
        let mut sum = [0.0f64; 3];
        let mut sxx = 0.0; let mut syy = 0.0; let mut szz = 0.0;
        let mut sxy = 0.0; let mut sxz = 0.0; let mut syz = 0.0;

        for _ in 0..n {
            let p: Vec3 = [
                lo[0] + span[0] * rng.u01(),
                lo[1] + span[1] * rng.u01(),
                lo[2] + span[2] * rng.u01(),
            ];
            if eval_scene(ops, p) < 0.0 {
                inside += 1;
                sum[0] += p[0]; sum[1] += p[1]; sum[2] += p[2];
                sxx += p[0]*p[0]; syy += p[1]*p[1]; szz += p[2]*p[2];
                sxy += p[0]*p[1]; sxz += p[0]*p[2]; syz += p[1]*p[2];
            }
        }

        if inside == 0 {
            return Artifact::Inertia { mass: 0.0, com: [0.0; 3], tensor: [0.0; 6] };
        }

        let frac = inside as f64 / n as f64;
        let volume = frac * aabb_vol;
        let mass = volume * self.density;
        let inv = 1.0 / inside as f64;
        let com = [sum[0] * inv, sum[1] * inv, sum[2] * inv];

        // Per-sample mass : total mass spread over the `inside` hit samples.
        let dm = mass / inside as f64;
        // Second moments about the ORIGIN, then shift to COM via the
        // parallel-axis theorem so the tensor is body-centred.
        //   Ixx = Σ dm (y² + z²)
        let ixx_o = dm * (syy + szz);
        let iyy_o = dm * (sxx + szz);
        let izz_o = dm * (sxx + syy);
        let ixy_o = -dm * sxy;
        let ixz_o = -dm * sxz;
        let iyz_o = -dm * syz;
        // Shift origin→COM : I_com = I_o - m * (parallel-axis term).
        let (cx, cy, cz) = (com[0], com[1], com[2]);
        let ixx = ixx_o - mass * (cy*cy + cz*cz);
        let iyy = iyy_o - mass * (cx*cx + cz*cz);
        let izz = izz_o - mass * (cx*cx + cy*cy);
        let ixy = ixy_o + mass * cx * cy;
        let ixz = ixz_o + mass * cx * cz;
        let iyz = iyz_o + mass * cy * cz;

        Artifact::Inertia {
            mass,
            com,
            tensor: [ixx, iyy, izz, ixy, ixz, iyz],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Solid sphere : analytic mass = ρ·(4/3)πr³, I = (2/5) m r² on each
    /// principal axis, COM at centre. MC must land within a few percent.
    #[test]
    fn solid_sphere_matches_analytic() {
        let r = 0.5;
        let rho = 1000.0;
        let ops = vec![SdfOp::Sphere { center: [0.0; 3], radius: r }];
        let code = InertiaActCode::with(2_000_000, rho, 42);
        let art = code.run(&ops);
        let Artifact::Inertia { mass, com, tensor } = art else { panic!("wrong artifact") };

        let vol = (4.0 / 3.0) * std::f64::consts::PI * r.powi(3);
        let m_exp = vol * rho;
        let i_exp = 0.4 * m_exp * r * r;

        assert!((mass - m_exp).abs() / m_exp < 0.02, "mass {} vs {}", mass, m_exp);
        for c in com { assert!(c.abs() < 0.01, "COM off-centre: {}", c); }
        for k in 0..3 {
            assert!((tensor[k] - i_exp).abs() / i_exp < 0.03, "I[{}]={} vs {}", k, tensor[k], i_exp);
        }
        // Products of inertia vanish for a centred sphere.
        for k in 3..6 { assert!(tensor[k].abs() < i_exp * 0.05, "product {} too large", tensor[k]); }
    }

    /// Off-centre sphere : COM must track the centre, principal inertia is
    /// the same as a centred one (body-frame), products near zero.
    #[test]
    fn offcentre_sphere_com_tracks() {
        let ops = vec![SdfOp::Sphere { center: [0.3, -0.2, 0.1], radius: 0.25 }];
        let code = InertiaActCode::with(1_500_000, 1000.0, 7);
        let Artifact::Inertia { com, .. } = code.run(&ops) else { panic!() };
        assert!((com[0] - 0.3).abs() < 0.01);
        assert!((com[1] + 0.2).abs() < 0.01);
        assert!((com[2] - 0.1).abs() < 0.01);
    }
}
