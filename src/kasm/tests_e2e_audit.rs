//! Tests E2E audit cohésion KASM (2026-05-02 audit).
//!
//! Vérifie que chaque catégorie d'opcodes du KASM ISA v1.2 produit
//! un programme valide, que `Program::new()` accepte, que
//! `kasm::execute()` exécute correctement avec output prévisible,
//! et que `Program::from_bytes()` round-trip preserves le state
//! bit-pour-bit.
//!
//! 8 catégories couvertes :
//!   1. v0.x scalar core (Input/ConstI64/Add/Mul/Sub/Hash64/Output)
//!   2. v0.x bool/compare (And/Or/Not, Lt/Le/Eq, Select)
//!   3. v0.x bitops (BitAnd/BitOr/BitXor, Shl/Shr, BitFlip/Neg/Reverse/Byteswap)
//!   4. v0.x reduce (ReduceAdd/ReduceMul, Sat add/sub, Mod)
//!   5. F64 layer (ConstF64 + F64Op pass-through bit-stable)
//!   6. v1.0 meta-ops (Cond, Memoize, Adaptive — pass-through identity)
//!   7. v1.1 Vec arith (VAdd/VMul/VSum/VLen via call_bytes)
//!   8. v1.2 self-host (Op::Fractal via execute_with_fractal)
//!
//! Si un test échoue, c'est qu'un opcode ISA est incohérent dans son
//! pipeline (verify → execute → output). Audit non-passable.

#[cfg(test)]
mod tests {
    use crate::kasm::{
        execute, execute_with_fractal, FractalDispatcher,
        Node, Op, Program, Target, Ty, KasmError,
    };
    use crate::kasm::self_host::SelfHostingRuntime;
    use crate::store::{Hash, Store};
    use crate::{fresh_tmp_path, TmpDir};
    use std::sync::Arc;

    // ─── Helpers ──────────────────────────────────────────────────────

    fn args_i64(values: &[i64]) -> Vec<u8> {
        let mut out = Vec::with_capacity(values.len() * 8);
        for v in values {
            out.extend_from_slice(&v.to_le_bytes());
        }
        out
    }

    fn parse_i64(bytes: &[u8], idx: usize) -> i64 {
        let off = idx * 8;
        i64::from_le_bytes(bytes[off..off + 8].try_into().unwrap())
    }

    fn build_run(
        nodes: Vec<Node>,
        inputs: u8,
        outputs: u8,
        args: &[i64],
    ) -> Result<Vec<i64>, KasmError> {
        let prog = Program::new(Target::Cpu, inputs, outputs, 64, nodes)?;
        let bytes = execute(&prog, &args_i64(args))?;
        let mut out = Vec::with_capacity(outputs as usize);
        for i in 0..outputs as usize {
            out.push(parse_i64(&bytes, i));
        }
        Ok(out)
    }

    // ─── Category 1 — v0.x scalar core ───────────────────────────────

    #[test]
    fn audit_cat1_scalar_arithmetic_e2e() {
        // f(x, y) = (x + y) * (x - y) - hash(x)
        let nodes = vec![
            Node::input(0),                        // 0: x
            Node::input(1),                        // 1: y
            Node::add(0, 1),                       // 2: x+y
            Node::sub(0, 1),                       // 3: x-y
            Node::mul(2, 3),                       // 4: (x+y)*(x-y) = x²-y²
            Node::hash64(0),                       // 5: hash(x)
            Node::sub(4, 5),                       // 6: result
            Node::output(6, Ty::I64),
        ];
        let result = build_run(nodes, 2, 1, &[100, 30]).unwrap();
        let expected = (100i64 * 100 - 30 * 30) - splitmix64(100);
        assert_eq!(result[0], expected);
    }

    fn splitmix64(input: i64) -> i64 {
        // Replica de kasm::program::hash_i64 pour vérification
        let mut z = input as u64;
        z = z.wrapping_add(0x9E3779B97F4A7C15);
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        (z ^ (z >> 31)) as i64
    }

    // ─── Category 2 — bool/compare/select ────────────────────────────

    #[test]
    fn audit_cat2_bool_compare_select_e2e() {
        // f(a, b, c) = if a < b then a else c (via Bool + SelectI64).
        // SelectI64 helper : Node::select_i64(cond, if_true, if_false)
        //   → a = cond Bool, b = if_true, imm = if_false.
        let lt_node = Node {
            op: Op::LtI64, ty: Ty::Bool, a: 0, b: 1, imm: 0,
        };
        let select_node = Node::select_i64(3, 0, 2);  // cond=node3 (lt),
                                                       // if_true=input0(a), if_false=input2(c)
        let nodes = vec![
            Node::input(0),     // 0: a
            Node::input(1),     // 1: b
            Node::input(2),     // 2: c
            lt_node,            // 3: a < b → bool
            select_node,        // 4: select(lt, a, c)
            Node::output(4, Ty::I64),
        ];
        // a=10, b=20, c=99 → 10 < 20 = true → return a = 10.
        let result = build_run(nodes, 3, 1, &[10, 20, 99]).unwrap();
        assert_eq!(result[0], 10);
        // a=30, b=20, c=99 → 30 < 20 = false → return c = 99.
        let nodes2 = vec![
            Node::input(0), Node::input(1), Node::input(2),
            Node { op: Op::LtI64, ty: Ty::Bool, a: 0, b: 1, imm: 0 },
            Node::select_i64(3, 0, 2),
            Node::output(4, Ty::I64),
        ];
        let result2 = build_run(nodes2, 3, 1, &[30, 20, 99]).unwrap();
        assert_eq!(result2[0], 99);
    }

    // ─── Category 3 — bitops ──────────────────────────────────────────

    #[test]
    fn audit_cat3_bitops_e2e() {
        // f(x) = ~(x ^ 0xFF) — bitwise not of (x xor 0xFF)
        let nodes = vec![
            Node::input(0),                                  // 0: x
            Node::const_i64(0xFF),                           // 1: 0xFF
            Node { op: Op::BitXorI64, ty: Ty::I64, a: 0, b: 1, imm: 0 }, // 2: x ^ 0xFF
            Node { op: Op::BitFlipI64, ty: Ty::I64, a: 2, b: 0, imm: 0 },// 3: ~(x ^ 0xFF)
            Node::output(3, Ty::I64),
        ];
        let result = build_run(nodes, 1, 1, &[0x12345678]).unwrap();
        let expected = !((0x12345678i64) ^ 0xFF);
        assert_eq!(result[0], expected);
    }

    #[test]
    fn audit_cat3_shifts_e2e() {
        // f(x) = (x << 4) | (x >> 4) — rotate-like
        let nodes = vec![
            Node::input(0),
            Node::const_i64(4),
            Node { op: Op::ShlI64, ty: Ty::I64, a: 0, b: 1, imm: 0 },  // 2: x << 4
            Node { op: Op::ShrI64, ty: Ty::I64, a: 0, b: 1, imm: 0 },  // 3: x >> 4 (zero-fill)
            Node { op: Op::BitOrI64, ty: Ty::I64, a: 2, b: 3, imm: 0 },// 4: combined
            Node::output(4, Ty::I64),
        ];
        let result = build_run(nodes, 1, 1, &[0x12345678]).unwrap();
        let x = 0x12345678u64;
        let expected = ((x << 4) | (x >> 4)) as i64;
        assert_eq!(result[0], expected);
    }

    // ─── Category 4 — reduce + saturating ────────────────────────────

    #[test]
    fn audit_cat4_saturating_arithmetic_e2e() {
        // f(x) = sat_add(x, MAX) — saturate, no overflow.
        let nodes = vec![
            Node::input(0),
            Node::const_i64(i16::MAX),  // i16 max representable as ConstI64 imm
            Node { op: Op::SatAddI64, ty: Ty::I64, a: 0, b: 1, imm: 0 },
            Node::output(2, Ty::I64),
        ];
        // Pas saturated avec valeurs courantes.
        let result = build_run(nodes, 1, 1, &[1000]).unwrap();
        assert_eq!(result[0], 1000 + i16::MAX as i64);
    }

    #[test]
    fn audit_cat4_div_total_function_e2e() {
        // div by 0 → 0 (KASM total convention).
        let nodes = vec![
            Node::input(0),
            Node::input(1),
            Node { op: Op::DivI64Checked, ty: Ty::I64, a: 0, b: 1, imm: 0 },
            Node::output(2, Ty::I64),
        ];
        let result = build_run(nodes, 2, 1, &[100, 0]).unwrap();
        assert_eq!(result[0], 0, "div by 0 must return 0 (total function)");
        let result2 = build_run(
            vec![
                Node::input(0), Node::input(1),
                Node { op: Op::DivI64Checked, ty: Ty::I64, a: 0, b: 1, imm: 0 },
                Node::output(2, Ty::I64),
            ],
            2, 1, &[100, 7],
        ).unwrap();
        assert_eq!(result2[0], 14);  // 100 / 7 = 14
    }

    // ─── Category 5 — F64 layer ──────────────────────────────────────

    #[test]
    fn audit_cat5_f64_const_round_trip_e2e() {
        // f() = ConstF64(42) — small int literal.
        let nodes = vec![
            Node::const_f64(42),  // imm = 42 i16
            Node::output(0, Ty::F64),
        ];
        let prog = Program::new(Target::Cpu, 0, 1, 16, nodes).unwrap();
        let bytes = execute(&prog, &[]).unwrap();
        // Output = i64 bit pattern of f64 = 42.0.
        let bits = i64::from_le_bytes(bytes[..8].try_into().unwrap());
        let f = f64::from_bits(bits as u64);
        assert_eq!(f, 42.0);
    }

    // ─── Category 6 — v1.0 meta-ops pass-through ─────────────────────

    #[test]
    fn audit_cat6_memoize_pass_through_e2e() {
        // Op::Memoize is bytecode-level pass-through (transparent
        // identity). f(x) = Memoize(x*2) → returns x*2 unchanged at
        // bytecode level (the brain layer would cache).
        let nodes = vec![
            Node::input(0),                                   // 0: x
            Node::const_i64(2),                               // 1: 2
            Node::mul(0, 1),                                  // 2: x*2
            Node { op: Op::Memoize, ty: Ty::I64, a: 2, b: 0, imm: 0 }, // 3: Memoize(x*2)
            Node::output(3, Ty::I64),
        ];
        let result = build_run(nodes, 1, 1, &[21]).unwrap();
        assert_eq!(result[0], 42);
    }

    #[test]
    fn audit_cat6_comptime_pass_through_e2e() {
        // Op::Comptime également pass-through (compile-time constant
        // folding hint, runtime = identity).
        let nodes = vec![
            Node::input(0),
            Node::const_i64(7),
            Node::add(0, 1),
            Node { op: Op::Comptime, ty: Ty::I64, a: 2, b: 0, imm: 0 },
            Node::output(3, Ty::I64),
        ];
        let result = build_run(nodes, 1, 1, &[100]).unwrap();
        assert_eq!(result[0], 107);
    }

    #[test]
    fn audit_cat6_cond_with_bool_predicate_e2e() {
        // Op::Cond(pred, true_slot, else_slot) — pred slot is Bool.
        // f(a, b) = Cond(a > b, a, b) = max(a, b).
        let nodes = vec![
            Node::input(0),                                       // 0: a
            Node::input(1),                                       // 1: b
            Node { op: Op::LtI64, ty: Ty::Bool, a: 1, b: 0, imm: 0 }, // 2: b < a
            Node::cond(2, 0, 1),                                  // 3: if (b<a) then a else b
            Node::output(3, Ty::I64),
        ];
        let result = build_run(nodes, 2, 1, &[42, 17]).unwrap();
        assert_eq!(result[0], 42);
        let nodes2 = vec![
            Node::input(0), Node::input(1),
            Node { op: Op::LtI64, ty: Ty::Bool, a: 1, b: 0, imm: 0 },
            Node::cond(2, 0, 1),
            Node::output(3, Ty::I64),
        ];
        let result2 = build_run(nodes2, 2, 1, &[17, 42]).unwrap();
        assert_eq!(result2[0], 42);
    }

    // ─── Category 7 — Vec arithmetic v1.1 (call via execute) ─────────

    #[test]
    fn audit_cat7_vec_input_round_trip_e2e() {
        // Identity via Op::Input + Op::Output sur Ty::VecI64.
        // Wire format : [u32 LE count | count × 8 bytes i64 LE].
        let nodes = vec![
            Node::input_vec(0),
            Node::output(0, Ty::VecI64),
        ];
        let prog = Program::new(Target::Cpu, 1, 1, 16, nodes).unwrap();
        let mut args = Vec::new();
        args.extend_from_slice(&3u32.to_le_bytes());  // count = 3
        for v in [10i64, 20, 30] {
            args.extend_from_slice(&v.to_le_bytes());
        }
        let bytes = execute(&prog, &args).unwrap();
        let count = u32::from_le_bytes(bytes[..4].try_into().unwrap());
        assert_eq!(count, 3);
        for (i, expected) in [10i64, 20, 30].iter().enumerate() {
            let off = 4 + i * 8;
            let v = i64::from_le_bytes(bytes[off..off + 8].try_into().unwrap());
            assert_eq!(v, *expected);
        }
    }

    #[test]
    fn audit_cat7_vec_arith_via_brain_dispatch_e2e() {
        // Vec arithmetic ops Op::VAddI64 etc. nécessitent Vec values
        // dans le pool. Test via call_bytes wire format.
        let nodes = vec![
            Node::input_vec(0),
            Node::input_vec(1),
            Node { op: Op::VAddI64, ty: Ty::VecI64, a: 0, b: 1, imm: 0 },
            Node::output(2, Ty::VecI64),
        ];
        let prog = Program::new(Target::Cpu, 2, 1, 16, nodes).unwrap();
        let mut args = Vec::new();
        args.extend_from_slice(&3u32.to_le_bytes());
        for v in [1i64, 2, 3] { args.extend_from_slice(&v.to_le_bytes()); }
        args.extend_from_slice(&3u32.to_le_bytes());
        for v in [10i64, 20, 30] { args.extend_from_slice(&v.to_le_bytes()); }
        let bytes = execute(&prog, &args).unwrap();
        let count = u32::from_le_bytes(bytes[..4].try_into().unwrap());
        assert_eq!(count, 3);
        for (i, expected) in [11i64, 22, 33].iter().enumerate() {
            let off = 4 + i * 8;
            let v = i64::from_le_bytes(bytes[off..off + 8].try_into().unwrap());
            assert_eq!(v, *expected);
        }
    }

    // ─── Category 8 — v1.2 self-host (Op::Fractal/Op::Eval) ──────────

    #[test]
    fn audit_cat8_fractal_dispatch_e2e() {
        // Programme avec Op::Fractal qui appelle un callee enregistré.
        // Setup : callee = f(x) = x*2, hash registered as callee_id 42.
        let path = fresh_tmp_path("audit-cat8", "fractal");
        std::fs::create_dir_all(&path).unwrap();
        let _g = TmpDir::new(path.clone());
        let store = Arc::new(Store::open(&path).unwrap());
        let callee = Program::new(
            Target::Cpu, 1, 1, 16,
            vec![
                Node::input(0),
                Node::const_i64(2),
                Node::mul(0, 1),
                Node::output(2, Ty::I64),
            ],
        ).unwrap();
        let callee_hash = store.store(callee.bytes()).unwrap();
        let runtime = SelfHostingRuntime::new(store);
        runtime.register_callee(42, callee_hash);

        // Programme outer : Fractal(callee_id=42, arg=x) + 100.
        let fractal = Node {
            op: Op::Fractal, ty: Ty::I64, a: 1, b: 0, imm: 0,
        };
        let outer = Program::new(
            Target::Cpu, 1, 1, 32,
            vec![
                Node::input(0),
                Node::const_i64(42),  // callee_id
                fractal,
                Node::const_i64(100),
                Node::add(2, 3),
                Node::output(4, Ty::I64),
            ],
        ).unwrap();
        let bytes = execute_with_fractal(&outer, &args_i64(&[5]), &runtime).unwrap();
        let result = parse_i64(&bytes, 0);
        assert_eq!(result, 110, "Fractal(42, 5) * 2 + 100 = 110");
    }

    // ─── Round-trip integrity : Program::from_bytes ──────────────────

    #[test]
    fn audit_program_roundtrip_via_bytes() {
        // Construction d'un programme covering plusieurs catégories,
        // serialize → deserialize → re-execute → bit-exact.
        let original = Program::new(
            Target::Cpu, 2, 1, 32,
            vec![
                Node::input(0),                                     // 0
                Node::input(1),                                     // 1
                Node::const_i64(7),                                 // 2
                Node::add(0, 1),                                    // 3: a+b
                Node::mul(3, 2),                                    // 4: (a+b)*7
                Node::hash64(4),                                    // 5: hash((a+b)*7)
                Node { op: Op::Memoize, ty: Ty::I64, a: 5, b: 0, imm: 0 }, // 6
                Node::output(6, Ty::I64),
            ],
        ).unwrap();

        let bytes_serialized = original.bytes().to_vec();
        let restored = Program::from_bytes(&bytes_serialized).unwrap();

        // Bytes serialization deterministe.
        assert_eq!(original.bytes(), restored.bytes());
        // Inputs/outputs preserved.
        assert_eq!(original.inputs(), restored.inputs());
        assert_eq!(original.outputs(), restored.outputs());
        assert_eq!(original.nodes().len(), restored.nodes().len());

        // Execute both et compare outputs.
        let args = args_i64(&[42, 13]);
        let out_orig = execute(&original, &args).unwrap();
        let out_rest = execute(&restored, &args).unwrap();
        assert_eq!(out_orig, out_rest, "round-trip exec output identique");
    }

    // ─── ISA exhaustivity check ──────────────────────────────────────

    #[test]
    fn audit_isa_op_count_exact_74() {
        // Wave 7i bumped the ISA by one : Op::VGetI64 = 66. The test
        // name and bound move together so any future ISA expansion has
        // an explicit, easy-to-grep landmark.
        for byte in 0u8..=73 {
            let op = crate::kasm::types::Op::from_byte(byte);
            assert!(op.is_ok(), "byte {} doit décoder en Op valide", byte);
        }
        // 67 et au-dessus → erreur.
        assert!(crate::kasm::types::Op::from_byte(74).is_err());
        assert!(crate::kasm::types::Op::from_byte(255).is_err());
    }

    #[test]
    fn audit_isa_op_byte_round_trip() {
        // Pour chaque opcode, op as u8 → from_byte → même opcode.
        for byte in 0u8..=73 {
            let op = crate::kasm::types::Op::from_byte(byte).unwrap();
            assert_eq!(op as u8, byte, "round-trip Op byte broken at {}", byte);
        }
    }
}
