use ingen_native_services::math_compute_service::run_native_math_contract_for_llm;
use scan::{
    fresh_tmp_path, kasm::ForgePrecision, MonsterMathClass, MonsterMathContract,
    MonsterMathOutputContract, MonsterMathSample, MonsterMathVariable,
};

fn main() {
    let codeact = heat_pde_codeact();
    let contract = parse_newcompute_codeact(codeact).expect("parse /newcompute_ CodeAct");
    let path = fresh_tmp_path("native-newcompute", "science-codeact-audit");
    let result = run_native_math_contract_for_llm(&path, &contract)
        .expect("run simulation_dynamics heat PDE /newcompute_ CodeAct");
    println!("CODEACT_NEWCOMPUTE_AUDIT schema=forge.codeact.newcompute_audit.v1");
    println!("codeact_command=/newcompute_");
    println!("codeact_lines={}", codeact.lines().count());
    println!("template_slots_filled={}", contract.template_slots.len());
    println!("variables={}", contract.variables.len());
    println!("operators={}", contract.operators.len());
    println!("equations={}", contract.equations.len());
    println!("samples={}", contract.samples.len());
    println!("validations={}", contract.validation.len());
    println!("outputs={}", contract.outputs.len());
    println!("{}", result.compact_text);
    let _ = std::fs::remove_dir_all(path);
}

fn heat_pde_codeact() -> &'static str {
    r#"/newcompute_
math_class=/simulation_dynamics
goal=Applied thermal-safety scientist validates a heavy electro-thermal 2D battery-pack compute with analytic and FEM references.
max_steps=100000000
max_memory_mb=512
precision=f64
slot.goal=role=thermal_safety_scientist; objective=validate battery pack heat-risk compute; success=field, safety metrics, convergence, analytic reference, FEM reference, experimental reference, calibration, Monte Carlo uncertainty and proof hash.
slot.workload_scale=max_steps=100000000; max_memory_mb=512; min_estimated_ops=100000000; gpu=max_power; lanes=100000.
slot.states=temperature_field tensor<f32,64x64> K; source_field tensor<f32,64x64> K; scalar electrothermal parameters with bounds and provenance.
slot.time_domain=t0=0s; dt=0.01s; steps=2000; final_time=20s.
slot.equations=next_temperature_field = PDEStencil(temperature_field, source_field, dt); heat terms=Joule+entropic+Arrhenius+hotspot-convection-cooling_plate.
slot.initial_conditions=baseline=313.15K; gaussian hotspot center=(37,31), sigma=5 cells, amplitude=62K.
slot.boundary_conditions=cooling_plate_edge; cooling_plate_temperature=295K; ambient=300K.
slot.events=separator_critical_temperature=408K; report threshold crossing time and runaway margin.
slot.integrator_request=explicit heat stencil; CFL gate; multi-step; Monster MassMath; GPU required.
slot.residual_checks=energy_balance_error,residual_norm,cpu_f64_reference_vs_f32_quantized_candidate,grid_dt_convergence,analytic_reference_check,fem_reference_check,experimental_reference_check.
slot.parameters=diffusivity,source,convection,dx,dy,hotspot,current,resistance,temperature_resistance_coeff,SOC_heat_factor,anisotropy,contact_resistance,heat_capacity,Arrhenius,SOC,provenance_codes.
slot.stability_policy=explicit CFL gate; dt halving convergence; coarse grid convergence; reject unstable CFL.
slot.mesh=64x64 Cartesian tensor field; cooling plate edge boundary; FEM reference uses independent Q1 mass-lumped stencil.
slot.forcing_functions=gaussian hotspot plus Joule, entropic and Arrhenius heat generation with convective/cooling sinks.
slot.solver_tolerances=dt_half_max_delta<=0.5K; fem_max_delta<=3K; analytic_linf<=0.05K; experimental_rmse<=1K; energy_balance_error<=1e-9.
slot.checkpointing=compact time series at 16 checkpoints for max, mean and runaway margin.
slot.artifact_handoff=typed tensor buffer hashes, compact metrics, output_hash, proof_hash.
var=temperature_field|tensor<f32,64x64>|K|250|460|313.15
var=source_field|tensor<f32,64x64>|K|0|1|0.05
var=dt|f32|s|0.0001|0.1|0.01
var=simulation_steps|f64|none|1|200000|2000
var=thermal_diffusivity|f64|none|0.0000001|0.001|0.000012
var=dx|f64|m|0.001|0.02|0.004
var=dy|f64|m|0.001|0.02|0.004
var=separator_critical_temperature|f64|K|350|500|408
var=ambient_temperature|f64|K|250|330|300
var=boundary_condition_code|f64|none|0|3|3
var=cooling_plate_temperature|f64|K|260|330|295
var=convection_rate|f64|none|0|0.2|0.015
var=hotspot_center_x|f64|none|0|63|37
var=hotspot_center_y|f64|none|0|63|31
var=hotspot_sigma_cells|f64|none|1|16|5
var=hotspot_amplitude|f64|K|0|120|62
var=source_peak|f64|K|0|5|0.85
var=current_a|f64|none|0|300|135
var=internal_resistance_ohm|f64|none|0.0001|0.02|0.0018
var=cell_mass_kg|f64|kg|0.05|5|0.92
var=heat_capacity_j_kg_k|f64|none|500|2000|960
var=entropic_coeff_v_k|f64|none|-0.001|0.001|0.00012
var=activation_energy_j_mol|f64|none|20000|150000|82000
var=gas_constant_j_mol_k|f64|none|8|8.5|8.314462618
var=side_reaction_prefactor_w_kg|f64|none|1000000|10000000000000000|2000000000000
var=state_of_charge|f64|none|0|1|0.85
var=thermal_anisotropy_ratio|f64|none|0.1|10|1.35
var=resistance_temp_coeff|f64|none|-0.02|0.05|0.004
var=soc_heat_factor|f64|none|-1|1|0.12
var=contact_resistance_rate|f64|none|0|0.1|0.002
var=thermal_diffusivity_provenance_code|f64|none|0|4|4
var=internal_resistance_provenance_code|f64|none|0|4|2
var=heat_capacity_provenance_code|f64|none|0|4|2
var=entropic_coeff_provenance_code|f64|none|0|4|1
var=arrhenius_provenance_code|f64|none|0|4|4
var=convection_provenance_code|f64|none|0|4|1
var=experimental_t00_s|f64|s|0|20|1.25
var=experimental_max00_k|f64|K|250|460|370.51
var=experimental_t01_s|f64|s|0|20|2.5
var=experimental_max01_k|f64|K|250|460|366.39
var=experimental_t02_s|f64|s|0|20|5.0
var=experimental_max02_k|f64|K|250|460|359.95
var=experimental_t03_s|f64|s|0|20|7.5
var=experimental_max03_k|f64|K|250|460|354.79
var=experimental_t04_s|f64|s|0|20|10.0
var=experimental_max04_k|f64|K|250|460|350.86
var=experimental_t05_s|f64|s|0|20|12.5
var=experimental_max05_k|f64|K|250|460|347.48
var=experimental_t06_s|f64|s|0|20|15.0
var=experimental_max06_k|f64|K|250|460|344.88
var=experimental_t07_s|f64|s|0|20|20.0
var=experimental_max07_k|f64|K|250|460|340.69
operators=PDEStencil,mean,max_temperature,hotspot_location_x,hotspot_location_y,temperature_gradient_max,thermal_runaway_margin,time_to_threshold_estimate,cfl_stability_ratio,energy_balance_error,residual_norm,sensitivity,joule_heat_rate,entropic_heat_rate,arrhenius_heat_rate,threshold_crossing_time,simulated_steps,simulation_final_time,grid_dt_convergence,energy_flux_breakdown,uncertainty_interval,analytic_reference_check,fem_reference_check,experimental_reference_check,calibration_result,monte_carlo_uncertainty,richardson_convergence,material_parameter_provenance,gpu_execution_audit,numerical_validity_score,engineering_decision_score,validation_readiness_score,validation_battery_thermal,finite
equation=next_temperature_field = PDEStencil(temperature_field, source_field, dt)
sample=battery_pack_nominal_field|4096|max_temperature|375|60|temperature_field=313.15,source_field=0.05,dt=0.01,thermal_diffusivity=0.000012,dx=0.004,dy=0.004,simulation_steps=2000,current_a=135,internal_resistance_ohm=0.0018,cell_mass_kg=0.92,heat_capacity_j_kg_k=960,state_of_charge=0.85,boundary_condition_code=3,cooling_plate_temperature=295,thermal_diffusivity_provenance_code=4,internal_resistance_provenance_code=2,heat_capacity_provenance_code=2,entropic_coeff_provenance_code=1,arrhenius_provenance_code=4,convection_provenance_code=1
validation=field_tensor_typecheck_kernel_profile_proof_hash
validation=validation_battery_thermal:grid_dt_convergence,analytic_reference_check,fem_reference_check,experimental_reference_check,calibration_result,monte_carlo_uncertainty,richardson_convergence,energy_flux_breakdown,uncertainty_interval,material_parameter_provenance,numerical_validity_score,engineering_decision_score
output=next_temperature_field|tensor<f32,64x64>|K|vector|temperature_field_next(PDEStencil(temperature_field, source_field, dt))
output=mean_temperature|f32|K|scalar|mean(PDEStencil(temperature_field, source_field, dt))
output=max_temperature|f64|K|scalar|max_temperature(PDEStencil(temperature_field, source_field, dt))
output=hotspot_location_x|u64|none|scalar|hotspot_location_x(PDEStencil(temperature_field, source_field, dt))
output=hotspot_location_y|u64|none|scalar|hotspot_location_y(PDEStencil(temperature_field, source_field, dt))
output=temperature_gradient_max|f64|none|scalar|temperature_gradient_max(PDEStencil(temperature_field, source_field, dt), dx, dy)
output=thermal_runaway_margin|f64|K|scalar|thermal_runaway_margin(PDEStencil(temperature_field, source_field, dt), separator_critical_temperature)
output=time_to_threshold_estimate|f64|none|scalar|time_to_threshold_estimate(PDEStencil(temperature_field, source_field, dt), separator_critical_temperature, dt)
output=cfl_stability_ratio|f64|none|scalar|cfl_stability_ratio(thermal_diffusivity, dt, dx, dy)
output=energy_balance_error|f64|none|scalar|energy_balance_error(temperature_field, PDEStencil(temperature_field, source_field, dt), source_field, dt)
output=residual_norm|f64|none|scalar|residual_norm(temperature_field, PDEStencil(temperature_field, source_field, dt))
output=sensitivity|f64|none|scalar|sensitivity(PDEStencil(temperature_field, source_field, dt), thermal_diffusivity, source_peak, convection_rate, dx)
"#
}

fn parse_newcompute_codeact(codeact: &str) -> Result<MonsterMathContract, String> {
    let mut command_seen = false;
    let mut class = None;
    let mut goal = "";
    let mut max_steps = None;
    let mut max_memory_mb = None;
    let mut precision = ForgePrecision::F64;
    let mut slots = Vec::new();
    let mut variables = Vec::new();
    let mut operators = Vec::new();
    let mut equations = Vec::new();
    let mut samples = Vec::new();
    let mut validation = Vec::new();
    let mut outputs = Vec::new();

    for raw in codeact.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if line == "/newcompute_" {
            command_seen = true;
            continue;
        }
        let (key, value) = line
            .split_once('=')
            .ok_or_else(|| format!("invalid CodeAct line without '=': {line}"))?;
        match key {
            "math_class" => {
                class = Some(match value {
                    "/simulation_dynamics" | "simulation_dynamics" => MonsterMathClass::SimulationDynamics,
                    other => return Err(format!("unsupported audit math class: {other}")),
                });
            }
            "goal" => goal = value,
            "max_steps" => max_steps = Some(parse_u64(value, key)?),
            "max_memory_mb" => max_memory_mb = Some(parse_u64(value, key)?),
            "precision" => {
                precision = match value {
                    "f32" => ForgePrecision::F32,
                    "f64" => ForgePrecision::F64,
                    other => return Err(format!("unsupported precision: {other}")),
                };
            }
            key if key.starts_with("slot.") => {
                slots.push((key.trim_start_matches("slot.").to_string(), value.to_string()));
            }
            "var" => variables.push(parse_variable(value)?),
            "operators" => operators.extend(
                value
                    .split(',')
                    .map(str::trim)
                    .filter(|operator| !operator.is_empty())
                    .map(str::to_string),
            ),
            "equation" => equations.push(value.to_string()),
            "sample" => samples.push(parse_sample(value)?),
            "validation" => validation.push(value.to_string()),
            "output" => outputs.push(parse_output(value)?),
            other => return Err(format!("unknown CodeAct key: {other}")),
        }
    }
    if !command_seen {
        return Err("missing /newcompute_ command".to_string());
    }
    let class = class.ok_or_else(|| "missing math_class".to_string())?;
    let mut contract = MonsterMathContract::new(class, goal);
    for (slot, value) in slots {
        contract.set_template_slot(&slot, value);
    }
    contract.max_steps = max_steps.ok_or_else(|| "missing max_steps".to_string())?;
    contract.max_memory_mb = max_memory_mb.ok_or_else(|| "missing max_memory_mb".to_string())?;
    contract.precision = precision;
    contract.variables = variables;
    contract.operators = operators;
    contract.equations = equations;
    contract.samples = samples;
    contract.validation = validation;
    contract.outputs = outputs;
    Ok(contract)
}

fn parse_variable(value: &str) -> Result<MonsterMathVariable, String> {
    let parts = split_exact(value, '|', 6, "var")?;
    Ok(MonsterMathVariable {
        name: parts[0].to_string(),
        ty: parts[1].to_string(),
        unit: parts[2].to_string(),
        min: parse_f64(parts[3], "var.min")?,
        max: parse_f64(parts[4], "var.max")?,
        nominal: parse_f64(parts[5], "var.nominal")?,
    })
}

fn parse_sample(value: &str) -> Result<MonsterMathSample, String> {
    let parts = split_exact(value, '|', 6, "sample")?;
    let givens = parts[5]
        .split(',')
        .map(|entry| {
            let (name, value) = entry
                .split_once('=')
                .ok_or_else(|| format!("invalid sample given: {entry}"))?;
            Ok((name.trim(), parse_f64(value.trim(), "sample.given")?))
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(MonsterMathSample::new(
        parts[0],
        parse_u64(parts[1], "sample.seed")?,
        givens,
        parts[2],
        parse_f64(parts[3], "sample.expected")?,
        parse_f64(parts[4], "sample.tolerance")?,
    ))
}

fn parse_output(value: &str) -> Result<MonsterMathOutputContract, String> {
    let parts = split_exact(value, '|', 5, "output")?;
    Ok(MonsterMathOutputContract {
        name: parts[0].to_string(),
        ty: parts[1].to_string(),
        unit: parts[2].to_string(),
        handoff: parts[3].to_string(),
        expression: parts[4].to_string(),
    })
}

fn split_exact<'a>(
    value: &'a str,
    delimiter: char,
    expected: usize,
    context: &str,
) -> Result<Vec<&'a str>, String> {
    let parts = value.split(delimiter).map(str::trim).collect::<Vec<_>>();
    if parts.len() != expected {
        return Err(format!("{context} expected {expected} fields, got {}", parts.len()));
    }
    Ok(parts)
}

fn parse_f64(value: &str, context: &str) -> Result<f64, String> {
    value
        .parse::<f64>()
        .map_err(|error| format!("{context} parse f64 failed for {value:?}: {error}"))
}

fn parse_u64(value: &str, context: &str) -> Result<u64, String> {
    value
        .parse::<u64>()
        .map_err(|error| format!("{context} parse u64 failed for {value:?}: {error}"))
}
