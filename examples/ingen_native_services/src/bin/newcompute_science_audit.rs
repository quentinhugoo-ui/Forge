use ingen_native_services::math_compute_service::run_native_math_contract_for_llm;
use scan::{
    fresh_tmp_path, kasm::ForgePrecision, MonsterMathClass, MonsterMathContract,
    MonsterMathOutputContract, MonsterMathSample, MonsterMathVariable,
};

fn main() {
    let path = fresh_tmp_path("native-newcompute", "science-audit");
    let contract = heat_pde_contract();
    let result = run_native_math_contract_for_llm(&path, &contract)
        .expect("run simulation_dynamics heat PDE newcompute");
    println!("{}", result.compact_text);
    let _ = std::fs::remove_dir_all(path);
}

fn heat_pde_contract() -> MonsterMathContract {
    let mut contract = MonsterMathContract::new(
        MonsterMathClass::SimulationDynamics,
        "Applied thermal-safety scientist runs a 2D heat diffusion stencil over a battery pack field",
    );
    for slot in MonsterMathClass::SimulationDynamics.required_slots() {
        contract.set_template_slot(
            slot,
        format!(
                "role=thermal_safety_scientist; required_slot={slot}; objective=run a multi-step electro-thermal 2D heat-equation stencil over a 64x64 battery-pack temperature field; boundary=cooling_plate_edge; sources=joule_heat,entropic_heat,arrhenius_side_reaction,hotspot,convection; diagnostics=temperature_field_next,mean,max,hotspot,gradient,runaway_margin,time_to_threshold,CFL,energy_balance,residual,sensitivity,reference_error,time_series; workload=max_power_gpu"
            ),
        );
    }
    for slot in MonsterMathClass::SimulationDynamics.optional_slots() {
        contract.set_template_slot(
            slot,
        format!(
                "role=thermal_safety_scientist; optional_slot={slot}; mesh=64x64; physics=diffusion plus Joule heat plus entropic heat plus Arrhenius side reaction plus convective cooling; stability_policy=explicit CFL gate; artifact=typed tensor field plus scientific diagnostics plus proof hash"
            ),
        );
    }
    contract.max_steps = 100_000_000;
    contract.max_memory_mb = 512;
    contract.precision = ForgePrecision::F64;
    contract.variables.push(MonsterMathVariable {
        name: "temperature_field".to_string(),
        ty: "tensor<f32,64x64>".to_string(),
        unit: "K".to_string(),
        min: 250.0,
        max: 460.0,
        nominal: 313.15,
    });
    contract.variables.push(MonsterMathVariable {
        name: "source_field".to_string(),
        ty: "tensor<f32,64x64>".to_string(),
        unit: "K".to_string(),
        min: 0.0,
        max: 1.0,
        nominal: 0.05,
    });
    contract.variables.push(MonsterMathVariable {
        name: "dt".to_string(),
        ty: "f32".to_string(),
        unit: "s".to_string(),
        min: 0.0001,
        max: 0.1,
        nominal: 0.01,
    });
    contract.variables.push(MonsterMathVariable::f64(
        "simulation_steps",
        "none",
        1.0,
        200_000.0,
        2000.0,
    ));
    contract.variables.push(MonsterMathVariable::f64(
        "thermal_diffusivity",
        "none",
        1.0e-7,
        1.0e-3,
        1.2e-5,
    ));
    contract.variables.push(MonsterMathVariable::f64("dx", "m", 0.001, 0.02, 0.004));
    contract.variables.push(MonsterMathVariable::f64("dy", "m", 0.001, 0.02, 0.004));
    contract.variables.push(MonsterMathVariable::f64(
        "separator_critical_temperature",
        "K",
        350.0,
        500.0,
        408.0,
    ));
    contract.variables.push(MonsterMathVariable::f64(
        "ambient_temperature",
        "K",
        250.0,
        330.0,
        300.0,
    ));
    contract.variables.push(MonsterMathVariable::f64(
        "boundary_condition_code",
        "none",
        0.0,
        3.0,
        3.0,
    ));
    contract.variables.push(MonsterMathVariable::f64(
        "cooling_plate_temperature",
        "K",
        260.0,
        330.0,
        295.0,
    ));
    contract.variables.push(MonsterMathVariable::f64(
        "convection_rate",
        "none",
        0.0,
        0.2,
        0.015,
    ));
    contract.variables.push(MonsterMathVariable::f64(
        "hotspot_center_x",
        "none",
        0.0,
        63.0,
        37.0,
    ));
    contract.variables.push(MonsterMathVariable::f64(
        "hotspot_center_y",
        "none",
        0.0,
        63.0,
        31.0,
    ));
    contract.variables.push(MonsterMathVariable::f64(
        "hotspot_sigma_cells",
        "none",
        1.0,
        16.0,
        5.0,
    ));
    contract.variables.push(MonsterMathVariable::f64(
        "hotspot_amplitude",
        "K",
        0.0,
        120.0,
        62.0,
    ));
    contract.variables.push(MonsterMathVariable::f64(
        "source_peak",
        "K",
        0.0,
        5.0,
        0.85,
    ));
    contract.variables.push(MonsterMathVariable::f64(
        "current_a",
        "none",
        0.0,
        300.0,
        135.0,
    ));
    contract.variables.push(MonsterMathVariable::f64(
        "internal_resistance_ohm",
        "none",
        0.0001,
        0.02,
        0.0018,
    ));
    contract.variables.push(MonsterMathVariable::f64(
        "cell_mass_kg",
        "kg",
        0.05,
        5.0,
        0.92,
    ));
    contract.variables.push(MonsterMathVariable::f64(
        "heat_capacity_j_kg_k",
        "none",
        500.0,
        2000.0,
        960.0,
    ));
    contract.variables.push(MonsterMathVariable::f64(
        "entropic_coeff_v_k",
        "none",
        -0.001,
        0.001,
        0.00012,
    ));
    contract.variables.push(MonsterMathVariable::f64(
        "activation_energy_j_mol",
        "none",
        20_000.0,
        150_000.0,
        82_000.0,
    ));
    contract.variables.push(MonsterMathVariable::f64(
        "gas_constant_j_mol_k",
        "none",
        8.0,
        8.5,
        8.314462618,
    ));
    contract.variables.push(MonsterMathVariable::f64(
        "side_reaction_prefactor_w_kg",
        "none",
        1.0e6,
        1.0e16,
        2.0e12,
    ));
    contract.variables.push(MonsterMathVariable::f64(
        "state_of_charge",
        "none",
        0.0,
        1.0,
        0.85,
    ));
    contract.operators = vec![
        "PDEStencil".to_string(),
        "mean".to_string(),
        "max_temperature".to_string(),
        "hotspot_location_x".to_string(),
        "hotspot_location_y".to_string(),
        "temperature_gradient_max".to_string(),
        "thermal_runaway_margin".to_string(),
        "time_to_threshold_estimate".to_string(),
        "cfl_stability_ratio".to_string(),
        "energy_balance_error".to_string(),
        "residual_norm".to_string(),
        "sensitivity".to_string(),
        "joule_heat_rate".to_string(),
        "entropic_heat_rate".to_string(),
        "arrhenius_heat_rate".to_string(),
        "threshold_crossing_time".to_string(),
        "simulated_steps".to_string(),
        "simulation_final_time".to_string(),
        "finite".to_string(),
    ];
    contract.equations.push(
        "next_temperature_field = PDEStencil(temperature_field, source_field, dt)".to_string(),
    );
    contract.samples.push(MonsterMathSample::new(
        "battery_pack_nominal_field",
        4096,
        vec![
            ("temperature_field", 313.15),
            ("source_field", 0.05),
            ("dt", 0.01),
            ("thermal_diffusivity", 1.2e-5),
            ("dx", 0.004),
            ("dy", 0.004),
            ("simulation_steps", 2000.0),
            ("current_a", 135.0),
            ("internal_resistance_ohm", 0.0018),
            ("cell_mass_kg", 0.92),
            ("heat_capacity_j_kg_k", 960.0),
            ("state_of_charge", 0.85),
            ("boundary_condition_code", 3.0),
            ("cooling_plate_temperature", 295.0),
        ],
        "max_temperature",
        375.0,
        60.0,
    ));
    contract
        .validation
        .push("field_tensor_typecheck_kernel_profile_proof_hash".to_string());
    contract.outputs.push(MonsterMathOutputContract {
        name: "next_temperature_field".to_string(),
        ty: "tensor<f32,64x64>".to_string(),
        unit: "K".to_string(),
        handoff: "vector".to_string(),
        expression: "temperature_field_next(PDEStencil(temperature_field, source_field, dt))".to_string(),
    });
    contract.outputs.push(MonsterMathOutputContract {
        name: "mean_temperature".to_string(),
        ty: "f32".to_string(),
        unit: "K".to_string(),
        handoff: "scalar".to_string(),
        expression: "mean(PDEStencil(temperature_field, source_field, dt))".to_string(),
    });
    contract.outputs.push(MonsterMathOutputContract {
        name: "max_temperature".to_string(),
        ty: "f64".to_string(),
        unit: "K".to_string(),
        handoff: "scalar".to_string(),
        expression: "max_temperature(PDEStencil(temperature_field, source_field, dt))".to_string(),
    });
    contract.outputs.push(MonsterMathOutputContract {
        name: "hotspot_location_x".to_string(),
        ty: "u64".to_string(),
        unit: "none".to_string(),
        handoff: "scalar".to_string(),
        expression: "hotspot_location_x(PDEStencil(temperature_field, source_field, dt))".to_string(),
    });
    contract.outputs.push(MonsterMathOutputContract {
        name: "hotspot_location_y".to_string(),
        ty: "u64".to_string(),
        unit: "none".to_string(),
        handoff: "scalar".to_string(),
        expression: "hotspot_location_y(PDEStencil(temperature_field, source_field, dt))".to_string(),
    });
    contract.outputs.push(MonsterMathOutputContract {
        name: "temperature_gradient_max".to_string(),
        ty: "f64".to_string(),
        unit: "none".to_string(),
        handoff: "scalar".to_string(),
        expression: "temperature_gradient_max(PDEStencil(temperature_field, source_field, dt), dx, dy)".to_string(),
    });
    contract.outputs.push(MonsterMathOutputContract {
        name: "thermal_runaway_margin".to_string(),
        ty: "f64".to_string(),
        unit: "K".to_string(),
        handoff: "scalar".to_string(),
        expression: "thermal_runaway_margin(PDEStencil(temperature_field, source_field, dt), separator_critical_temperature)".to_string(),
    });
    contract.outputs.push(MonsterMathOutputContract {
        name: "time_to_threshold_estimate".to_string(),
        ty: "f64".to_string(),
        unit: "none".to_string(),
        handoff: "scalar".to_string(),
        expression: "time_to_threshold_estimate(PDEStencil(temperature_field, source_field, dt), separator_critical_temperature, dt)".to_string(),
    });
    contract.outputs.push(MonsterMathOutputContract {
        name: "cfl_stability_ratio".to_string(),
        ty: "f64".to_string(),
        unit: "none".to_string(),
        handoff: "scalar".to_string(),
        expression: "cfl_stability_ratio(thermal_diffusivity, dt, dx, dy)".to_string(),
    });
    contract.outputs.push(MonsterMathOutputContract {
        name: "energy_balance_error".to_string(),
        ty: "f64".to_string(),
        unit: "none".to_string(),
        handoff: "scalar".to_string(),
        expression: "energy_balance_error(temperature_field, PDEStencil(temperature_field, source_field, dt), source_field, dt)".to_string(),
    });
    contract.outputs.push(MonsterMathOutputContract {
        name: "residual_norm".to_string(),
        ty: "f64".to_string(),
        unit: "none".to_string(),
        handoff: "scalar".to_string(),
        expression: "residual_norm(temperature_field, PDEStencil(temperature_field, source_field, dt))".to_string(),
    });
    contract.outputs.push(MonsterMathOutputContract {
        name: "sensitivity".to_string(),
        ty: "f64".to_string(),
        unit: "none".to_string(),
        handoff: "scalar".to_string(),
        expression: "sensitivity(PDEStencil(temperature_field, source_field, dt), thermal_diffusivity, source_peak, convection_rate, dx)".to_string(),
    });
    contract
}
