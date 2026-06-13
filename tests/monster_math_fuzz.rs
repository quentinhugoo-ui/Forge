use scan::{
    fresh_tmp_path, MemoryGovernor, MonsterMathClass, MonsterMathConstant, MonsterMathContract,
    MonsterMathOutputContract, MonsterMathSample, MonsterMathVariable, MonsterNode, Store,
};

#[derive(Clone, Copy)]
struct Lcg(u64);

impl Lcg {
    fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next_u64(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0
    }

    fn choose(&mut self, n: usize) -> usize {
        (self.next_u64() as usize) % n
    }

    fn f64(&mut self, min: f64, max: f64) -> f64 {
        let unit = ((self.next_u64() >> 11) as f64) / ((1u64 << 53) as f64);
        min + (max - min) * unit
    }
}

#[derive(Clone)]
struct FuzzCase {
    name: String,
    variables: Vec<MonsterMathVariable>,
    constants: Vec<MonsterMathConstant>,
    operators: Vec<String>,
    equation: String,
    output_name: &'static str,
    output_unit: &'static str,
    expression: String,
    givens: Vec<(String, f64)>,
    expected: f64,
    tolerance: f64,
}

fn aerodynamic_drag_case(rng: &mut Lcg, index: usize) -> FuzzCase {
    let rho = rng.f64(0.12, 1.25);
    let velocity = rng.f64(45.0, 310.0);
    let area = rng.f64(8.0, 110.0);
    let cd = rng.f64(0.018, 0.09);
    let safety = rng.f64(1.0, 1.35);
    let expected = 0.5 * rho * velocity.powi(2) * area * cd * safety;
    FuzzCase {
        name: format!("aero_drag_{index}"),
        variables: vec![
            MonsterMathVariable::f64("rho", "kg/m3", 0.05, 1.5, rho),
            MonsterMathVariable::f64("velocity", "m/s", 1.0, 400.0, velocity),
            MonsterMathVariable::f64("wing_area", "m2", 1.0, 200.0, area),
            MonsterMathVariable::f64("drag_coefficient", "none", 0.001, 0.3, cd),
        ],
        constants: vec![MonsterMathConstant::f64("safety_factor", "none", safety)],
        operators: words(&["Power", "*", "finite"]),
        equation: "required_thrust = 0.5 * rho * Power(velocity, 2.0) * wing_area * drag_coefficient * safety_factor".to_string(),
        output_name: "required_thrust",
        output_unit: "N",
        expression:
            "0.5 * rho * Power(velocity, 2.0) * wing_area * drag_coefficient * safety_factor"
                .to_string(),
        givens: vec![
            ("rho".to_string(), rho),
            ("velocity".to_string(), velocity),
            ("wing_area".to_string(), area),
            ("drag_coefficient".to_string(), cd),
        ],
        expected,
        tolerance: 1e-6_f64.max(expected.abs() * 1e-10),
    }
}

fn vector_norm_case(rng: &mut Lcg, index: usize) -> FuzzCase {
    let x = rng.f64(-250.0, 250.0);
    let y = rng.f64(-250.0, 250.0);
    let expected = (x.powi(2) + y.powi(2)).sqrt();
    FuzzCase {
        name: format!("norm_{index}"),
        variables: vec![
            MonsterMathVariable::f64("x", "m", -500.0, 500.0, x),
            MonsterMathVariable::f64("y", "m", -500.0, 500.0, y),
        ],
        constants: Vec::new(),
        operators: words(&["Sqrt", "Power", "+", "finite"]),
        equation: "distance = Sqrt(Power(x, 2.0) + Power(y, 2.0))".to_string(),
        output_name: "distance",
        output_unit: "m",
        expression: "Sqrt(Power(x, 2.0) + Power(y, 2.0))".to_string(),
        givens: vec![("x".to_string(), x), ("y".to_string(), y)],
        expected,
        tolerance: 1e-9,
    }
}

fn trig_identity_case(rng: &mut Lcg, index: usize) -> FuzzCase {
    let theta = rng.f64(-12.0, 12.0);
    FuzzCase {
        name: format!("trig_identity_{index}"),
        variables: vec![MonsterMathVariable::f64("theta", "rad", -20.0, 20.0, theta)],
        constants: Vec::new(),
        operators: words(&["Power", "Sin", "Cos", "+", "finite"]),
        equation: "identity = Power(Sin(theta), 2.0) + Power(Cos(theta), 2.0)".to_string(),
        output_name: "identity",
        output_unit: "none",
        expression: "Power(Sin(theta), 2.0) + Power(Cos(theta), 2.0)".to_string(),
        givens: vec![("theta".to_string(), theta)],
        expected: 1.0,
        tolerance: 1e-9,
    }
}

fn affine_case(rng: &mut Lcg, index: usize) -> FuzzCase {
    let x = rng.f64(-1_000.0, 1_000.0);
    let y = rng.f64(-1_000.0, 1_000.0);
    let a = rng.f64(-30.0, 30.0);
    let b = rng.f64(-30.0, 30.0);
    let c = rng.f64(-100.0, 100.0);
    let expected = a * x + b * y + c;
    FuzzCase {
        name: format!("affine_{index}"),
        variables: vec![
            MonsterMathVariable::f64("x", "none", -2_000.0, 2_000.0, x),
            MonsterMathVariable::f64("y", "none", -2_000.0, 2_000.0, y),
        ],
        constants: vec![
            MonsterMathConstant::f64("a", "none", a),
            MonsterMathConstant::f64("b", "none", b),
            MonsterMathConstant::f64("c", "none", c),
        ],
        operators: words(&["*", "+", "finite"]),
        equation: "score = a * x + b * y + c".to_string(),
        output_name: "score",
        output_unit: "none",
        expression: "a * x + b * y + c".to_string(),
        givens: vec![("x".to_string(), x), ("y".to_string(), y)],
        expected,
        tolerance: 1e-7,
    }
}

fn exp_log_case(rng: &mut Lcg, index: usize) -> FuzzCase {
    let x = rng.f64(-20.0, 20.0);
    FuzzCase {
        name: format!("exp_log_{index}"),
        variables: vec![MonsterMathVariable::f64("x", "none", -40.0, 40.0, x)],
        constants: Vec::new(),
        operators: words(&["Log", "Exp", "finite"]),
        equation: "roundtrip = Log(Exp(x))".to_string(),
        output_name: "roundtrip",
        output_unit: "none",
        expression: "Log(Exp(x))".to_string(),
        givens: vec![("x".to_string(), x)],
        expected: x,
        tolerance: 1e-9,
    }
}

fn words(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| value.to_string()).collect()
}

fn fuzz_case(seed: u64, index: usize) -> FuzzCase {
    let mut rng = Lcg::new(seed ^ ((index as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15)));
    match rng.choose(5) {
        0 => aerodynamic_drag_case(&mut rng, index),
        1 => vector_norm_case(&mut rng, index),
        2 => trig_identity_case(&mut rng, index),
        3 => affine_case(&mut rng, index),
        _ => exp_log_case(&mut rng, index),
    }
}

fn run_numeric_newcompute_fuzz(cases: usize, seed: u64) {
    let started = std::time::Instant::now();
    let path = fresh_tmp_path("monster-math-fuzz", &format!("cases-{cases}-seed-{seed}"));
    let monster = MonsterNode::new(
        Store::open(&path).unwrap(),
        MemoryGovernor::new(8 * 1024 * 1024),
    );
    let manifest = monster.math_capability_manifest();
    let numeric = manifest
        .classes
        .iter()
        .find(|class| class.command == "/numeric_model")
        .expect("/newcompute_ must expose /numeric_model");
    assert!(numeric.required_slots.contains(&"equations"));
    assert!(numeric.optional_slots.contains(&"unit_conversions"));
    assert!(numeric.accepted_operators.contains(&"pow"));

    for index in 0..cases {
        if index > 0 && index % 10_000 == 0 {
            println!(
                "monster_math_fuzz progress: {index}/{cases} cases elapsed={:?}",
                started.elapsed()
            );
        }
        let case = fuzz_case(seed, index);
        let mut contract = MonsterMathContract::new(
            MonsterMathClass::NumericModel,
            &format!(
                "Fuzz generated numeric engineering MathContract seed={seed} index={index}"
            ),
        );
        contract.variables = case.variables;
        contract.constants = case.constants;
        contract.operators = case.operators;
        contract.equations.push(case.equation);
        contract.samples.push(MonsterMathSample::new(
            &case.name,
            seed.wrapping_add(index as u64),
            case.givens
                .iter()
                .map(|(name, value)| (name.as_str(), *value))
                .collect(),
            case.output_name,
            case.expected,
            case.tolerance,
        ));
        contract.validation.push(format!(
            "numeric_newcompute_fuzz seed={seed} index={index} family={}",
            case.name
        ));
        contract.outputs.push(MonsterMathOutputContract::scalar(
            case.output_name,
            case.output_unit,
            &case.expression,
        ));

        let (compiled, prepared, execution) = monster
            .execute_math_contract(&contract)
            .unwrap_or_else(|error| panic!("fuzz case failed seed={seed} index={index} name={}: {error:?}", case.name));
        assert!(
            compiled.forge_source.contains("forge_module:"),
            "missing Forge module seed={seed} index={index}"
        );
        let oracle = prepared
            .route
            .plan
            .scalar_oracle_outputs
            .first()
            .unwrap_or_else(|| panic!("missing scalar oracle seed={seed} index={index}"));
        assert_eq!(
            oracle.status, "sample_value_matched",
            "oracle mismatch seed={seed} index={index} name={} expected={} got={}",
            case.name,
            case.expected,
            f64::from_bits(oracle.value_bits)
        );
        assert!(
            !execution.typed_result_buffers.is_empty(),
            "missing typed buffers seed={seed} index={index}"
        );
        assert_eq!(execution.proof_hash.len(), 64);
    }

    let _ = std::fs::remove_dir_all(path);
}

#[test]
fn monster_newcompute_numeric_math_fuzz_quick() {
    run_numeric_newcompute_fuzz(256, 0x4d4f_4e53_5445_5221);
}

#[test]
#[ignore = "set FORGE_MONSTER_MATH_FUZZ_CASES for long brute-force runs"]
fn monster_newcompute_numeric_math_fuzz_soak() {
    let cases = std::env::var("FORGE_MONSTER_MATH_FUZZ_CASES")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(1_000_000);
    let seed = std::env::var("FORGE_MONSTER_MATH_FUZZ_SEED")
        .ok()
        .and_then(|value| u64::from_str_radix(value.trim_start_matches("0x"), 16).ok())
        .unwrap_or(0x534f_414b_5f4d_4154);
    run_numeric_newcompute_fuzz(cases, seed);
}
