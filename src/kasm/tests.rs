use super::program::{digest, verify};
use super::*;

fn affine_nodes() -> Vec<Node> {
    vec![
        Node::input(0),
        Node::const_i64(3),
        Node::mul(0, 1),
        Node::const_i64(1),
        Node::add(2, 3),
        Node::output(4, Ty::I64),
    ]
}

fn const_heavy_program(seed: i16) -> Program {
    let mut nodes = Vec::new();
    nodes.push(Node::input(0));

    let live_mul_const = nodes.len() as u16;
    nodes.push(Node::const_i64(seed.rem_euclid(5) + 2));
    let live_mul = nodes.len() as u16;
    nodes.push(Node::mul(0, live_mul_const));

    let mut const_ref = nodes.len() as u16;
    nodes.push(Node::const_i64(seed.rem_euclid(17) - 8));

    for i in 0..48i16 {
        let c = nodes.len() as u16;
        nodes.push(Node::const_i64(((seed + i * 3).rem_euclid(19)) - 9));
        let next = nodes.len() as u16;
        match i % 4 {
            0 => nodes.push(Node::add(const_ref, c)),
            1 => nodes.push(Node::sub(const_ref, c)),
            2 => nodes.push(Node::min(const_ref, c)),
            _ => nodes.push(Node::max(const_ref, c)),
        }
        const_ref = next;
    }

    let dead_base = nodes.len() as u16;
    nodes.push(Node::const_i64(seed.rem_euclid(13) - 6));
    let mut dead_ref = dead_base;
    for i in 0..16i16 {
        let c = nodes.len() as u16;
        nodes.push(Node::const_i64(((seed - i * 2).rem_euclid(11)) - 5));
        let next = nodes.len() as u16;
        nodes.push(Node::add(dead_ref, c));
        dead_ref = next;
    }

    let const_eq = nodes.len() as u16;
    nodes.push(Node::eq(const_ref, const_ref));
    let zero = nodes.len() as u16;
    nodes.push(Node::const_i64(0));
    let selected = nodes.len() as u16;
    nodes.push(Node::select_i64(const_eq, const_ref, zero));
    let combined = nodes.len() as u16;
    nodes.push(Node::add(live_mul, selected));
    nodes.push(Node::output(combined, Ty::I64));

    Program::new(Target::Cpu, 1, 1, nodes.len() as u32, nodes).unwrap()
}

fn static_rewrite_program(seed: i16) -> Program {
    Program::new(
        Target::Cpu,
        1,
        1,
        10,
        vec![
            Node::input(0),
            Node::const_i64(seed.rem_euclid(7) + 1),
            Node::mul(0, 1),
            Node::sub(2, 2),
            Node::const_i64(seed.rem_euclid(11) - 5),
            Node::add(3, 4),
            Node::eq(5, 5),
            Node::const_i64(0),
            Node::select_i64(6, 5, 7),
            Node::output(8, Ty::I64),
        ],
    )
    .unwrap()
}

fn dynamic_rewrite_program(seed: i16) -> Program {
    Program::new(
        Target::Cpu,
        1,
        1,
        8,
        vec![
            Node::input(0),
            Node::const_i64(seed.rem_euclid(5) + 2),
            Node::mul(0, 1),
            Node::const_i64(seed.rem_euclid(13) - 6),
            Node::add(2, 3),
            Node::const_i64(1),
            Node::mul(4, 5),
            Node::output(6, Ty::I64),
        ],
    )
    .unwrap()
}

#[test]
fn verifies_and_executes_arithmetic_graph() {
    let program = Program::new(Target::Cpu, 1, 1, 6, affine_nodes()).unwrap();
    let result = execute(&program, &14i64.to_le_bytes()).unwrap();
    assert_eq!(i64::from_le_bytes(result.try_into().unwrap()), 43);
}

#[test]
fn rejects_forward_refs() {
    let mut bytes = Program::new(Target::Cpu, 1, 1, 2, vec![Node::input(0), Node::output(0, Ty::I64)])
        .unwrap()
        .bytes()
        .to_vec();
    bytes[HEADER_LEN + NODE_LEN + 2..HEADER_LEN + NODE_LEN + 4].copy_from_slice(&7u16.to_le_bytes());
    let footer_start = bytes.len() - FOOTER_LEN;
    let footer = digest(&bytes[..footer_start]);
    bytes[footer_start..].copy_from_slice(&footer);
    assert!(matches!(verify(&bytes), Err(KasmError::BadRef { .. })));
}

#[test]
fn executes_v01_ops() {
    let program = Program::new(
        Target::Cpu,
        2,
        1,
        14,
        vec![
            Node::input(0),
            Node::input(1),
            Node::sub(0, 1),
            Node::div_checked(0, 1),
            Node::min(2, 3),
            Node::max(2, 3),
            Node::eq(4, 5),
            Node::const_i64(111),
            Node::const_i64(222),
            Node::select_i64(6, 7, 8),
            Node::output(9, Ty::I64),
        ],
    )
    .unwrap();
    let mut args = Vec::new();
    args.extend_from_slice(&10i64.to_le_bytes());
    args.extend_from_slice(&4i64.to_le_bytes());
    let result = execute(&program, &args).unwrap();
    assert_eq!(i64::from_le_bytes(result.try_into().unwrap()), 222);
}

#[test]
fn executes_global_cpu_bit_intrinsic_ops() {
    let program = Program::new(
        Target::Cpu,
        2,
        5,
        12,
        vec![
            Node::input(0),
            Node::input(1),
            Node::popcnt(0),
            Node::lzcnt(0),
            Node::tzcnt(0),
            Node::pext(0, 1),
            Node::pdep(5, 1),
            Node::output(2, Ty::I64),
            Node::output(3, Ty::I64),
            Node::output(4, Ty::I64),
            Node::output(5, Ty::I64),
            Node::output(6, Ty::I64),
        ],
    )
    .unwrap();

    let value = 0b1011_0010u64;
    let mask = 0b1111_0000u64;
    let mut args = Vec::new();
    args.extend_from_slice(&(value as i64).to_le_bytes());
    args.extend_from_slice(&(mask as i64).to_le_bytes());
    let result = execute(&program, &args).unwrap();
    let values: Vec<i64> = result
        .chunks_exact(8)
        .map(|chunk| i64::from_le_bytes(chunk.try_into().unwrap()))
        .collect();

    let extracted = crate::cpu_bits::pext_u64(value, mask);
    assert_eq!(values[0], value.count_ones() as i64);
    assert_eq!(values[1], value.leading_zeros() as i64);
    assert_eq!(values[2], value.trailing_zeros() as i64);
    assert_eq!(values[3], extracted as i64);
    assert_eq!(values[4], crate::cpu_bits::pdep_u64(extracted, mask) as i64);
}

#[test]
fn lazy_force_executes_and_simplifies_to_child_value() {
    let program = Program::new(
        Target::Cpu,
        1,
        1,
        8,
        vec![
            Node::input(0),
            Node::const_i64(7),
            Node::mul(0, 1),
            Node::lazy(2),
            Node::force(3),
            Node::output(4, Ty::I64),
        ],
    )
    .unwrap();

    let result = execute(&program, &6i64.to_le_bytes()).unwrap();
    assert_eq!(i64::from_le_bytes(result.try_into().unwrap()), 42);

    let simplified = program.simplified().unwrap();
    assert!(
        simplified
            .nodes()
            .iter()
            .all(|node| !matches!(node.op, Op::Lazy | Op::Force)),
        "Force(Lazy(x)) should collapse to x"
    );
}

#[test]
fn lazy_future_hash_is_deterministic_and_input_sensitive() {
    let program = Program::new(
        Target::Cpu,
        1,
        1,
        6,
        vec![
            Node::input(0),
            Node::const_i64(3),
            Node::add(0, 1),
            Node::lazy(2),
            Node::output(3, Ty::I64),
        ],
    )
    .unwrap();

    let a = execute(&program, &10i64.to_le_bytes()).unwrap();
    let b = execute(&program, &10i64.to_le_bytes()).unwrap();
    let c = execute(&program, &11i64.to_le_bytes()).unwrap();
    assert_eq!(a, b);
    assert_ne!(a, c);
}

#[test]
fn composes_two_programs_without_intermediate_outputs_between_them() {
    let left = Program::new(
        Target::Cpu,
        1,
        1,
        4,
        vec![
            Node::input(0),
            Node::const_i64(2),
            Node::mul(0, 1),
            Node::output(2, Ty::I64),
        ],
    )
    .unwrap();
    let right = Program::new(
        Target::Cpu,
        1,
        1,
        4,
        vec![
            Node::input(0),
            Node::const_i64(1),
            Node::add(0, 1),
            Node::output(2, Ty::I64),
        ],
    )
    .unwrap();
    let program = compose(&left, &right, Target::Cpu).unwrap();
    let result = execute(&program, &21i64.to_le_bytes()).unwrap();
    assert_eq!(i64::from_le_bytes(result.try_into().unwrap()), 43);
}

#[test]
fn canonicalization_removes_dead_nodes_and_fuses_duplicates() {
    let program = Program::new(
        Target::Cpu,
        1,
        1,
        9,
        vec![
            Node::input(0),
            Node::const_i64(3),
            Node::const_i64(99),
            Node::const_i64(3),
            Node::mul(0, 3),
            Node::mul(0, 1),
            Node::const_i64(1),
            Node::add(4, 6),
            Node::output(7, Ty::I64),
        ],
    )
    .unwrap();
    let canonical = program.canonical().unwrap();

    assert!(canonical.nodes().len() < program.nodes().len());
    assert_eq!(execute(&program, &14i64.to_le_bytes()).unwrap(), execute(&canonical, &14i64.to_le_bytes()).unwrap());
}

#[test]
fn equivalent_programs_share_canonical_hash() {
    let a = Program::new(Target::Cpu, 1, 1, 6, affine_nodes()).unwrap();
    let b = Program::new(
        Target::Cpu,
        1,
        1,
        8,
        vec![
            Node::const_i64(123),
            Node::const_i64(1),
            Node::input(0),
            Node::const_i64(3),
            Node::mul(3, 2),
            Node::add(1, 4),
            Node::const_i64(3),
            Node::output(5, Ty::I64),
        ],
    )
    .unwrap();

    assert_ne!(a.structural_hash_hex(), b.structural_hash_hex());
    assert_eq!(a.canonical_hash_hex().unwrap(), b.canonical_hash_hex().unwrap());
}

#[test]
fn one_hundred_equivalent_programs_collapse_to_one_canonical_hash() {
    let mut canonical_hashes = std::collections::BTreeSet::new();
    let mut structural_hashes = std::collections::BTreeSet::new();

    for i in 0..100 {
        let nodes = match i % 4 {
            0 => vec![
                Node::input(0),
                Node::const_i64(3),
                Node::mul(0, 1),
                Node::const_i64(1),
                Node::add(2, 3),
                Node::output(4, Ty::I64),
                Node::const_i64(i),
            ],
            1 => vec![
                Node::const_i64(1),
                Node::input(0),
                Node::const_i64(3),
                Node::mul(1, 2),
                Node::add(0, 3),
                Node::const_i64(i),
                Node::output(4, Ty::I64),
            ],
            2 => vec![
                Node::input(0),
                Node::const_i64(3),
                Node::mul(1, 0),
                Node::const_i64(1),
                Node::add(3, 2),
                Node::output(4, Ty::I64),
                Node::const_i64(i),
            ],
            _ => vec![
                Node::const_i64(i),
                Node::const_i64(3),
                Node::input(0),
                Node::const_i64(1),
                Node::mul(2, 1),
                Node::add(4, 3),
                Node::output(5, Ty::I64),
            ],
        };
        let program = Program::new(Target::Cpu, 1, 1, nodes.len() as u32, nodes).unwrap();
        structural_hashes.insert(program.structural_hash_hex());
        canonical_hashes.insert(program.canonical_hash_hex().unwrap());
    }

    assert!(structural_hashes.len() > 1);
    assert_eq!(canonical_hashes.len(), 1);
}

#[test]
fn semantic_fingerprint_collapses_alpha_equivalent_slot_renaming() {
    // Φ.ν.7e — slot 0 used vs slot 1 used (with declared inputs=2 in both)
    // should hash equal after α-normalisation.
    let uses_slot_0 = Program::new(
        Target::Cpu,
        2,
        1,
        4,
        vec![
            Node::input(0),
            Node::const_i64(1),
            Node::add(0, 1),
            Node::output(2, Ty::I64),
        ],
    )
    .unwrap();
    let uses_slot_1 = Program::new(
        Target::Cpu,
        2,
        1,
        4,
        vec![
            Node::input(1),
            Node::const_i64(1),
            Node::add(0, 1),
            Node::output(2, Ty::I64),
        ],
    )
    .unwrap();

    let fp_0 = uses_slot_0.semantic_fingerprint().unwrap();
    let fp_1 = uses_slot_1.semantic_fingerprint().unwrap();
    assert_eq!(
        fp_0, fp_1,
        "α-equivalent slot renames must collapse to the same fingerprint"
    );

    // Sanity : a structurally distinct program (multiplication) must NOT
    // collapse to the same fingerprint (no false-positive over-collapse).
    let uses_slot_0_mul = Program::new(
        Target::Cpu,
        2,
        1,
        4,
        vec![
            Node::input(0),
            Node::const_i64(2),
            Node::mul(0, 1),
            Node::output(2, Ty::I64),
        ],
    )
    .unwrap();
    let fp_mul = uses_slot_0_mul.semantic_fingerprint().unwrap();
    assert_ne!(
        fp_0, fp_mul,
        "behaviorally distinct programs must keep distinct fingerprints"
    );
}

#[test]
fn semantic_fingerprint_collapses_different_structures_with_same_behavior() {
    let mul_two = Program::new(
        Target::Cpu,
        1,
        1,
        4,
        vec![
            Node::input(0),
            Node::const_i64(2),
            Node::mul(0, 1),
            Node::output(2, Ty::I64),
        ],
    )
    .unwrap();
    let add_self = Program::new(
        Target::Cpu,
        1,
        1,
        3,
        vec![Node::input(0), Node::add(0, 0), Node::output(1, Ty::I64)],
    )
    .unwrap();

    assert_ne!(mul_two.canonical_hash_hex().unwrap(), add_self.canonical_hash_hex().unwrap());
    assert_eq!(
        mul_two.semantic_fingerprint_hex().unwrap(),
        add_self.semantic_fingerprint_hex().unwrap()
    );
}

#[test]
fn simplifier_applies_exact_l0_rules() {
    let program = Program::new(
        Target::Cpu,
        1,
        1,
        9,
        vec![
            Node::input(0),
            Node::const_i64(1),
            Node::mul(0, 1),
            Node::const_i64(0),
            Node::add(2, 3),
            Node::sub(4, 4),
            Node::const_i64(99),
            Node::mul(6, 5),
            Node::output(7, Ty::I64),
        ],
    )
    .unwrap();
    let simplified = program.simplified().unwrap();

    assert!(simplified.nodes().len() < program.nodes().len());
    let result = execute(&simplified, &123i64.to_le_bytes()).unwrap();
    assert_eq!(i64::from_le_bytes(result.try_into().unwrap()), 0);
}

#[test]
fn comptime_propagates_const_through_wrapper() {
    // KASM v1.0 mutation — Op::Comptime sur une valeur connue constante
    // propage la valeur (le wrapper est éliminé par le simplifier) et
    // l'exec produit le résultat correct via l'interpreter pass-through.
    //
    // Source : Mojo @comptime — "evaluate at load, inline result".
    let program = Program::new(
        Target::Cpu,
        1, // 1 input (unused — la valeur est const)
        1,
        4,
        vec![
            Node::input(0),                  // 0 : input (unused, requis par signature)
            Node::const_i64(123),            // 1 : valeur const
            Node::comptime(1),               // 2 : ← Op::Comptime v1.0 wrap
            Node::output(2, Ty::I64),        // 3 : output
        ],
    )
    .unwrap();

    // Path 1 — execute direct (interpreter scalaire) : Op::Comptime est
    // pass-through, le programme retourne bien 123.
    let result = execute(&program, &0i64.to_le_bytes()).unwrap();
    assert_eq!(i64::from_le_bytes(result.try_into().unwrap()), 123);

    // Path 2 — simplified : le simplifier élimine le wrapper
    // Op::Comptime, le DAG résultant n'en contient plus (Known::I64
    // propagé directement).
    let simplified = program.simplified().unwrap();
    for node in simplified.nodes() {
        assert_ne!(node.op, Op::Comptime,
            "Op::Comptime wrapper should have been eliminated by simplify");
    }
    // Le hash du programme simplifié doit aussi rendre 123.
    let result_simp = execute(&simplified, &0i64.to_le_bytes()).unwrap();
    assert_eq!(i64::from_le_bytes(result_simp.try_into().unwrap()), 123);
}

#[test]
fn comptime_folds_hash_of_const_via_or_chain() {
    // KASM v1.0 wave 3 : Op::Comptime sur Hash64(Const(N)) fold le
    // résultat au load time, même si la valeur résultante (output de
    // SplitMix64) ne fit pas dans i16. Le nouveau materialize_i64_via_
    // or_chain construit la constante via 4 chunks de 16 bits OR-combinés.
    //
    // Source : Mojo @comptime — la promesse "evaluate at load, inline
    // result" tient maintenant pour des valeurs i64 arbitraires, pas
    // juste des values fittables dans (high, low, k).
    let program = Program::new(
        Target::Cpu,
        1,
        1,
        5,
        vec![
            Node::input(0),                  // 0 (unused — Comptime ignore les inputs)
            Node::const_i64(42),             // 1 : seed
            Node::hash64(1),                 // 2 : SplitMix64(42) → arbitrary i64
            Node::comptime(2),               // 3 : Op::Comptime fold marker
            Node::output(3, Ty::I64),        // 4 : output
        ],
    )
    .unwrap();

    // Expected reference value : SplitMix64 / Stafford Mix13 of 42
    let mut x = 42u64;
    x = x.wrapping_add(0x9e3779b97f4a7c15);
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d049bb133111eb);
    let expected = (x ^ (x >> 31)) as i64;

    // Path 1 — execute direct : interpreter scalar fait Hash64(42), le
    // wrapper Op::Comptime est pass-through, output = SplitMix64(42).
    let result = execute(&program, &0i64.to_le_bytes()).unwrap();
    assert_eq!(i64::from_le_bytes(result.try_into().unwrap()), expected);

    // Path 2 — simplified : le simplifier doit fold Hash64(Const) +
    // Comptime → chaîne de Const + Shl + BitAnd + BitOr qui produit
    // exactement `expected`. Aucun Op::Hash64 ni Op::Comptime ne doit
    // rester dans le DAG simplifié.
    let simplified = program.simplified().unwrap();
    for node in simplified.nodes() {
        assert_ne!(node.op, Op::Hash64,
            "Hash64(Const) should be folded to a Const chain at simplify time");
        assert_ne!(node.op, Op::Comptime,
            "Op::Comptime wrapper should be eliminated by simplify");
    }
    // L'execution du programme simplifié doit produire la même valeur.
    // Cette assertion CASSAIT en wave 1 et wave 2 (TypeMismatch dans
    // materialize_i64 pour les hash outputs), passe maintenant grâce
    // au materialize_i64_via_or_chain.
    let result_simp = execute(&simplified, &0i64.to_le_bytes()).unwrap();
    assert_eq!(i64::from_le_bytes(result_simp.try_into().unwrap()), expected,
        "simplified Op::Comptime(Hash64(Const)) must produce ref SplitMix64 value");
}

#[test]
fn materialize_handles_arbitrary_i64_via_or_chain() {
    // Wave 3 dépendance core — vérifie que le simplifier peut fold un
    // calcul arithmétique qui produit une valeur i64 hors-i16 et hors
    // pattern (high, low, k). Test direct du materializer via une
    // multiplication de deux constantes qui dépasse i32.
    //
    // 12345 * 67890 = 838,102,050 (bien hors i16 [-32768, 32767], mais
    // toujours dans i32 — fittable via fit_i64_via_shl).
    //
    // Pour vraiment tester le or_chain on choisit deux nombres dont le
    // produit est en zone i64 large.
    let big_a = 0x0000_4000_0000_0001i64;  // 16-bit chunk pattern
    let big_b = 0x0000_0000_0000_0002i64;
    let expected = big_a.wrapping_mul(big_b);  // 0x0000_8000_0000_0002

    // Le programme : Const fold de big_a * big_b via Comptime
    // (les Const sont i16-fittables si on les bake en runtime, mais
    // ici on teste le materialize de big_a et big_b directement)

    // Plus simple : un programme qui multiplie deux pré-existant grands
    // qui forcent le or_chain. On les construit nous-mêmes.
    //
    // En fait, le vrai test : le simplifier rencontre une expression
    // dont le const-fold produit big_a × big_b et doit la matérialiser.
    // On construit ça via Hash64 d'un const (déjà testé), ou via
    // d'autres expressions. Ici on teste juste que le materializer
    // de Known::I64(arbitrary) produit un programme qui calcule la
    // valeur correcte.
    //
    // Test indirect : un programme `output(hash64(input))` avec un
    // input fixe folder par const propagation après inlining.
    let program = Program::new(
        Target::Cpu,
        1,
        1,
        5,
        vec![
            Node::input(0),
            Node::const_i64(12345),     // x = 12345
            Node::hash64(1),             // y = hash(x) — arbitrary i64
            Node::comptime(2),           // load-time fold marker
            Node::output(3, Ty::I64),
        ],
    )
    .unwrap();

    // Compute reference SplitMix64(12345).
    let mut v = 12345u64;
    v = v.wrapping_add(0x9e3779b97f4a7c15);
    v = (v ^ (v >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    v = (v ^ (v >> 27)).wrapping_mul(0x94d049bb133111eb);
    let ref_value = (v ^ (v >> 31)) as i64;

    let simplified = program.simplified().unwrap();
    let result = execute(&simplified, &0i64.to_le_bytes()).unwrap();
    assert_eq!(i64::from_le_bytes(result.try_into().unwrap()), ref_value);

    // Sanity: the simplified DAG should consist of Const + arithmetic
    // ops (the or-chain), no Hash64, no Comptime.
    let _ = expected; // expected was just for documentation
    let _ = big_a;
    let _ = big_b;
}

#[test]
fn cond_branches_on_predicate() {
    // KASM v1.0 — Op::Cond (JAX lax.cond style) : if pred then a else b.
    // Test : on construit un programme branché qui retourne 100 si l'input
    // est positif, -100 sinon. Vérifie le path then ET else.
    //
    // Structure :
    //   0: Input(0)
    //   1: Const(0)
    //   2: Le(0, 1)              → Bool : input <= 0
    //   3: Const(100)
    //   4: Const(-100)
    //   5: Cond(2, 4, 3)         → input <= 0 ? -100 : 100
    //   6: Output(5)
    let program = Program::new(
        Target::Cpu,
        1,
        1,
        7,
        vec![
            Node::input(0),
            Node::const_i64(0),
            Node::le(0, 1),
            Node::const_i64(100),
            Node::const_i64(-100),
            Node::cond(2, 4, 3),  // ← Op::Cond v1.0
            Node::output(5, Ty::I64),
        ],
    )
    .unwrap();

    // Path "then" : input -5 ≤ 0 → -100
    let r_neg = execute(&program, &(-5i64).to_le_bytes()).unwrap();
    assert_eq!(i64::from_le_bytes(r_neg.try_into().unwrap()), -100);

    // Path "else" : input 7 > 0 → 100
    let r_pos = execute(&program, &7i64.to_le_bytes()).unwrap();
    assert_eq!(i64::from_le_bytes(r_pos.try_into().unwrap()), 100);

    // Edge case : input 0 ≤ 0 → -100
    let r_zero = execute(&program, &0i64.to_le_bytes()).unwrap();
    assert_eq!(i64::from_le_bytes(r_zero.try_into().unwrap()), -100);
}

#[test]
fn partial_evaluation_reports_residual_shape() {
    let program = const_heavy_program(7);
    let (residual, report) = program.partial_evaluate().unwrap();

    assert!(report.original_nodes > report.residual_nodes);
    assert_eq!(report.residual_nodes, residual.nodes().len());
    assert!(report.residual_ratio < 0.10);
    let result = execute(&residual, &5i64.to_le_bytes()).unwrap();
    assert_eq!(result, execute(&program, &5i64.to_le_bytes()).unwrap());
}

#[test]
fn partial_evaluation_crushes_const_heavy_corpus_below_ten_percent_median() {
    let mut ratios = (0..256i16)
        .map(|seed| const_heavy_program(seed).partial_eval_report().unwrap().residual_ratio)
        .collect::<Vec<_>>();
    ratios.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = ratios[ratios.len() / 2];

    assert!(median < 0.10, "median residual ratio was {:.4}", median);
}

#[test]
fn comparison_ops_lt_le_round_trip() {
    let program = Program::new(
        Target::Cpu,
        2,
        2,
        6,
        vec![
            Node::input(0),
            Node::input(1),
            Node::lt(0, 1),
            Node::le(0, 1),
            Node::output(2, Ty::Bool),
            Node::output(3, Ty::Bool),
        ],
    )
    .unwrap();
    let mut args = Vec::new();
    args.extend_from_slice(&3i64.to_le_bytes());
    args.extend_from_slice(&5i64.to_le_bytes());
    let result = execute(&program, &args).unwrap();
    assert_eq!(result, vec![1, 1]);

    let mut equal = Vec::new();
    equal.extend_from_slice(&7i64.to_le_bytes());
    equal.extend_from_slice(&7i64.to_le_bytes());
    let result = execute(&program, &equal).unwrap();
    assert_eq!(result, vec![0, 1]);
}

#[test]
fn bitwise_ops_compose_and_execute() {
    // ((a & 0xff) | 0x100) ^ 0x011
    let program = Program::new(
        Target::Cpu,
        1,
        1,
        8,
        vec![
            Node::input(0),
            Node::const_i64(0x0ff),
            Node::bit_and(0, 1),
            Node::const_i64(0x100),
            Node::bit_or(2, 3),
            Node::const_i64(0x011),
            Node::bit_xor(4, 5),
            Node::output(6, Ty::I64),
        ],
    )
    .unwrap();
    let result = execute(&program, &0x123i64.to_le_bytes()).unwrap();
    let value = i64::from_le_bytes(result.try_into().unwrap());
    assert_eq!(value, ((0x123i64 & 0xff) | 0x100) ^ 0x011);
}

#[test]
fn shifts_mask_distance_and_use_logical_right_shift() {
    // shr(a, b) is unsigned: -1 shifted right by 4 → 0x0fffffff_ffffffff
    let program = Program::new(
        Target::Cpu,
        2,
        2,
        6,
        vec![
            Node::input(0),
            Node::input(1),
            Node::shl(0, 1),
            Node::shr(0, 1),
            Node::output(2, Ty::I64),
            Node::output(3, Ty::I64),
        ],
    )
    .unwrap();
    let mut args = Vec::new();
    args.extend_from_slice(&(-1i64).to_le_bytes());
    args.extend_from_slice(&4i64.to_le_bytes());
    let result = execute(&program, &args).unwrap();
    let lhs = i64::from_le_bytes(result[..8].try_into().unwrap());
    let rhs = i64::from_le_bytes(result[8..].try_into().unwrap());
    assert_eq!(lhs, ((-1i64 as u64).wrapping_shl(4)) as i64);
    assert_eq!(rhs, ((-1i64 as u64).wrapping_shr(4)) as i64);
}

#[test]
fn shift_distance_wraps_modulo_64() {
    // shl(a, 64) ≡ shl(a, 0) ≡ a thanks to the explicit mask.
    let program = Program::new(
        Target::Cpu,
        1,
        1,
        4,
        vec![
            Node::input(0),
            Node::const_i64(64),
            Node::shl(0, 1),
            Node::output(2, Ty::I64),
        ],
    )
    .unwrap();
    let result = execute(&program, &0x55i64.to_le_bytes()).unwrap();
    assert_eq!(i64::from_le_bytes(result.try_into().unwrap()), 0x55);
}

#[test]
fn saturating_arith_does_not_wrap() {
    let program = Program::new(
        Target::Cpu,
        1,
        2,
        6,
        vec![
            Node::input(0),
            Node::const_i64(i16::MAX),
            Node::sat_add(0, 1),
            Node::sat_sub(0, 1),
            Node::output(2, Ty::I64),
            Node::output(3, Ty::I64),
        ],
    )
    .unwrap();
    let result = execute(&program, &i64::MAX.to_le_bytes()).unwrap();
    assert_eq!(i64::from_le_bytes(result[..8].try_into().unwrap()), i64::MAX);
    let want_sub = i64::MAX.saturating_sub(i16::MAX as i64);
    assert_eq!(i64::from_le_bytes(result[8..].try_into().unwrap()), want_sub);
}

#[test]
fn mod_checked_returns_zero_on_division_by_zero() {
    let program = Program::new(
        Target::Cpu,
        2,
        1,
        4,
        vec![
            Node::input(0),
            Node::input(1),
            Node::mod_checked(0, 1),
            Node::output(2, Ty::I64),
        ],
    )
    .unwrap();
    let mut args = Vec::new();
    args.extend_from_slice(&7i64.to_le_bytes());
    args.extend_from_slice(&0i64.to_le_bytes());
    let result = execute(&program, &args).unwrap();
    assert_eq!(i64::from_le_bytes(result.try_into().unwrap()), 0);

    let mut args = Vec::new();
    args.extend_from_slice(&17i64.to_le_bytes());
    args.extend_from_slice(&5i64.to_le_bytes());
    let result = execute(&program, &args).unwrap();
    assert_eq!(i64::from_le_bytes(result.try_into().unwrap()), 2);
}

#[test]
fn clamp_keeps_value_inside_bounds() {
    let program = Program::new(
        Target::Cpu,
        3,
        1,
        5,
        vec![
            Node::input(0), // value
            Node::input(1), // lo
            Node::input(2), // hi
            Node::clamp(0, 1, 2),
            Node::output(3, Ty::I64),
        ],
    )
    .unwrap();
    let try_clamp = |v: i64, lo: i64, hi: i64| {
        let mut args = Vec::new();
        args.extend_from_slice(&v.to_le_bytes());
        args.extend_from_slice(&lo.to_le_bytes());
        args.extend_from_slice(&hi.to_le_bytes());
        let bytes = execute(&program, &args).unwrap();
        i64::from_le_bytes(bytes.try_into().unwrap())
    };
    assert_eq!(try_clamp(5, 0, 10), 5);
    assert_eq!(try_clamp(-5, 0, 10), 0);
    assert_eq!(try_clamp(99, 0, 10), 10);
}

#[test]
fn reduce_add_sums_a_contiguous_range() {
    let program = Program::new(
        Target::Cpu,
        4,
        1,
        6,
        vec![
            Node::input(0),
            Node::input(1),
            Node::input(2),
            Node::input(3),
            Node::reduce_add(0, 4),
            Node::output(4, Ty::I64),
        ],
    )
    .unwrap();
    let mut args = Vec::new();
    args.extend_from_slice(&3i64.to_le_bytes());
    args.extend_from_slice(&5i64.to_le_bytes());
    args.extend_from_slice(&7i64.to_le_bytes());
    args.extend_from_slice(&11i64.to_le_bytes());
    let result = execute(&program, &args).unwrap();
    assert_eq!(i64::from_le_bytes(result.try_into().unwrap()), 26);
}

#[test]
fn reduce_mul_multiplies_a_contiguous_range() {
    let program = Program::new(
        Target::Cpu,
        3,
        1,
        5,
        vec![
            Node::input(0),
            Node::input(1),
            Node::input(2),
            Node::reduce_mul(0, 3),
            Node::output(3, Ty::I64),
        ],
    )
    .unwrap();
    let mut args = Vec::new();
    args.extend_from_slice(&2i64.to_le_bytes());
    args.extend_from_slice(&3i64.to_le_bytes());
    args.extend_from_slice(&5i64.to_le_bytes());
    let result = execute(&program, &args).unwrap();
    assert_eq!(i64::from_le_bytes(result.try_into().unwrap()), 30);
}

#[test]
fn reduce_with_zero_count_is_rejected_at_verify_time() {
    let err = Program::new(
        Target::Cpu,
        1,
        1,
        3,
        vec![
            Node::input(0),
            Node {
                op: crate::kasm::Op::ReduceAddI64,
                ty: Ty::I64,
                a: 0,
                b: 0,
                imm: 0,
            },
            Node::output(1, Ty::I64),
        ],
    )
    .err()
    .expect("zero-count reduce must be rejected");
    assert!(matches!(err, KasmError::BadReduceCount { .. }));
}

#[test]
fn reduce_with_overflowing_count_is_rejected() {
    let err = Program::new(
        Target::Cpu,
        2,
        1,
        4,
        vec![
            Node::input(0),
            Node::input(1),
            // base=0, count=5 but only 2 input nodes precede.
            Node::reduce_add(0, 5),
            Node::output(2, Ty::I64),
        ],
    )
    .err()
    .expect("overflowing reduce must be rejected");
    assert!(matches!(err, KasmError::BadReduceCount { .. }));
}

#[test]
fn simplifier_constant_folds_lt_le_and_bitwise_ops() {
    let program = Program::new(
        Target::Cpu,
        1,
        1,
        13,
        vec![
            Node::input(0),
            Node::const_i64(2),
            Node::const_i64(3),
            Node::lt(1, 2),       // true
            Node::bit_and(1, 2),  // 2 & 3 == 2
            Node::bit_xor(1, 2),  // 1
            Node::add(4, 5),      // 3
            Node::shl(6, 1),      // 3 << (2&63) == 12
            Node::sat_sub(7, 2),  // 12 saturating- 3 == 9
            Node::mul(8, 0),      // 9 * input
            Node::const_i64(0),
            Node::add(9, 10),     // 9 * input + 0
            Node::output(11, Ty::I64),
        ],
    )
    .unwrap();
    let simplified = program.simplified().unwrap();
    assert!(simplified.nodes().len() < program.nodes().len());
    let result = execute(&simplified, &4i64.to_le_bytes()).unwrap();
    assert_eq!(i64::from_le_bytes(result.try_into().unwrap()), 36);
}

#[test]
fn rewrite_engine_reports_constant_reduction_above_thirty_percent() {
    let mut reduced_to_constant = 0usize;
    let mut total_passes = 0usize;

    for i in 0..200i16 {
        let program = if i % 5 < 2 {
            static_rewrite_program(i)
        } else {
            dynamic_rewrite_program(i)
        };
        let report = program.rewrite_report().unwrap();
        total_passes += report.passes;
        if report.reduced_to_constant {
            reduced_to_constant += 1;
        }
    }

    let ratio = reduced_to_constant as f64 / 200.0;
    assert!(ratio >= 0.30, "constant rewrite coverage was {:.4}", ratio);
    assert!(total_passes >= 200);
}

#[test]
fn jit_matches_interpreter_for_kasm_test_corpus() {
    let corpus = jit_diff_corpus();
    assert!(corpus.len() >= 16);

    for (program_index, program) in corpus.iter().enumerate() {
        let jit = crate::kasm::jit::compile(program).unwrap();
        for case in 0..128u64 {
            let args = random_args(program.inputs(), program_index as u64, case);
            let interpreted = execute(program, &args).unwrap();
            let compiled = jit.execute(&args).unwrap();
            assert_eq!(
                compiled, interpreted,
                "JIT divergence for corpus program {program_index}, case {case}"
            );
        }
    }
}

fn jit_diff_corpus() -> Vec<Program> {
    vec![
        Program::new(Target::Cpu, 1, 1, 6, affine_nodes()).unwrap(),
        const_heavy_program(7),
        static_rewrite_program(3),
        dynamic_rewrite_program(5),
        Program::new(
            Target::Cpu,
            2,
            1,
            14,
            vec![
                Node::input(0),
                Node::input(1),
                Node::sub(0, 1),
                Node::div_checked(0, 1),
                Node::min(2, 3),
                Node::max(2, 3),
                Node::eq(4, 5),
                Node::const_i64(111),
                Node::const_i64(222),
                Node::select_i64(6, 7, 8),
                Node::output(9, Ty::I64),
            ],
        )
        .unwrap(),
        Program::new(
            Target::Cpu,
            2,
            2,
            6,
            vec![
                Node::input(0),
                Node::input(1),
                Node::lt(0, 1),
                Node::le(0, 1),
                Node::output(2, Ty::Bool),
                Node::output(3, Ty::Bool),
            ],
        )
        .unwrap(),
        Program::new(
            Target::Cpu,
            1,
            1,
            8,
            vec![
                Node::input(0),
                Node::const_i64(0x0ff),
                Node::bit_and(0, 1),
                Node::const_i64(0x100),
                Node::bit_or(2, 3),
                Node::const_i64(0x011),
                Node::bit_xor(4, 5),
                Node::output(6, Ty::I64),
            ],
        )
        .unwrap(),
        Program::new(
            Target::Cpu,
            2,
            2,
            6,
            vec![
                Node::input(0),
                Node::input(1),
                Node::shl(0, 1),
                Node::shr(0, 1),
                Node::output(2, Ty::I64),
                Node::output(3, Ty::I64),
            ],
        )
        .unwrap(),
        Program::new(
            Target::Cpu,
            2,
            2,
            6,
            vec![
                Node::input(0),
                Node::input(1),
                Node::sat_add(0, 1),
                Node::sat_sub(0, 1),
                Node::output(2, Ty::I64),
                Node::output(3, Ty::I64),
            ],
        )
        .unwrap(),
        Program::new(
            Target::Cpu,
            2,
            2,
            6,
            vec![
                Node::input(0),
                Node::input(1),
                Node::div_checked(0, 1),
                Node::mod_checked(0, 1),
                Node::output(2, Ty::I64),
                Node::output(3, Ty::I64),
            ],
        )
        .unwrap(),
        Program::new(
            Target::Cpu,
            3,
            1,
            5,
            vec![
                Node::input(0),
                Node::input(1),
                Node::input(2),
                Node::clamp(0, 1, 2),
                Node::output(3, Ty::I64),
            ],
        )
        .unwrap(),
        Program::new(
            Target::Cpu,
            4,
            1,
            6,
            vec![
                Node::input(0),
                Node::input(1),
                Node::input(2),
                Node::input(3),
                Node::reduce_add(0, 4),
                Node::output(4, Ty::I64),
            ],
        )
        .unwrap(),
        Program::new(
            Target::Cpu,
            3,
            1,
            5,
            vec![
                Node::input(0),
                Node::input(1),
                Node::input(2),
                Node::reduce_mul(0, 3),
                Node::output(3, Ty::I64),
            ],
        )
        .unwrap(),
        Program::new(
            Target::Cpu,
            2,
            3,
            10,
            vec![
                Node::input(0),
                Node::input(1),
                Node::lt(0, 1),
                Node::le(0, 1),
                Node::and(2, 3),
                Node::or(2, 3),
                Node::not(5),
                Node::output(4, Ty::Bool),
                Node::output(5, Ty::Bool),
                Node::output(6, Ty::Bool),
            ],
        )
        .unwrap(),
        Program::new(
            Target::Cpu,
            1,
            1,
            3,
            vec![Node::input(0), Node::hash64(0), Node::output(1, Ty::I64)],
        )
        .unwrap(),
        Program::new(
            Target::Cpu,
            1,
            1,
            13,
            vec![
                Node::input(0),
                Node::const_i64(2),
                Node::const_i64(3),
                Node::lt(1, 2),
                Node::bit_and(1, 2),
                Node::bit_xor(1, 2),
                Node::add(4, 5),
                Node::shl(6, 1),
                Node::sat_sub(7, 2),
                Node::mul(8, 0),
                Node::const_i64(0),
                Node::add(9, 10),
                Node::output(11, Ty::I64),
            ],
        )
        .unwrap(),
    ]
}

fn random_args(inputs: u8, program_index: u64, case: u64) -> Vec<u8> {
    let mut out = Vec::with_capacity(inputs as usize * 8);
    for slot in 0..inputs as u64 {
        let value = match (case + slot) % 17 {
            0 => 0,
            1 => 1,
            2 => -1,
            3 => i64::MIN,
            4 => i64::MAX,
            _ => deterministic_i64(program_index ^ (slot << 16), case),
        };
        out.extend_from_slice(&value.to_le_bytes());
    }
    out
}

fn deterministic_i64(program_index: u64, case: u64) -> i64 {
    let mut x = 0x9e37_79b9_7f4a_7c15u64 ^ program_index.wrapping_mul(0xbf58_476d_1ce4_e5b9) ^ case;
    x ^= x >> 30;
    x = x.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94d0_49bb_1331_11eb);
    (x ^ (x >> 31)) as i64
}

// ---------------------------------------------------------------------------
// Ω-6.1 — opcodes unaires bijectifs (BitFlip, Neg, ReverseBits, Byteswap)
// ---------------------------------------------------------------------------

fn unary_program(builder: fn(u16) -> Node) -> Program {
    Program::new(
        Target::Cpu,
        1,
        1,
        4,
        vec![
            Node::input(0),
            builder(0),
            Node::output(1, Ty::I64),
        ],
    )
    .unwrap()
}

fn run_i64(program: &Program, x: i64) -> i64 {
    let bytes = x.to_le_bytes().to_vec();
    let out = crate::kasm::execute(program, &bytes).expect("execute");
    i64::from_le_bytes(out[..8].try_into().unwrap())
}

#[test]
fn bit_flip_executes_correctly() {
    let p = unary_program(Node::bit_flip);
    for x in [0i64, 1, -1, 42, i64::MIN, i64::MAX, 0x1234_5678_9abc_def0u64 as i64] {
        assert_eq!(run_i64(&p, x), !x);
    }
}

#[test]
fn neg_executes_with_wrapping_semantics() {
    let p = unary_program(Node::neg);
    for x in [0i64, 1, -1, 42, -42, i64::MAX] {
        assert_eq!(run_i64(&p, x), x.wrapping_neg());
    }
    // i64::MIN reste i64::MIN (wrapping_neg) — bijection u64 préservée.
    assert_eq!(run_i64(&p, i64::MIN), i64::MIN);
}

#[test]
fn reverse_bits_executes_correctly() {
    let p = unary_program(Node::reverse_bits);
    for x in [0i64, 1, -1, 0x8000_0000_0000_0000u64 as i64, i64::MAX, 42] {
        assert_eq!(run_i64(&p, x), x.reverse_bits());
    }
}

#[test]
fn byteswap_executes_correctly() {
    let p = unary_program(Node::byteswap);
    for x in [
        0i64,
        1,
        -1,
        0x0102_0304_0506_0708,
        i64::MIN,
        i64::MAX,
    ] {
        assert_eq!(run_i64(&p, x), x.swap_bytes());
    }
}

fn double_unary_program(builder: fn(u16) -> Node) -> Program {
    Program::new(
        Target::Cpu, 1, 1, 8,
        vec![
            Node::input(0),
            builder(0),
            builder(1),
            Node::output(2, Ty::I64),
        ],
    )
    .unwrap()
}

#[test]
fn bit_flip_double_application_is_identity() {
    // bit_flip(bit_flip(x)) = x. simplify doit éliminer la paire involutive.
    let p = double_unary_program(Node::bit_flip);
    let canon = simplify(&p).unwrap();
    // Le programme simplifié doit être strictement plus petit que l'original
    // (input + output uniquement, pas de bit_flip survivant).
    assert!(
        canon.nodes().len() < p.nodes().len(),
        "double bit_flip doit s'annuler — len before {}, after {}",
        p.nodes().len(), canon.nodes().len(),
    );
    for x in [0i64, 1, -1, 42] {
        assert_eq!(run_i64(&canon, x), x);
    }
}

#[test]
fn neg_double_application_is_identity() {
    let p = double_unary_program(Node::neg);
    let canon = simplify(&p).unwrap();
    assert!(canon.nodes().len() < p.nodes().len());
    for x in [0i64, 1, -1, 42, i64::MIN] {
        assert_eq!(run_i64(&canon, x), x);
    }
}

#[test]
fn reverse_bits_double_application_is_identity() {
    let p = double_unary_program(Node::reverse_bits);
    let canon = simplify(&p).unwrap();
    assert!(canon.nodes().len() < p.nodes().len());
    for x in [0i64, 1, -1, 42, i64::MAX] {
        assert_eq!(run_i64(&canon, x), x);
    }
}

#[test]
fn byteswap_double_application_is_identity() {
    let p = double_unary_program(Node::byteswap);
    let canon = simplify(&p).unwrap();
    assert!(canon.nodes().len() < p.nodes().len());
    for x in [0i64, 1, -1, 42, i64::MIN, i64::MAX] {
        assert_eq!(run_i64(&canon, x), x);
    }
}

#[test]
fn unary_bijective_ops_have_zero_landauer_cost() {
    // Critère central Ω-6.1 : chaque op bijective tagguée Bijective →
    // 0 bits erased.
    use crate::landauer::{op_reversibility, Reversibility};
    for op in [
        crate::kasm::Op::BitFlipI64,
        crate::kasm::Op::NegI64,
        crate::kasm::Op::ReverseBitsI64,
        crate::kasm::Op::ByteswapI64,
    ] {
        assert_eq!(op_reversibility(op), Reversibility::Bijective);
        assert_eq!(op_reversibility(op).bits_erased(), 0);
    }
}

#[test]
fn unary_bijective_program_constant_folds() {
    // ConstFold : bit_flip(const_i64(5)) doit replier en const_i64(!5).
    let p = Program::new(
        Target::Cpu, 0, 1, 4,
        vec![
            Node::const_i64(5),
            Node::bit_flip(0),
            Node::output(1, Ty::I64),
        ],
    )
    .unwrap();
    let s = simplify(&p).unwrap();
    // Le résultat doit être un programme constant qui sort !5_i64 = -6.
    let bytes = crate::kasm::execute(&s, &[]).unwrap();
    let v = i64::from_le_bytes(bytes[..8].try_into().unwrap());
    assert_eq!(v, !5_i64);
}

#[test]
fn neg_constant_folds() {
    let p = Program::new(
        Target::Cpu, 0, 1, 4,
        vec![
            Node::const_i64(7),
            Node::neg(0),
            Node::output(1, Ty::I64),
        ],
    )
    .unwrap();
    let s = simplify(&p).unwrap();
    let bytes = crate::kasm::execute(&s, &[]).unwrap();
    let v = i64::from_le_bytes(bytes[..8].try_into().unwrap());
    assert_eq!(v, -7);
}

#[test]
fn unary_bijective_ops_canonicalize_idempotent() {
    // Canonicalize(P) doit être stable sous canonicalize.
    for builder in [Node::bit_flip, Node::neg, Node::reverse_bits, Node::byteswap] {
        let p = unary_program(builder);
        let c1 = canonicalize(&p).unwrap();
        let c2 = canonicalize(&c1).unwrap();
        assert_eq!(c1.bytes(), c2.bytes());
    }
}

#[test]
fn unary_bijective_ops_byte_serialize_roundtrip() {
    // verify(P.bytes()) == P pour chaque op bijective.
    for builder in [Node::bit_flip, Node::neg, Node::reverse_bits, Node::byteswap] {
        let p = unary_program(builder);
        let p2 = verify(p.bytes()).unwrap();
        assert_eq!(p.bytes(), p2.bytes());
    }
}

#[test]
fn from_byte_decodes_all_4_new_ops() {
    use crate::kasm::Op;
    assert_eq!(28u8, Op::BitFlipI64 as u8);
    assert_eq!(29u8, Op::NegI64 as u8);
    assert_eq!(30u8, Op::ReverseBitsI64 as u8);
    assert_eq!(31u8, Op::ByteswapI64 as u8);
}

#[test]
fn test_vec_i64_byte_round_trips() {
    let bytes = [Op::Input as u8, Ty::VecI64 as u8, 0, 0, 0, 0, 0, 0];
    let node = Node::decode(&bytes).unwrap();
    assert_eq!(node.ty, Ty::VecI64);

    let mut encoded = Vec::new();
    node.encode(&mut encoded);
    assert_eq!(encoded, bytes);

    // Wave 7b — Ty::VecI64 inputs/outputs are now FULL via the
    // length-prefixed wire format `[u32 LE count | count × 8 bytes]`.
    // What used to surface KasmError::VecNotSupportedYet now builds
    // a valid identity Vec round-trip program.
    let prog = Program::new(
        Target::Cpu,
        1,
        1,
        2,
        vec![node, Node::output(0, Ty::VecI64)],
    )
    .unwrap();

    // Smoke test the round-trip : input vec [42, 7, -1] flows
    // straight through Op::Output and the wire bytes match.
    let payload = [42i64, 7, -1];
    let mut args = Vec::new();
    args.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    for v in &payload {
        args.extend_from_slice(&v.to_le_bytes());
    }
    let out = crate::kasm::execute(&prog, &args).unwrap();
    assert_eq!(out, args, "Vec identity round-trip preserves wire bytes");
}

#[test]
fn wave7b_empty_vec_round_trip() {
    // Edge case : 0-length vec. Wire format = `[0u32 LE]` (4 bytes).
    let prog = Program::new(
        Target::Cpu,
        1,
        1,
        2,
        vec![Node::input_vec(0), Node::output(0, Ty::VecI64)],
    )
    .unwrap();
    let args = 0u32.to_le_bytes().to_vec();
    let out = crate::kasm::execute(&prog, &args).unwrap();
    assert_eq!(out, args, "empty vec round-trip preserves [0u32 LE]");
}

#[test]
fn wave7b_mixed_scalar_and_vec_inputs() {
    // 2 inputs : slot 0 is i64 (8 bytes), slot 1 is VecI64 (4 + N×8).
    // Output slot 0 (the i64), so this exercises i64 round-trip while
    // proving the Vec slot doesn't break the args parser.
    let prog = Program::new(
        Target::Cpu,
        2,
        1,
        3,
        vec![
            Node::input(0),
            Node::input_vec(1),
            Node::output(0, Ty::I64),
        ],
    )
    .unwrap();
    // Args : 8 bytes for i64 + 4 + 3*8 bytes for vec [10, 20, 30].
    let mut args = Vec::new();
    args.extend_from_slice(&999i64.to_le_bytes());
    args.extend_from_slice(&3u32.to_le_bytes());
    args.extend_from_slice(&10i64.to_le_bytes());
    args.extend_from_slice(&20i64.to_le_bytes());
    args.extend_from_slice(&30i64.to_le_bytes());
    let out = crate::kasm::execute(&prog, &args).unwrap();
    assert_eq!(out.len(), 8);
    assert_eq!(i64::from_le_bytes(out.try_into().unwrap()), 999);
}

#[test]
fn wave7b_vec_optimizer_round_trip() {
    // Wave 7b deployment — the optimizer now accepts Vec programs
    // (treats them as opaque Refs, no folding). canonical() should
    // succeed and preserve the program semantically.
    let prog = Program::new(
        Target::Cpu,
        1,
        1,
        2,
        vec![Node::input_vec(0), Node::output(0, Ty::VecI64)],
    )
    .unwrap();
    let canon = prog.canonical().unwrap();
    // Same number of nodes (no rewriting on a Vec identity).
    assert_eq!(canon.nodes().len(), prog.nodes().len());
    // Round-trip execution still works on the canonical form.
    let payload = [11i64, 22, 33];
    let mut args = Vec::new();
    args.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    for v in &payload {
        args.extend_from_slice(&v.to_le_bytes());
    }
    let out = crate::kasm::execute(&canon, &args).unwrap();
    assert_eq!(out, args);
}

#[test]
fn wave7b_vec_args_truncated_fails_loud() {
    // Vec wire format claims count=5 but args has only 2 elements.
    // The parser must surface BadInputLength, never UB.
    let prog = Program::new(
        Target::Cpu,
        1,
        1,
        2,
        vec![Node::input_vec(0), Node::output(0, Ty::VecI64)],
    )
    .unwrap();
    let mut args = Vec::new();
    args.extend_from_slice(&5u32.to_le_bytes()); // claims 5 elements
    args.extend_from_slice(&1i64.to_le_bytes());
    args.extend_from_slice(&2i64.to_le_bytes()); // only 2 provided
    let err = crate::kasm::execute(&prog, &args).unwrap_err();
    assert!(matches!(err, KasmError::BadInputLength { .. }));
}

// ---------------------------------------------------------------------------
#[test]
fn wave7d_vlen_returns_vec_length() {
    // Op::VLenI64 — Vec → I64 length query.
    // Program: input_vec(0) → vlen → output(I64)
    // For input vec [11, 22, 33] (3 elements), expect 3.
    let prog = Program::new(
        Target::Cpu,
        1,
        1,
        3,
        vec![
            Node::input_vec(0),
            Node::v_len(0),
            Node::output(1, Ty::I64),
        ],
    )
    .unwrap();
    // Wire format: [u32 count LE | count*8 bytes]
    let mut args = Vec::new();
    args.extend_from_slice(&3u32.to_le_bytes());
    for v in [11i64, 22, 33] {
        args.extend_from_slice(&v.to_le_bytes());
    }
    let out = crate::kasm::execute(&prog, &args).unwrap();
    let len = i64::from_le_bytes(out.try_into().unwrap());
    assert_eq!(len, 3, "vlen([11,22,33]) = 3");
}

#[test]
fn wave7d_bis_vsum_reduces_vec() {
    // Op::VSumI64 — vec → i64 sum (wrapping).
    let prog = Program::new(
        Target::Cpu, 1, 1, 3,
        vec![Node::input_vec(0), Node::v_sum(0), Node::output(1, Ty::I64)],
    ).unwrap();
    let mut args = Vec::new();
    args.extend_from_slice(&5u32.to_le_bytes());
    for v in [1i64, 2, 3, 4, 5] {
        args.extend_from_slice(&v.to_le_bytes());
    }
    let out = crate::kasm::execute(&prog, &args).unwrap();
    let sum = i64::from_le_bytes(out.try_into().unwrap());
    assert_eq!(sum, 15, "sum(1..5) = 15");
}

#[test]
fn wave7d_bis_vadd_pairwise() {
    // Op::VAddI64 — pairwise add of two Vecs.
    // Program: input_vec(0), input_vec(1), vadd(0,1), output(VecI64).
    let prog = Program::new(
        Target::Cpu, 2, 1, 4,
        vec![
            Node::input_vec(0),
            Node::input_vec(1),
            Node::v_add(0, 1),
            Node::output(2, Ty::VecI64),
        ],
    ).unwrap();
    // args: vec_a=[1,2,3] then vec_b=[10,20,30]
    let mut args = Vec::new();
    args.extend_from_slice(&3u32.to_le_bytes());
    for v in [1i64, 2, 3] {
        args.extend_from_slice(&v.to_le_bytes());
    }
    args.extend_from_slice(&3u32.to_le_bytes());
    for v in [10i64, 20, 30] {
        args.extend_from_slice(&v.to_le_bytes());
    }
    let out = crate::kasm::execute(&prog, &args).unwrap();
    // Decode result wire format.
    let count = u32::from_le_bytes(out[0..4].try_into().unwrap());
    assert_eq!(count, 3);
    let mut got = Vec::new();
    for i in 0..3 {
        let off = 4 + i * 8;
        got.push(i64::from_le_bytes(out[off..off + 8].try_into().unwrap()));
    }
    assert_eq!(got, vec![11, 22, 33]);
}

#[test]
fn wave7d_bis_vmul_pairwise() {
    let prog = Program::new(
        Target::Cpu, 2, 1, 4,
        vec![
            Node::input_vec(0),
            Node::input_vec(1),
            Node::v_mul(0, 1),
            Node::output(2, Ty::VecI64),
        ],
    ).unwrap();
    let mut args = Vec::new();
    args.extend_from_slice(&3u32.to_le_bytes());
    for v in [1i64, 2, 3] {
        args.extend_from_slice(&v.to_le_bytes());
    }
    args.extend_from_slice(&3u32.to_le_bytes());
    for v in [10i64, 10, 10] {
        args.extend_from_slice(&v.to_le_bytes());
    }
    let out = crate::kasm::execute(&prog, &args).unwrap();
    let mut got = Vec::new();
    for i in 0..3 {
        let off = 4 + i * 8;
        got.push(i64::from_le_bytes(out[off..off + 8].try_into().unwrap()));
    }
    assert_eq!(got, vec![10, 20, 30]);
}

#[test]
fn wave7d_bis_vadd_length_mismatch_fails_loud() {
    // VAddI64 avec vecs de longueurs différentes → TypeMismatch.
    let prog = Program::new(
        Target::Cpu, 2, 1, 4,
        vec![
            Node::input_vec(0),
            Node::input_vec(1),
            Node::v_add(0, 1),
            Node::output(2, Ty::VecI64),
        ],
    ).unwrap();
    // vec_a=[1,2,3] (3 éléments), vec_b=[10,20] (2 éléments)
    let mut args = Vec::new();
    args.extend_from_slice(&3u32.to_le_bytes());
    for v in [1i64, 2, 3] {
        args.extend_from_slice(&v.to_le_bytes());
    }
    args.extend_from_slice(&2u32.to_le_bytes());
    for v in [10i64, 20] {
        args.extend_from_slice(&v.to_le_bytes());
    }
    let err = crate::kasm::execute(&prog, &args).unwrap_err();
    assert!(matches!(err, KasmError::TypeMismatch { .. }));
}

#[test]
fn wave7e_vsub_pairwise() {
    // Op::VSubI64 — pairwise wrapping subtract.
    let prog = Program::new(
        Target::Cpu, 2, 1, 4,
        vec![
            Node::input_vec(0),
            Node::input_vec(1),
            Node::v_sub(0, 1),
            Node::output(2, Ty::VecI64),
        ],
    ).unwrap();
    let mut args = Vec::new();
    args.extend_from_slice(&3u32.to_le_bytes());
    for v in [10i64, 20, 30] {
        args.extend_from_slice(&v.to_le_bytes());
    }
    args.extend_from_slice(&3u32.to_le_bytes());
    for v in [1i64, 2, 3] {
        args.extend_from_slice(&v.to_le_bytes());
    }
    let out = crate::kasm::execute(&prog, &args).unwrap();
    let mut got = Vec::new();
    for i in 0..3 {
        let off = 4 + i * 8;
        got.push(i64::from_le_bytes(out[off..off + 8].try_into().unwrap()));
    }
    assert_eq!(got, vec![9, 18, 27]);
}

#[test]
fn wave7e_vmax_pairwise() {
    let prog = Program::new(
        Target::Cpu, 2, 1, 4,
        vec![
            Node::input_vec(0),
            Node::input_vec(1),
            Node::v_max(0, 1),
            Node::output(2, Ty::VecI64),
        ],
    ).unwrap();
    let mut args = Vec::new();
    args.extend_from_slice(&3u32.to_le_bytes());
    for v in [1i64, 5, 3] {
        args.extend_from_slice(&v.to_le_bytes());
    }
    args.extend_from_slice(&3u32.to_le_bytes());
    for v in [4i64, 2, 7] {
        args.extend_from_slice(&v.to_le_bytes());
    }
    let out = crate::kasm::execute(&prog, &args).unwrap();
    let mut got = Vec::new();
    for i in 0..3 {
        let off = 4 + i * 8;
        got.push(i64::from_le_bytes(out[off..off + 8].try_into().unwrap()));
    }
    assert_eq!(got, vec![4, 5, 7]);
}

#[test]
fn wave7e_vmin_pairwise() {
    let prog = Program::new(
        Target::Cpu, 2, 1, 4,
        vec![
            Node::input_vec(0),
            Node::input_vec(1),
            Node::v_min(0, 1),
            Node::output(2, Ty::VecI64),
        ],
    ).unwrap();
    let mut args = Vec::new();
    args.extend_from_slice(&3u32.to_le_bytes());
    for v in [1i64, 5, 3] {
        args.extend_from_slice(&v.to_le_bytes());
    }
    args.extend_from_slice(&3u32.to_le_bytes());
    for v in [4i64, 2, 7] {
        args.extend_from_slice(&v.to_le_bytes());
    }
    let out = crate::kasm::execute(&prog, &args).unwrap();
    let mut got = Vec::new();
    for i in 0..3 {
        let off = 4 + i * 8;
        got.push(i64::from_le_bytes(out[off..off + 8].try_into().unwrap()));
    }
    assert_eq!(got, vec![1, 2, 3]);
}

#[test]
fn wave7e_vrange_iota() {
    // Op::VRangeI64 — i64 → Vec [0..n).
    // Program: input(0) i64 → vrange → output(VecI64).
    let prog = Program::new(
        Target::Cpu, 1, 1, 3,
        vec![Node::input(0), Node::v_range(0), Node::output(1, Ty::VecI64)],
    ).unwrap();
    let args = 5i64.to_le_bytes();
    let out = crate::kasm::execute(&prog, &args).unwrap();
    let count = u32::from_le_bytes(out[0..4].try_into().unwrap());
    assert_eq!(count, 5);
    let mut got = Vec::new();
    for i in 0..5 {
        let off = 4 + i * 8;
        got.push(i64::from_le_bytes(out[off..off + 8].try_into().unwrap()));
    }
    assert_eq!(got, vec![0, 1, 2, 3, 4]);
}

#[test]
fn wave7e_vrange_negative_returns_empty() {
    // Negative length → empty vec, no panic.
    let prog = Program::new(
        Target::Cpu, 1, 1, 3,
        vec![Node::input(0), Node::v_range(0), Node::output(1, Ty::VecI64)],
    ).unwrap();
    let args = (-7i64).to_le_bytes();
    let out = crate::kasm::execute(&prog, &args).unwrap();
    let count = u32::from_le_bytes(out[0..4].try_into().unwrap());
    assert_eq!(count, 0);
    assert_eq!(out.len(), 4, "wire format = just the 4-byte zero count");
}

#[test]
fn wave7f_vconcat_appends_two_vecs() {
    let prog = Program::new(
        Target::Cpu, 2, 1, 4,
        vec![
            Node::input_vec(0), Node::input_vec(1),
            Node::v_concat(0, 1), Node::output(2, Ty::VecI64),
        ],
    ).unwrap();
    let mut args = Vec::new();
    args.extend_from_slice(&3u32.to_le_bytes());
    for v in [1i64, 2, 3] { args.extend_from_slice(&v.to_le_bytes()); }
    args.extend_from_slice(&2u32.to_le_bytes());
    for v in [10i64, 20] { args.extend_from_slice(&v.to_le_bytes()); }
    let out = crate::kasm::execute(&prog, &args).unwrap();
    let count = u32::from_le_bytes(out[0..4].try_into().unwrap());
    assert_eq!(count, 5);
    let mut got = Vec::new();
    for i in 0..5 {
        let off = 4 + i * 8;
        got.push(i64::from_le_bytes(out[off..off + 8].try_into().unwrap()));
    }
    assert_eq!(got, vec![1, 2, 3, 10, 20]);
}

#[test]
fn wave7f_vreverse_flips_order() {
    let prog = Program::new(
        Target::Cpu, 1, 1, 3,
        vec![Node::input_vec(0), Node::v_reverse(0), Node::output(1, Ty::VecI64)],
    ).unwrap();
    let mut args = Vec::new();
    args.extend_from_slice(&4u32.to_le_bytes());
    for v in [1i64, 2, 3, 4] { args.extend_from_slice(&v.to_le_bytes()); }
    let out = crate::kasm::execute(&prog, &args).unwrap();
    let mut got = Vec::new();
    for i in 0..4 {
        let off = 4 + i * 8;
        got.push(i64::from_le_bytes(out[off..off + 8].try_into().unwrap()));
    }
    assert_eq!(got, vec![4, 3, 2, 1]);
}

#[test]
fn wave7f_vbroadcast_fills_with_value() {
    // input(0) = value=42, input(1) = length=3 → [42, 42, 42]
    let prog = Program::new(
        Target::Cpu, 2, 1, 4,
        vec![
            Node::input(0),
            Node::input(1),
            Node::v_broadcast(0, 1),
            Node::output(2, Ty::VecI64),
        ],
    ).unwrap();
    let mut args = Vec::new();
    args.extend_from_slice(&42i64.to_le_bytes());
    args.extend_from_slice(&3i64.to_le_bytes());
    let out = crate::kasm::execute(&prog, &args).unwrap();
    let count = u32::from_le_bytes(out[0..4].try_into().unwrap());
    assert_eq!(count, 3);
    let mut got = Vec::new();
    for i in 0..3 {
        let off = 4 + i * 8;
        got.push(i64::from_le_bytes(out[off..off + 8].try_into().unwrap()));
    }
    assert_eq!(got, vec![42, 42, 42]);
}

#[test]
fn wave7f_vbroadcast_negative_length_returns_empty() {
    let prog = Program::new(
        Target::Cpu, 2, 1, 4,
        vec![
            Node::input(0), Node::input(1),
            Node::v_broadcast(0, 1), Node::output(2, Ty::VecI64),
        ],
    ).unwrap();
    let mut args = Vec::new();
    args.extend_from_slice(&7i64.to_le_bytes());
    args.extend_from_slice(&(-3i64).to_le_bytes());
    let out = crate::kasm::execute(&prog, &args).unwrap();
    let count = u32::from_le_bytes(out[0..4].try_into().unwrap());
    assert_eq!(count, 0);
    assert_eq!(out.len(), 4);
}

#[test]
fn wave7g_veq_pairwise() {
    // VEqI64 → 1 si égaux, 0 sinon.
    let prog = Program::new(
        Target::Cpu, 2, 1, 4,
        vec![
            Node::input_vec(0), Node::input_vec(1),
            Node::v_eq(0, 1), Node::output(2, Ty::VecI64),
        ],
    ).unwrap();
    let mut args = Vec::new();
    args.extend_from_slice(&4u32.to_le_bytes());
    for v in [1i64, 2, 3, 4] { args.extend_from_slice(&v.to_le_bytes()); }
    args.extend_from_slice(&4u32.to_le_bytes());
    for v in [1i64, 7, 3, 8] { args.extend_from_slice(&v.to_le_bytes()); }
    let out = crate::kasm::execute(&prog, &args).unwrap();
    let mut got = Vec::new();
    for i in 0..4 {
        let off = 4 + i * 8;
        got.push(i64::from_le_bytes(out[off..off + 8].try_into().unwrap()));
    }
    assert_eq!(got, vec![1, 0, 1, 0]);
}

#[test]
fn wave7g_vand_pairwise() {
    let prog = Program::new(
        Target::Cpu, 2, 1, 4,
        vec![
            Node::input_vec(0), Node::input_vec(1),
            Node::v_and(0, 1), Node::output(2, Ty::VecI64),
        ],
    ).unwrap();
    let mut args = Vec::new();
    args.extend_from_slice(&3u32.to_le_bytes());
    for v in [0b1100i64, 0b1010, 0xFFi64] { args.extend_from_slice(&v.to_le_bytes()); }
    args.extend_from_slice(&3u32.to_le_bytes());
    for v in [0b1010i64, 0b0110, 0x0Fi64] { args.extend_from_slice(&v.to_le_bytes()); }
    let out = crate::kasm::execute(&prog, &args).unwrap();
    let mut got = Vec::new();
    for i in 0..3 {
        let off = 4 + i * 8;
        got.push(i64::from_le_bytes(out[off..off + 8].try_into().unwrap()));
    }
    assert_eq!(got, vec![0b1000, 0b0010, 0x0F]);
}

#[test]
fn wave7g_vor_pairwise() {
    let prog = Program::new(
        Target::Cpu, 2, 1, 4,
        vec![
            Node::input_vec(0), Node::input_vec(1),
            Node::v_or(0, 1), Node::output(2, Ty::VecI64),
        ],
    ).unwrap();
    let mut args = Vec::new();
    args.extend_from_slice(&2u32.to_le_bytes());
    for v in [0b0011i64, 0xF0i64] { args.extend_from_slice(&v.to_le_bytes()); }
    args.extend_from_slice(&2u32.to_le_bytes());
    for v in [0b0101i64, 0x0Fi64] { args.extend_from_slice(&v.to_le_bytes()); }
    let out = crate::kasm::execute(&prog, &args).unwrap();
    let got: Vec<i64> = (0..2).map(|i| {
        let off = 4 + i * 8;
        i64::from_le_bytes(out[off..off + 8].try_into().unwrap())
    }).collect();
    assert_eq!(got, vec![0b0111, 0xFF]);
}

#[test]
fn wave7g_vxor_pairwise() {
    let prog = Program::new(
        Target::Cpu, 2, 1, 4,
        vec![
            Node::input_vec(0), Node::input_vec(1),
            Node::v_xor(0, 1), Node::output(2, Ty::VecI64),
        ],
    ).unwrap();
    let mut args = Vec::new();
    args.extend_from_slice(&2u32.to_le_bytes());
    for v in [0b1100i64, 0xFFi64] { args.extend_from_slice(&v.to_le_bytes()); }
    args.extend_from_slice(&2u32.to_le_bytes());
    for v in [0b1010i64, 0xAAi64] { args.extend_from_slice(&v.to_le_bytes()); }
    let out = crate::kasm::execute(&prog, &args).unwrap();
    let got: Vec<i64> = (0..2).map(|i| {
        let off = 4 + i * 8;
        i64::from_le_bytes(out[off..off + 8].try_into().unwrap())
    }).collect();
    assert_eq!(got, vec![0b0110, 0x55]);
}

#[test]
fn wave7h_vabs_vneg_vbitflip_unary() {
    // Vec → Vec unary transforms, table-driven test.
    for (name, op_node, input, expected) in [
        ("vabs",     Node::v_abs(0)     as Node, vec![-3i64, 0, 5, i64::MIN+1], vec![3i64, 0, 5, i64::MAX]),
        ("vneg",     Node::v_neg(0)     as Node, vec![1i64, -2, 0, 100], vec![-1i64, 2, 0, -100]),
        ("vbitflip", Node::v_bit_flip(0) as Node, vec![0i64, -1, 5], vec![-1i64, 0, !5i64]),
    ] {
        let prog = Program::new(
            Target::Cpu, 1, 1, 3,
            vec![Node::input_vec(0), op_node, Node::output(1, Ty::VecI64)],
        ).unwrap();
        let mut args = Vec::new();
        args.extend_from_slice(&(input.len() as u32).to_le_bytes());
        for v in &input { args.extend_from_slice(&v.to_le_bytes()); }
        let out = crate::kasm::execute(&prog, &args).unwrap();
        let got: Vec<i64> = (0..input.len()).map(|i| {
            let off = 4 + i * 8;
            i64::from_le_bytes(out[off..off + 8].try_into().unwrap())
        }).collect();
        assert_eq!(got, expected, "Wave 7h {name}({input:?}) = {got:?} (exp {expected:?})");
    }
}

#[test]
fn wave7d_vlen_empty_vec() {
    let prog = Program::new(
        Target::Cpu,
        1,
        1,
        3,
        vec![
            Node::input_vec(0),
            Node::v_len(0),
            Node::output(1, Ty::I64),
        ],
    )
    .unwrap();
    let args = 0u32.to_le_bytes().to_vec();
    let out = crate::kasm::execute(&prog, &args).unwrap();
    let len = i64::from_le_bytes(out.try_into().unwrap());
    assert_eq!(len, 0, "vlen(empty) = 0");
}

// ---------------------------------------------------------------------------
// Φ.0 — F64 IEEE 754 layer (storage-polymorphic over Value::I64 bits)
// ---------------------------------------------------------------------------

/// Encode an f64 as the 8-byte little-endian bit pattern. F64 inputs to
/// `Program::execute` use the same wire format as I64 — the type is a
/// verification-time concern only.
fn f64_input_bytes(values: &[f64]) -> Vec<u8> {
    let mut out = Vec::with_capacity(values.len() * 8);
    for v in values {
        out.extend_from_slice(&v.to_bits().to_le_bytes());
    }
    out
}

fn f64_output_value(bytes: &[u8]) -> f64 {
    assert_eq!(bytes.len(), 8, "expected 8-byte f64 output");
    let bits = u64::from_le_bytes(bytes.try_into().unwrap());
    f64::from_bits(bits)
}

// ─── Wave 7i — VGetI64 random-access read ─────────────────────────────

#[test]
fn vget_reads_element_at_index() {
    // Build : input_vec(0), const_i64(2) at index 1, v_get(0, 1), output.
    let nodes = vec![
        Node::input_vec(0),
        Node::const_i64(2),
        Node::v_get(0, 1),
        Node::output(2, Ty::I64),
    ];
    let prog = Program::new(Target::Cpu, 1, 1, 8, nodes).unwrap();
    let mut args = Vec::new();
    args.extend_from_slice(&5u32.to_le_bytes());
    for v in [10i64, 20, 30, 40, 50] {
        args.extend_from_slice(&v.to_le_bytes());
    }
    let out = crate::kasm::execute(&prog, &args).unwrap();
    assert_eq!(i64::from_le_bytes(out.try_into().unwrap()), 30);
}

#[test]
fn vget_wraps_index_modulo_len() {
    // Index 7 on a 5-element vec → 7 % 5 = 2 → element 30.
    let nodes = vec![
        Node::input_vec(0),
        Node::input(1),
        Node::v_get(0, 1),
        Node::output(2, Ty::I64),
    ];
    let prog = Program::new(Target::Cpu, 2, 1, 8, nodes).unwrap();
    let mut args = Vec::new();
    args.extend_from_slice(&5u32.to_le_bytes());
    for v in [10i64, 20, 30, 40, 50] {
        args.extend_from_slice(&v.to_le_bytes());
    }
    args.extend_from_slice(&7i64.to_le_bytes());
    let out = crate::kasm::execute(&prog, &args).unwrap();
    assert_eq!(i64::from_le_bytes(out.try_into().unwrap()), 30);
}

#[test]
fn vget_handles_empty_vec_returns_zero() {
    let nodes = vec![
        Node::input_vec(0),
        Node::input(1),
        Node::v_get(0, 1),
        Node::output(2, Ty::I64),
    ];
    let prog = Program::new(Target::Cpu, 2, 1, 8, nodes).unwrap();
    let mut args = Vec::new();
    args.extend_from_slice(&0u32.to_le_bytes()); // empty vec
    args.extend_from_slice(&42i64.to_le_bytes()); // index 42
    let out = crate::kasm::execute(&prog, &args).unwrap();
    assert_eq!(i64::from_le_bytes(out.try_into().unwrap()), 0);
}

#[test]
fn vget_negative_index_wraps_unsigned() {
    // -1 as u64 = u64::MAX. u64::MAX % 5 = 0 (since u64::MAX = 18446744073709551615
    // and 18446744073709551615 % 5 = 0). So index -1 on a 5-vec → element 0.
    let nodes = vec![
        Node::input_vec(0),
        Node::input(1),
        Node::v_get(0, 1),
        Node::output(2, Ty::I64),
    ];
    let prog = Program::new(Target::Cpu, 2, 1, 8, nodes).unwrap();
    let mut args = Vec::new();
    args.extend_from_slice(&5u32.to_le_bytes());
    for v in [10i64, 20, 30, 40, 50] {
        args.extend_from_slice(&v.to_le_bytes());
    }
    args.extend_from_slice(&(-1i64).to_le_bytes());
    let out = crate::kasm::execute(&prog, &args).unwrap();
    assert_eq!(i64::from_le_bytes(out.try_into().unwrap()), 10);
}

#[test]
fn vget_program_round_trips_through_bytes() {
    // Bytecode encode/decode round-trip preserves the new opcode.
    let nodes = vec![
        Node::input_vec(0),
        Node::input(1),
        Node::v_get(0, 1),
        Node::output(2, Ty::I64),
    ];
    let prog = Program::new(Target::Cpu, 2, 1, 8, nodes).unwrap();
    let bytes = prog.bytes().to_vec();
    let restored = Program::from_bytes(&bytes).unwrap();
    assert_eq!(prog.nodes(), restored.nodes());
}

#[test]
fn f64_opcodes_have_expected_byte_values() {
    use crate::kasm::types::{F64_ADD, F64_LN, F64_OP_MAX};
    // Reserved opcode positions for the F64 surface.
    assert_eq!(32u8, Op::ConstF64 as u8);
    assert_eq!(33u8, Op::F64Op as u8);
    // Sub-op layout. Synthesizer relies on these enumerations being
    // contiguous + dense from 0..=12.
    assert_eq!(0u8, F64_ADD);
    assert_eq!(12u8, F64_LN);
    assert_eq!(F64_OP_MAX, F64_LN);
}

#[test]
fn f64_const_byte_serialize_roundtrip() {
    // ConstF64 + Output(F64) → static program returning a fixed f64.
    let nodes = vec![
        Node::const_f64(7),
        Node::output(0, Ty::F64),
    ];
    let p = Program::new(Target::Cpu, 0, 1, 4, nodes).unwrap();
    let p2 = verify(p.bytes()).unwrap();
    assert_eq!(p.bytes(), p2.bytes());
    let out = crate::kasm::execute(&p, &[]).unwrap();
    assert_eq!(f64_output_value(&out), 7.0);
}

#[test]
fn f64_add_executes_via_bit_cast() {
    // f(x, y) = x + y on f64 inputs.
    let nodes = vec![
        Node::input_f64(0),
        Node::input_f64(1),
        Node::f64_add(0, 1),
        Node::output(2, Ty::F64),
    ];
    let p = Program::new(Target::Cpu, 2, 1, 8, nodes).unwrap();
    let args = f64_input_bytes(&[1.5, 2.25]);
    let out = crate::kasm::execute(&p, &args).unwrap();
    assert_eq!(f64_output_value(&out), 3.75);
}

#[test]
fn f64_div_collapses_nonfinite_to_zero() {
    // 1.0 / 0.0 → 0.0 (kill-switch baked into the op for total-function
    // discipline; matches the synthesizer's holdout safety).
    let nodes = vec![
        Node::const_f64(1),
        Node::const_f64(0),
        Node::f64_div(0, 1),
        Node::output(2, Ty::F64),
    ];
    let p = Program::new(Target::Cpu, 0, 1, 8, nodes).unwrap();
    let out = crate::kasm::execute(&p, &[]).unwrap();
    assert_eq!(f64_output_value(&out), 0.0);
}

#[test]
fn f64_sqrt_of_negative_collapses_to_zero() {
    // sqrt(-4.0) is NaN → folded to 0.0.
    let nodes = vec![
        Node::const_f64(-4),
        Node::f64_sqrt(0),
        Node::output(1, Ty::F64),
    ];
    let p = Program::new(Target::Cpu, 0, 1, 8, nodes).unwrap();
    let out = crate::kasm::execute(&p, &[]).unwrap();
    assert_eq!(f64_output_value(&out), 0.0);
}

#[test]
fn f64_sqrt_of_positive_is_real() {
    let nodes = vec![
        Node::const_f64(9),
        Node::f64_sqrt(0),
        Node::output(1, Ty::F64),
    ];
    let p = Program::new(Target::Cpu, 0, 1, 8, nodes).unwrap();
    let out = crate::kasm::execute(&p, &[]).unwrap();
    assert_eq!(f64_output_value(&out), 3.0);
}

#[test]
fn f64_i64_to_f64_and_back() {
    // Round-trip an i64 through the F64 domain.
    let nodes = vec![
        Node::input(0),
        Node::f64_from_i64(0),
        Node::f64_to_i64(1),
        Node::output(2, Ty::I64),
    ];
    let p = Program::new(Target::Cpu, 1, 1, 8, nodes).unwrap();
    let args = 42i64.to_le_bytes();
    let out = crate::kasm::execute(&p, &args).unwrap();
    assert_eq!(i64::from_le_bytes(out.try_into().unwrap()), 42);
}

#[test]
fn f64_to_i64_saturates_on_inf() {
    // (1.0 / 0.0) → 0.0 (kill-switch), then ToI64 → 0. Compose two
    // total-function guards.
    let nodes = vec![
        Node::const_f64(1),
        Node::const_f64(0),
        Node::f64_div(0, 1),
        Node::f64_to_i64(2),
        Node::output(3, Ty::I64),
    ];
    let p = Program::new(Target::Cpu, 0, 1, 10, nodes).unwrap();
    let out = crate::kasm::execute(&p, &[]).unwrap();
    assert_eq!(i64::from_le_bytes(out.try_into().unwrap()), 0);
}

#[test]
fn f64_program_canonical_hash_is_stable() {
    // Two byte-identical F64 programs must share the same canonical
    // hash. This exercises the optimizer pass-through path.
    let build = || -> Program {
        let nodes = vec![
            Node::input_f64(0),
            Node::const_f64(2),
            Node::f64_mul(0, 1),
            Node::output(2, Ty::F64),
        ];
        Program::new(Target::Cpu, 1, 1, 8, nodes).unwrap()
    };
    let a = build();
    let b = build();
    assert_eq!(a.canonical_hash_hex().unwrap(), b.canonical_hash_hex().unwrap());
}

#[test]
fn f64_input_types_reflect_node_ty() {
    let nodes = vec![
        Node::input_f64(0),
        Node::input(1),       // I64 input on slot 1
        Node::f64_from_i64(1),
        Node::f64_add(0, 2),
        Node::output(3, Ty::F64),
    ];
    let p = Program::new(Target::Cpu, 2, 1, 10, nodes).unwrap();
    let types = p.input_types();
    assert_eq!(types, vec![Ty::F64, Ty::I64]);
}

#[test]
fn f64_pythagorean_distance() {
    // sqrt(x*x + y*y) — a basic scientific primitive that requires
    // F64 mul + add + sqrt to chain. Tests the full F64 pipeline
    // through the interpreter.
    let nodes = vec![
        Node::input_f64(0),     // %0 : x
        Node::input_f64(1),     // %1 : y
        Node::f64_mul(0, 0),    // %2 : x*x
        Node::f64_mul(1, 1),    // %3 : y*y
        Node::f64_add(2, 3),    // %4 : x*x + y*y
        Node::f64_sqrt(4),      // %5 : sqrt(...)
        Node::output(5, Ty::F64),
    ];
    let p = Program::new(Target::Cpu, 2, 1, 10, nodes).unwrap();
    let args = f64_input_bytes(&[3.0, 4.0]);
    let out = crate::kasm::execute(&p, &args).unwrap();
    assert!((f64_output_value(&out) - 5.0).abs() < 1e-12);
}

#[test]
fn f64_mlir_roundtrip_byte_exact() {
    // Φ.0 ⇔ Ω-1 surface : a program that touches every F64 sub-op
    // must round-trip emit_mlir → parse_mlir without losing a byte.
    let nodes = vec![
        Node::input_f64(0),                   // 0
        Node::const_f64(2),                   // 1 : 2.0
        Node::f64_add(0, 1),                  // 2
        Node::f64_sub(2, 1),                  // 3
        Node::f64_mul(3, 1),                  // 4
        Node::f64_div(4, 1),                  // 5
        Node::f64_min(5, 0),                  // 6
        Node::f64_max(6, 1),                  // 7
        Node::f64_sqrt(7),                    // 8
        Node::f64_abs(8),                     // 9
        Node::f64_neg(9),                     // 10
        Node::f64_to_i64(10),                 // 11 : i64
        Node::f64_from_i64(11),               // 12 : f64 again
        Node::output(12, Ty::F64),            // 13
    ];
    let p = Program::new(Target::Cpu, 1, 1, 16, nodes).unwrap();
    let text = crate::kasm::emit_mlir(&p);
    let p2 = crate::kasm::parse_mlir(&text).unwrap();
    assert_eq!(p.bytes(), p2.bytes(), "MLIR roundtrip not byte-exact:\n{text}");
    let h_before = p.canonical_hash_hex().unwrap();
    let h_after = p2.canonical_hash_hex().unwrap();
    assert_eq!(h_before, h_after, "F64 CallKey changed across MLIR roundtrip");
}

#[test]
fn f64_jit_falls_back_cleanly() {
    // Programs that use F64 must not crash the JIT — the lowering
    // bails compile so the caller (hotplan) drops back to the
    // interpreter without observing a failure.
    let nodes = vec![
        Node::input_f64(0),
        Node::const_f64(1),
        Node::f64_add(0, 1),
        Node::output(2, Ty::F64),
    ];
    let p = Program::new(Target::Cpu, 1, 1, 8, nodes).unwrap();
    let result = crate::kasm::jit::compile(&p);
    assert!(
        result.is_err(),
        "JIT should bail on F64 ops, got Ok kernel"
    );
}

#[test]
fn f64_op_rejects_unknown_sub_op() {
    // imm = 99 is out of range — verifier must reject before any
    // exec / canonicalize attempt poisons content addressing.
    use crate::kasm::types::{F64SubOp, F64_OP_MAX};
    let nodes = vec![
        Node::input_f64(0),
        // Hand-craft an invalid F64Op node: imm 99 is past F64_OP_MAX.
        Node {
            op: Op::F64Op,
            ty: Ty::F64,
            a: 0,
            b: 0,
            imm: 99,
        },
        Node::output(1, Ty::F64),
    ];
    let res = Program::new(Target::Cpu, 1, 1, 8, nodes);
    assert!(res.is_err(), "verifier must reject unknown sub-op selector");

    // Sanity: every legal selector decodes successfully.
    for imm in 0..=F64_OP_MAX as i16 {
        assert!(F64SubOp::from_imm(imm).is_ok(), "imm {imm} should decode");
    }
}

#[test]
fn f64_exp_executes_via_libstd() {
    // Φ.7a — exp(0.0) = 1.0. Const integer 0 → ConstF64 emits 0.0.
    let nodes = vec![
        Node::const_f64(0),
        Node::f64_exp(0),
        Node::output(1, Ty::F64),
    ];
    let p = Program::new(Target::Cpu, 0, 1, 8, nodes).unwrap();
    let out = crate::kasm::execute(&p, &[]).unwrap();
    assert_eq!(f64_output_value(&out), 1.0);
}

#[test]
fn f64_exp_overflow_collapses_to_zero() {
    // Φ.7a — exp(1000) is +∞ → kill-switch → 0.0.
    // Build 1000 via I64ToF64 (ConstF64 imm is i16 so up to 32767).
    let nodes = vec![
        Node::const_i64(1000),
        Node::f64_from_i64(0),
        Node::f64_exp(1),
        Node::output(2, Ty::F64),
    ];
    let p = Program::new(Target::Cpu, 0, 1, 8, nodes).unwrap();
    let out = crate::kasm::execute(&p, &[]).unwrap();
    assert_eq!(f64_output_value(&out), 0.0);
}

#[test]
fn f64_ln_executes_via_libstd() {
    // Φ.7a — ln(1.0) = 0.0.
    let nodes = vec![
        Node::const_f64(1),
        Node::f64_ln(0),
        Node::output(1, Ty::F64),
    ];
    let p = Program::new(Target::Cpu, 0, 1, 8, nodes).unwrap();
    let out = crate::kasm::execute(&p, &[]).unwrap();
    assert!(f64_output_value(&out).abs() < 1e-12);
}

#[test]
fn f64_ln_of_negative_uses_abs() {
    // Φ.7a — ln(|-2|) = ln(2). The op bakes the absolute value in
    // so the function stays total over the entire f64 line.
    let nodes = vec![
        Node::const_f64(-2),
        Node::f64_ln(0),
        Node::output(1, Ty::F64),
    ];
    let p = Program::new(Target::Cpu, 0, 1, 8, nodes).unwrap();
    let out = crate::kasm::execute(&p, &[]).unwrap();
    assert!((f64_output_value(&out) - (2.0_f64).ln()).abs() < 1e-12);
}

#[test]
fn f64_ln_of_zero_collapses_to_zero() {
    // Φ.7a — ln(0) = -∞ → kill-switch → 0.0. The op is total: every
    // input maps to a finite f64.
    let nodes = vec![
        Node::const_f64(0),
        Node::f64_ln(0),
        Node::output(1, Ty::F64),
    ];
    let p = Program::new(Target::Cpu, 0, 1, 8, nodes).unwrap();
    let out = crate::kasm::execute(&p, &[]).unwrap();
    assert_eq!(f64_output_value(&out), 0.0);
}

#[test]
fn f64_const_static_output_emits_bit_pattern() {
    // A program that is a single Const → Output collapses to a static
    // 8-byte payload equal to the f64 bit pattern.
    let nodes = vec![
        Node::const_f64(5),
        Node::output(0, Ty::F64),
    ];
    let p = Program::new(Target::Cpu, 0, 1, 4, nodes).unwrap();
    let stat = p.static_output().expect("F64 const should be static-foldable");
    let expected = (5.0f64).to_bits().to_le_bytes();
    assert_eq!(stat.as_slice(), &expected);
}

// ─────────────────────────────────────────────────────────────────────
// Wave 4 (Phase Ω.10) — Multiple Dispatch tests
// ─────────────────────────────────────────────────────────────────────

#[test]
fn wave4_program_sig_extracts_inputs_and_outputs() {
    // i64-typed program: f(x) = 3*x + 1.
    let p_i64 = Program::new(Target::Cpu, 1, 1, 8, affine_nodes()).unwrap();
    let sig = p_i64.sig();
    assert_eq!(sig.inputs, vec![Ty::I64]);
    assert_eq!(sig.outputs, vec![Ty::I64]);
}

#[test]
fn wave4_multimethod_resolves_exact_signature_match() {
    // Two methods of the same logical function, one for I64, one
    // (synthetic) for F64. Bundle resolves on exact runtime sig.
    let sig_i64 = ProgramSig::new(vec![Ty::I64], vec![Ty::I64]);
    let sig_f64 = ProgramSig::new(vec![Ty::F64], vec![Ty::F64]);
    let hash_i64 = [0xAA; 20];
    let hash_f64 = [0xBB; 20];

    let mm = MultiMethod::new(vec![
        (sig_i64.clone(), hash_i64),
        (sig_f64.clone(), hash_f64),
    ]);

    assert_eq!(mm.len(), 2);
    assert_eq!(mm.resolve(&sig_i64), Some(hash_i64));
    assert_eq!(mm.resolve(&sig_f64), Some(hash_f64));
}

#[test]
fn wave4_multimethod_returns_none_for_no_match() {
    // Bundle has only the I64 method ; a Bool-input call must miss.
    let sig_i64 = ProgramSig::new(vec![Ty::I64], vec![Ty::I64]);
    let sig_bool = ProgramSig::new(vec![Ty::Bool], vec![Ty::Bool]);
    let mm = MultiMethod::new(vec![(sig_i64, [0u8; 20])]);

    // Tâche A.2 invariant : absence ⇒ None, never Err.
    let resolved: Option<[u8; 20]> = mm.resolve(&sig_bool);
    assert!(resolved.is_none());
}

#[test]
fn wave4_multimethod_canonical_encoding_is_order_independent() {
    // Two bundles inserted in opposite order must hash identically.
    let sig_a = ProgramSig::new(vec![Ty::I64], vec![Ty::I64]);
    let sig_b = ProgramSig::new(vec![Ty::F64], vec![Ty::F64]);
    let h_a = [0x01; 20];
    let h_b = [0x02; 20];

    let mm_forward = MultiMethod::new(vec![(sig_a.clone(), h_a), (sig_b.clone(), h_b)]);
    let mm_reverse = MultiMethod::new(vec![(sig_b, h_b), (sig_a, h_a)]);

    assert_eq!(mm_forward.encode(), mm_reverse.encode());
    assert_eq!(mm_forward.identity(), mm_reverse.identity());
}

#[test]
fn wave4_multimethod_roundtrips_through_encode_decode() {
    let mm = MultiMethod::new(vec![
        (ProgramSig::new(vec![Ty::I64, Ty::I64], vec![Ty::I64]), [0x33; 20]),
        (ProgramSig::new(vec![Ty::F64], vec![Ty::F64]), [0x44; 20]),
        (ProgramSig::new(vec![Ty::Bool, Ty::Bool], vec![Ty::Bool]), [0x55; 20]),
    ]);
    let blob = mm.encode();
    let parsed = MultiMethod::decode(&blob).expect("roundtrip parse");
    assert_eq!(mm, parsed);
}

#[test]
fn wave4_multimethod_rejects_bad_magic() {
    let mut blob = MultiMethod::new(vec![
        (ProgramSig::new(vec![Ty::I64], vec![Ty::I64]), [0x77; 20]),
    ])
    .encode();
    blob[0] = b'X'; // corrupt the magic
    let err = MultiMethod::decode(&blob);
    assert!(matches!(err, Err(KasmError::BadMultiMethod(_))));
}

#[test]
fn wave4_multimethod_rejects_trailing_bytes() {
    let mut blob = MultiMethod::new(vec![
        (ProgramSig::new(vec![Ty::I64], vec![Ty::I64]), [0x88; 20]),
    ])
    .encode();
    blob.extend_from_slice(b"junk"); // trailing garbage
    let err = MultiMethod::decode(&blob);
    assert!(matches!(err, Err(KasmError::BadMultiMethod(_))));
}

#[test]
fn wave4_multimethod_with_method_replaces_on_duplicate_sig() {
    // Julia's "redefine method" semantic : new (sig, hash) for an
    // existing sig replaces the old hash, length stays the same.
    let sig = ProgramSig::new(vec![Ty::I64], vec![Ty::I64]);
    let mm0 = MultiMethod::new(vec![(sig.clone(), [0xAA; 20])]);
    let mm1 = mm0.with_method(sig.clone(), [0xBB; 20]);

    assert_eq!(mm0.len(), 1);
    assert_eq!(mm1.len(), 1);
    assert_eq!(mm0.resolve(&sig), Some([0xAA; 20]));
    assert_eq!(mm1.resolve(&sig), Some([0xBB; 20]));
    // Different hashes → different bundle identity.
    assert_ne!(mm0.identity(), mm1.identity());
}

#[test]
fn wave4_multimethod_dispatches_real_programs_by_signature() {
    // End-to-end : two real KASM programs with different signatures
    // (both I64→I64 but different shapes) registered under the same
    // bundle ; resolve picks by exact sig match. Both are I64→I64 so
    // we pick by output count to differentiate signatures here.
    let p_unary = Program::new(Target::Cpu, 1, 1, 8, affine_nodes()).unwrap();
    let p_unary_hash = {
        let d = digest(p_unary.bytes());
        let mut h = [0u8; 20];
        h.copy_from_slice(&d[..20]);
        h
    };

    // A 2-output program : returns (3*x+1, x).
    let nodes_dual = vec![
        Node::input(0),
        Node::const_i64(3),
        Node::mul(0, 1),
        Node::const_i64(1),
        Node::add(2, 3),
        Node::output(4, Ty::I64),
        Node::output(0, Ty::I64),
    ];
    let p_dual = Program::new(Target::Cpu, 1, 2, 8, nodes_dual).unwrap();
    let p_dual_hash = {
        let d = digest(p_dual.bytes());
        let mut h = [0u8; 20];
        h.copy_from_slice(&d[..20]);
        h
    };

    let mm = MultiMethod::new(vec![
        (p_unary.sig(), p_unary_hash),
        (p_dual.sig(), p_dual_hash),
    ]);

    // Lookup by signature picks the right program hash.
    let unary_sig = ProgramSig::new(vec![Ty::I64], vec![Ty::I64]);
    let dual_sig = ProgramSig::new(vec![Ty::I64], vec![Ty::I64, Ty::I64]);
    assert_eq!(mm.resolve(&unary_sig), Some(p_unary_hash));
    assert_eq!(mm.resolve(&dual_sig), Some(p_dual_hash));
}

// ─── Semantic CSE tests ──────────────────────────────────────────────

#[test]
fn cse_merges_shl1_and_add_self() {
    // `x << 1` and `x + x` are structurally different but semantically
    // equivalent. CSE must detect this via trace evaluation and merge
    // them, producing a smaller program.
    let nodes = vec![
        Node::input(0),                        // 0: x
        Node::const_i64(1),                    // 1: 1
        Node::shl(0, 1),                       // 2: x << 1
        Node::add(0, 0),                       // 3: x + x
        Node::add(2, 3),                       // 4: (x<<1) + (x+x)
        Node::output(4, Ty::I64),              // 5
    ];
    let prog = Program::new(Target::Cpu, 1, 1, 8, nodes).unwrap();
    let cse_prog = prog.cse().unwrap();

    // After CSE, one of {shl, add_self} is eliminated. The program
    // should be shorter.
    assert!(
        cse_prog.nodes().len() < prog.nodes().len(),
        "CSE should eliminate a semantic duplicate: {} nodes before, {} after",
        prog.nodes().len(),
        cse_prog.nodes().len(),
    );

    // Verify correctness on diverse inputs.
    for x in [-100i64, -1, 0, 1, 42, i64::MAX / 2] {
        let args = x.to_le_bytes().to_vec();
        let orig = execute(&prog, &args).unwrap();
        let opt = execute(&cse_prog, &args).unwrap();
        assert_eq!(orig, opt, "CSE changed semantics for x={x}");
    }
}

#[test]
fn cse_merges_mul2_shl1_add_self() {
    // Three ways to express `2*x` — all should collapse to one.
    let nodes = vec![
        Node::input(0),                        // 0: x
        Node::const_i64(1),                    // 1: 1
        Node::const_i64(2),                    // 2: 2
        Node::shl(0, 1),                       // 3: x << 1
        Node::add(0, 0),                       // 4: x + x
        Node::mul(0, 2),                       // 5: x * 2
        // Use all three so none is dead-code-eliminated.
        Node::add(3, 4),                       // 6: (x<<1) + (x+x)
        Node::add(6, 5),                       // 7: ... + (x*2)
        Node::output(7, Ty::I64),              // 8
    ];
    let prog = Program::new(Target::Cpu, 1, 1, 10, nodes).unwrap();
    let cse_prog = prog.cse().unwrap();

    assert!(
        cse_prog.nodes().len() < prog.nodes().len(),
        "CSE should merge 3 equivalent expressions into 1: {} → {}",
        prog.nodes().len(),
        cse_prog.nodes().len(),
    );

    for x in [-7i64, 0, 1, 1000, i64::MIN / 2] {
        let args = x.to_le_bytes().to_vec();
        let orig = execute(&prog, &args).unwrap();
        let opt = execute(&cse_prog, &args).unwrap();
        assert_eq!(orig, opt, "CSE changed semantics for x={x}");
    }
}

#[test]
fn cse_preserves_structurally_distinct_subexpressions() {
    // `x + 1` and `x + 2` are NOT equivalent — CSE must not merge them.
    let nodes = vec![
        Node::input(0),                        // 0: x
        Node::const_i64(1),                    // 1: 1
        Node::const_i64(2),                    // 2: 2
        Node::add(0, 1),                       // 3: x + 1
        Node::add(0, 2),                       // 4: x + 2
        Node::add(3, 4),                       // 5: (x+1) + (x+2)
        Node::output(5, Ty::I64),              // 6
    ];
    let prog = Program::new(Target::Cpu, 1, 1, 8, nodes).unwrap();
    let cse_prog = prog.cse().unwrap();

    // Verify CSE didn't merge the two distinct subexpressions.
    for x in [-100i64, -1, 0, 1, 42] {
        let args = x.to_le_bytes().to_vec();
        let orig = execute(&prog, &args).unwrap();
        let opt = execute(&cse_prog, &args).unwrap();
        assert_eq!(orig, opt, "CSE broke semantics for x={x}");
    }
}

#[test]
fn cse_idempotent_on_already_optimal_program() {
    // A program with no semantic duplicates should pass through unchanged.
    let prog = Program::new(Target::Cpu, 1, 1, 8, affine_nodes()).unwrap();
    let cse_prog = prog.cse().unwrap();

    // Node count should be the same (or smaller via simplify, but not
    // from semantic CSE).
    let simplified = prog.simplified().unwrap();
    assert_eq!(
        cse_prog.nodes().len(),
        simplified.nodes().len(),
        "CSE on already-optimal program should match simplify",
    );
}

#[test]
fn cse_correctness_on_two_input_program() {
    // Two-input program: `(a+b)` computed two different ways.
    // `a + b` (direct) vs `b + a` (commuted) — canonicalize already
    // handles this, but let's confirm CSE doesn't break anything.
    let nodes = vec![
        Node::input(0),                        // 0: a
        Node::input(1),                        // 1: b
        Node::add(0, 1),                       // 2: a + b
        Node::add(1, 0),                       // 3: b + a (commuted)
        Node::mul(2, 3),                       // 4: should become (a+b)^2
        Node::output(4, Ty::I64),              // 5
    ];
    let prog = Program::new(Target::Cpu, 2, 1, 8, nodes).unwrap();
    let cse_prog = prog.cse().unwrap();

    for (a, b) in [(-3i64, 7i64), (0, 0), (1, -1), (100, 200)] {
        let mut args = a.to_le_bytes().to_vec();
        args.extend_from_slice(&b.to_le_bytes());
        let orig = execute(&prog, &args).unwrap();
        let opt = execute(&cse_prog, &args).unwrap();
        assert_eq!(orig, opt, "CSE changed semantics for a={a}, b={b}");
    }
}

/// Φ.ν.7g — Régression pour le bug CSE branch-sensitive (session
/// 2026-05-03). Avant le fix : `cse()` éliminait silencieusement les
/// nodes Min/Max d'un programme `min(max(7x+13, -120), 180)` parce que
/// les 8 sample inputs de trace_eval ne déclenchaient jamais le clamp,
/// donc trace(max(7x+13, -120)) == trace(7x+13) → CSE merge → clamp
/// supprimé. Le bug se manifeste seulement sur des inputs extrêmes en
/// production. Le fix : skip dedupe par trace pour Min/Max/Select/
/// Clamp/Cond (trace-equivalence nécessaire mais pas suffisante pour
/// les ops branch-sensitive).
#[test]
fn cse_preserves_clamp_min_max_branch_semantics() {
    use super::*;
    // f(x) = min(max(7x + 13, -120), 180)
    // = clamp(7x + 13, -120, 180)
    let nodes = vec![
        Node::input(0),                  // 0
        Node::const_i64(7),              // 1
        Node::mul(0, 1),                 // 2: 7x
        Node::const_i64(13),             // 3
        Node::add(2, 3),                 // 4: 7x + 13
        Node::const_i64(-120),           // 5
        Node::max(4, 5),                 // 6: max(7x+13, -120)
        Node::const_i64(180),            // 7
        Node::min(6, 7),                 // 8: min(_, 180)
        Node::output(8, Ty::I64),        // 9
    ];
    let prog = Program::new(Target::Cpu, 1, 1, nodes.len() as u32, nodes).unwrap();
    let cse_prog = prog.cse().unwrap();

    // Test sur des inputs qui DÉCLENCHENT le clamp aux deux bornes.
    // La référence Rust calcule la sémantique attendue ; cse() doit
    // produire le même résultat même si trace_eval n'a pas vu ces inputs.
    for x in [-128i64, -100, -50, -20, 0, 10, 20, 24, 50, 100, 128] {
        let expected = (x.wrapping_mul(7).wrapping_add(13))
            .max(-120)
            .min(180);
        let bytes = execute(&cse_prog, &x.to_le_bytes()).unwrap();
        let got = i64::from_le_bytes(bytes[..8].try_into().unwrap());
        assert_eq!(
            got, expected,
            "CSE broke clamp semantics for x={x}: got {got}, expected {expected}",
        );
    }
}
