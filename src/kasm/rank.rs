//! Π.10 (Wave 4, 2026-05-02) — APL/J rank semantics for tensors.
//!
//! **Origine** : APL (Iverson, 1962) → J (Iverson + Hui, 1990) →
//! NumPy/Julia broadcasting. Idée centrale APL : un tenseur N-dim a
//! une "rank" (= nombre de dimensions). L'opérateur `rank` permet
//! d'appliquer une fonction à un sous-rang choisi de l'array, avec
//! auto-broadcasting des résultats.
//!
//! Exemple APL/J :
//!   - `+/"1 mat` : sum sur rank 1 (chaque ligne) → vecteur.
//!   - `+/"2 mat` : sum sur rank 2 (la matrice entière) → scalaire.
//!   - `(*/)"0` : produit elementwise sur rank 0 (scalaires).
//!   - Outer product `mat ∘.+ vec` : auto-broadcast jusqu'à match.
//!
//! ## Pourquoi pour Forge ?
//!
//! Le synthétiseur lab génère des programmes KASM qui calculent sur
//! des inputs de shape variable. Pour les targets domain (chimie,
//! finance, sequence), la cible est souvent multi-dim (vecteur de
//! mesures, matrice de correlations). Avec rank semantics :
//!   1. Une seule définition de "sum" → applicable à scalar, vector,
//!      matrix, tensor 3D — sans dispatch manuel.
//!   2. Auto-broadcasting compatible NumPy (broadcasting rules) —
//!      programmes plus compacts.
//!   3. Composition avec Op::VLenI64/VSumI64/etc. (Wave 7d-h) →
//!      surface KASM Vec étendue à N-dim sans nouveau opcode.
//!
//! ## Architecture Wave 4 minimal viable
//!
//! - `RankedTensor` = `Tensor { data: Vec<i64>, shape: Vec<usize> }`
//!   (i64 universel ; multi-type Wave 11+).
//! - Stockage : row-major (NumPy/C-style) — inverse de Fortran/Julia.
//! - `rank()` = `shape.len()`.
//! - `apply_rank_0(op)` : appliquer `op: i64 → i64` elementwise.
//! - `apply_rank_1(op)` : appliquer `op: &[i64] → i64` sur chaque
//!   "row" (dernière dim itérée par-dim).
//! - `broadcast_add(a, b)` : NumPy-style broadcasting binaire.
//! - `outer_product(a, b)` : APL `∘.×` produit cartésien.
//! - `reshape(new_shape)` : changer shape sans changer data.
//! - `sum_along_axis(axis)` : reduction.
//!
//! ## Limitations Wave 4 minimal
//!
//! - i64 only.
//! - Pas de strides custom (toujours contigu row-major).
//! - Pas de broadcasting >2 tenseurs simultanés (Wave 11+).
//! - apply_rank limité à rank 0 et 1 (rank N>1 = compositions).

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RankError {
    /// Shape produit ≠ longueur de data.
    ShapeMismatch { shape_product: usize, data_len: usize },
    /// Broadcasting impossible entre deux shapes.
    IncompatibleBroadcast { a: Vec<usize>, b: Vec<usize> },
    /// Axis hors range.
    BadAxis { axis: usize, rank: usize },
    /// Reshape change le nombre total d'éléments.
    BadReshape { from: Vec<usize>, to: Vec<usize> },
}

impl fmt::Display for RankError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RankError::ShapeMismatch { shape_product, data_len } =>
                write!(f, "shape product {} != data len {}", shape_product, data_len),
            RankError::IncompatibleBroadcast { a, b } =>
                write!(f, "shapes {:?} and {:?} cannot broadcast", a, b),
            RankError::BadAxis { axis, rank } =>
                write!(f, "axis {} out of rank-{} tensor", axis, rank),
            RankError::BadReshape { from, to } =>
                write!(f, "reshape from {:?} to {:?} changes element count", from, to),
        }
    }
}

/// Tenseur N-dim contigu row-major, valeurs i64.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RankedTensor {
    pub data: Vec<i64>,
    pub shape: Vec<usize>,
}

impl RankedTensor {
    /// Construit depuis (data, shape). Vérifie que produit shape =
    /// longueur data.
    pub fn new(data: Vec<i64>, shape: Vec<usize>) -> Result<Self, RankError> {
        let prod: usize = shape.iter().product();
        // Convention APL : un tenseur scalaire a shape=[] avec prod=1.
        let expected = if shape.is_empty() { 1 } else { prod };
        if expected != data.len() {
            return Err(RankError::ShapeMismatch {
                shape_product: expected,
                data_len: data.len(),
            });
        }
        Ok(Self { data, shape })
    }

    /// Constructeur scalaire (shape = []).
    pub fn scalar(v: i64) -> Self {
        Self { data: vec![v], shape: Vec::new() }
    }

    /// Constructeur vecteur (rank 1).
    pub fn vector(data: Vec<i64>) -> Self {
        let n = data.len();
        Self { data, shape: vec![n] }
    }

    /// Constructeur matrice (rank 2).
    pub fn matrix(data: Vec<i64>, rows: usize, cols: usize) -> Result<Self, RankError> {
        Self::new(data, vec![rows, cols])
    }

    /// Rank = nombre de dimensions.
    pub fn rank(&self) -> usize {
        self.shape.len()
    }

    /// Nombre total d'éléments (= produit shape, ou 1 si scalaire).
    pub fn elements(&self) -> usize {
        self.data.len()
    }

    /// Reshape sans changer data (vérifie que prod nouveau shape = prod ancien).
    pub fn reshape(&self, new_shape: Vec<usize>) -> Result<Self, RankError> {
        let new_prod: usize = if new_shape.is_empty() { 1 } else { new_shape.iter().product() };
        if new_prod != self.data.len() {
            return Err(RankError::BadReshape {
                from: self.shape.clone(),
                to: new_shape,
            });
        }
        Ok(Self {
            data: self.data.clone(),
            shape: new_shape,
        })
    }

    /// `apply_rank_0(op)` : applique `op` elementwise.
    /// Equivalent APL `op"0`. Préserve shape.
    pub fn apply_rank_0<F: Fn(i64) -> i64>(&self, op: F) -> Self {
        let data = self.data.iter().map(|&v| op(v)).collect();
        Self { data, shape: self.shape.clone() }
    }

    /// `apply_rank_1(op)` : applique `op: &[i64] -> i64` sur chaque
    /// "row" (dernière dimension itérée).
    /// Equivalent APL `op"1`. Réduit la dernière dimension.
    /// Pour scalaire/rank 0 → erreur (pas de rank 1 dispo).
    pub fn apply_rank_1<F: Fn(&[i64]) -> i64>(
        &self,
        op: F,
    ) -> Result<Self, RankError> {
        if self.shape.is_empty() {
            return Err(RankError::BadAxis {
                axis: 0, rank: 0,
            });
        }
        let last_dim = *self.shape.last().unwrap();
        if last_dim == 0 {
            // Edge case : dim de taille 0 → vector vide pour chaque row.
            return Ok(Self {
                data: Vec::new(),
                shape: self.shape[..self.shape.len() - 1].to_vec(),
            });
        }
        let n_rows = self.data.len() / last_dim;
        let mut result = Vec::with_capacity(n_rows);
        for i in 0..n_rows {
            let row = &self.data[i * last_dim..(i + 1) * last_dim];
            result.push(op(row));
        }
        let new_shape = self.shape[..self.shape.len() - 1].to_vec();
        Ok(Self { data: result, shape: new_shape })
    }

    /// `sum_along_axis(axis)` : reduction par sum sur l'axe donné.
    /// Equivalent NumPy `np.sum(arr, axis=k)`.
    /// Pour Wave 4 minimal, supporte `axis = rank - 1` (last axis)
    /// via apply_rank_1 — extension multi-axis Wave 11+.
    pub fn sum_along_last_axis(&self) -> Result<Self, RankError> {
        self.apply_rank_1(|row| {
            let mut acc: i64 = 0;
            for &v in row {
                acc = acc.wrapping_add(v);
            }
            acc
        })
    }

    /// Broadcasting NumPy-style entre deux tenseurs.
    /// Règles :
    ///   1. Aligner shapes par la droite.
    ///   2. Pour chaque dim alignée : OK si égales ou si l'une = 1.
    ///   3. La dim sortie = max des deux.
    /// Wave 4 minimal : retourne le shape résultant + un tenseur add.
    pub fn broadcast_add(&self, other: &Self) -> Result<Self, RankError> {
        let result_shape = compute_broadcast_shape(&self.shape, &other.shape)?;
        // Si le résultat est scalaire (shapes vides ou 1×1), fast path.
        if result_shape.is_empty() {
            return Ok(Self::scalar(self.data[0].wrapping_add(other.data[0])));
        }
        // Algorithme générique : itérer chaque index multi-dim du
        // résultat, calculer l'index correspondant dans a/b avec
        // broadcasting (size-1 dim → toujours index 0).
        let n: usize = result_shape.iter().product();
        let mut data = Vec::with_capacity(n);
        for flat_idx in 0..n {
            // Décoder flat_idx en multi-dim selon result_shape.
            let multi = decode_multi_index(flat_idx, &result_shape);
            // Encoder dans a et b avec broadcasting.
            let ia = encode_broadcast_index(&multi, &result_shape, &self.shape);
            let ib = encode_broadcast_index(&multi, &result_shape, &other.shape);
            data.push(self.data[ia].wrapping_add(other.data[ib]));
        }
        Self::new(data, result_shape)
    }

    /// Outer product APL `a ∘.* b` : pour deux vecteurs `a[m]` et `b[n]`,
    /// retourne matrice `[m, n]` où `result[i, j] = op(a[i], b[j])`.
    /// Pour Wave 4 minimal, fixé sur la multiplication.
    pub fn outer_product_mul(&self, other: &Self) -> Result<Self, RankError> {
        if self.rank() != 1 || other.rank() != 1 {
            return Err(RankError::IncompatibleBroadcast {
                a: self.shape.clone(),
                b: other.shape.clone(),
            });
        }
        let m = self.shape[0];
        let n = other.shape[0];
        let mut data = Vec::with_capacity(m * n);
        for i in 0..m {
            for j in 0..n {
                data.push(self.data[i].wrapping_mul(other.data[j]));
            }
        }
        Self::new(data, vec![m, n])
    }
}

// ─── Internal broadcasting helpers ─────────────────────────────────

/// Calcule le shape résultant du broadcasting de deux shapes.
/// Règles NumPy : aligner par la droite, dim égales OU l'une = 1.
fn compute_broadcast_shape(
    a: &[usize],
    b: &[usize],
) -> Result<Vec<usize>, RankError> {
    let na = a.len();
    let nb = b.len();
    let n = na.max(nb);
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let ai = if i < na { a[na - 1 - i] } else { 1 };
        let bi = if i < nb { b[nb - 1 - i] } else { 1 };
        let dim = if ai == bi {
            ai
        } else if ai == 1 {
            bi
        } else if bi == 1 {
            ai
        } else {
            return Err(RankError::IncompatibleBroadcast {
                a: a.to_vec(),
                b: b.to_vec(),
            });
        };
        out.push(dim);
    }
    out.reverse();
    Ok(out)
}

/// Décode un index plat en index multi-dim (row-major).
fn decode_multi_index(flat: usize, shape: &[usize]) -> Vec<usize> {
    let mut out = vec![0; shape.len()];
    let mut rem = flat;
    for i in (0..shape.len()).rev() {
        let d = shape[i];
        if d == 0 {
            return out; // shape contient 0 → degenerate
        }
        out[i] = rem % d;
        rem /= d;
    }
    out
}

/// Encode un index multi-dim de result_shape vers un index plat dans
/// `target_shape` en appliquant les règles broadcasting (dim=1 →
/// toujours index 0).
fn encode_broadcast_index(
    result_idx: &[usize],
    result_shape: &[usize],
    target_shape: &[usize],
) -> usize {
    if target_shape.is_empty() {
        return 0; // scalaire
    }
    let nr = result_shape.len();
    let nt = target_shape.len();
    // Aligner par la droite : target_shape[k] correspond à result_shape[nr - nt + k].
    let mut flat = 0usize;
    let mut stride = 1usize;
    for k in (0..nt).rev() {
        let target_dim = target_shape[k];
        let r_axis = nr - nt + k;
        let r_idx = result_idx[r_axis];
        // Si target_dim==1, broadcasting → toujours 0.
        let t_idx = if target_dim == 1 { 0 } else { r_idx };
        flat += t_idx * stride;
        stride *= target_dim.max(1);
    }
    flat
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rank_scalar_construction() {
        let s = RankedTensor::scalar(42);
        assert_eq!(s.rank(), 0);
        assert_eq!(s.elements(), 1);
        assert_eq!(s.data, vec![42]);
        assert!(s.shape.is_empty());
    }

    #[test]
    fn rank_vector_construction() {
        let v = RankedTensor::vector(vec![1, 2, 3, 4]);
        assert_eq!(v.rank(), 1);
        assert_eq!(v.shape, vec![4]);
        assert_eq!(v.elements(), 4);
    }

    #[test]
    fn rank_matrix_construction() {
        let m = RankedTensor::matrix(vec![1, 2, 3, 4, 5, 6], 2, 3).unwrap();
        assert_eq!(m.rank(), 2);
        assert_eq!(m.shape, vec![2, 3]);
        assert_eq!(m.elements(), 6);
    }

    #[test]
    fn rank_rejects_shape_mismatch() {
        let err = RankedTensor::new(vec![1, 2, 3], vec![2, 2]).unwrap_err();
        assert!(matches!(err, RankError::ShapeMismatch { shape_product: 4, data_len: 3 }));
    }

    #[test]
    fn rank_apply_rank_0_elementwise() {
        // op = double sur matrix.
        let m = RankedTensor::matrix(vec![1, 2, 3, 4], 2, 2).unwrap();
        let doubled = m.apply_rank_0(|x| x * 2);
        assert_eq!(doubled.shape, vec![2, 2]);
        assert_eq!(doubled.data, vec![2, 4, 6, 8]);
    }

    #[test]
    fn rank_apply_rank_1_reduces_last_axis() {
        // matrix 2×3 :
        //   [1, 2, 3]
        //   [4, 5, 6]
        // sum sur rank 1 → [6, 15] (vecteur de longueur 2).
        let m = RankedTensor::matrix(vec![1, 2, 3, 4, 5, 6], 2, 3).unwrap();
        let summed = m.sum_along_last_axis().unwrap();
        assert_eq!(summed.shape, vec![2]);
        assert_eq!(summed.data, vec![6, 15]);
    }

    #[test]
    fn rank_reshape_preserves_data() {
        let v = RankedTensor::vector(vec![1, 2, 3, 4, 5, 6]);
        let m = v.reshape(vec![2, 3]).unwrap();
        assert_eq!(m.shape, vec![2, 3]);
        assert_eq!(m.data, vec![1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn rank_reshape_rejects_mismatch() {
        let v = RankedTensor::vector(vec![1, 2, 3, 4, 5, 6]);
        let err = v.reshape(vec![2, 4]).unwrap_err();
        assert!(matches!(err, RankError::BadReshape { .. }));
    }

    #[test]
    fn rank_broadcast_add_scalar_and_vector() {
        // [1, 2, 3] + scalar(10) = [11, 12, 13].
        let v = RankedTensor::vector(vec![1, 2, 3]);
        let s = RankedTensor::scalar(10);
        let r = v.broadcast_add(&s).unwrap();
        assert_eq!(r.shape, vec![3]);
        assert_eq!(r.data, vec![11, 12, 13]);
    }

    #[test]
    fn rank_broadcast_add_vector_to_matrix() {
        // matrix 2×3 + vector size 3 → broadcast row-wise.
        // m = [[1,2,3], [4,5,6]], v = [10, 20, 30]
        // Expected : [[11, 22, 33], [14, 25, 36]]
        let m = RankedTensor::matrix(vec![1, 2, 3, 4, 5, 6], 2, 3).unwrap();
        let v = RankedTensor::vector(vec![10, 20, 30]);
        let r = m.broadcast_add(&v).unwrap();
        assert_eq!(r.shape, vec![2, 3]);
        assert_eq!(r.data, vec![11, 22, 33, 14, 25, 36]);
    }

    #[test]
    fn rank_broadcast_rejects_incompatible() {
        // shapes [3] et [4] sont incompatibles (ni égaux ni l'un=1).
        let a = RankedTensor::vector(vec![1, 2, 3]);
        let b = RankedTensor::vector(vec![1, 2, 3, 4]);
        let err = a.broadcast_add(&b).unwrap_err();
        assert!(matches!(err, RankError::IncompatibleBroadcast { .. }));
    }

    #[test]
    fn rank_outer_product_apl_style() {
        // [1, 2, 3] ∘.× [10, 20]
        // = [[10, 20], [20, 40], [30, 60]]
        let a = RankedTensor::vector(vec![1, 2, 3]);
        let b = RankedTensor::vector(vec![10, 20]);
        let r = a.outer_product_mul(&b).unwrap();
        assert_eq!(r.shape, vec![3, 2]);
        assert_eq!(r.data, vec![10, 20, 20, 40, 30, 60]);
    }

    #[test]
    fn rank_outer_product_rejects_non_vectors() {
        let m = RankedTensor::matrix(vec![1, 2, 3, 4], 2, 2).unwrap();
        let v = RankedTensor::vector(vec![1, 2]);
        let err = m.outer_product_mul(&v).unwrap_err();
        assert!(matches!(err, RankError::IncompatibleBroadcast { .. }));
    }

    #[test]
    fn rank_compose_rank_0_then_rank_1() {
        // (square then sum) sur matrix 2×3.
        // m = [[1,2,3], [4,5,6]]
        // squared = [[1,4,9], [16,25,36]]
        // sum row-wise = [14, 77]
        let m = RankedTensor::matrix(vec![1, 2, 3, 4, 5, 6], 2, 3).unwrap();
        let sq = m.apply_rank_0(|x| x.wrapping_mul(x));
        let r = sq.sum_along_last_axis().unwrap();
        assert_eq!(r.shape, vec![2]);
        assert_eq!(r.data, vec![14, 77]);
    }
}
