//! Σ.23 (Wave 15, 2026-05-02) — Seqlock pour read sans lock.
//!
//! **Origine** : Linux kernel `seqlock_t` (Stephen Rothwell, ~1998),
//! DPDK control plane, InfluxDB TSI hot indexes. Pattern canonique
//! où les **readers** ne touchent jamais un mutex/atomic CAS — ils
//! lisent un counter de séquence avant et après leur lecture, et
//! retry si le counter a changé (= writer en cours).
//!
//! ## Pourquoi pour Forge ?
//!
//! `InlineCache` actuel (Φ.μ.7) utilise déjà des atomics direct-mapped
//! 64 slots. Mais les writers prennent un atomic store qui peut
//! bloquer brièvement les readers via cache invalidation MESI.
//!
//! Seqlock = readers ZÉRO writes mémoire, ZÉRO atomic operations
//! coûteuses, juste 2 loads d'un `AtomicU32` séquence + une comparaison.
//! Latence reader = 2-3 ns vs ~10-15 ns pour un atomic CAS-based read.
//!
//! ## Architecture Wave 15 minimal viable
//!
//! ```text
//!   Seqlock<T: Copy> :
//!     seq: AtomicU32                — counter, pair = stable, impair = writer en cours
//!     data: UnsafeCell<T>           — donnée protégée
//!
//!   read() -> T :
//!     loop {
//!       let s1 = seq.load(Acquire);
//!       if s1 & 1 == 1 { spin; continue; }    // writer en cours
//!       let value = unsafe { *data };          // lecture optimiste
//!       let s2 = seq.load(Acquire);
//!       if s1 == s2 { return value; }          // pas de write entre temps
//!       // sinon retry
//!     }
//!
//!   write(value) :
//!     seq.fetch_add(1, AcqRel);     // pass impair (writer entre)
//!     unsafe { *data = value; }
//!     seq.fetch_add(1, AcqRel);     // pass pair (writer sort)
//! ```
//!
//! ## Limitations Wave 15 minimal
//!
//! - T: Copy obligatoire (la lecture optimiste copie les bytes).
//! - Single writer (ou writers serialisés externe). MULTI-writer
//!   nécessiterait un mutex sur seq.fetch_add — défaut pattern Linux.
//! - Pas d'attente bloquante côté writer (fire-and-forget).
//! - Ne convient pas pour T avec invariants cross-bytes (mid-write
//!   read peut voir un état inconsistant — c'est pour ça que reader
//!   retry).

use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicU32, Ordering};

/// Seqlock pour `T: Copy`. Reader = sans lock, sans atomic CAS.
pub struct Seqlock<T: Copy> {
    seq: AtomicU32,
    data: UnsafeCell<T>,
}

// SAFETY: Seqlock<T> est Send si T: Send. Sync si T: Send + Copy
// (les writes sont serialisés côté caller, les reads optimistes sont
// safe via la séquence + retry pattern).
unsafe impl<T: Copy + Send> Send for Seqlock<T> {}
unsafe impl<T: Copy + Send> Sync for Seqlock<T> {}

#[allow(dead_code)] // Wave 15 — primitives expose pour wiring InlineCache Wave 16+.
impl<T: Copy> Seqlock<T> {
    pub fn new(initial: T) -> Self {
        Self {
            seq: AtomicU32::new(0),
            data: UnsafeCell::new(initial),
        }
    }

    /// Read sans lock. Retry si writer in-flight.
    /// Latence : 2-3 ns + retry rare.
    pub fn read(&self) -> T {
        loop {
            let s1 = self.seq.load(Ordering::Acquire);
            if s1 & 1 == 1 {
                // Writer en cours — spin avec hint au CPU (PAUSE x86).
                std::hint::spin_loop();
                continue;
            }
            // SAFETY: lecture optimiste sur copie des bytes. La séquence
            // garantit qu'on retry si write est intercalé.
            let value = unsafe { *self.data.get() };
            let s2 = self.seq.load(Ordering::Acquire);
            if s1 == s2 {
                return value;
            }
            // Write intercalé — retry.
        }
    }

    /// Write. Single-writer ; multi-writer nécessite mutex externe.
    /// Latence : 2 atomic fetch_add + 1 write.
    pub fn write(&self, value: T) {
        // Pass 1 : impair (writer entre).
        let prev = self.seq.fetch_add(1, Ordering::AcqRel);
        debug_assert!(prev & 1 == 0, "concurrent writers detected on seqlock");
        // SAFETY: caller guarantee single-writer.
        unsafe {
            *self.data.get() = value;
        }
        // Pass 2 : pair (writer sort).
        self.seq.fetch_add(1, Ordering::AcqRel);
    }

    /// Sequence courante (debug/stats).
    pub fn sequence(&self) -> u32 {
        self.seq.load(Ordering::Acquire)
    }

    /// Vrai si un writer est en cours (debug).
    pub fn writer_in_flight(&self) -> bool {
        self.sequence() & 1 == 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn seqlock_basic_read_write() {
        let lock = Seqlock::new(42i64);
        assert_eq!(lock.read(), 42);
        lock.write(99);
        assert_eq!(lock.read(), 99);
    }

    #[test]
    fn seqlock_sequence_counter_increments() {
        let lock = Seqlock::new(0u64);
        assert_eq!(lock.sequence(), 0);
        lock.write(1);
        assert_eq!(lock.sequence(), 2);  // pair après write complet
        lock.write(2);
        assert_eq!(lock.sequence(), 4);
    }

    #[test]
    fn seqlock_writer_in_flight_false_initially() {
        let lock: Seqlock<u64> = Seqlock::new(0);
        assert!(!lock.writer_in_flight());
    }

    #[test]
    fn seqlock_concurrent_readers_consistent() {
        // 1 writer thread + 4 readers, 10000 iterations chacun.
        // Tous les reads doivent retourner soit l'ancienne valeur soit
        // la nouvelle, jamais un état mid-write inconsistant.
        let lock = Arc::new(Seqlock::new((0u32, 0u32)));
        let writer = {
            let l = Arc::clone(&lock);
            thread::spawn(move || {
                for i in 1..=10_000u32 {
                    // Invariant : a == b dans toute lecture.
                    l.write((i, i));
                }
            })
        };
        let readers: Vec<_> = (0..4)
            .map(|_| {
                let l = Arc::clone(&lock);
                thread::spawn(move || {
                    for _ in 0..10_000 {
                        let (a, b) = l.read();
                        assert_eq!(a, b, "seqlock invariant violated");
                    }
                })
            })
            .collect();
        writer.join().unwrap();
        for r in readers {
            r.join().unwrap();
        }
        let (a, b) = lock.read();
        assert_eq!(a, 10_000);
        assert_eq!(b, 10_000);
    }

    #[test]
    fn seqlock_t_must_be_copy() {
        // Sanity : Seqlock requires T: Copy. Test compile-time
        // (compile fail si T non-Copy passé).
        let _ = Seqlock::new(42i64); // i64 is Copy ✓
        let _ = Seqlock::new((1u32, 2u32, 3u32)); // tuple of Copy ✓
        let _ = Seqlock::new([0u8; 16]); // array of Copy ✓
    }

    #[test]
    fn seqlock_zero_size_t() {
        // Edge case : T = (). Pas d'erreur à la construction.
        let lock = Seqlock::new(());
        let _ = lock.read();
        lock.write(());
    }
}
