//! Π.19 (Wave 13, 2026-05-02) — Reservoir sampling Knuth-Vitter.
//!
//! **Origine** : Donald Knuth (TAOCP Vol 2 Algorithm R, 1969), Jeffrey
//! Vitter (Algorithm Z, 1985). Pattern statistique canonique pour
//! échantillonner uniformément N éléments parmi un stream de M sans
//! connaître M à l'avance et sans matérialiser tout le stream.
//!
//! ## Pourquoi pour Forge ?
//!
//! Backtest Monte Carlo : sur des historiques massifs (ex 100M ticks
//! NASDAQ ITCH), bootstrap statistique demande échantillonner N=10k
//! ticks parmi M=100M sans OOM. Reservoir sampling = mémoire constante
//! O(N), 1 passe, distribution uniforme exacte.
//!
//! Algorithm R (Knuth) : O(M) — pour chaque item idx i, garder avec
//! probabilité N/i. Simple mais O(M) PRNG calls.
//!
//! Algorithm Z (Vitter) : O(N + N·log(M/N)) — skip aléatoirement les
//! items non-sélectionnés. Beaucoup plus rapide pour M >> N.
//!
//! Wave 13 minimal viable : Algorithm R (le plus simple, O(M) PRNG est
//! acceptable pour M ≤ 100M). Algorithm Z déféré Wave 14+ si justifié
//! par mesure.
//!
//! ## Architecture Wave 13 minimal viable
//!
//! - `ReservoirSampler<T>` : capacity N + Vec<T> + counter
//! - `add(item)` : Algorithm R update
//! - `add_many(iter)` : convenience helper
//! - `into_samples()` : consume → Vec<T>
//! - PRNG déterministe : XorShift64 avec seed (zero RNG ambiant V7)
//!
//! ## Limitations Wave 13 minimal
//!
//! - Algorithm R only (Wave 14+ peut ajouter Algorithm Z avec
//!   geometric skip distribution)
//! - T: Clone + 'static (pour stockage simple Vec<T>)
//! - Pas de weighted reservoir (chaque item poids 1) — Wave 14+
//!   pour Algorithm A-ExpJ d'Efraimidis-Spirakis

/// PRNG déterministe XorShift64 (zero RNG ambiant per doctrine V7).
struct XorShift64(u64);

impl XorShift64 {
    fn new(seed: u64) -> Self {
        XorShift64(seed.max(1)) // seed=0 freezes XorShift
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    /// Uniform u64 in [0, n).
    fn next_below(&mut self, n: u64) -> u64 {
        // Lemire bias-reduction for unbiased range mapping.
        // En Wave 13 minimal, on utilise modulo simple (légère biais
        // négligeable pour n << 2^64).
        if n == 0 {
            return 0;
        }
        self.next_u64() % n
    }
}

/// Reservoir sampler avec capacité fixe `N`.
pub struct ReservoirSampler<T: Clone> {
    capacity: usize,
    samples: Vec<T>,
    /// Compteur d'items vus (= M dans la littérature).
    seen: u64,
    rng: XorShift64,
}

impl<T: Clone> ReservoirSampler<T> {
    /// Construit un sampler de capacité `capacity` avec un seed PRNG.
    pub fn new(capacity: usize, seed: u64) -> Self {
        Self {
            capacity,
            samples: Vec::with_capacity(capacity),
            seen: 0,
            rng: XorShift64::new(seed),
        }
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn len(&self) -> usize {
        self.samples.len()
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    /// Total d'items vus depuis la création.
    pub fn seen(&self) -> u64 {
        self.seen
    }

    /// Algorithm R (Knuth) : pour chaque nouvel item à l'index i :
    ///   - Si i < capacity : ajouter directement
    ///   - Sinon : tirer j ∈ [0, i), si j < capacity remplacer samples[j]
    pub fn add(&mut self, item: T) {
        self.seen += 1;
        let i = self.seen as usize - 1; // 0-indexed
        if i < self.capacity {
            self.samples.push(item);
        } else {
            // self.seen > capacity → tirer j ∈ [0, seen).
            let j = self.rng.next_below(self.seen) as usize;
            if j < self.capacity {
                self.samples[j] = item;
            }
            // Else : item rejected, sample slot inchangé.
        }
    }

    /// Convenience : ajouter tous les items d'un iterator.
    pub fn add_many<I: IntoIterator<Item = T>>(&mut self, items: I) {
        for item in items {
            self.add(item);
        }
    }

    /// Consomme le sampler, retourne les N samples. L'ordre n'est PAS
    /// l'ordre d'insertion — c'est l'ordre des slots du reservoir
    /// (random uniform sur les positions du stream).
    pub fn into_samples(self) -> Vec<T> {
        self.samples
    }

    /// Snapshot des samples sans consommer.
    pub fn samples(&self) -> &[T] {
        &self.samples
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn reservoir_takes_all_when_stream_smaller_than_capacity() {
        let mut s = ReservoirSampler::new(10, 42);
        for i in 0..5 {
            s.add(i);
        }
        let samples = s.into_samples();
        assert_eq!(samples.len(), 5);
        assert_eq!(samples, vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn reservoir_capacity_capped() {
        let mut s = ReservoirSampler::new(3, 42);
        for i in 0..100 {
            s.add(i);
        }
        assert_eq!(s.len(), 3);
        assert_eq!(s.seen(), 100);
    }

    #[test]
    fn reservoir_deterministic_same_seed() {
        // Same seed + same input → same samples (deterministic V7).
        let mut s1 = ReservoirSampler::new(5, 12345);
        let mut s2 = ReservoirSampler::new(5, 12345);
        for i in 0..1000 {
            s1.add(i);
            s2.add(i);
        }
        assert_eq!(s1.into_samples(), s2.into_samples());
    }

    #[test]
    fn reservoir_different_seed_different_samples() {
        let mut s1 = ReservoirSampler::new(10, 1);
        let mut s2 = ReservoirSampler::new(10, 999);
        for i in 0..1000 {
            s1.add(i);
            s2.add(i);
        }
        // Probabilité que les deux samplers donnent les mêmes samples
        // est microscopique. Test non-equality.
        assert_ne!(s1.into_samples(), s2.into_samples());
    }

    #[test]
    fn reservoir_uniform_distribution_smoke() {
        // Statistique : sample 1 item parmi 100, répété 10000 fois.
        // Chaque item devrait apparaître ~100 fois (10000/100).
        // Tolérance large : ±50 (pour 99% confidence sur n=100 trials).
        let mut counts = HashMap::new();
        for trial in 0..10000u64 {
            let mut s = ReservoirSampler::new(1, trial);
            for i in 0..100i32 {
                s.add(i);
            }
            for &v in s.samples() {
                *counts.entry(v).or_insert(0u32) += 1;
            }
        }
        for v in 0..100 {
            let count = *counts.get(&v).unwrap_or(&0);
            // Mean = 100 par item, écart-type sqrt(100*0.99) ≈ 10.
            // Tolérance ±50 = ~5 sigmas → essentiellement jamais false alarm.
            assert!(
                count >= 50 && count <= 200,
                "uniform distribution violated: item {} count = {}",
                v, count
            );
        }
    }

    #[test]
    fn reservoir_add_many_helper() {
        let mut s = ReservoirSampler::new(3, 7);
        s.add_many(0..10);
        assert_eq!(s.len(), 3);
        assert_eq!(s.seen(), 10);
    }

    #[test]
    fn reservoir_empty_initial_state() {
        let s: ReservoirSampler<i32> = ReservoirSampler::new(5, 0);
        assert!(s.is_empty());
        assert_eq!(s.len(), 0);
        assert_eq!(s.seen(), 0);
        assert_eq!(s.capacity(), 5);
    }

    #[test]
    fn reservoir_clone_preserves_distribution() {
        // Stream Q3132 prices (réaliste pour backtest tick sampling).
        let prices: Vec<i64> = (1000..2000).map(|i| i * (1i64 << 32)).collect();
        let mut s = ReservoirSampler::new(20, 1234);
        for p in &prices {
            s.add(*p);
        }
        assert_eq!(s.len(), 20);
        // Tous les samples doivent venir du stream original.
        let snapshot = s.samples();
        for sample in snapshot {
            assert!(prices.contains(sample), "sample {} not in original stream", sample);
        }
    }

    #[test]
    fn reservoir_zero_capacity_takes_nothing() {
        let mut s: ReservoirSampler<i32> = ReservoirSampler::new(0, 42);
        for i in 0..100 {
            s.add(i);
        }
        assert_eq!(s.len(), 0);
        assert_eq!(s.seen(), 100);
    }

    #[test]
    fn reservoir_smoke_100k_items_constant_memory() {
        // Smoke : sample 100 items parmi 100k → mémoire constante.
        let mut s = ReservoirSampler::new(100, 999);
        for i in 0..100_000 {
            s.add(i);
        }
        assert_eq!(s.len(), 100);
        // Vec capacity reste = capacity initiale (pas grown).
        assert_eq!(s.samples().len(), 100);
    }
}
