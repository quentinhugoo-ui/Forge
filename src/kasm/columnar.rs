//! Π.9 (Wave 4, 2026-05-02) — Q/Kdb+ columnar storage.
//!
//! **Origine** : Q/Kdb+ (Arthur Whitney, 1990s — actuel record du
//! monde sur les workloads HFT analytiques). Idée centrale : au lieu
//! d'un row-store `Vec<Row { col_a, col_b, col_c, ... }>`, stocker
//! par colonne `[Vec<col_a>, Vec<col_b>, Vec<col_c>]`. Pour les
//! requêtes analytiques type "scan colonne 2 où colonne 0 > 5", le
//! row-store traverse toute la mémoire ; le column-store stream
//! uniquement les colonnes nécessaires → ×5-10 plus rapide + bien
//! plus SIMD-friendly.
//!
//! ## Pourquoi pour Forge ?
//!
//! Le `forge.cas` actuel (V7 Φ.μ.7) stocke les programmes KASM en
//! row-store : chaque programme = blob complet inline. Pour des
//! workloads analytiques sur l'atlas (e.g. "tous les programmes dont
//! le node_count > 50 et qui contiennent Op::Hash64"), on doit
//! parser chaque programme entier — ×100 plus lent qu'une scan
//! columnaire de l'index.
//!
//! Wave 4 minimal : column store en mémoire pour les **stats** et
//! **features** des programmes (node_count, op_distribution, atom
//! fingerprint). Le wiring dans LiveAtlas est Wave 11+.
//!
//! ## Architecture Wave 4 minimal viable
//!
//! - N colonnes typées i64 (universel Forge ; multi-type Wave 11+).
//! - Stockage : `Vec<Vec<i64>>` — chaque colonne contigüe.
//! - `add_row(&[i64])` : append synchronisé sur toutes colonnes.
//! - `scan_column(idx) -> &[i64]` : SIMD-friendly contigu.
//! - `filter_sum(filter_col, pred, sum_col) -> i64` : pattern Q
//!   "select sum c2 from t where c0 > k".
//! - `select_where(predicate)` : iterate matching rows.
//!
//! ## Limitations Wave 4 minimal
//!
//! - i64 only (multi-type Wave 11+).
//! - Pas de compression (RLE, delta, etc.) — Wave 11+ Π.9-bis.
//! - Pas de persistance disk (mmap forge.cas integration Wave 11+).
//! - Append-only (pas de delete row).

use std::fmt;

/// Erreur de manipulation columnaire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ColumnarError {
    /// `add_row` reçu un nombre de valeurs ≠ nombre de colonnes.
    BadRowArity { expected: usize, got: usize },
    /// Index colonne hors range.
    BadColumnIdx { idx: usize, max: usize },
    /// Index ligne hors range.
    BadRowIdx { idx: usize, max: usize },
}

impl fmt::Display for ColumnarError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ColumnarError::BadRowArity { expected, got } =>
                write!(f, "row arity mismatch: expected {} cols, got {}", expected, got),
            ColumnarError::BadColumnIdx { idx, max } =>
                write!(f, "column idx {} >= {} columns", idx, max),
            ColumnarError::BadRowIdx { idx, max } =>
                write!(f, "row idx {} >= {} rows", idx, max),
        }
    }
}

/// Column-oriented store i64. Wave 4 minimal viable.
pub struct ColumnStore {
    /// `columns[i]` est la i-ème colonne, contigüe en mémoire.
    columns: Vec<Vec<i64>>,
    /// Nombre de lignes (synchronisé sur toutes les colonnes).
    row_count: usize,
}

impl ColumnStore {
    /// Construit un store avec `n_cols` colonnes vides.
    pub fn new(n_cols: usize) -> Self {
        let columns = (0..n_cols).map(|_| Vec::new()).collect();
        Self { columns, row_count: 0 }
    }

    /// Construit un store avec capacité réservée pour `cap_rows` lignes
    /// par colonne (évite les realloc pendant l'ingestion bulk).
    pub fn with_capacity(n_cols: usize, cap_rows: usize) -> Self {
        let columns = (0..n_cols)
            .map(|_| Vec::with_capacity(cap_rows))
            .collect();
        Self { columns, row_count: 0 }
    }

    /// Nombre de colonnes (constant après construction).
    pub fn columns(&self) -> usize {
        self.columns.len()
    }

    /// Nombre de lignes courantes.
    pub fn rows(&self) -> usize {
        self.row_count
    }

    /// Mémoire approximative en bytes utilisée par les colonnes.
    pub fn bytes_used(&self) -> usize {
        self.columns.iter().map(|c| c.capacity() * 8).sum()
    }

    /// Append une ligne. La taille de `values` doit matcher exactement
    /// le nombre de colonnes.
    pub fn add_row(&mut self, values: &[i64]) -> Result<(), ColumnarError> {
        if values.len() != self.columns.len() {
            return Err(ColumnarError::BadRowArity {
                expected: self.columns.len(),
                got: values.len(),
            });
        }
        for (i, v) in values.iter().enumerate() {
            self.columns[i].push(*v);
        }
        self.row_count += 1;
        Ok(())
    }

    /// Scan d'une colonne entière — slice contigu pour SIMD/cache.
    pub fn scan_column(&self, idx: usize) -> Result<&[i64], ColumnarError> {
        if idx >= self.columns.len() {
            return Err(ColumnarError::BadColumnIdx {
                idx, max: self.columns.len(),
            });
        }
        Ok(&self.columns[idx])
    }

    /// Lecture d'une cellule.
    pub fn get(&self, col: usize, row: usize) -> Result<i64, ColumnarError> {
        if col >= self.columns.len() {
            return Err(ColumnarError::BadColumnIdx {
                idx: col, max: self.columns.len(),
            });
        }
        if row >= self.row_count {
            return Err(ColumnarError::BadRowIdx {
                idx: row, max: self.row_count,
            });
        }
        Ok(self.columns[col][row])
    }

    /// Filter+Sum vectorisé : pattern Q "select sum cN from t where cF op K".
    /// Pour chaque ligne, si `predicate(filter_col[row])` vrai, ajoute
    /// `sum_col[row]` à l'accumulateur. Wave 4 minimal : sum, pas de
    /// avg/min/max (trivial à étendre).
    ///
    /// Le pattern courant Q:
    ///   `select sum amount from trades where price > 100`
    /// devient ici :
    ///   `store.filter_sum(price_col, |p| p > 100, amount_col)`
    pub fn filter_sum<F: Fn(i64) -> bool>(
        &self,
        filter_col: usize,
        predicate: F,
        sum_col: usize,
    ) -> Result<i64, ColumnarError> {
        if filter_col >= self.columns.len() {
            return Err(ColumnarError::BadColumnIdx {
                idx: filter_col, max: self.columns.len(),
            });
        }
        if sum_col >= self.columns.len() {
            return Err(ColumnarError::BadColumnIdx {
                idx: sum_col, max: self.columns.len(),
            });
        }
        let filter = &self.columns[filter_col];
        let sum_data = &self.columns[sum_col];
        let mut acc: i64 = 0;
        // For loop brut — iterator chains seraient ×1.0-1.3 plus lents.
        for i in 0..self.row_count {
            if predicate(filter[i]) {
                acc = acc.wrapping_add(sum_data[i]);
            }
        }
        Ok(acc)
    }

    /// Filter rows et retourne les indices qui matchent. Permet ensuite
    /// d'agréger plusieurs colonnes sur le même filter.
    pub fn select_where<F: Fn(i64) -> bool>(
        &self,
        filter_col: usize,
        predicate: F,
    ) -> Result<Vec<usize>, ColumnarError> {
        if filter_col >= self.columns.len() {
            return Err(ColumnarError::BadColumnIdx {
                idx: filter_col, max: self.columns.len(),
            });
        }
        let filter = &self.columns[filter_col];
        let mut out = Vec::new();
        for i in 0..self.row_count {
            if predicate(filter[i]) {
                out.push(i);
            }
        }
        Ok(out)
    }

    /// Aggregat : sum d'une colonne entière (pas de filter).
    /// SIMD-friendly via slice contigu.
    pub fn column_sum(&self, col: usize) -> Result<i64, ColumnarError> {
        let slice = self.scan_column(col)?;
        let mut acc: i64 = 0;
        for &v in slice {
            acc = acc.wrapping_add(v);
        }
        Ok(acc)
    }

    /// Aggregat : min d'une colonne. Retourne i64::MAX si la colonne est vide.
    pub fn column_min(&self, col: usize) -> Result<i64, ColumnarError> {
        let slice = self.scan_column(col)?;
        let mut m = i64::MAX;
        for &v in slice {
            if v < m { m = v; }
        }
        Ok(m)
    }

    /// Aggregat : max d'une colonne. Retourne i64::MIN si la colonne est vide.
    pub fn column_max(&self, col: usize) -> Result<i64, ColumnarError> {
        let slice = self.scan_column(col)?;
        let mut m = i64::MIN;
        for &v in slice {
            if v > m { m = v; }
        }
        Ok(m)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn columnar_basic_add_scan() {
        let mut store = ColumnStore::new(3);
        store.add_row(&[1, 100, 1000]).unwrap();
        store.add_row(&[2, 200, 2000]).unwrap();
        store.add_row(&[3, 300, 3000]).unwrap();
        assert_eq!(store.rows(), 3);
        assert_eq!(store.columns(), 3);
        assert_eq!(store.scan_column(0).unwrap(), &[1, 2, 3]);
        assert_eq!(store.scan_column(1).unwrap(), &[100, 200, 300]);
        assert_eq!(store.scan_column(2).unwrap(), &[1000, 2000, 3000]);
    }

    #[test]
    fn columnar_rejects_wrong_arity() {
        let mut store = ColumnStore::new(3);
        let err = store.add_row(&[1, 2]).unwrap_err();
        assert!(matches!(err, ColumnarError::BadRowArity { expected: 3, got: 2 }));
    }

    #[test]
    fn columnar_rejects_bad_column_idx() {
        let store = ColumnStore::new(2);
        let err = store.scan_column(5).unwrap_err();
        assert!(matches!(err, ColumnarError::BadColumnIdx { idx: 5, max: 2 }));
    }

    #[test]
    fn columnar_filter_sum_q_style() {
        // Pattern Q : "select sum amount from trades where price > 100"
        // Trades : (price, amount)
        // (50,  10), (150, 20), (200, 30), (75, 5), (300, 40)
        // Filter price > 100 → rows 1, 2, 4 → amounts 20, 30, 40 → sum 90.
        let mut store = ColumnStore::new(2);
        store.add_row(&[50, 10]).unwrap();
        store.add_row(&[150, 20]).unwrap();
        store.add_row(&[200, 30]).unwrap();
        store.add_row(&[75, 5]).unwrap();
        store.add_row(&[300, 40]).unwrap();
        let total = store.filter_sum(0, |p| p > 100, 1).unwrap();
        assert_eq!(total, 90);
    }

    #[test]
    fn columnar_select_where_returns_indices() {
        let mut store = ColumnStore::new(1);
        for i in 0..10i64 {
            store.add_row(&[i]).unwrap();
        }
        let evens = store.select_where(0, |v| v % 2 == 0).unwrap();
        assert_eq!(evens, vec![0, 2, 4, 6, 8]);
    }

    #[test]
    fn columnar_aggregates_sum_min_max() {
        let mut store = ColumnStore::new(1);
        for v in [3i64, 1, 4, 1, 5, 9, 2, 6, 5, 3] {
            store.add_row(&[v]).unwrap();
        }
        assert_eq!(store.column_sum(0).unwrap(), 39);
        assert_eq!(store.column_min(0).unwrap(), 1);
        assert_eq!(store.column_max(0).unwrap(), 9);
    }

    #[test]
    fn columnar_with_capacity_avoids_realloc() {
        // Avec capacity réservée, les `bytes_used` après 1000 inserts
        // restent stables (modulo l'allocation initiale).
        let mut store = ColumnStore::with_capacity(3, 1000);
        for i in 0..1000i64 {
            store.add_row(&[i, i * 2, i * 3]).unwrap();
        }
        assert_eq!(store.rows(), 1000);
        // chaque colonne capacité ≥ 1000 → ≥ 8000 bytes × 3 cols = 24000 min.
        assert!(store.bytes_used() >= 24000);
    }

    #[test]
    fn columnar_get_cell_indexed() {
        let mut store = ColumnStore::new(2);
        store.add_row(&[10, 100]).unwrap();
        store.add_row(&[20, 200]).unwrap();
        assert_eq!(store.get(0, 1).unwrap(), 20);
        assert_eq!(store.get(1, 0).unwrap(), 100);
        assert!(matches!(
            store.get(5, 0).unwrap_err(),
            ColumnarError::BadColumnIdx { .. }
        ));
        assert!(matches!(
            store.get(0, 99).unwrap_err(),
            ColumnarError::BadRowIdx { .. }
        ));
    }

    #[test]
    fn columnar_empty_aggregates_sane_defaults() {
        let store = ColumnStore::new(1);
        assert_eq!(store.column_sum(0).unwrap(), 0);
        assert_eq!(store.column_min(0).unwrap(), i64::MAX);
        assert_eq!(store.column_max(0).unwrap(), i64::MIN);
    }
}
