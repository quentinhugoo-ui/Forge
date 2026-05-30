//! Act Code `modal.helmholtz.v1` — vibration spectrum of an SDF solid.
//!
//! Voxelises the implicit interior, builds the scalar Laplacian on the
//! occupied cells (7-point stencil, Neumann/free boundary), and extracts
//! the k lowest non-trivial eigenvalues by Lanczos iteration on the
//! spectrum-shifted operator `M = shift·I − L` (largest eigenvalues of M
//! = smallest of L — Lanczos converges fast to extremes, and we never
//! need a linear solve, only sparse mat-vecs).
//!
//! Eigenvalues are calibrated to Hz through the wave equation
//! `f = (c / 2π)·√(λ_cont)`, with `λ_cont ≈ λ_graph / h²` for voxel pitch
//! `h` and material wave speed `c = √(E/ρ)`. This is the acoustic/membrane
//! spectrum of the domain — a first-order proxy for the structural modes
//! (exact for a uniform-thickness shell's breathing modes, which is the
//! relevant family for the drone-cage flutter question). A full elastic
//! FEM modal (`modal.elastic.v1`, 3 DOF/node) is a later act code ; this
//! one is honest about being the scalar Helmholtz spectrum.
//!
//! Why it matters : the lowest cage modes must avoid the propeller
//! blade-pass frequency (rotor RPM × blade count). A mode sitting on the
//! blade-pass band turns the cage into a resonator and the drone shakes
//! itself apart. `modal` is the act code that surfaces that collision.

use super::voxel::Voxels;
use super::{ActCode, Artifact, SdfOp};

#[derive(Clone, Debug)]
pub struct ModalActCode {
    /// Voxels per longest AABB axis. 48–96 is the sweet spot : coarse
    /// enough for an interactive run, fine enough that the lowest handful
    /// of modes are within ~10 % of the converged value.
    pub grid: u32,
    /// Number of lowest non-trivial modes to return.
    pub modes: u32,
    /// Material wave speed c = √(E/ρ) in m/s. ABS ≈ √(2.3e9/1050) ≈ 1480.
    pub wave_speed: f64,
    /// Lanczos iterations. ~4× modes + 24 gives clean convergence with
    /// full reorthogonalisation.
    pub lanczos_steps: u32,
}

impl Default for ModalActCode {
    fn default() -> Self {
        Self { grid: 64, modes: 8, wave_speed: 1480.0, lanczos_steps: 60 }
    }
}

impl ModalActCode {
    pub fn with(grid: u32, modes: u32, wave_speed: f64, lanczos_steps: u32) -> Self {
        Self { grid, modes, wave_speed, lanczos_steps }
    }
}

// ---- linear-algebra helpers ------------------------------------------------

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}
fn axpy(alpha: f64, x: &[f64], y: &mut [f64]) {
    for i in 0..y.len() { y[i] += alpha * x[i]; }
}
fn scale(x: &mut [f64], s: f64) {
    for v in x.iter_mut() { *v *= s; }
}

/// Eigenvalues of a symmetric tridiagonal matrix (diagonal `d`, sub-diagonal
/// `e` with `e[i]` between rows i-1 and i, `e[0]` unused) by cyclic Jacobi
/// rotations on the dense k×k matrix. k ≤ lanczos_steps (≤ 60), so the
/// O(k³·sweeps) cost is negligible and the method is unconditionally stable
/// and index-bug-free — preferred over a hand QL port.
fn tridiag_eigenvalues(d: Vec<f64>, e: Vec<f64>) -> Vec<f64> {
    let n = d.len();
    if n == 0 { return d; }
    if n == 1 { return d; }
    // Dense symmetric matrix from the tridiagonal.
    let mut a = vec![0.0f64; n * n];
    for i in 0..n { a[i * n + i] = d[i]; }
    for i in 1..n {
        a[i * n + (i - 1)] = e[i];
        a[(i - 1) * n + i] = e[i];
    }
    // Cyclic Jacobi.
    for _sweep in 0..100 {
        // Off-diagonal Frobenius norm.
        let mut off = 0.0;
        for p in 0..n {
            for q in (p + 1)..n {
                off += a[p * n + q] * a[p * n + q];
            }
        }
        if off.sqrt() < 1e-14 { break; }
        for p in 0..n {
            for q in (p + 1)..n {
                let apq = a[p * n + q];
                if apq.abs() < 1e-300 { continue; }
                let app = a[p * n + p];
                let aqq = a[q * n + q];
                let phi = 0.5 * (aqq - app).atan2(2.0 * apq);
                let (s, c) = phi.sin_cos();
                // Rotate rows/cols p,q.
                for k in 0..n {
                    let akp = a[k * n + p];
                    let akq = a[k * n + q];
                    a[k * n + p] = c * akp - s * akq;
                    a[k * n + q] = s * akp + c * akq;
                }
                for k in 0..n {
                    let apk = a[p * n + k];
                    let aqk = a[q * n + k];
                    a[p * n + k] = c * apk - s * aqk;
                    a[q * n + k] = s * apk + c * aqk;
                }
            }
        }
    }
    let mut out: Vec<f64> = (0..n).map(|i| a[i * n + i]).collect();
    out.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    out
}

/// Lanczos on `M = shift·I − L` with full reorthogonalisation. Returns the
/// `want` largest eigenvalues of M (→ smallest of L after un-shifting).
fn lanczos_top_eigs(vox: &Voxels, shift: f64, steps: usize, want: usize, seed: u64) -> Vec<f64> {
    let n = vox.ndof();
    if n == 0 { return vec![]; }
    let m = steps.min(n).max(1);

    // Deterministic start vector (SplitMix64), normalised.
    let mut s = seed | 1;
    let mut next = || {
        s = s.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = s;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        ((z ^ (z >> 31)) >> 11) as f64 / (1u64 << 53) as f64 - 0.5
    };
    let mut q_prev = vec![0.0f64; n];
    let mut q = (0..n).map(|_| next()).collect::<Vec<_>>();
    let nrm = dot(&q, &q).sqrt();
    scale(&mut q, 1.0 / nrm);

    let mut basis: Vec<Vec<f64>> = Vec::with_capacity(m);
    let mut alpha = Vec::with_capacity(m);
    let mut beta = Vec::with_capacity(m);
    let mut lx = vec![0.0f64; n];
    let mut w = vec![0.0f64; n];
    let mut beta_prev = 0.0;

    for _ in 0..m {
        basis.push(q.clone());
        // w = M q = shift·q − L q
        vox.laplacian_matvec(&q, &mut lx);
        for i in 0..n { w[i] = shift * q[i] - lx[i]; }
        let a = dot(&q, &w);
        alpha.push(a);
        // w -= a q + beta_prev q_prev
        axpy(-a, &q, &mut w);
        axpy(-beta_prev, &q_prev, &mut w);
        // Full reorthogonalisation against the whole basis (m small).
        for v in &basis {
            let proj = dot(&w, v);
            axpy(-proj, v, &mut w);
        }
        let b = dot(&w, &w).sqrt();
        if b < 1e-12 { break; }
        beta.push(b);
        beta_prev = b;
        q_prev = q;
        q = w.clone();
        scale(&mut q, 1.0 / b);
    }

    // Tridiagonal (alpha, beta) eigenvalues → eigenvalues of M.
    let k = alpha.len();
    let mut e = vec![0.0f64; k];
    for i in 1..k { e[i] = beta[i - 1]; }
    let theta = tridiag_eigenvalues(alpha, e); // ascending
    // Largest `want` of M.
    theta.iter().rev().take(want).cloned().collect()
}

impl ActCode for ModalActCode {
    fn id(&self) -> &'static str { "modal.helmholtz.v1" }

    fn params_bytes(&self) -> Vec<u8> {
        let mut v = Vec::with_capacity(24);
        v.extend_from_slice(&self.grid.to_le_bytes());
        v.extend_from_slice(&self.modes.to_le_bytes());
        v.extend_from_slice(&self.wave_speed.to_le_bytes());
        v.extend_from_slice(&self.lanczos_steps.to_le_bytes());
        v
    }

    fn run(&self, ops: &[SdfOp]) -> Artifact {
        let vox = Voxels::occupy(ops, self.grid.max(4));
        if vox.ndof() < 8 {
            return Artifact::Scalars { label: "modal_hz".into(), values: vec![] };
        }
        // Spectral upper bound for the graph Laplacian : 2·max_degree ≤ 12.
        let shift = 12.0;
        // Want a few extra to drop the (near-)zero constant mode cleanly.
        let want = (self.modes as usize) + 2;
        let m_eigs = lanczos_top_eigs(&vox, shift, self.lanczos_steps as usize, want, 0x4D_4F_44_41_u64);
        // λ_graph = shift − θ. Discard the near-zero constant mode(s).
        let mut lam: Vec<f64> = m_eigs.iter().map(|t| (shift - t).max(0.0)).collect();
        lam.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let lam: Vec<f64> = lam.into_iter().filter(|&l| l > 1e-6).collect();

        // Continuous eigenvalue λ_cont = λ_graph / h². Frequency in Hz :
        // f = (c / 2π) · √(λ_cont).
        let inv_h2 = 1.0 / (vox.h * vox.h);
        let coef = self.wave_speed / (2.0 * std::f64::consts::PI);
        let freqs: Vec<f64> = lam
            .iter()
            .take(self.modes as usize)
            .map(|&l| coef * (l * inv_h2).sqrt())
            .collect();

        Artifact::Scalars { label: "modal_hz".into(), values: freqs }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cube(side: f64) -> Vec<SdfOp> {
        vec![SdfOp::Box { center: [0.0; 3], half_extents: [side * 0.5; 3] }]
    }

    fn first_mode(ops: &[SdfOp], grid: u32, c: f64) -> f64 {
        let code = ModalActCode::with(grid, 4, c, 50);
        let Artifact::Scalars { values, .. } = code.run(ops) else { panic!() };
        assert!(!values.is_empty(), "expected at least one mode");
        values[0]
    }

    /// Frequency scales linearly with the material wave speed — an exact
    /// property of the wave equation, independent of discretisation.
    #[test]
    fn freq_scales_linearly_with_wave_speed() {
        let ops = cube(0.1);
        let f1 = first_mode(&ops, 32, 1000.0);
        let f2 = first_mode(&ops, 32, 2000.0);
        assert!((f2 / f1 - 2.0).abs() < 0.05, "f2/f1 = {} (expected ~2)", f2 / f1);
    }

    /// A bigger object has lower modes : f ∝ 1/size (same exact scaling).
    #[test]
    fn freq_scales_inversely_with_size() {
        let small = first_mode(&cube(0.1), 32, 1500.0);
        let big = first_mode(&cube(0.2), 32, 1500.0);
        assert!((small / big - 2.0).abs() < 0.10, "small/big = {} (expected ~2)", small / big);
    }

    /// Deterministic : same input → identical spectrum.
    #[test]
    fn modal_is_deterministic() {
        let ops = cube(0.12);
        let code = ModalActCode::with(40, 6, 1480.0, 56);
        let a = code.run(&ops);
        let b = code.run(&ops);
        assert_eq!(a, b);
    }

    /// Modes are sorted ascending and strictly positive (zero mode dropped).
    #[test]
    fn modes_sorted_and_positive() {
        let code = ModalActCode::with(40, 6, 1480.0, 56);
        let Artifact::Scalars { values, .. } = code.run(&cube(0.1)) else { panic!() };
        assert!(values.len() >= 3);
        for w in values.windows(2) { assert!(w[1] >= w[0] - 1e-6, "not sorted"); }
        assert!(values[0] > 0.0, "first mode must be positive");
    }
}
