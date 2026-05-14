//! Ω-3 — Bench tri-frontal : f32 vs Posit16 vs Rational sur le mur d'associativité.
//!
//! Reproduit le workload `float_assoc_wall.rs` (matmul, deux interpréteurs
//! honnêtes : LTR et Pairwise) avec les TROIS arithmétiques et compare le
//! nombre de positions divergentes byte-pour-byte.
//!
//! Résultat attendu :
//!   * **f32**       : divergence systématique sur des matmuls ≥ 64 (mur ouvert)
//!   * **Posit16**   : améliore la résistance mais N'élimine PAS la divergence
//!     (les posits ne sont pas exactement associatifs, juste *plus* associatifs)
//!   * **Rational**  : zéro divergence, mur fermé (Ω-3.0 promesse)
//!
//! Honnêteté : Posit16 ne ferme pas le mur — il le déplace. Le seul type
//! qui le ferme est Rational. La doctrine Ω-3.3 (migration kasm/tensor)
//! devra trancher : où on veut perf+approx (Posit16) vs où on veut
//! déterminisme bit-exact (Rational).

use scan::numeric::{Numeric, Posit16, Rational};
use scan::Hash;

const FIXED_DENOM: i128 = 1000;

// ---------------------------------------------------------------------------
// Génération de données — même seeds xorshift que float_assoc_wall.rs.
// ---------------------------------------------------------------------------

fn pseudo_data_f32(seed: u64, n: usize) -> Vec<f32> {
    let mut s = seed.max(1);
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        let bits = (s & 0x7fff_ffff) as u32;
        let f = (bits as f32) / (i32::MAX as f32);
        out.push(f * 2.0 - 1.0);
    }
    out
}

fn pseudo_data_posit(seed: u64, n: usize) -> Vec<Posit16> {
    pseudo_data_f32(seed, n).into_iter().map(|f| Posit16::from_f64(f as f64)).collect()
}

fn pseudo_data_rational(seed: u64, n: usize) -> Vec<Rational> {
    let mut s = seed.max(1);
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        let bits = (s & 0x7fff_ffff) as i64;
        let num = (bits as i128) % (FIXED_DENOM + 1);
        let signed_num = if s & 0x8000_0000 != 0 { -num } else { num };
        out.push(Rational::new(signed_num, FIXED_DENOM).unwrap());
    }
    out
}

// ---------------------------------------------------------------------------
// Matmul génériques (LTR et Pairwise) sur trois types numériques.
// ---------------------------------------------------------------------------

trait Sum: Sized + Copy {
    fn sum_zero() -> Self;
    fn sum_add(self, other: Self) -> Self;
    fn sum_mul(self, other: Self) -> Self;
    fn sum_to_bytes(self) -> Vec<u8>;
}

impl Sum for f32 {
    fn sum_zero() -> Self {
        0.0
    }
    fn sum_add(self, other: Self) -> Self {
        self + other
    }
    fn sum_mul(self, other: Self) -> Self {
        self * other
    }
    fn sum_to_bytes(self) -> Vec<u8> {
        self.to_le_bytes().to_vec()
    }
}

impl Sum for Posit16 {
    fn sum_zero() -> Self {
        Posit16::ZERO
    }
    fn sum_add(self, other: Self) -> Self {
        self.checked_add(other).unwrap_or(Posit16::NAR)
    }
    fn sum_mul(self, other: Self) -> Self {
        self.checked_mul(other).unwrap_or(Posit16::NAR)
    }
    fn sum_to_bytes(self) -> Vec<u8> {
        self.to_canonical_bytes()
    }
}

impl Sum for Rational {
    fn sum_zero() -> Self {
        Rational::zero()
    }
    fn sum_add(self, other: Self) -> Self {
        self.checked_add(other).expect("rational add overflow")
    }
    fn sum_mul(self, other: Self) -> Self {
        self.checked_mul(other).expect("rational mul overflow")
    }
    fn sum_to_bytes(self) -> Vec<u8> {
        self.to_canonical_bytes()
    }
}

fn matmul_ltr<T: Sum>(m: usize, k: usize, n: usize, lhs: &[T], rhs: &[T]) -> Vec<T> {
    let mut out = vec![T::sum_zero(); m * n];
    for row in 0..m {
        for col in 0..n {
            let mut acc = T::sum_zero();
            for kk in 0..k {
                let p = lhs[row * k + kk].sum_mul(rhs[kk * n + col]);
                acc = acc.sum_add(p);
            }
            out[row * n + col] = acc;
        }
    }
    out
}

fn pair_tree_sum<T: Sum>(mut buf: Vec<T>) -> T {
    while buf.len() > 1 {
        let mut next = Vec::with_capacity((buf.len() + 1) / 2);
        let mut i = 0;
        while i + 1 < buf.len() {
            next.push(buf[i].sum_add(buf[i + 1]));
            i += 2;
        }
        if i < buf.len() {
            next.push(buf[i]);
        }
        buf = next;
    }
    *buf.first().unwrap_or(&T::sum_zero())
}

fn matmul_pairwise<T: Sum>(m: usize, k: usize, n: usize, lhs: &[T], rhs: &[T]) -> Vec<T> {
    let mut out = vec![T::sum_zero(); m * n];
    let mut buf = vec![T::sum_zero(); k];
    for row in 0..m {
        for col in 0..n {
            for kk in 0..k {
                buf[kk] = lhs[row * k + kk].sum_mul(rhs[kk * n + col]);
            }
            out[row * n + col] = pair_tree_sum(buf.clone());
        }
    }
    out
}

fn output_hash<T: Sum>(values: &[T]) -> Hash {
    let mut bytes = Vec::with_capacity(values.len() * 16);
    for v in values {
        bytes.extend_from_slice(&v.sum_to_bytes());
    }
    Hash::for_blob(&bytes)
}

fn count_byte_diffs<T: Sum>(a: &[T], b: &[T]) -> usize {
    a.iter().zip(b.iter()).filter(|(x, y)| x.sum_to_bytes() != y.sum_to_bytes()).count()
}

// ---------------------------------------------------------------------------
// Comparaison sur un cas de matmul.
// ---------------------------------------------------------------------------

fn run_case(label: &str, m: usize, k: usize, n: usize, seed_l: u64, seed_r: u64) {
    println!("=== {label} ({m}x{k} · {k}x{n}) ===");

    // f32
    {
        let lhs = pseudo_data_f32(seed_l, m * k);
        let rhs = pseudo_data_f32(seed_r, k * n);
        let ltr = matmul_ltr(m, k, n, &lhs, &rhs);
        let pair = matmul_pairwise(m, k, n, &lhs, &rhs);
        let bd = count_byte_diffs(&ltr, &pair);
        let h_ltr = output_hash(&ltr);
        let h_pair = output_hash(&pair);
        println!(
            "  f32      : bytes-differ {:4}/{:4}  hash {} vs {}  →  {}",
            bd,
            ltr.len(),
            &h_ltr.as_hex()[..12],
            &h_pair.as_hex()[..12],
            if h_ltr == h_pair { "✓ same" } else { "✗ wall OPEN" },
        );
    }

    // Posit16
    {
        let lhs = pseudo_data_posit(seed_l, m * k);
        let rhs = pseudo_data_posit(seed_r, k * n);
        let ltr = matmul_ltr(m, k, n, &lhs, &rhs);
        let pair = matmul_pairwise(m, k, n, &lhs, &rhs);
        let bd = count_byte_diffs(&ltr, &pair);
        let h_ltr = output_hash(&ltr);
        let h_pair = output_hash(&pair);
        println!(
            "  Posit16  : bytes-differ {:4}/{:4}  hash {} vs {}  →  {}",
            bd,
            ltr.len(),
            &h_ltr.as_hex()[..12],
            &h_pair.as_hex()[..12],
            if h_ltr == h_pair { "✓ same" } else { "✗ wall OPEN" },
        );
    }

    // Rational
    {
        let lhs = pseudo_data_rational(seed_l, m * k);
        let rhs = pseudo_data_rational(seed_r, k * n);
        let ltr = matmul_ltr(m, k, n, &lhs, &rhs);
        let pair = matmul_pairwise(m, k, n, &lhs, &rhs);
        let bd = count_byte_diffs(&ltr, &pair);
        let h_ltr = output_hash(&ltr);
        let h_pair = output_hash(&pair);
        println!(
            "  Rational : bytes-differ {:4}/{:4}  hash {} vs {}  →  {}",
            bd,
            ltr.len(),
            &h_ltr.as_hex()[..12],
            &h_pair.as_hex()[..12],
            if h_ltr == h_pair { "✓ same — wall CLOSED" } else { "✗ wall OPEN" },
        );
    }

    println!();
}

fn main() {
    println!("Ω-3 bench tri-frontal : f32 vs Posit16 vs Rational");
    println!("Trois interpréteurs honnêtes (LTR vs Pairwise) sur le même workload.");
    println!("Le verdict idéal Ω-3 : 0 byte-differ, même hash.\n");

    run_case("tiny", 4, 8, 4, 0xdead_beef, 0xcafe_babe);
    run_case("medium", 16, 64, 16, 0x1234_5678, 0x9abc_def0);
    run_case("ML-realistic", 8, 512, 8, 0x0fff_eeee, 0xddee_aaff);

    println!("=== conclusion ===");
    println!("• f32       : mur ouvert, divergences fréquentes.");
    println!("• Posit16   : meilleur que f32, mais le mur reste ouvert (les posits");
    println!("              sont *plus* associatifs, pas exactement associatifs).");
    println!("• Rational  : mur fermé, hashes identiques garantis (Ω-3.0).");
    println!();
    println!("Posit16 = compromis perf+approx ; Rational = déterminisme bit-exact.");
    println!("La migration Ω-3.3 devra arbitrer par couche.");
}
