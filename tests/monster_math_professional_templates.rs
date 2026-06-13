use scan::{
    kasm::ForgePrecision,
    fresh_tmp_path, MemoryGovernor, MonsterMathClass, MonsterMathConstant, MonsterMathContract,
    MonsterMathOutputContract, MonsterMathSample, MonsterMathVariable,
    MonsterNode, Store,
};

struct ProfessionalCase {
    role: &'static str,
    user_request: &'static str,
    class: MonsterMathClass,
    variables: Vec<MonsterMathVariable>,
    constants: Vec<MonsterMathConstant>,
    operators: Vec<&'static str>,
    equations: Vec<&'static str>,
    constraints: Vec<&'static str>,
    samples: Vec<MonsterMathSample>,
    outputs: Vec<MonsterMathOutputContract>,
    expected_status: ExpectedStatus,
}

enum ExpectedStatus {
    Executes { output: &'static str, value: f64, tolerance: f64 },
    ExecutesTensorSolver { expected_op: &'static str },
}

fn fill_all_template_slots(contract: &mut MonsterMathContract, case: &ProfessionalCase) {
    let class = case.class;
    for slot in class.required_slots() {
        contract.set_template_slot(
            slot,
            format!(
                "role={}; request={}; required_slot={}; professional_math_payload=filled",
                case.role, case.user_request, slot
            ),
        );
    }
    for slot in class.optional_slots() {
        contract.set_template_slot(
            slot,
            format!(
                "role={}; optional_slot={}; policy=explicitly_configured_for_professional_compute",
                case.role, slot
            ),
        );
    }
    for slot in class
        .required_slots()
        .iter()
        .chain(class.optional_slots().iter())
    {
        assert!(
            contract.template_slot_value(slot).is_some(),
            "slot {slot} was not filled for {}",
            class.command()
        );
    }
}

fn expected_classical_alias(class: MonsterMathClass) -> &'static str {
    match class {
        MonsterMathClass::FormulaSymbolic => "Derivative->diff",
        MonsterMathClass::NumericModel => "Power->pow",
        MonsterMathClass::SimulationDynamics => "PDEStencil->pde_stencil_step",
        MonsterMathClass::OptimizationDesign => "Gradient->grad",
        MonsterMathClass::UncertaintyStatistics => "StdDev->std",
        MonsterMathClass::TensorLinalgAutodiff => "Jacobian->jacobian",
        MonsterMathClass::SignalTimeseries => "RFFT->rfft",
        MonsterMathClass::GraphSparseDiscrete => "PageRank->pagerank_step",
    }
}

fn run_case(monster: &MonsterNode, case: ProfessionalCase) {
    let manifest = monster.math_capability_manifest();
    let template = manifest
        .classes
        .iter()
        .find(|template| template.class == case.class)
        .expect("template must exist in /newcompute_ manifest");
    assert_eq!(template.command, case.class.command());
    assert!(!template.required_slots.is_empty());
    assert!(!template.optional_slots.is_empty());
    assert_eq!(
        template.slot_specs.len(),
        template.required_slots.len() + template.optional_slots.len(),
        "template {} must expose one typed spec per slot",
        case.class.command()
    );
    assert!(template
        .slot_specs
        .iter()
        .any(|slot| slot.name == "goal" && slot.forge_binding == "contract.goal"));
    assert!(
        template
            .classical_aliases
            .contains(&expected_classical_alias(case.class)),
        "template {} must expose the classical alias vocabulary needed by an external LLM",
        case.class.command()
    );

    let mut contract = MonsterMathContract::new(case.class, case.user_request);
    if matches!(case.class, MonsterMathClass::SimulationDynamics) {
        contract.precision = ForgePrecision::F32;
    }
    fill_all_template_slots(&mut contract, &case);
    contract.variables = case.variables;
    contract.constants = case.constants;
    contract.operators = case.operators.iter().map(|op| op.to_string()).collect();
    contract.equations = case.equations.iter().map(|eq| eq.to_string()).collect();
    contract.constraints = case.constraints.iter().map(|c| c.to_string()).collect();
    contract.samples = case.samples;
    contract.outputs = case.outputs;
    contract.validation.push(format!(
        "role={} template={} all_required_and_optional_slots_filled",
        case.role,
        case.class.command()
    ));
    assert!(
        contract.missing_required_template_slots().is_empty(),
        "role={} template={} must satisfy every required slot before Monster execution",
        case.role,
        case.class.command()
    );

    match case.expected_status {
        ExpectedStatus::Executes {
            output,
            value,
            tolerance,
        } => {
            let compiled = monster.compile_math_contract(&contract).unwrap_or_else(|error| {
                panic!(
                    "professional template compile failed role={} class={} error={error:?}",
                    case.role,
                    case.class.command()
                )
            });
            let (compiled, prepared) = monster
                .prepare_math_contract(&contract, std::iter::empty::<String>())
                .unwrap_or_else(|error| {
                    panic!(
                        "professional template prepare failed role={} class={} error={error:?}\nsource:\n{}",
                        case.role,
                        case.class.command(),
                        compiled.forge_source
                    )
                });
            let execution = monster.execute_prepared_compute(&prepared).unwrap_or_else(|error| {
                panic!(
                    "professional template execute failed role={} class={} error={error:?}",
                    case.role,
                    case.class.command()
                )
            });
            assert_eq!(compiled.class, case.class);
            assert_eq!(compiled.contract_hash, contract.contract_hash());
            assert!(
                contract.template_slot_value("goal").is_some(),
                "template slot payloads must stay in the Monster contract envelope"
            );
            assert_eq!(prepared.route.lane, scan::MonsterEngineLane::MassMath);
            let oracle = prepared
                .route
                .plan
                .scalar_oracle_outputs
                .first()
                .expect("scalar oracle output");
            assert_eq!(oracle.output_name, output, "role={}", case.role);
            assert_eq!(oracle.status, "sample_value_matched", "role={}", case.role);
            assert!(
                (f64::from_bits(oracle.value_bits) - value).abs() <= tolerance,
                "role={} expected={} got={}",
                case.role,
                value,
                f64::from_bits(oracle.value_bits)
            );
            assert!(!execution.typed_result_buffers.is_empty());
            assert_eq!(execution.proof_hash.len(), 64);
        }
        ExpectedStatus::ExecutesTensorSolver { expected_op } => {
            let (compiled, prepared) = monster
                .prepare_math_contract(&contract, std::iter::empty::<String>())
                .unwrap_or_else(|error| {
                    panic!(
                        "professional simulation template prepare failed role={} class={} error={error:?}",
                        case.role,
                        case.class.command()
                    )
                });
            assert_eq!(compiled.class, MonsterMathClass::SimulationDynamics);
            assert!(prepared
                .route
                .plan
                .compute_ir_kernel_hints
                .iter()
                .any(|kernel| kernel.primitive_ops.iter().any(|op| op == expected_op)));
            assert!(!prepared.route.plan.result_artifacts.is_empty());
        }
    }
}

fn professional_cases() -> Vec<ProfessionalCase> {
    vec![
        ProfessionalCase {
            role: "aerospace_engineer",
            user_request: "Compute cruise required thrust for a high altitude aircraft sizing loop",
            class: MonsterMathClass::NumericModel,
            variables: vec![
                MonsterMathVariable::f64("rho", "kg/m3", 0.05, 1.5, 0.38),
                MonsterMathVariable::f64("velocity", "m/s", 80.0, 320.0, 230.0),
                MonsterMathVariable::f64("wing_area", "m2", 5.0, 200.0, 42.0),
                MonsterMathVariable::f64("drag_coefficient", "none", 0.01, 0.2, 0.032),
            ],
            constants: vec![MonsterMathConstant::f64("safety_factor", "none", 1.15)],
            operators: vec!["Power", "*", "finite"],
            equations: vec![
                "required_thrust = 0.5 * rho * Power(velocity, 2.0) * wing_area * drag_coefficient * safety_factor",
            ],
            constraints: vec!["required_thrust >= 0"],
            samples: vec![MonsterMathSample::new(
                "cruise_drag_nominal",
                101,
                vec![
                    ("rho", 0.38),
                    ("velocity", 230.0),
                    ("wing_area", 42.0),
                    ("drag_coefficient", 0.032),
                ],
                "required_thrust",
                15534.8256,
                0.001,
            )],
            outputs: vec![MonsterMathOutputContract::scalar(
                "required_thrust",
                "N",
                "0.5 * rho * Power(velocity, 2.0) * wing_area * drag_coefficient * safety_factor",
            )],
            expected_status: ExpectedStatus::Executes {
                output: "required_thrust",
                value: 15534.8256,
                tolerance: 0.001,
            },
        },
        ProfessionalCase {
            role: "applied_mathematician",
            user_request: "Check a symbolic polynomial derivative planning contract with scalar replay",
            class: MonsterMathClass::FormulaSymbolic,
            variables: vec![MonsterMathVariable::f64("x", "none", -20.0, 20.0, 3.0)],
            constants: vec![MonsterMathConstant::f64("a", "none", 2.0)],
            operators: vec!["Derivative"],
            equations: vec!["proof_score = Power(x, 2.0) + a * x + 1.0"],
            constraints: vec!["proof_score >= 0"],
            samples: vec![MonsterMathSample::new(
                "symbolic_replay_nominal",
                102,
                vec![("x", 3.0)],
                "proof_score",
                16.0,
                0.001,
            )],
            outputs: vec![MonsterMathOutputContract::scalar(
                "proof_score",
                "none",
                "Power(x, 2.0) + a * x + 1.0",
            )],
            expected_status: ExpectedStatus::Executes {
                output: "proof_score",
                value: 16.0,
                tolerance: 0.001,
            },
        },
        ProfessionalCase {
            role: "thermal_systems_engineer",
            user_request: "Run a 2D heat diffusion stencil over a battery pack temperature field",
            class: MonsterMathClass::SimulationDynamics,
            variables: vec![
                MonsterMathVariable {
                    name: "temperature_field".to_string(),
                    ty: "tensor<f32,64x64>".to_string(),
                    unit: "none".to_string(),
                    min: 250.0,
                    max: 460.0,
                    nominal: 313.15,
                },
                MonsterMathVariable {
                    name: "source_field".to_string(),
                    ty: "tensor<f32,64x64>".to_string(),
                    unit: "none".to_string(),
                    min: 0.0,
                    max: 1.0,
                    nominal: 0.05,
                },
                MonsterMathVariable {
                    name: "dt".to_string(),
                    ty: "f32".to_string(),
                    unit: "s".to_string(),
                    min: 0.0001,
                    max: 0.1,
                    nominal: 0.01,
                },
            ],
            constants: Vec::new(),
            operators: vec!["PDEStencil", "mean", "finite"],
            equations: vec![
                "next_temperature_field = PDEStencil(temperature_field, source_field, dt)",
            ],
            constraints: Vec::new(),
            samples: vec![MonsterMathSample::new(
                "battery_pack_nominal_field",
                103,
                vec![("temperature_field", 313.15), ("source_field", 0.05), ("dt", 0.01)],
                "mean_temperature_k",
                313.15,
                20.0,
            )],
            outputs: vec![
                MonsterMathOutputContract {
                    name: "next_temperature_field".to_string(),
                    ty: "tensor<f32,64x64>".to_string(),
                    unit: "none".to_string(),
                    handoff: "vector".to_string(),
                    expression: "PDEStencil(temperature_field, source_field, dt)".to_string(),
                },
                MonsterMathOutputContract {
                    name: "mean_temperature_k".to_string(),
                    ty: "f32".to_string(),
                    unit: "none".to_string(),
                    handoff: "scalar".to_string(),
                    expression: "mean(PDEStencil(temperature_field, source_field, dt))"
                        .to_string(),
                },
            ],
            expected_status: ExpectedStatus::ExecutesTensorSolver {
                expected_op: "pde_stencil_step",
            },
        },
        ProfessionalCase {
            role: "propulsion_design_optimizer",
            user_request: "Optimize a chamber pressure surrogate objective under engineering constraints",
            class: MonsterMathClass::OptimizationDesign,
            variables: vec![MonsterMathVariable::f64(
                "chamber_pressure",
                "MPa",
                2.0,
                25.0,
                12.0,
            )],
            constants: vec![MonsterMathConstant::f64("thermal_weight", "none", 0.8)],
            operators: vec!["optimize"],
            equations: vec!["objective_score = optimize(chamber_pressure, chamber_pressure)"],
            constraints: vec!["objective_score >= 0"],
            samples: vec![MonsterMathSample::new(
                "pressure_design_nominal",
                104,
                vec![("chamber_pressure", 12.0)],
                "objective_score",
                12_000_000.0,
                0.001,
            )],
            outputs: vec![MonsterMathOutputContract::scalar(
                "objective_score",
                "Pa",
                "optimize(chamber_pressure, chamber_pressure)",
            )],
            expected_status: ExpectedStatus::Executes {
                output: "objective_score",
                value: 12_000_000.0,
                tolerance: 0.001,
            },
        },
        ProfessionalCase {
            role: "biostatistician",
            user_request: "Estimate conservative clinical response yield from low/mid/high uncertainty bounds",
            class: MonsterMathClass::UncertaintyStatistics,
            variables: vec![
                MonsterMathVariable::f64("yield_low", "none", 0.0, 1.0, 0.42),
                MonsterMathVariable::f64("yield_mid", "none", 0.0, 1.0, 0.61),
                MonsterMathVariable::f64("yield_high", "none", 0.0, 1.0, 0.77),
            ],
            constants: Vec::new(),
            operators: vec!["p95", "uncertainty"],
            equations: vec!["response_p95 = p95(uncertainty(yield_low, yield_mid, yield_high))"],
            constraints: vec!["response_p95 >= 0"],
            samples: vec![MonsterMathSample::new(
                "trial_response_nominal",
                105,
                vec![("yield_low", 0.42), ("yield_mid", 0.61), ("yield_high", 0.77)],
                "response_p95",
                0.61,
                0.001,
            )],
            outputs: vec![MonsterMathOutputContract::scalar(
                "response_p95",
                "none",
                "p95(uncertainty(yield_low, yield_mid, yield_high))",
            )],
            expected_status: ExpectedStatus::Executes {
                output: "response_p95",
                value: 0.61,
                tolerance: 0.001,
            },
        },
        ProfessionalCase {
            role: "autodiff_researcher",
            user_request: "Run a gradient replay for a quadratic loss used in calibration",
            class: MonsterMathClass::TensorLinalgAutodiff,
            variables: vec![MonsterMathVariable::f64("lift_coeff", "none", -2.0, 4.0, 1.7)],
            constants: Vec::new(),
            operators: vec!["Gradient"],
            equations: vec!["grad_score = Gradient(Power(lift_coeff, 2.0))"],
            constraints: vec!["grad_score >= 0"],
            samples: vec![MonsterMathSample::new(
                "gradient_nominal",
                106,
                vec![("lift_coeff", 1.7)],
                "grad_score",
                2.89,
                0.001,
            )],
            outputs: vec![MonsterMathOutputContract::scalar(
                "grad_score",
                "none",
                "Gradient(Power(lift_coeff, 2.0))",
            )],
            expected_status: ExpectedStatus::Executes {
                output: "grad_score",
                value: 2.89,
                tolerance: 0.001,
            },
        },
        ProfessionalCase {
            role: "quant_trader",
            user_request: "Extract a scalar spectral risk proxy from a high frequency return stream template",
            class: MonsterMathClass::SignalTimeseries,
            variables: vec![MonsterMathVariable::f64("return_rms", "none", 0.0, 1.0, 0.027)],
            constants: vec![MonsterMathConstant::f64("microstructure_penalty", "none", 0.004)],
            operators: vec!["RFFT"],
            equations: vec!["spectral_risk = return_rms + microstructure_penalty"],
            constraints: vec!["spectral_risk >= 0"],
            samples: vec![MonsterMathSample::new(
                "spectral_risk_nominal",
                107,
                vec![("return_rms", 0.027)],
                "spectral_risk",
                0.031,
                0.001,
            )],
            outputs: vec![MonsterMathOutputContract::scalar(
                "spectral_risk",
                "none",
                "return_rms + microstructure_penalty",
            )],
            expected_status: ExpectedStatus::Executes {
                output: "spectral_risk",
                value: 0.031,
                tolerance: 0.001,
            },
        },
        ProfessionalCase {
            role: "network_sparse_analyst",
            user_request: "Check a sparse capacity feasibility score for an infrastructure graph",
            class: MonsterMathClass::GraphSparseDiscrete,
            variables: vec![
                MonsterMathVariable::f64("node_load", "none", 0.0, 1.0, 0.72),
                MonsterMathVariable::f64("node_capacity", "none", 0.0, 1.0, 0.91),
            ],
            constants: Vec::new(),
            operators: vec!["constraint_solve"],
            equations: vec!["capacity_score = constraint_solve(node_load, node_capacity)"],
            constraints: vec!["capacity_score >= 0"],
            samples: vec![MonsterMathSample::new(
                "capacity_nominal",
                108,
                vec![("node_load", 0.72), ("node_capacity", 0.91)],
                "capacity_score",
                0.72,
                0.001,
            )],
            outputs: vec![MonsterMathOutputContract::scalar(
                "capacity_score",
                "none",
                "constraint_solve(node_load, node_capacity)",
            )],
            expected_status: ExpectedStatus::Executes {
                output: "capacity_score",
                value: 0.72,
                tolerance: 0.001,
            },
        },
    ]
}

#[test]
fn monster_professional_newcompute_templates_run_or_fail_loud_by_class() {
    let path = fresh_tmp_path("monster-professional-template", "all-classes");
    let monster = MonsterNode::new(
        Store::open(&path).unwrap(),
        MemoryGovernor::new(8 * 1024 * 1024),
    );

    let cases = professional_cases();
    assert_eq!(cases.len(), 8);
    for case in cases {
        run_case(&monster, case);
    }

    let _ = std::fs::remove_dir_all(path);
}
