//! Shared voxelisation of an SDF interior + the implicit graph Laplacian.
//!
//! Extracted so every field act code (modal, thermal, stress…) walks the
//! same occupancy grid and the same sparse operator — one definition, no
//! per-act-code duplication. The Laplacian is computed on the fly from the
//! occupancy (no stored matrix), which is exactly the matvec-only shape the
//! doctrine wants for massively-iterative GPU dispatch later.

use super::{eval_scene, scene_aabb, SdfOp};

/// Voxelised occupancy + the index map needed for the sparse Laplacian.
pub struct Voxels {
    pub nx: usize,
    pub ny: usize,
    pub nz: usize,
    /// cell-linear-index → dof-index (or usize::MAX if empty).
    dof_of_cell: Vec<usize>,
    /// dof-index → (ix, iy, iz).
    pub cell_of_dof: Vec<(usize, usize, usize)>,
    /// lower AABB corner (m).
    pub lo: [f64; 3],
    /// voxel pitch (m).
    pub h: f64,
}

impl Voxels {
    /// Sample the SDF on a centred grid sized to the AABB, marking cells
    /// whose centre is interior (`eval_scene < 0`) as degrees of freedom.
    pub fn occupy(ops: &[SdfOp], grid: u32) -> Self {
        let (lo, hi) = scene_aabb(ops);
        let span = [hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]];
        let longest = span[0].max(span[1]).max(span[2]).max(1e-6);
        let h = longest / grid as f64;
        let nx = ((span[0] / h).ceil() as usize + 1).max(1);
        let ny = ((span[1] / h).ceil() as usize + 1).max(1);
        let nz = ((span[2] / h).ceil() as usize + 1).max(1);

        let mut dof_of_cell = vec![usize::MAX; nx * ny * nz];
        let mut cell_of_dof = Vec::new();
        for iz in 0..nz {
            for iy in 0..ny {
                for ix in 0..nx {
                    let p = [
                        lo[0] + (ix as f64 + 0.5) * h,
                        lo[1] + (iy as f64 + 0.5) * h,
                        lo[2] + (iz as f64 + 0.5) * h,
                    ];
                    if eval_scene(ops, p) < 0.0 {
                        let lin = (iz * ny + iy) * nx + ix;
                        dof_of_cell[lin] = cell_of_dof.len();
                        cell_of_dof.push((ix, iy, iz));
                    }
                }
            }
        }
        Self { nx, ny, nz, dof_of_cell, cell_of_dof, lo, h }
    }

    pub fn ndof(&self) -> usize { self.cell_of_dof.len() }

    /// World-space centre of a dof's voxel.
    pub fn center_of_dof(&self, d: usize) -> [f64; 3] {
        let (ix, iy, iz) = self.cell_of_dof[d];
        [
            self.lo[0] + (ix as f64 + 0.5) * self.h,
            self.lo[1] + (iy as f64 + 0.5) * self.h,
            self.lo[2] + (iz as f64 + 0.5) * self.h,
        ]
    }

    #[inline]
    pub fn dof_at(&self, ix: usize, iy: usize, iz: usize) -> Option<usize> {
        let lin = (iz * self.ny + iy) * self.nx + ix;
        let d = self.dof_of_cell[lin];
        if d == usize::MAX { None } else { Some(d) }
    }

    /// Visit each occupied neighbour of dof `d` with its dof index.
    #[inline]
    pub fn for_each_neighbour(&self, d: usize, mut f: impl FnMut(usize)) {
        let (ix, iy, iz) = self.cell_of_dof[d];
        if ix > 0 { if let Some(n) = self.dof_at(ix - 1, iy, iz) { f(n); } }
        if ix + 1 < self.nx { if let Some(n) = self.dof_at(ix + 1, iy, iz) { f(n); } }
        if iy > 0 { if let Some(n) = self.dof_at(ix, iy - 1, iz) { f(n); } }
        if iy + 1 < self.ny { if let Some(n) = self.dof_at(ix, iy + 1, iz) { f(n); } }
        if iz > 0 { if let Some(n) = self.dof_at(ix, iy, iz - 1) { f(n); } }
        if iz + 1 < self.nz { if let Some(n) = self.dof_at(ix, iy, iz + 1) { f(n); } }
    }

    /// y = L x, graph Laplacian with Neumann (free) boundary : diagonal =
    /// occupied-neighbour count, off-diagonal = −1 per edge. Implicit.
    pub fn laplacian_matvec(&self, x: &[f64], y: &mut [f64]) {
        for d in 0..self.cell_of_dof.len() {
            let mut deg = 0.0;
            let mut acc = 0.0;
            self.for_each_neighbour(d, |n| { deg += 1.0; acc += x[n]; });
            y[d] = deg * x[d] - acc;
        }
    }
}
