use scan::numeric::{Associative, BitStable, Numeric, Rational};

#[derive(Clone, Copy)]
struct XorShift64 {
    state: u64,
}

impl XorShift64 {
    fn new(seed: u64) -> Self {
        Self { state: seed.max(1) }
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    fn range_i128(&mut self, lo: i128, hi: i128) -> i128 {
        debug_assert!(lo <= hi);
        let span = (hi - lo + 1) as u128;
        lo + (self.next_u64() as u128 % span) as i128
    }

    fn rational(&mut self) -> Rational {
        let num = self.range_i128(-1000, 1000);
        let denom = self.range_i128(1, 1000);
        Rational::new(num, denom).unwrap()
    }

}

fn bytes(v: Rational) -> Vec<u8> {
    v.to_canonical_bytes()
}

#[test]
fn rational_associativity_addition() {
    assert!(Rational::add_is_exact());
    let mut rng = XorShift64::new(0xa551_a551);
    let mut checked = 0usize;
    for _ in 0..10_000 {
        let a = rng.rational();
        let b = rng.rational();
        let c = rng.rational();
        let lhs = match a.checked_add(b).and_then(|x| x.checked_add(c)) {
            Some(v) => v,
            None => continue,
        };
        let rhs = match b.checked_add(c).and_then(|x| a.checked_add(x)) {
            Some(v) => v,
            None => continue,
        };
        assert_eq!(bytes(lhs), bytes(rhs));
        checked += 1;
    }
    assert!(checked > 9_900);
}

#[test]
fn rational_associativity_multiplication() {
    assert!(Rational::mul_is_exact());
    let mut rng = XorShift64::new(0xb662_b662);
    let mut checked = 0usize;
    for _ in 0..10_000 {
        let a = rng.rational();
        let b = rng.rational();
        let c = rng.rational();
        let lhs = match a.checked_mul(b).and_then(|x| x.checked_mul(c)) {
            Some(v) => v,
            None => continue,
        };
        let rhs = match b.checked_mul(c).and_then(|x| a.checked_mul(x)) {
            Some(v) => v,
            None => continue,
        };
        assert_eq!(bytes(lhs), bytes(rhs));
        checked += 1;
    }
    assert!(checked > 9_900);
}

#[test]
fn rational_commutativity_add() {
    let mut rng = XorShift64::new(0xc773_c773);
    for _ in 0..10_000 {
        let a = rng.rational();
        let b = rng.rational();
        let lhs = match a.checked_add(b) {
            Some(v) => v,
            None => continue,
        };
        let rhs = match b.checked_add(a) {
            Some(v) => v,
            None => continue,
        };
        assert_eq!(bytes(lhs), bytes(rhs));
    }
}

#[test]
fn rational_commutativity_mul() {
    let mut rng = XorShift64::new(0xd884_d884);
    for _ in 0..10_000 {
        let a = rng.rational();
        let b = rng.rational();
        let lhs = match a.checked_mul(b) {
            Some(v) => v,
            None => continue,
        };
        let rhs = match b.checked_mul(a) {
            Some(v) => v,
            None => continue,
        };
        assert_eq!(bytes(lhs), bytes(rhs));
    }
}

#[test]
fn rational_distributivity() {
    let mut rng = XorShift64::new(0xe995_e995);
    let mut checked = 0usize;
    for _ in 0..10_000 {
        let a = rng.rational();
        let b = rng.rational();
        let c = rng.rational();
        let lhs = match b.checked_add(c).and_then(|x| a.checked_mul(x)) {
            Some(v) => v,
            None => continue,
        };
        let rhs = match a
            .checked_mul(b)
            .and_then(|ab| a.checked_mul(c).and_then(|ac| ab.checked_add(ac)))
        {
            Some(v) => v,
            None => continue,
        };
        assert_eq!(bytes(lhs), bytes(rhs));
        checked += 1;
    }
    assert!(checked > 9_900);
}

#[test]
fn rational_additive_identity() {
    let mut rng = XorShift64::new(0xfaa6_faa6);
    let zero = Rational::zero();
    for _ in 0..10_000 {
        let a = rng.rational();
        let lhs = a.checked_add(zero).unwrap();
        assert_eq!(bytes(lhs), bytes(a));
    }
}

#[test]
fn rational_multiplicative_identity() {
    let mut rng = XorShift64::new(0x0bb7_0bb7);
    let one = Rational::one();
    for _ in 0..10_000 {
        let a = rng.rational();
        let lhs = a.checked_mul(one).unwrap();
        assert_eq!(bytes(lhs), bytes(a));
    }
}

#[test]
fn rational_canonical_form_unique() {
    for n in -250i128..=250 {
        for d in 1i128..=50 {
            let a = Rational::new(n, d).unwrap();
            let b = Rational::new(n * 12, d * 12).unwrap();
            let c = Rational::new(-n * 15, -d * 15).unwrap();
            assert_eq!(a.to_canonical_bytes(), b.to_canonical_bytes());
            assert_eq!(a.to_canonical_bytes(), c.to_canonical_bytes());
        }
    }
}

#[test]
fn rational_bitstable_roundtrip() {
    let mut rng = XorShift64::new(0x1cc8_1cc8);
    for _ in 0..10_000 {
        let a = rng.rational();
        let round = Rational::from_canonical_bytes(&a.to_canonical_bytes());
        assert_eq!(round, Some(a));
    }
}

#[test]
fn rational_hash_equiv_bytes_iff_value() {
    let mut rng = XorShift64::new(0x2dd9_2dd9);
    for i in 0..10_000 {
        let a = rng.rational();
        let b = if i % 2 == 0 {
            Rational::new(a.num() * 7, a.denom() * 7).unwrap()
        } else {
            rng.rational()
        };
        assert_eq!(a == b, a.to_canonical_bytes() == b.to_canonical_bytes());
    }
}

#[test]
fn rational_i128_boundaries_stay_safe_when_checked() {
    let max = Rational::new(i128::MAX, 1).unwrap();
    let min = Rational::new(i128::MIN + 1, 1).unwrap();
    assert_eq!(Rational::from_canonical_bytes(&max.to_canonical_bytes()), Some(max));
    assert_eq!(Rational::from_canonical_bytes(&min.to_canonical_bytes()), Some(min));

    let near = Rational::new(i128::MAX - 1, 1).unwrap();
    let one = Rational::one();
    assert_eq!(near.checked_add(one), Some(max));

    let safe_a = Rational::new(1i128 << 60, 1).unwrap();
    let safe_b = Rational::new(1i128 << 60, 1).unwrap();
    assert_eq!(safe_a.checked_mul(safe_b).unwrap().num(), 1i128 << 120);
}

#[test]
fn division_by_zero_returns_none_and_never_panics() {
    let one = Rational::one();
    let zero = Rational::zero();
    assert!(Rational::new(1, 0).is_none());
    assert!(one.checked_div(zero).is_none());
}

#[test]
fn negative_denominators_are_normalized_positive() {
    let mut rng = XorShift64::new(0x722e_722e);
    for _ in 0..10_000 {
        let num = rng.range_i128(-1000, 1000);
        let denom = rng.range_i128(1, 1000);
        let a = Rational::new(num, -denom).unwrap();
        let b = Rational::new(-num, denom).unwrap();
        assert!(a.denom() > 0);
        assert_eq!(a.to_canonical_bytes(), b.to_canonical_bytes());
    }
}
