//! Π.31 (Wave 17, 2026-05-02) — Copy-on-write snapshot via Arc.
//!
//! **Origine** : Linux/POSIX `fork()`, Redis BGSAVE pattern,
//! PostgreSQL CHECKPOINT, Erlang BEAM process isolation. Idée
//! centrale : capturer un snapshot **instantane** de l'etat RAM
//! sans pause, sans serialization, sans copy initial. La copy
//! n'arrive que quand le parent ou le child ecrit un page (CoW
//! kernel page fault).
//!
//! ## Pourquoi pour Forge ?
//!
//! Backtest checkpoint : on veut snapshotter le state Forge (atlas
//! warm + RAM cache + memos) AVANT un experiment risque, pour
//! pouvoir restorer si l'experiment perturbe l'etat. Pause-stop-the-
//! world serialization = inacceptable (1000+ nodes co-residents).
//!
//! Vraie fork() CoW Linux fournit ca gratuitement : le snapshot est
//! instant, la copy memoire arrive paginated par 4KB seulement quand
//! le parent ou child ecrit (la majorite ne sera jamais touchée).
//!
//! ## Doctrine V7 vs vraie fork()
//!
//! Vraie fork() necessite libc::fork(). V7 doctrine interdit libc.
//! Wave 17 livre une **simulation in-process** via `Arc<Box<[u8]>>` :
//!
//!   - Snapshot = Arc::clone() du backing buffer (instant, O(1))
//!   - Lecture du snapshot = zero-copy slice references
//!   - Si parent ecrit, on remplace son Arc par un new buffer
//!     (compute le diff vs snapshot puis swap) — explicit CoW au
//!     niveau "buffer entier" plutot que page-level kernel.
//!
//! Trade-off vs vraie fork() :
//!   - Vraie fork() : CoW page-level (4KB grain) au kernel, lazy
//!   - Wave 17 logical : CoW buffer-level (full backing) au userspace,
//!     eager copy quand parent ecrit
//!
//! Pour des snapshots read-only (audit, replay backtest) la simulation
//! est equivalente — le parent ne ecrit pas pendant que le child read.
//!
//! ## Architecture Wave 17 minimal viable
//!
//! - `CowSnapshot { backing: Arc<Box<[u8]>>, captured_at: SystemTime,
//!   blob_count: usize, snapshot_id: u64 }`
//! - `CowSnapshotter { current: Arc<Box<[u8]>>, snapshots: Vec<...> }`
//!   produit des snapshots a la demande.
//! - `take_snapshot()` -> CowSnapshot : O(1) Arc clone
//! - `restore(snapshot)` : remplace current par snapshot.backing
//! - Stats : `SnapshotStats { snapshots_taken, restores_performed,
//!   buffer_bytes }`
//!
//! ## Limitations Wave 17 minimal
//!
//! - In-process (vraie fork() Wave 18+ avec libc).
//! - Buffer-level CoW (vs page-level kernel).
//! - Pas d'auto-compaction des snapshots (caller drop manually).

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::SystemTime;

/// Snapshot CoW : backing partage Arc + metadata.
#[allow(dead_code)] // Wave 17 — primitives expose pour wiring backtest checkpoint Wave 18+.
#[derive(Clone)]
pub struct CowSnapshot {
    backing: Arc<Box<[u8]>>,
    pub snapshot_id: u64,
    pub captured_at: SystemTime,
    pub blob_count: usize,
}

#[allow(dead_code)]
impl CowSnapshot {
    /// Slice immutable du backing snapshotté.
    pub fn as_slice(&self) -> &[u8] {
        &self.backing
    }

    /// Bytes total du snapshot.
    pub fn len(&self) -> usize {
        self.backing.len()
    }

    pub fn is_empty(&self) -> bool {
        self.backing.is_empty()
    }

    /// Nombre de Arc strong count = combien de readers partagent
    /// ce snapshot.
    pub fn strong_count(&self) -> usize {
        Arc::strong_count(&self.backing)
    }
}

impl std::fmt::Debug for CowSnapshot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CowSnapshot")
            .field("snapshot_id", &self.snapshot_id)
            .field("len", &self.backing.len())
            .field("blob_count", &self.blob_count)
            .field("strong_count", &self.strong_count())
            .finish()
    }
}

/// Stats observability snapshotter.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, Default)]
pub struct SnapshotStats {
    pub snapshots_taken: u64,
    pub restores_performed: u64,
    pub current_buffer_bytes: usize,
    pub current_arc_strong: usize,
}

/// Snapshotter : maintient le current backing + produit des snapshots
/// CoW a la demande.
pub struct CowSnapshotter {
    current: Arc<Box<[u8]>>,
    next_id: AtomicU64,
    snapshots_taken: AtomicU64,
    restores_performed: AtomicU64,
    blob_count: usize,
}

#[allow(dead_code)]
impl CowSnapshotter {
    /// Construit avec un backing initial (typiquement le hot-atlas
    /// RAM ou le Store buffer post-MmapStore).
    pub fn new(initial: Box<[u8]>, blob_count: usize) -> Self {
        Self {
            current: Arc::new(initial),
            next_id: AtomicU64::new(0),
            snapshots_taken: AtomicU64::new(0),
            restores_performed: AtomicU64::new(0),
            blob_count,
        }
    }

    /// Construit empty.
    pub fn empty() -> Self {
        Self::new(Box::new([]), 0)
    }

    /// Take snapshot : O(1) Arc::clone du backing courant.
    /// Le snapshot reste valide tant qu'il est detenu, meme si le
    /// snapshotter swap son backing (parent write).
    pub fn take_snapshot(&self) -> CowSnapshot {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        self.snapshots_taken.fetch_add(1, Ordering::Relaxed);
        CowSnapshot {
            backing: Arc::clone(&self.current),
            snapshot_id: id,
            captured_at: SystemTime::now(),
            blob_count: self.blob_count,
        }
    }

    /// Replace le backing courant. CoW : si des snapshots tiennent
    /// l'ancien Arc, ils restent valides — le nouveau backing est
    /// alloue separement.
    pub fn replace_backing(&mut self, new_buffer: Box<[u8]>, blob_count: usize) {
        self.current = Arc::new(new_buffer);
        self.blob_count = blob_count;
    }

    /// Restore depuis un snapshot. Remplace le backing courant par
    /// celui du snapshot (Arc::clone, partage memory si possible).
    pub fn restore(&mut self, snapshot: &CowSnapshot) {
        self.current = Arc::clone(&snapshot.backing);
        self.blob_count = snapshot.blob_count;
        self.restores_performed.fetch_add(1, Ordering::Relaxed);
    }

    /// Slice immutable du backing courant.
    pub fn current_slice(&self) -> &[u8] {
        &self.current
    }

    /// Strong count Arc du backing courant.
    pub fn current_strong_count(&self) -> usize {
        Arc::strong_count(&self.current)
    }

    /// Stats observability.
    pub fn stats(&self) -> SnapshotStats {
        SnapshotStats {
            snapshots_taken: self.snapshots_taken.load(Ordering::Relaxed),
            restores_performed: self.restores_performed.load(Ordering::Relaxed),
            current_buffer_bytes: self.current.len(),
            current_arc_strong: Arc::strong_count(&self.current),
        }
    }
}

impl Default for CowSnapshotter {
    fn default() -> Self {
        Self::empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cow_snapshot_basic_take() {
        let buf = vec![0x42u8; 1024].into_boxed_slice();
        let snap = CowSnapshotter::new(buf, 0);
        let s1 = snap.take_snapshot();
        assert_eq!(s1.len(), 1024);
        assert_eq!(s1.snapshot_id, 0);
        assert!(!s1.is_empty());
    }

    #[test]
    fn cow_snapshot_o1_arc_clone() {
        // Snapshot = Arc::clone, pas de bytes copy.
        let buf = vec![0xAAu8; 1_000_000].into_boxed_slice();  // 1MB
        let snap = CowSnapshotter::new(buf, 50);
        let s1 = snap.take_snapshot();
        let s2 = snap.take_snapshot();
        // Both snapshots partagent le meme backing Arc.
        assert!(Arc::ptr_eq(&s1.backing, &s2.backing));
    }

    #[test]
    fn cow_snapshot_survives_replace() {
        let buf = vec![0x11u8; 100].into_boxed_slice();
        let mut snap = CowSnapshotter::new(buf, 0);
        let snapshot = snap.take_snapshot();
        // Replace backing — ancien snapshot doit rester valide.
        let new_buf = vec![0x22u8; 200].into_boxed_slice();
        snap.replace_backing(new_buf, 10);
        // Snapshot doit toujours voir l'ancien backing 0x11×100.
        assert_eq!(snapshot.as_slice()[0], 0x11);
        assert_eq!(snapshot.len(), 100);
        // Current voit le nouveau.
        assert_eq!(snap.current_slice()[0], 0x22);
        assert_eq!(snap.current_slice().len(), 200);
    }

    #[test]
    fn cow_snapshot_restore_via_swap() {
        let initial = vec![0xAAu8; 100].into_boxed_slice();
        let mut snap = CowSnapshotter::new(initial, 5);
        let original = snap.take_snapshot();
        // Modify : replace par new buffer.
        let modified = vec![0xBBu8; 200].into_boxed_slice();
        snap.replace_backing(modified, 10);
        assert_eq!(snap.current_slice()[0], 0xBB);
        // Restore depuis snapshot.
        snap.restore(&original);
        assert_eq!(snap.current_slice()[0], 0xAA);
        assert_eq!(snap.current_slice().len(), 100);
    }

    #[test]
    fn cow_snapshot_stats_track() {
        let buf = vec![0u8; 64].into_boxed_slice();
        let mut snap = CowSnapshotter::new(buf, 0);
        let s1 = snap.take_snapshot();
        let _s2 = snap.take_snapshot();
        let _s3 = snap.take_snapshot();
        snap.restore(&s1);
        let stats = snap.stats();
        assert_eq!(stats.snapshots_taken, 3);
        assert_eq!(stats.restores_performed, 1);
        assert_eq!(stats.current_buffer_bytes, 64);
    }

    #[test]
    fn cow_snapshot_strong_count_reflects_clones() {
        let buf = vec![0u8; 32].into_boxed_slice();
        let snap = CowSnapshotter::new(buf, 0);
        assert_eq!(snap.current_strong_count(), 1);
        let s1 = snap.take_snapshot();
        // Strong count = 1 (snapshotter) + 1 (s1) = 2.
        assert_eq!(snap.current_strong_count(), 2);
        let s2 = snap.take_snapshot();
        assert_eq!(snap.current_strong_count(), 3);
        let _ = (s1, s2);
    }

    #[test]
    fn cow_snapshot_empty() {
        let snap = CowSnapshotter::empty();
        let s = snap.take_snapshot();
        assert!(s.is_empty());
        assert_eq!(s.len(), 0);
    }

    #[test]
    fn cow_snapshot_id_monotonic() {
        let buf = vec![0u8; 8].into_boxed_slice();
        let snap = CowSnapshotter::new(buf, 0);
        for i in 0..10u64 {
            let s = snap.take_snapshot();
            assert_eq!(s.snapshot_id, i);
        }
    }

    #[test]
    fn cow_snapshot_zero_copy_until_write() {
        // 100MB backing → snapshot O(1) (Arc::clone), zero memcpy.
        let large = vec![0xCDu8; 10_000_000].into_boxed_slice();  // 10MB
        let snap = CowSnapshotter::new(large, 0);
        let start = std::time::Instant::now();
        let s = snap.take_snapshot();
        let elapsed = start.elapsed();
        // Arc::clone < 1µs typically. Tolerance 1ms.
        assert!(elapsed.as_micros() < 1000, "snapshot took {} µs", elapsed.as_micros());
        assert_eq!(s.len(), 10_000_000);
    }

    #[test]
    fn cow_snapshot_blob_count_metadata() {
        let buf = vec![0u8; 64].into_boxed_slice();
        let snap = CowSnapshotter::new(buf, 42);
        let s = snap.take_snapshot();
        assert_eq!(s.blob_count, 42);
    }

    #[test]
    fn cow_snapshot_debug_format() {
        let buf = vec![0u8; 8].into_boxed_slice();
        let snap = CowSnapshotter::new(buf, 5);
        let s = snap.take_snapshot();
        let dbg = format!("{:?}", s);
        assert!(dbg.contains("CowSnapshot"));
        assert!(dbg.contains("blob_count: 5"));
    }
}
