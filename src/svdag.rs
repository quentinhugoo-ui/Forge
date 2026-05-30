//! Sparse Voxel DAG — INGEN COMPUTE §18 Pillar A, Phase 5.
//!
//! Bottom-up octree DAG construction with content-addressed dedup.
//! Identical subtrees collapse to a single pool index → a "forest" of
//! 10 000 copies of the same tree stores ONE tree. This is the KASM
//! content-addressing applied to geometry : `hash(node_children)` is
//! the node identity.
//!
//! Pool layout (flat `Vec<u32>` for GPU consumption) — fixed 9-word
//! nodes so the WGSL traverser resolves a pool index by simple
//! arithmetic instead of a separate offset table :
//!   header[0..4] = root, dim, depth, _pad
//!   then per node : 9 u32 starting at offset 4 + (idx - 2) * 9
//!   word[0] = childmask (low 8 bits)
//!   word[1..=8] = child indices, one per octant (0 = SVDAG_EMPTY)
//!
//! Traversal (CPU and WGSL) starts at the root index, descends one
//! octree level per step. At each level, the point's octant in [0,1]³
//! determines which bit of `childmask` matters ; if 0, the subtree is
//! empty ; if 1, look up the corresponding child index. If a child
//! index is 1 (FULL_LEAF), the voxel at that depth is occupied.

use std::collections::HashMap;
use std::hash::Hasher;

use crate::fast_hash::FastBuildHasher;

/// Sentinel : refers to an entirely empty subtree (no occupancy).
pub const SVDAG_EMPTY: u32 = 0;

/// Sentinel : refers to a fully occupied leaf voxel.
pub const SVDAG_FULL: u32 = 1;

/// Maximum supported depth (octree levels) = log2(2 048) — large enough
/// for a 2 048³ ≈ 8.6×10⁹ voxel scene without losing precision.
pub const SVDAG_MAX_DEPTH: u32 = 11;

/// Bytes per node in the packed pool : 1 childmask + 8 child indices.
pub const SVDAG_WORDS_PER_NODE: usize = 9;
/// Word offset of the first node body (after the 4-word header).
pub const SVDAG_HEADER_WORDS: usize = 4;

#[derive(Clone, Debug, Default)]
pub struct Svdag {
    /// Flat pool, packed for GPU upload : header + node bodies.
    /// header[0] = root, header[1] = dim (voxel resolution per axis),
    /// header[2] = depth, header[3] = 0 (reserved).
    /// Node bodies start at offset 4.
    pub packed: Vec<u32>,
    /// Root pool index (within `nodes`, not the packed offset).
    pub root: u32,
    /// Dimensions of the voxel grid this SVDAG was built from (per axis).
    pub dim: u32,
    /// Octree depth = ceil(log2(dim)).
    pub depth: u32,
}

impl Svdag {
    /// Build a deduplicated SVDAG from a cubic boolean occupancy grid.
    /// `occupancy` is row-major in Z, Y, X (index = z*dim² + y*dim + x).
    /// Panics if `dim` is not a power of two or `occupancy.len() != dim³`.
    pub fn from_occupancy(occupancy: &[bool], dim: usize) -> Self {
        assert!(dim.is_power_of_two(), "svdag: dim must be a power of two");
        let dim_u = dim as u32;
        let total = dim * dim * dim;
        assert_eq!(occupancy.len(), total, "svdag: occupancy.len() != dim³");
        let depth = (dim_u.trailing_zeros()) as u32;
        assert!(depth <= SVDAG_MAX_DEPTH, "svdag: depth > SVDAG_MAX_DEPTH");

        // Build bottom-up. At each level we have a virtual cube of size
        // dim/(2^level) per axis ; the value at each cell is a pool index.
        // Level 0 cells = individual voxels. We dedup level-by-level.

        let mut current: Vec<u32> = occupancy
            .iter()
            .map(|&b| if b { SVDAG_FULL } else { SVDAG_EMPTY })
            .collect();
        let mut cur_dim = dim;

        // Pool stores fixed 9-word nodes. Indices 0 and 1 are sentinels
        // and never appear in `bodies`.
        let mut bodies: Vec<SvdagBody> = Vec::new();
        let mut interner: HashMap<u64, u32, FastBuildHasher> =
            HashMap::with_hasher(FastBuildHasher);

        while cur_dim > 1 {
            let next_dim = cur_dim / 2;
            let mut next = vec![SVDAG_EMPTY; next_dim * next_dim * next_dim];

            for z in 0..next_dim {
                for y in 0..next_dim {
                    for x in 0..next_dim {
                        let mut childmask: u32 = 0;
                        let mut child_indices: [u32; 8] = [SVDAG_EMPTY; 8];
                        // Octant ordering : bit 0 = -x-y-z, bit 7 = +x+y+z.
                        for oct in 0..8 {
                            let cx = 2 * x + (oct & 1);
                            let cy = 2 * y + ((oct >> 1) & 1);
                            let cz = 2 * z + ((oct >> 2) & 1);
                            let ci = cz * cur_dim * cur_dim + cy * cur_dim + cx;
                            let idx = current[ci];
                            if idx != SVDAG_EMPTY {
                                childmask |= 1 << oct;
                                child_indices[oct] = idx;
                            }
                        }
                        let node_index = if childmask == 0 {
                            SVDAG_EMPTY
                        } else if childmask == 0xff
                            && child_indices.iter().all(|&c| c == SVDAG_FULL)
                        {
                            SVDAG_FULL
                        } else {
                            intern_internal(&mut bodies, &mut interner, childmask, &child_indices)
                        };
                        let ni = z * next_dim * next_dim + y * next_dim + x;
                        next[ni] = node_index;
                    }
                }
            }

            current = next;
            cur_dim = next_dim;
        }

        let root = current.first().copied().unwrap_or(SVDAG_EMPTY);

        // Pack : header (4 words) + per-node bodies in fixed 9-word slots.
        // Pool index i (>= 2) lives at packed offset 4 + (i - 2) * 9.
        let mut packed: Vec<u32> = Vec::with_capacity(
            SVDAG_HEADER_WORDS + bodies.len() * SVDAG_WORDS_PER_NODE,
        );
        packed.extend_from_slice(&[root, dim_u, depth, 0]);
        for body in &bodies {
            packed.push(body.childmask);
            packed.extend_from_slice(&body.children);
        }
        debug_assert_eq!(
            packed.len(),
            SVDAG_HEADER_WORDS + bodies.len() * SVDAG_WORDS_PER_NODE,
        );

        Self { packed, root, dim: dim_u, depth }
    }

    /// CPU traversal for tests / CPU-fallback. Returns true if (x, y, z)
    /// is inside an occupied voxel. Coordinates must be in [0, dim).
    /// Mirrors the WGSL traverser one-for-one — both walk fixed 9-word
    /// nodes, both index children by full octant (0..8), both stop at
    /// SVDAG_EMPTY / SVDAG_FULL sentinels.
    pub fn sample(&self, x: u32, y: u32, z: u32) -> bool {
        if x >= self.dim || y >= self.dim || z >= self.dim {
            return false;
        }
        let mut idx = self.root;
        let mut size = self.dim;
        let mut px = x;
        let mut py = y;
        let mut pz = z;
        while size > 1 {
            if idx == SVDAG_EMPTY {
                return false;
            }
            if idx == SVDAG_FULL {
                return true;
            }
            let half = size / 2;
            let ox = (px >= half) as u32;
            let oy = (py >= half) as u32;
            let oz = (pz >= half) as u32;
            let oct = (ox | (oy << 1) | (oz << 2)) as usize;
            px -= ox * half;
            py -= oy * half;
            pz -= oz * half;
            let off = SVDAG_HEADER_WORDS + ((idx as usize) - 2) * SVDAG_WORDS_PER_NODE;
            idx = self.packed[off + 1 + oct];
            size = half;
        }
        idx == SVDAG_FULL
    }

    /// Number of distinct node bodies stored (excluding the two sentinels).
    /// Reveals the dedup factor : a 256³ "forest of 1 000 identical trees"
    /// stores roughly the node count of ONE tree.
    pub fn node_count(&self) -> usize {
        (self.packed.len().saturating_sub(SVDAG_HEADER_WORDS)) / SVDAG_WORDS_PER_NODE
    }

    /// Total packed size in bytes — what gets uploaded to the GPU.
    pub fn bytes(&self) -> usize {
        self.packed.len() * 4
    }
}

/// One internal-node body, fixed 9 u32 (childmask + 8 child indices).
#[derive(Clone, Eq, PartialEq, Debug)]
struct SvdagBody {
    childmask: u32,
    children: [u32; 8],
}

fn intern_internal(
    bodies: &mut Vec<SvdagBody>,
    interner: &mut HashMap<u64, u32, FastBuildHasher>,
    childmask: u32,
    children: &[u32; 8],
) -> u32 {
    let body = SvdagBody { childmask, children: *children };
    // Content hash → dedup probe.
    let mut h = interner.hasher().build_hasher();
    let words: [u32; 9] = [
        body.childmask,
        body.children[0], body.children[1], body.children[2], body.children[3],
        body.children[4], body.children[5], body.children[6], body.children[7],
    ];
    let bytes: &[u8] = unsafe {
        std::slice::from_raw_parts(words.as_ptr() as *const u8, 9 * 4)
    };
    h.write(bytes);
    let key = h.finish();
    if let Some(&idx) = interner.get(&key) {
        // Hash collision guard : FastHasher is non-cryptographic so we
        // verify by exact comparison before reusing the index.
        if bodies[(idx as usize) - 2] == body {
            return idx;
        }
    }
    let new_idx = (bodies.len() as u32) + 2;
    bodies.push(body);
    interner.insert(key, new_idx);
    new_idx
}

use std::hash::BuildHasher;

#[cfg(test)]
mod tests {
    use super::*;

    /// Empty grid → root is the empty sentinel ; pool is just the header.
    #[test]
    fn empty_grid_collapses_to_sentinel() {
        let grid = vec![false; 8 * 8 * 8];
        let svdag = Svdag::from_occupancy(&grid, 8);
        assert_eq!(svdag.root, SVDAG_EMPTY);
        assert_eq!(svdag.node_count(), 0);
        assert!(!svdag.sample(0, 0, 0));
        assert!(!svdag.sample(7, 7, 7));
    }

    /// Fully occupied grid → root is the full sentinel ; pool is still empty.
    #[test]
    fn full_grid_collapses_to_full_sentinel() {
        let grid = vec![true; 8 * 8 * 8];
        let svdag = Svdag::from_occupancy(&grid, 8);
        assert_eq!(svdag.root, SVDAG_FULL);
        assert_eq!(svdag.node_count(), 0);
        assert!(svdag.sample(0, 0, 0));
        assert!(svdag.sample(7, 7, 7));
    }

    /// A 16³ cube containing a single voxel at the center.
    /// CPU traversal must hit it and miss its neighbours.
    #[test]
    fn single_voxel_roundtrip() {
        let dim = 16;
        let mut grid = vec![false; dim * dim * dim];
        let cx = dim / 2;
        let cy = dim / 2;
        let cz = dim / 2;
        grid[cz * dim * dim + cy * dim + cx] = true;
        let svdag = Svdag::from_occupancy(&grid, dim);
        assert!(svdag.sample(cx as u32, cy as u32, cz as u32));
        assert!(!svdag.sample((cx - 1) as u32, cy as u32, cz as u32));
        assert!(!svdag.sample(cx as u32, (cy + 1) as u32, cz as u32));
        assert!(svdag.node_count() <= (svdag.depth as usize), "single voxel should chain through one node per level");
    }

    /// Dedup proof : a 32³ grid filled with 64 copies of the same 8³
    /// pattern collapses to roughly the node count of ONE pattern.
    /// Without dedup we'd pay ~64× the storage.
    #[test]
    fn dedup_collapses_repeated_pattern() {
        let dim = 32;
        let block = 8;
        let mut grid = vec![false; dim * dim * dim];
        // Single voxel at the centre of each 8³ block (64 blocks total).
        for bz in 0..(dim / block) {
            for by in 0..(dim / block) {
                for bx in 0..(dim / block) {
                    let cx = bx * block + block / 2;
                    let cy = by * block + block / 2;
                    let cz = bz * block + block / 2;
                    grid[cz * dim * dim + cy * dim + cx] = true;
                }
            }
        }
        let svdag = Svdag::from_occupancy(&grid, dim);
        // Two levels above the leaf (8³) the subtrees are identical.
        // The DAG dedup pulls them to a single representative each level.
        // Naïve octree would have ~64× more nodes than the deduped DAG.
        let naive = 64 * 8;
        assert!(
            svdag.node_count() <= naive / 4,
            "expected ≥4× dedup, got {} nodes (naïve ≈ {})",
            svdag.node_count(),
            naive,
        );
    }
}
