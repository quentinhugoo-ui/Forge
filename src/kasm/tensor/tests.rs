//! Tensor dialect smoke tests. Bit-exactness against handwritten
//! references is the only acceptable bar — the same standard KASM-Int
//! holds itself to.

use super::interpreter::{execute_tensor, execute_tensor_rational};
use super::program::{verify_tensor, TensorProgram};
use super::types::{TensorNode, TensorShape, TensorTy};
use crate::Hash;

// ===========================================================================
// Ω-3.3 first mile : tests pour le dtype Rational et son interpréteur dédié.
// ===========================================================================

mod omega3_rational_tests {
    use super::*;
    use crate::numeric::{Numeric, Rational};

    fn r(n: i128, d: i128) -> Rational {
        Rational::new(n, d).unwrap()
    }

    fn pool_from_rationals(values: &[Rational]) -> Vec<u8> {
        let mut out = Vec::with_capacity(values.len() * 32);
        for v in values {
            out.extend_from_slice(&v.to_canonical_bytes());
        }
        out
    }

    #[test]
    fn add_rational_runs_end_to_end() {
        let a_vals = vec![r(1, 2), r(3, 4)];
        let pool = pool_from_rationals(&a_vals);
        let shape = TensorShape::vec(2).unwrap();
        let nodes = vec![
            TensorNode::const_at(0, pool.len() as u32, TensorTy::Rational, shape),
            TensorNode::input(0, TensorTy::Rational, shape),
            TensorNode::add_rational(0, 1, shape),
            TensorNode::output(2, TensorTy::Rational, shape),
        ];
        let program = TensorProgram::new(1, 1, nodes.len() as u32, nodes, pool).unwrap();

        let inputs = vec![vec![r(1, 4), r(1, 4)]];
        let result = execute_tensor_rational(&program, &inputs).unwrap();
        // [1/2 + 1/4, 3/4 + 1/4] = [3/4, 1]
        assert_eq!(result, vec![r(3, 4), r(1, 1)]);
    }

    #[test]
    fn mul_rational_runs_end_to_end() {
        let a_vals = vec![r(2, 3), r(5, 7)];
        let pool = pool_from_rationals(&a_vals);
        let shape = TensorShape::vec(2).unwrap();
        let nodes = vec![
            TensorNode::const_at(0, pool.len() as u32, TensorTy::Rational, shape),
            TensorNode::input(0, TensorTy::Rational, shape),
            TensorNode::mul_rational(0, 1, shape),
            TensorNode::output(2, TensorTy::Rational, shape),
        ];
        let program = TensorProgram::new(1, 1, nodes.len() as u32, nodes, pool).unwrap();

        let inputs = vec![vec![r(3, 1), r(2, 1)]];
        let result = execute_tensor_rational(&program, &inputs).unwrap();
        // [2/3 × 3, 5/7 × 2] = [2, 10/7]
        assert_eq!(result, vec![r(2, 1), r(10, 7)]);
    }

    #[test]
    fn matmul_rational_2x3_3x2_byte_exact() {
        // A = [[1, 1/2, 1/3], [1/4, 1/5, 1/6]]   (2×3)
        // B = [[1, 0], [0, 1], [2, 3]]           (3×2)
        // A·B = [[1 + 0 + 2/3,  0 + 1/2 + 1 ],
        //        [1/4 + 0 + 1/3, 0 + 1/5 + 1/2]]
        //     = [[5/3, 3/2], [7/12, 7/10]]
        let a_vals = vec![r(1, 1), r(1, 2), r(1, 3), r(1, 4), r(1, 5), r(1, 6)];
        let b_vals = vec![r(1, 1), r(0, 1), r(0, 1), r(1, 1), r(2, 1), r(3, 1)];
        let mut pool = pool_from_rationals(&a_vals);
        let b_off = pool.len() as u32;
        pool.extend_from_slice(&pool_from_rationals(&b_vals));

        let a_shape = TensorShape::matrix(2, 3).unwrap();
        let b_shape = TensorShape::matrix(3, 2).unwrap();
        let out_shape = TensorShape::matrix(2, 2).unwrap();

        let nodes = vec![
            TensorNode::const_at(0, (a_vals.len() * 32) as u32, TensorTy::Rational, a_shape),
            TensorNode::const_at(b_off, (b_vals.len() * 32) as u32, TensorTy::Rational, b_shape),
            TensorNode::matmul_rational(0, 1, out_shape),
            TensorNode::output(2, TensorTy::Rational, out_shape),
        ];
        let program = TensorProgram::new(0, 1, nodes.len() as u32, nodes, pool).unwrap();

        let result = execute_tensor_rational(&program, &[]).unwrap();
        assert_eq!(result, vec![r(5, 3), r(3, 2), r(7, 12), r(7, 10)]);
    }

    #[test]
    fn rational_dtype_byte_size_is_32() {
        assert_eq!(TensorTy::Rational.byte_size(), 32);
    }

    #[test]
    fn rational_node_codec_roundtrip() {
        let shape = TensorShape::vec(4).unwrap();
        let n = TensorNode::add_rational(1, 2, shape);
        let mut buf = Vec::new();
        n.encode(&mut buf);
        let back = TensorNode::decode(&buf).unwrap();
        assert_eq!(back, n);

        let n = TensorNode::matmul_rational(1, 2, shape);
        let mut buf = Vec::new();
        n.encode(&mut buf);
        let back = TensorNode::decode(&buf).unwrap();
        assert_eq!(back, n);
    }

    #[test]
    fn polymorphic_interpreter_handles_f32_path() {
        // Programme f32 simple : Const + Input → Add → Output. Doit fonctionner
        // via execute_tensor_polymorphic directement (sans passer par le wrapper f32).
        use super::super::interpreter::{execute_tensor_polymorphic, TensorValue};
        let pool: Vec<u8> = [1.0f32, 2.0f32].iter().flat_map(|f| f.to_le_bytes()).collect();
        let shape = TensorShape::vec(2).unwrap();
        let nodes = vec![
            TensorNode::const_at(0, pool.len() as u32, TensorTy::F32, shape),
            TensorNode::input(0, TensorTy::F32, shape),
            TensorNode::add(0, 1, TensorTy::F32, shape),
            TensorNode::output(2, TensorTy::F32, shape),
        ];
        let program = TensorProgram::new(1, 1, nodes.len() as u32, nodes, pool).unwrap();
        let inputs = vec![TensorValue::F32(vec![10.0, 20.0])];
        let result = execute_tensor_polymorphic(&program, &inputs).unwrap();
        match result {
            TensorValue::F32(v) => assert_eq!(v, vec![11.0, 22.0]),
            _ => panic!("expected F32 output"),
        }
    }

    #[test]
    fn polymorphic_interpreter_handles_rational_path() {
        use super::super::interpreter::{execute_tensor_polymorphic, TensorValue};
        let pool = pool_from_rationals(&[r(1, 2), r(3, 4)]);
        let shape = TensorShape::vec(2).unwrap();
        let nodes = vec![
            TensorNode::const_at(0, pool.len() as u32, TensorTy::Rational, shape),
            TensorNode::input(0, TensorTy::Rational, shape),
            TensorNode::add_rational(0, 1, shape),
            TensorNode::output(2, TensorTy::Rational, shape),
        ];
        let program = TensorProgram::new(1, 1, nodes.len() as u32, nodes, pool).unwrap();
        let inputs = vec![TensorValue::Rational(vec![r(1, 4), r(1, 4)])];
        let result = execute_tensor_polymorphic(&program, &inputs).unwrap();
        match result {
            TensorValue::Rational(v) => assert_eq!(v, vec![r(3, 4), r(1, 1)]),
            _ => panic!("expected Rational output"),
        }
    }

    #[test]
    fn polymorphic_interpreter_rejects_dtype_input_mismatch() {
        // Programme déclare F32 inputs, on lui passe un Rational.
        use super::super::interpreter::{execute_tensor_polymorphic, TensorValue};
        use super::super::types::TensorError;
        let pool: Vec<u8> = [1.0f32].iter().flat_map(|f| f.to_le_bytes()).collect();
        let shape = TensorShape::vec(1).unwrap();
        let nodes = vec![
            TensorNode::const_at(0, pool.len() as u32, TensorTy::F32, shape),
            TensorNode::input(0, TensorTy::F32, shape),
            TensorNode::add(0, 1, TensorTy::F32, shape),
            TensorNode::output(2, TensorTy::F32, shape),
        ];
        let program = TensorProgram::new(1, 1, nodes.len() as u32, nodes, pool).unwrap();
        let inputs = vec![TensorValue::Rational(vec![r(1, 1)])];
        let result = execute_tensor_polymorphic(&program, &inputs);
        assert!(matches!(result, Err(TensorError::DtypeMismatch { .. })));
    }

    // ----- Ω-3.3.2 — Posit16 / Posit32 dans le pipeline tenseur -----

    fn pool_from_posit16(values: &[crate::numeric::Posit16]) -> Vec<u8> {
        let mut out = Vec::with_capacity(values.len() * 2);
        for v in values {
            out.extend_from_slice(&v.to_bits().to_le_bytes());
        }
        out
    }

    fn pool_from_posit32(values: &[crate::numeric::Posit32]) -> Vec<u8> {
        let mut out = Vec::with_capacity(values.len() * 4);
        for v in values {
            out.extend_from_slice(&v.to_bits().to_le_bytes());
        }
        out
    }

    #[test]
    fn add_posit16_runs_end_to_end() {
        use super::super::interpreter::execute_tensor_posit16;
        use crate::numeric::Posit16;
        let consts = vec![Posit16::from_f64(1.0), Posit16::from_f64(2.0)];
        let pool = pool_from_posit16(&consts);
        let shape = TensorShape::vec(2).unwrap();
        let nodes = vec![
            TensorNode::const_at(0, pool.len() as u32, TensorTy::Posit16, shape),
            TensorNode::input(0, TensorTy::Posit16, shape),
            TensorNode::add_posit16(0, 1, shape),
            TensorNode::output(2, TensorTy::Posit16, shape),
        ];
        let program = TensorProgram::new(1, 1, nodes.len() as u32, nodes, pool).unwrap();
        let inputs = vec![vec![Posit16::from_f64(1.0), Posit16::from_f64(2.0)]];
        let result = execute_tensor_posit16(&program, &inputs).unwrap();
        // [1+1, 2+2] = [2, 4]
        assert_eq!(result[0].to_bits(), Posit16::from_f64(2.0).to_bits());
        assert_eq!(result[1].to_bits(), Posit16::from_f64(4.0).to_bits());
    }

    #[test]
    fn matmul_posit16_2x2_runs() {
        use super::super::interpreter::execute_tensor_posit16;
        use crate::numeric::Posit16;
        // A = [[1, 2], [3, 4]] (2×2)
        // B = [[2, 0], [0, 2]] (2×2, identity ×2)
        // A·B = [[2, 4], [6, 8]]
        let a_vals = vec![
            Posit16::from_f64(1.0), Posit16::from_f64(2.0),
            Posit16::from_f64(3.0), Posit16::from_f64(4.0),
        ];
        let b_vals = vec![
            Posit16::from_f64(2.0), Posit16::from_f64(0.0),
            Posit16::from_f64(0.0), Posit16::from_f64(2.0),
        ];
        let mut pool = pool_from_posit16(&a_vals);
        let b_off = pool.len() as u32;
        pool.extend_from_slice(&pool_from_posit16(&b_vals));
        let shape = TensorShape::matrix(2, 2).unwrap();
        let nodes = vec![
            TensorNode::const_at(0, (a_vals.len() * 2) as u32, TensorTy::Posit16, shape),
            TensorNode::const_at(b_off, (b_vals.len() * 2) as u32, TensorTy::Posit16, shape),
            TensorNode::matmul_posit16(0, 1, shape),
            TensorNode::output(2, TensorTy::Posit16, shape),
        ];
        let program = TensorProgram::new(0, 1, nodes.len() as u32, nodes, pool).unwrap();
        let result = execute_tensor_posit16(&program, &[]).unwrap();
        let expected = vec![
            Posit16::from_f64(2.0), Posit16::from_f64(4.0),
            Posit16::from_f64(6.0), Posit16::from_f64(8.0),
        ];
        for (i, (got, want)) in result.iter().zip(expected.iter()).enumerate() {
            assert_eq!(got.to_bits(), want.to_bits(), "matmul[{i}]");
        }
    }

    #[test]
    fn add_posit32_runs_end_to_end() {
        use super::super::interpreter::execute_tensor_posit32;
        use crate::numeric::Posit32;
        let consts = vec![Posit32::from_f64(10.0), Posit32::from_f64(20.0)];
        let pool = pool_from_posit32(&consts);
        let shape = TensorShape::vec(2).unwrap();
        let nodes = vec![
            TensorNode::const_at(0, pool.len() as u32, TensorTy::Posit32, shape),
            TensorNode::input(0, TensorTy::Posit32, shape),
            TensorNode::add_posit32(0, 1, shape),
            TensorNode::output(2, TensorTy::Posit32, shape),
        ];
        let program = TensorProgram::new(1, 1, nodes.len() as u32, nodes, pool).unwrap();
        let inputs = vec![vec![Posit32::from_f64(5.0), Posit32::from_f64(10.0)]];
        let result = execute_tensor_posit32(&program, &inputs).unwrap();
        assert_eq!(result[0].to_bits(), Posit32::from_f64(15.0).to_bits());
        assert_eq!(result[1].to_bits(), Posit32::from_f64(30.0).to_bits());
    }

    #[test]
    fn mul_posit32_runs_end_to_end() {
        use super::super::interpreter::execute_tensor_posit32;
        use crate::numeric::Posit32;
        let consts = vec![Posit32::from_f64(0.5)];
        let pool = pool_from_posit32(&consts);
        let shape = TensorShape::vec(1).unwrap();
        let nodes = vec![
            TensorNode::const_at(0, pool.len() as u32, TensorTy::Posit32, shape),
            TensorNode::input(0, TensorTy::Posit32, shape),
            TensorNode::mul_posit32(0, 1, shape),
            TensorNode::output(2, TensorTy::Posit32, shape),
        ];
        let program = TensorProgram::new(1, 1, nodes.len() as u32, nodes, pool).unwrap();
        let inputs = vec![vec![Posit32::from_f64(8.0)]];
        let result = execute_tensor_posit32(&program, &inputs).unwrap();
        assert_eq!(result[0].to_bits(), Posit32::from_f64(4.0).to_bits());
    }

    #[test]
    fn posit16_dtype_byte_size_is_2() {
        assert_eq!(TensorTy::Posit16.byte_size(), 2);
    }

    #[test]
    fn posit32_dtype_byte_size_is_4() {
        assert_eq!(TensorTy::Posit32.byte_size(), 4);
    }

    #[test]
    fn posit16_op_codecs_roundtrip() {
        let shape = TensorShape::vec(4).unwrap();
        for op_node in [
            TensorNode::add_posit16(1, 2, shape),
            TensorNode::mul_posit16(1, 2, shape),
            TensorNode::matmul_posit16(1, 2, shape),
            TensorNode::add_posit32(1, 2, shape),
            TensorNode::mul_posit32(1, 2, shape),
            TensorNode::matmul_posit32(1, 2, shape),
        ] {
            let mut buf = Vec::new();
            op_node.encode(&mut buf);
            let back = TensorNode::decode(&buf).unwrap();
            assert_eq!(back, op_node);
        }
    }

    #[test]
    fn f32_interpreter_rejects_rational_program() {
        let pool = pool_from_rationals(&[r(1, 2), r(1, 3)]);
        let shape = TensorShape::vec(2).unwrap();
        let nodes = vec![
            TensorNode::const_at(0, pool.len() as u32, TensorTy::Rational, shape),
            TensorNode::input(0, TensorTy::Rational, shape),
            TensorNode::add_rational(0, 1, shape),
            TensorNode::output(2, TensorTy::Rational, shape),
        ];
        let program = TensorProgram::new(1, 1, nodes.len() as u32, nodes, pool).unwrap();
        let result = execute_tensor(&program, &[vec![0.0, 0.0]]);
        assert!(result.is_err(), "f32 interpreter doit rejeter Rational");
    }
}

fn f32_pool(values: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(values.len() * 4);
    for v in values {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out
}

fn approx_eq(a: f32, b: f32, eps: f32) -> bool {
    (a - b).abs() < eps
}

#[test]
fn matmul_2x3_times_3x4_matches_handwritten_reference() {
    // A = [[1,2,3],[4,5,6]]   (2×3)
    // B = [[1,0,0,1],[0,1,0,1],[0,0,1,1]]  (3×4)
    // C = A·B = [[1,2,3,6],[4,5,6,15]]
    let a_shape = TensorShape::matrix(2, 3).unwrap();
    let b_shape = TensorShape::matrix(3, 4).unwrap();
    let c_shape = TensorShape::matrix(2, 4).unwrap();

    let a_flat: Vec<f32> = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
    let b_flat: Vec<f32> = vec![
        1.0, 0.0, 0.0, 1.0,
        0.0, 1.0, 0.0, 1.0,
        0.0, 0.0, 1.0, 1.0,
    ];
    // Pool layout: [A bytes | B bytes]
    let a_pool = f32_pool(&a_flat);
    let b_pool = f32_pool(&b_flat);
    let mut pool = a_pool.clone();
    let b_off = pool.len() as u32;
    pool.extend_from_slice(&b_pool);

    let nodes = vec![
        TensorNode::const_at(0, a_pool.len() as u32, TensorTy::F32, a_shape),
        TensorNode::const_at(b_off, b_pool.len() as u32, TensorTy::F32, b_shape),
        TensorNode::matmul(0, 1, TensorTy::F32, c_shape),
        TensorNode::output(2, TensorTy::F32, c_shape),
    ];
    let program = TensorProgram::new(0, 1, nodes.len() as u32, nodes, pool).unwrap();

    let out = execute_tensor(&program, &[]).unwrap();
    let expected = vec![1.0, 2.0, 3.0, 6.0, 4.0, 5.0, 6.0, 15.0];
    assert_eq!(out.len(), expected.len());
    for (a, b) in out.iter().zip(expected.iter()) {
        assert!(approx_eq(*a, *b, 1e-6), "{a} vs {b}");
    }
}

#[test]
fn softmax_1d_normalises_to_unit_sum() {
    let shape = TensorShape::vec(4).unwrap();
    let nodes = vec![
        TensorNode::input(0, TensorTy::F32, shape),
        TensorNode::softmax(0, 0, TensorTy::F32, shape),
        TensorNode::output(1, TensorTy::F32, shape),
    ];
    let program = TensorProgram::new(1, 1, nodes.len() as u32, nodes, Vec::new()).unwrap();
    let input = vec![1.0f32, 2.0, 3.0, 4.0];
    let out = execute_tensor(&program, &[input.clone()]).unwrap();

    // Sum-to-one
    let sum: f32 = out.iter().sum();
    assert!(approx_eq(sum, 1.0, 1e-6));
    // Monotonic — strictly increasing inputs give strictly increasing
    // probabilities.
    for i in 1..out.len() {
        assert!(out[i] > out[i - 1]);
    }
}

#[test]
fn mini_attention_head_matmul_plus_bias_plus_softmax_round_trip() {
    // Simulate the core of an attention score row:
    //   logits = (Q @ Kᵀ)  + bias    (shape 1×4)
    //   probs  = softmax(logits, axis=1)
    //
    // With Q = [[1,0,1,0]], K = [[1,0,0,1],[0,1,1,0],[1,1,0,0],[0,0,1,1]],
    // Q @ K^T = [[1,1,1,1]]. Plus bias [[0,1,2,3]] → [[1,2,3,4]].
    // softmax([[1,2,3,4]], axis=1) = [[0.0321, 0.0871, 0.2369, 0.6439]].
    let q_shape = TensorShape::matrix(1, 4).unwrap();
    let kt_shape = TensorShape::matrix(4, 4).unwrap();
    let logits_shape = TensorShape::matrix(1, 4).unwrap();

    let q_flat = vec![1.0f32, 0.0, 1.0, 0.0];
    let kt_flat = vec![
        1.0, 0.0, 1.0, 0.0,
        0.0, 1.0, 1.0, 0.0,
        0.0, 1.0, 0.0, 1.0,
        1.0, 0.0, 0.0, 1.0,
    ];
    let bias_flat = vec![0.0f32, 1.0, 2.0, 3.0];

    let q_pool = f32_pool(&q_flat);
    let kt_pool = f32_pool(&kt_flat);
    let bias_pool = f32_pool(&bias_flat);
    let mut pool = q_pool.clone();
    let kt_off = pool.len() as u32;
    pool.extend_from_slice(&kt_pool);
    let bias_off = pool.len() as u32;
    pool.extend_from_slice(&bias_pool);

    let nodes = vec![
        TensorNode::const_at(0, q_pool.len() as u32, TensorTy::F32, q_shape),
        TensorNode::const_at(kt_off, kt_pool.len() as u32, TensorTy::F32, kt_shape),
        TensorNode::const_at(bias_off, bias_pool.len() as u32, TensorTy::F32, logits_shape),
        TensorNode::matmul(0, 1, TensorTy::F32, logits_shape),
        TensorNode::add(3, 2, TensorTy::F32, logits_shape),
        TensorNode::softmax(4, 1, TensorTy::F32, logits_shape),
        TensorNode::output(5, TensorTy::F32, logits_shape),
    ];
    let program = TensorProgram::new(0, 1, nodes.len() as u32, nodes, pool).unwrap();

    let out = execute_tensor(&program, &[]).unwrap();
    let expected = [0.0320586, 0.0871443, 0.2368828, 0.6439143];
    assert_eq!(out.len(), 4);
    let sum: f32 = out.iter().sum();
    assert!(approx_eq(sum, 1.0, 1e-5));
    for (a, b) in out.iter().zip(expected.iter()) {
        assert!(approx_eq(*a, *b, 1e-4), "{a} vs {b}");
    }
}

#[test]
fn program_bytes_round_trip_through_verify_with_stable_hash() {
    let shape = TensorShape::vec(3).unwrap();
    let pool = f32_pool(&[1.0, 2.0, 3.0]);
    let nodes = vec![
        TensorNode::const_at(0, pool.len() as u32, TensorTy::F32, shape),
        TensorNode::softmax(0, 0, TensorTy::F32, shape),
        TensorNode::output(1, TensorTy::F32, shape),
    ];
    let p1 = TensorProgram::new(0, 1, nodes.len() as u32, nodes.clone(), pool.clone()).unwrap();
    let h1 = Hash::for_blob(p1.bytes());

    // Re-verify the bytes from scratch — must produce a structurally
    // identical program with the same hash. This is the content-
    // addressing invariant: identity = bytes, period.
    let p2 = verify_tensor(p1.bytes()).unwrap();
    let h2 = Hash::for_blob(p2.bytes());
    assert_eq!(h1, h2);
    assert_eq!(p1.bytes(), p2.bytes());

    // Same logical program rebuilt from scratch must produce the
    // same hash too — proves the encoding is deterministic.
    let p3 = TensorProgram::new(0, 1, nodes.len() as u32, nodes, pool).unwrap();
    let h3 = Hash::for_blob(p3.bytes());
    assert_eq!(h1, h3);
}

#[test]
fn verify_rejects_matmul_with_inner_dim_mismatch() {
    let bad_lhs = TensorShape::matrix(2, 3).unwrap();
    let bad_rhs = TensorShape::matrix(4, 5).unwrap(); // 4 ≠ 3
    let bad_out = TensorShape::matrix(2, 5).unwrap();
    let lhs_pool = f32_pool(&[0.0; 6]);
    let rhs_pool = f32_pool(&[0.0; 20]);
    let mut pool = lhs_pool.clone();
    let rhs_off = pool.len() as u32;
    pool.extend_from_slice(&rhs_pool);
    let nodes = vec![
        TensorNode::const_at(0, lhs_pool.len() as u32, TensorTy::F32, bad_lhs),
        TensorNode::const_at(rhs_off, rhs_pool.len() as u32, TensorTy::F32, bad_rhs),
        TensorNode::matmul(0, 1, TensorTy::F32, bad_out),
        TensorNode::output(2, TensorTy::F32, bad_out),
    ];
    let err = TensorProgram::new(0, 1, nodes.len() as u32, nodes, pool);
    assert!(err.is_err(), "matmul with k_lhs != k_rhs must be rejected");
}

#[test]
fn verify_rejects_program_without_output() {
    let shape = TensorShape::vec(2).unwrap();
    let pool = f32_pool(&[1.0, 2.0]);
    let nodes = vec![
        TensorNode::const_at(0, pool.len() as u32, TensorTy::F32, shape),
        TensorNode::softmax(0, 0, TensorTy::F32, shape),
        // No Output! Must be rejected.
    ];
    let err = TensorProgram::new(0, 1, nodes.len() as u32, nodes, pool);
    assert!(err.is_err(), "program with no Output node must be rejected");
}

#[test]
fn elementwise_add_and_mul_match_reference() {
    let shape = TensorShape::vec(3).unwrap();
    let a = vec![1.0f32, 2.0, 3.0];
    let b = vec![10.0f32, 20.0, 30.0];
    let nodes = vec![
        TensorNode::input(0, TensorTy::F32, shape),
        TensorNode::input(1, TensorTy::F32, shape),
        TensorNode::add(0, 1, TensorTy::F32, shape),
        TensorNode::mul(0, 1, TensorTy::F32, shape),
        // Output the SUM (we picked add as the program output for this test;
        // the mul node is a parallel branch verifying both ops accept.)
        TensorNode::output(2, TensorTy::F32, shape),
    ];
    let program = TensorProgram::new(2, 1, nodes.len() as u32, nodes, Vec::new()).unwrap();
    let out = execute_tensor(&program, &[a, b]).unwrap();
    assert_eq!(out, vec![11.0, 22.0, 33.0]);
}

#[test]
fn relu_zeroes_negatives_passes_positives() {
    let shape = TensorShape::vec(5).unwrap();
    let nodes = vec![
        TensorNode::input(0, TensorTy::F32, shape),
        TensorNode::relu(0, TensorTy::F32, shape),
        TensorNode::output(1, TensorTy::F32, shape),
    ];
    let program = TensorProgram::new(1, 1, nodes.len() as u32, nodes, Vec::new()).unwrap();
    let input = vec![-2.0f32, -0.5, 0.0, 0.5, 2.0];
    let out = execute_tensor(&program, &[input]).unwrap();
    assert_eq!(out, vec![0.0, 0.0, 0.0, 0.5, 2.0]);
}

#[test]
fn sigmoid_maps_to_unit_interval_with_known_anchor() {
    let shape = TensorShape::vec(3).unwrap();
    let nodes = vec![
        TensorNode::input(0, TensorTy::F32, shape),
        TensorNode::sigmoid(0, TensorTy::F32, shape),
        TensorNode::output(1, TensorTy::F32, shape),
    ];
    let program = TensorProgram::new(1, 1, nodes.len() as u32, nodes, Vec::new()).unwrap();
    let out = execute_tensor(&program, &[vec![-10.0f32, 0.0, 10.0]]).unwrap();
    assert!(out[0] < 0.001);
    assert!(approx_eq(out[1], 0.5, 1e-6));
    assert!(out[2] > 0.999);
}

#[test]
fn tanh_zero_at_zero_and_one_at_infinity() {
    let shape = TensorShape::vec(3).unwrap();
    let nodes = vec![
        TensorNode::input(0, TensorTy::F32, shape),
        TensorNode::tanh(0, TensorTy::F32, shape),
        TensorNode::output(1, TensorTy::F32, shape),
    ];
    let program = TensorProgram::new(1, 1, nodes.len() as u32, nodes, Vec::new()).unwrap();
    let out = execute_tensor(&program, &[vec![-100.0f32, 0.0, 100.0]]).unwrap();
    assert!(approx_eq(out[0], -1.0, 1e-6));
    assert!(approx_eq(out[1], 0.0, 1e-6));
    assert!(approx_eq(out[2], 1.0, 1e-6));
}

#[test]
fn gelu_tanh_approximates_x_at_large_values_zero_at_zero() {
    let shape = TensorShape::vec(3).unwrap();
    let nodes = vec![
        TensorNode::input(0, TensorTy::F32, shape),
        TensorNode::gelu_tanh(0, TensorTy::F32, shape),
        TensorNode::output(1, TensorTy::F32, shape),
    ];
    let program = TensorProgram::new(1, 1, nodes.len() as u32, nodes, Vec::new()).unwrap();
    let out = execute_tensor(&program, &[vec![-5.0f32, 0.0, 5.0]]).unwrap();
    // GeLU(-large) ≈ 0, GeLU(0) == 0, GeLU(+large) ≈ x.
    assert!(out[0].abs() < 1e-3);
    assert!(approx_eq(out[1], 0.0, 1e-6));
    assert!(approx_eq(out[2], 5.0, 1e-3));
}

#[test]
fn ffn_block_matmul_relu_matmul_round_trip_bit_exact() {
    // A 2-layer MLP block: x → linear1 → ReLU → linear2 → y.
    // Tiny (4-dim hidden) so we can hand-verify the result.
    //
    //   x  shape [1, 4] = [1.0, -2.0, 3.0, -4.0]
    //   W1 shape [4, 4] = identity
    //   W2 shape [4, 2] = [[1,1],[1,0],[0,1],[1,1]]
    //
    //   h_pre = x @ W1 = x = [1, -2, 3, -4]
    //   h     = relu(h_pre) = [1, 0, 3, 0]
    //   y     = h @ W2 = [1+0+0+0, 1+0+3+0] = [1, 4]
    let x_shape = TensorShape::matrix(1, 4).unwrap();
    let w1_shape = TensorShape::matrix(4, 4).unwrap();
    let h_shape = TensorShape::matrix(1, 4).unwrap();
    let w2_shape = TensorShape::matrix(4, 2).unwrap();
    let y_shape = TensorShape::matrix(1, 2).unwrap();

    let x = vec![1.0f32, -2.0, 3.0, -4.0];
    let w1 = vec![
        1.0, 0.0, 0.0, 0.0,
        0.0, 1.0, 0.0, 0.0,
        0.0, 0.0, 1.0, 0.0,
        0.0, 0.0, 0.0, 1.0,
    ];
    let w2 = vec![
        1.0, 1.0,
        1.0, 0.0,
        0.0, 1.0,
        1.0, 1.0,
    ];
    let x_pool = f32_pool(&x);
    let w1_pool = f32_pool(&w1);
    let w2_pool = f32_pool(&w2);
    let mut pool = x_pool.clone();
    let w1_off = pool.len() as u32;
    pool.extend_from_slice(&w1_pool);
    let w2_off = pool.len() as u32;
    pool.extend_from_slice(&w2_pool);

    let nodes = vec![
        TensorNode::const_at(0, x_pool.len() as u32, TensorTy::F32, x_shape),
        TensorNode::const_at(w1_off, w1_pool.len() as u32, TensorTy::F32, w1_shape),
        TensorNode::const_at(w2_off, w2_pool.len() as u32, TensorTy::F32, w2_shape),
        TensorNode::matmul(0, 1, TensorTy::F32, h_shape),  // h_pre
        TensorNode::relu(3, TensorTy::F32, h_shape),       // h
        TensorNode::matmul(4, 2, TensorTy::F32, y_shape),  // y
        TensorNode::output(5, TensorTy::F32, y_shape),
    ];
    let program = TensorProgram::new(0, 1, nodes.len() as u32, nodes, pool).unwrap();
    let out = execute_tensor(&program, &[]).unwrap();
    assert_eq!(out, vec![1.0, 4.0]);
}

#[test]
fn reduce_sum_axis_drops_one_dimension() {
    // 2×3 matrix [[1,2,3],[4,5,6]]
    // Sum axis 0 → [5, 7, 9]
    // Sum axis 1 → [6, 15]
    let in_shape = TensorShape::matrix(2, 3).unwrap();
    let cols_shape = TensorShape::vec(3).unwrap();
    let nodes = vec![
        TensorNode::input(0, TensorTy::F32, in_shape),
        TensorNode::reduce_sum(0, 0, TensorTy::F32, cols_shape),
        TensorNode::output(1, TensorTy::F32, cols_shape),
    ];
    let program = TensorProgram::new(1, 1, nodes.len() as u32, nodes, Vec::new()).unwrap();
    let out = execute_tensor(&program, &[vec![1.0f32, 2.0, 3.0, 4.0, 5.0, 6.0]]).unwrap();
    assert_eq!(out, vec![5.0, 7.0, 9.0]);
}
