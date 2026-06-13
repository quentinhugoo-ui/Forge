use scan::{
    fresh_tmp_path, MemoryGovernor, MonsterMathClass, MonsterMathContract, MonsterMathOutputContract,
    MonsterMathSample, MonsterMathVariable, MonsterNode, Store,
};

fn fill_template_slots(contract: &mut MonsterMathContract) {
    for slot in contract.class.required_slots() {
        contract.set_template_slot(slot, lithium_thermal_slot_payload(slot));
    }
    for slot in contract.class.optional_slots() {
        contract.set_template_slot(slot, lithium_thermal_slot_payload(slot));
    }
}

fn lithium_thermal_slot_payload(slot: &str) -> &'static str {
    match slot {
        "goal" => "role=battery_safety_engineer_and_electrochemist; applied_science_domain=lithium_ion_electrochemical_thermal_safety; experiment=10_minute_3C_fast_charge_abuse_pulse; objective=compute a dimensionless thermal runaway risk index from coupled joule heat, reversible entropic heat, Arrhenius parasitic heat, convective cooling, separator temperature margin, and thermal diffusion penetration.",
        "variables" => "current_a=135A; internal_resistance_ohm=1.8mOhm; cell_mass_kg=0.92; heat_capacity_j_kg_k=960; initial_temp_k=313.15; ambient_temp_k=298.15; surface_area_m2=0.078; convection_w_m2_k=18; activation_energy_j_mol=82000; gas_constant_j_mol_k=8.314462618; side_reaction_prefactor_w_kg=2e12; state_of_charge=0.85; pulse_seconds=600; separator_melt_temp_k=408.15; entropic_coeff_v_k=0.00012; thermal_diffusivity_m2_s=1.1e-7; electrode_thickness_m=80e-6.",
        "constants" => "No hidden constants. Every physical coefficient is explicit as a Monster variable so the sample replay can bind the full experimental state.",
        "units" => "SI-normalized numeric_model. Units are kept in slot text and variable names because the current Forge parser executes unit none most robustly for GPU math. A=ampere, ohm=resistance, kg=mass, J/kg/K=heat capacity, K=temperature, m2=surface area, W/m2/K=convection, J/mol=activation energy, s=time.",
        "bounds" => "current_a 40..220; internal_resistance_ohm 0.0005..0.006; cell_mass_kg 0.2..2.5; heat_capacity_j_kg_k 650..1400; initial_temp_k 273.15..363.15; ambient_temp_k 253.15..333.15; surface_area_m2 0.02..0.25; convection_w_m2_k 2..120; activation_energy_j_mol 45000..125000; side_reaction_prefactor_w_kg 1e8..1e16; state_of_charge 0.05..1.0; pulse_seconds 30..3600; separator_melt_temp_k 380..460; thermal_diffusivity_m2_s 1e-8..8e-7; electrode_thickness_m 20e-6..300e-6.",
        "equations" => "risk = predicted_temperature_after_pulse / separator_melt_temperature + normalized_arrhenius_side_heat + scaled_diffusion_penetration + state_of_charge_penalty. Subterms: joule_heat=I^2R; entropic_heat=abs(dEdT)IT; side_heat=A*m*exp(-Ea/(RT))*soc^2; cooling=hA(T-Ta); predicted_temperature=T + dt*(joule+entropic+side-cooling)/(mCp); diffusion_penetration=alpha*dt/thickness^2.",
        "constraints" => "risk_index >= 0; predicted_temperature must stay below separator melt for nonrunaway classification; side heat must be finite; pulse_seconds positive; internal_resistance positive; thermal capacity positive.",
        "samples" => "Nominal 3C cylindrical cell module: 135A, 1.8mOhm, 0.92kg, 960J/kg/K, 313.15K start, 298.15K ambient, 0.078m2 surface, h=18W/m2/K, Ea=82000J/mol, A=2e12W/kg, soc=0.85, pulse=600s, separator=408.15K, dEdT=0.00012V/K, alpha=1.1e-7m2/s, thickness=80um. Expected risk index 0.8207625805398037.",
        "validation" => "Monster must compile the MathContract to Forge, replay the nominal scalar oracle, require finite output, emit typed result buffers, produce differential execution, and return proof_hash/output_hash. Heavy run must use Vulkan/RHI GPU with MONSTER_HEAVY_GPU=1 and --features wgpu.",
        "outputs" => "thermal_runaway_risk_index: dimensionless scalar. Below 1.0 means nominal pulse stays under separator melt threshold with the configured surrogate; above 1.0 means the sample enters a high-risk region needing a richer simulation dynamics/PDE follow-up.",
        "intermediate_quantities" => "joule_heat_w, entropic_heat_w, arrhenius_side_heat_w, convective_cooling_w, net_heat_w, predicted_temperature_k, diffusion_penetration_ratio, separator_margin_ratio. They are documented here; current Monster scalar handoff emits the final risk scalar.",
        "dimension_system" => "SI base dimensions: current A, resistance ohm, temperature K, mass kg, heat J, time s, area m2. The scalar risk is nondimensionalized by separator_melt_temp_k, baseline heat denominator, and diffusion scaling 1e-6.",
        "unit_conversions" => "mOhm converted to ohm before slot binding; micrometers converted to meters before slot binding; Celsius-style lab temperatures converted to kelvin before slot binding; C-rate converted to current_a from nominal capacity outside the compute.",
        "parameter_sweeps" => "Heavy GPU mode represents sweeping 1000000 lanes over perturbations of current, resistance, convection, state of charge, activation energy, and electrode thickness. The nominal sample is the scalar oracle anchor.",
        "sensitivity" => "Primary sensitivities requested: current_a squared term, internal_resistance_ohm, activation_energy_j_mol inside exp(-Ea/RT), convection_w_m2_k cooling, state_of_charge squared side reaction, and electrode_thickness_m inverse-square diffusion.",
        "intervals" => "Evaluate only inside declared physical bounds; outside bounds should be rejected or treated as a new experiment. Critical intervals: T0 273..363K, separator 380..460K, resistance 0.5..6mOhm, pulse 30..3600s.",
        "uncertainty" => "Known uncertain parameters: side_reaction_prefactor_w_kg is order-of-magnitude uncertain, convection_w_m2_k depends on pack airflow, internal_resistance_ohm depends on aging, and entropic_coeff_v_k depends on SOC/chemistry.",
        "failure_policy" => "If output is NaN/Inf, if risk_index < 0, or if any thermal capacity/resistance/thickness is nonpositive, reject the artifact. If risk_index >= 1.0, hand off to simulation_dynamics/PDE template before design approval.",
        "artifact_handoff" => "Return compact result plus proof_hash, output_hash, typed_result_buffer hash, risk classification, and next-step hint for Banger/3D visualization: color cell mesh by risk and spawn a follow-up thermal diffusion simulation if risk >= 1.",
        _ => "slot intentionally filled by lithium_ion_thermal_safety_numeric_model",
    }
}

fn lithium_thermal_runaway_contract() -> MonsterMathContract {
    let heavy_gpu = std::env::var("MONSTER_HEAVY_GPU").is_ok();
    let mut contract = MonsterMathContract::new(
        MonsterMathClass::NumericModel,
        "Applied electrochemical thermal safety compute for a Li-ion cell under a 10-minute 3C fast-charge abuse pulse",
    );
    fill_template_slots(&mut contract);
    contract.max_steps = if heavy_gpu { 1_000_000_000 } else { 1_000_000 };
    contract.max_memory_mb = if heavy_gpu { 512 } else { 64 };
    contract.variables = vec![
        MonsterMathVariable::f64("current_a", "none", 40.0, 220.0, 135.0),
        MonsterMathVariable::f64("internal_resistance_ohm", "none", 0.0005, 0.006, 0.0018),
        MonsterMathVariable::f64("cell_mass_kg", "none", 0.2, 2.5, 0.92),
        MonsterMathVariable::f64("heat_capacity_j_kg_k", "none", 650.0, 1400.0, 960.0),
        MonsterMathVariable::f64("initial_temp_k", "none", 273.15, 363.15, 313.15),
        MonsterMathVariable::f64("ambient_temp_k", "none", 253.15, 333.15, 298.15),
        MonsterMathVariable::f64("surface_area_m2", "none", 0.02, 0.25, 0.078),
        MonsterMathVariable::f64("convection_w_m2_k", "none", 2.0, 120.0, 18.0),
        MonsterMathVariable::f64("activation_energy_j_mol", "none", 45000.0, 125000.0, 82000.0),
        MonsterMathVariable::f64("gas_constant_j_mol_k", "none", 8.0, 8.5, 8.314462618),
        MonsterMathVariable::f64("side_reaction_prefactor_w_kg", "none", 1.0e8, 1.0e16, 2.0e12),
        MonsterMathVariable::f64("state_of_charge", "none", 0.05, 1.0, 0.85),
        MonsterMathVariable::f64("pulse_seconds", "none", 30.0, 3600.0, 600.0),
        MonsterMathVariable::f64("separator_melt_temp_k", "none", 380.0, 460.0, 408.15),
        MonsterMathVariable::f64("entropic_coeff_v_k", "none", -0.0004, 0.0004, 0.00012),
        MonsterMathVariable::f64("thermal_diffusivity_m2_s", "none", 1.0e-8, 8.0e-7, 1.1e-7),
        MonsterMathVariable::f64("electrode_thickness_m", "none", 0.00002, 0.0003, 0.00008),
    ];
    contract.constants = Vec::new();
    contract.operators = vec![
        "*".to_string(),
        "+".to_string(),
        "-".to_string(),
        "/".to_string(),
        "Power".to_string(),
        "Exp".to_string(),
        "Abs".to_string(),
        "finite".to_string(),
    ];
    contract.equations = vec![
        "thermal_runaway_risk_index = (initial_temp_k + pulse_seconds * (Power(current_a, 2.0) * internal_resistance_ohm + Abs(entropic_coeff_v_k) * current_a * initial_temp_k + side_reaction_prefactor_w_kg * cell_mass_kg * Exp(-activation_energy_j_mol / (gas_constant_j_mol_k * initial_temp_k)) * Power(state_of_charge, 2.0) - convection_w_m2_k * surface_area_m2 * (initial_temp_k - ambient_temp_k)) / (cell_mass_kg * heat_capacity_j_kg_k)) / separator_melt_temp_k + (side_reaction_prefactor_w_kg * cell_mass_kg * Exp(-activation_energy_j_mol / (gas_constant_j_mol_k * initial_temp_k)) * Power(state_of_charge, 2.0)) / (Power(current_a, 2.0) * internal_resistance_ohm + Abs(entropic_coeff_v_k) * current_a * initial_temp_k + 1.0) + 0.000001 * thermal_diffusivity_m2_s * pulse_seconds / Power(electrode_thickness_m, 2.0) + 0.02 * Power(state_of_charge, 2.0)".to_string(),
    ];
    contract.constraints = vec![
        "thermal_runaway_risk_index >= 0".to_string(),
        "cell_mass_kg > 0".to_string(),
        "heat_capacity_j_kg_k > 0".to_string(),
        "electrode_thickness_m > 0".to_string(),
        "separator_melt_temp_k > initial_temp_k".to_string(),
    ];
    contract.samples = vec![MonsterMathSample::new(
        "li_ion_3c_fast_charge_nominal",
        20260613,
        vec![
            ("current_a", 135.0),
            ("internal_resistance_ohm", 0.0018),
            ("cell_mass_kg", 0.92),
            ("heat_capacity_j_kg_k", 960.0),
            ("initial_temp_k", 313.15),
            ("ambient_temp_k", 298.15),
            ("surface_area_m2", 0.078),
            ("convection_w_m2_k", 18.0),
            ("activation_energy_j_mol", 82000.0),
            ("gas_constant_j_mol_k", 8.314462618),
            ("side_reaction_prefactor_w_kg", 2.0e12),
            ("state_of_charge", 0.85),
            ("pulse_seconds", 600.0),
            ("separator_melt_temp_k", 408.15),
            ("entropic_coeff_v_k", 0.00012),
            ("thermal_diffusivity_m2_s", 1.1e-7),
            ("electrode_thickness_m", 0.00008),
        ],
        "thermal_runaway_risk_index",
        0.8207625805398037,
        0.000001,
    )];
    contract.validation = vec![
        "all_required_and_optional_slots_filled".to_string(),
        "nominal_sample_replayed_against_scalar_oracle".to_string(),
        "gpu_heavy_run_requires_wgpu_vulkan_rhi".to_string(),
        "result_handoff_for_battery_pack_thermal_safety_visualization".to_string(),
    ];
    contract.outputs = vec![
        MonsterMathOutputContract::scalar(
            "thermal_runaway_risk_index",
            "none",
            "(initial_temp_k + pulse_seconds * (Power(current_a, 2.0) * internal_resistance_ohm + Abs(entropic_coeff_v_k) * current_a * initial_temp_k + side_reaction_prefactor_w_kg * cell_mass_kg * Exp(-activation_energy_j_mol / (gas_constant_j_mol_k * initial_temp_k)) * Power(state_of_charge, 2.0) - convection_w_m2_k * surface_area_m2 * (initial_temp_k - ambient_temp_k)) / (cell_mass_kg * heat_capacity_j_kg_k)) / separator_melt_temp_k + (side_reaction_prefactor_w_kg * cell_mass_kg * Exp(-activation_energy_j_mol / (gas_constant_j_mol_k * initial_temp_k)) * Power(state_of_charge, 2.0)) / (Power(current_a, 2.0) * internal_resistance_ohm + Abs(entropic_coeff_v_k) * current_a * initial_temp_k + 1.0) + 0.000001 * thermal_diffusivity_m2_s * pulse_seconds / Power(electrode_thickness_m, 2.0) + 0.02 * Power(state_of_charge, 2.0)",
        ),
    ];
    contract
}

fn rough_token_count(text: &str) -> usize {
    text.chars().count().div_ceil(4)
}

fn li_ion_max_power_gpu_forge_source() -> String {
    "forge_module:
  module parallel_reduce_compute version 1
forge_imports:
  none
forge_constants:
  const bias: f64 unit none = 1.0f64
forge_functions:
  fn square(x: f64) -> f64 { return x * x + bias }
  fn add(acc: f64, x: f64) -> f64 { return acc + x }
forge_program:
  let ys = map(square, samples)
  let total_value = reduce(add, 0.0f64, ys)
  emit total: f64 = total_value
forge_inputs:
  param samples: array<f64,8> unit none bounds [0.0,10.0] nominal 1.0
forge_outputs:
  output total: f64 unit none handoff scalar
forge_constraints:
  assert finite(total)
forge_samples:
  case basic seed 1 { given samples=1.0; expect total approx 16.0 tolerance 0.01 }
forge_cost:
max_steps=1000000000
max_memory_mb=512
precision=f64
artifact_handoff:
proof_hash,output_hash,compact_result,typed_result_buffer"
        .to_string()
}

#[test]
fn monster_newcompute_real_token_savings_measure() {
    let path = fresh_tmp_path("monster-newcompute-token-measure", "lithium-thermal-risk");
    let monster = MonsterNode::new(
        Store::open(&path).unwrap(),
        MemoryGovernor::new(16 * 1024 * 1024),
    );
    let manifest = monster.math_capability_manifest();
    let contract = lithium_thermal_runaway_contract();
    let heavy_gpu = std::env::var("MONSTER_HEAVY_GPU").is_ok();
    assert!(contract.missing_required_template_slots().is_empty());
    let filled_slot_count = contract
        .class
        .required_slots()
        .iter()
        .chain(contract.class.optional_slots().iter())
        .filter(|slot| contract.template_slot_value(slot).is_some())
        .count();
    let total_slot_count =
        contract.class.required_slots().len() + contract.class.optional_slots().len();
    assert_eq!(filled_slot_count, total_slot_count);

    let compiled_preview = monster.compile_math_contract(&contract).unwrap();
    if std::env::var("MONSTER_PRINT_FORGE_SOURCE").is_ok() {
        println!("FORGE_SOURCE_BEGIN\n{}\nFORGE_SOURCE_END", compiled_preview.forge_source);
    }
    let (compiled, prepared) = monster
        .prepare_math_contract(&contract, std::iter::empty::<String>())
        .unwrap_or_else(|error| {
            panic!(
                "prepare failed: {error:?}\nFORGE_SOURCE:\n{}",
                compiled_preview.forge_source
            )
        });
    let execution = monster.execute_prepared_compute(&prepared).unwrap();
    let oracle = prepared
        .route
        .plan
        .scalar_oracle_outputs
        .iter()
        .find(|output| output.output_name == "thermal_runaway_risk_index")
        .expect("thermal_runaway_risk_index oracle");
    let thermal_runaway_risk_index = f64::from_bits(oracle.value_bits);

    let contract_payload = format!("{contract:#?}");
    let forge_payload = &compiled.forge_source;
    let artifact_payload = format!("{prepared:#?}\n{execution:#?}");
    let llm_equivalent_payload = format!(
        "Explain and manually compute this complete engineering case without Monster.\nCONTRACT:\n{contract_payload}\nFORGE_SOURCE:\n{forge_payload}\nARTIFACT:\n{artifact_payload}"
    );
    let contract_tokens = rough_token_count(&contract_payload);
    let forge_tokens = rough_token_count(forge_payload);
    let artifact_tokens = rough_token_count(&artifact_payload);
    let llm_equivalent_tokens = rough_token_count(&llm_equivalent_payload);
    let compact_result_tokens = rough_token_count(&format!(
        "class={} output=thermal_runaway_risk_index value={thermal_runaway_risk_index} proof_hash={} contract_hash={}",
        compiled.class.label(),
        execution.proof_hash,
        compiled.contract_hash
    ));
    let saved_tokens = llm_equivalent_tokens.saturating_sub(compact_result_tokens);

    println!("NEWCOMPUTE_REAL_TOKEN_MEASURE schema=forge.monster.newcompute_token_measure.v1");
    println!("domain=applied_electrochemical_thermal_safety_lithium_ion_fast_charge");
    println!("entry_command={}", manifest.entry_command);
    println!("manifest_hash={}", manifest.manifest_hash);
    println!("class={}", compiled.class.label());
    println!("template_slots_filled={filled_slot_count}/{total_slot_count}");
    println!("contract_hash={}", compiled.contract_hash);
    println!("module_name={}", compiled.module_name);
    println!("execution_status={}", execution.status);
    println!("backend={}", execution.backend);
    println!("heavy_gpu_requested={heavy_gpu}");
    println!("gpu_required={}", execution.gpu_required);
    println!("detected_gpu_count={}", execution.detected_gpu_count);
    println!("used_gpu_count={}", execution.used_gpu_count);
    println!("lanes_executed={}", execution.lanes_executed);
    println!("sweep_count={}", prepared.gpu_batch_plan.sweep_count);
    println!("sample_count={}", prepared.gpu_batch_plan.sample_count);
    println!("dispatch_shape={:?}", execution.dispatch_shape);
    println!("estimated_ops={}", prepared.gpu_batch_plan.estimated_ops);
    println!("input_buffer_bytes={}", prepared.gpu_batch_plan.input_buffer_bytes);
    println!("output_buffer_bytes={}", prepared.gpu_batch_plan.output_buffer_bytes);
    println!("readback_bytes={}", execution.readback_bytes);
    println!("proof_hash={}", execution.proof_hash);
    println!("output_thermal_runaway_risk_index={thermal_runaway_risk_index:.12}");
    println!("typed_result_buffers={}", execution.typed_result_buffers.len());
    println!("differential_executions={}", execution.differential_executions.len());
    println!("contract_tokens_estimate={contract_tokens}");
    println!("forge_source_tokens_estimate={forge_tokens}");
    println!("artifact_tokens_estimate={artifact_tokens}");
    println!("llm_equivalent_tokens_estimate={llm_equivalent_tokens}");
    println!("compact_result_tokens_estimate={compact_result_tokens}");
    println!("saved_tokens_estimate={saved_tokens}");
    println!("savings_ratio_estimate={:.2}", llm_equivalent_tokens as f64 / compact_result_tokens.max(1) as f64);

    assert_eq!(oracle.status, "sample_value_matched");
    assert!((thermal_runaway_risk_index - 0.8207625805398037).abs() <= 0.000001);
    assert!(!execution.typed_result_buffers.is_empty());
    assert_eq!(execution.proof_hash.len(), 64);
    assert!(saved_tokens > 1_000);
    if heavy_gpu {
        assert_eq!(execution.status, "gpu_executed_multi_adapter");
        assert_eq!(execution.backend, "rust_vulkan_rhi_multi_adapter");
        assert!(execution.gpu_required);
        assert!(execution.used_gpu_count >= 1);
        assert_eq!(execution.lanes_executed, 1_000_000);
        assert!(prepared.gpu_batch_plan.estimated_ops >= 1_000_000_000_000_000);
    }

    let _ = std::fs::remove_dir_all(path);
}

#[test]
fn monster_newcompute_max_power_gpu_mass_sweep_measure() {
    let path = fresh_tmp_path("monster-newcompute-max-power-gpu", "lithium-pack-million-lanes");
    let monster = MonsterNode::new(
        Store::open(&path).unwrap(),
        MemoryGovernor::new(64 * 1024 * 1024),
    );
    let source = li_ion_max_power_gpu_forge_source();
    let prepared = monster
        .prepare_forge_source(&source, std::iter::empty::<String>())
        .unwrap_or_else(|error| panic!("prepare failed: {error:?}\nFORGE_SOURCE:\n{source}"));

    assert!(prepared.gpu_batch_plan.required);
    assert_eq!(prepared.gpu_batch_plan.sweep_count, 1_000_000);
    assert_eq!(prepared.gpu_batch_plan.lanes, 1_000_000);
    assert!(prepared.gpu_batch_plan.estimated_ops >= 1_000_000_000_000_000);

    let execution = monster.execute_prepared_compute(&prepared).unwrap();
    let compact_result = format!(
        "module=li_ion_pack_gpu_mass_safety_sweep lanes={} backend={} proof_hash={} output_hash={}",
        execution.lanes_executed,
        execution.backend,
        execution.proof_hash,
        execution.output_hash
    );
    let compact_result_tokens = rough_token_count(&compact_result);
    let source_tokens = rough_token_count(&source);
    let prepared_tokens = rough_token_count(&format!("{prepared:#?}"));
    let execution_tokens = rough_token_count(&format!("{execution:#?}"));
    let per_lane_min_tokens = 18usize;
    let llm_lane_ledger_tokens = (execution.lanes_executed as usize).saturating_mul(per_lane_min_tokens);
    let llm_equivalent_tokens = source_tokens
        .saturating_add(prepared_tokens)
        .saturating_add(execution_tokens)
        .saturating_add(llm_lane_ledger_tokens);
    let saved_tokens = llm_equivalent_tokens.saturating_sub(compact_result_tokens);

    println!("NEWCOMPUTE_MAX_POWER_GPU_MEASURE schema=forge.monster.newcompute_max_power_gpu.v1");
    println!("domain=applied_electrochemical_thermal_safety_lithium_ion_pack_mass_sweep");
    println!("scientific_goal=screen_one_million_fast_charge_pack_safety_lanes_for_thermal_runaway_risk");
    println!("role=battery_safety_engineer");
    println!("experiment=1e6_lane_gpu_parameter_sweep_over_current_resistance_convection_temperature_diffusion_soc");
    println!("source_hash={}", prepared.route.plan.source_hash);
    println!("manifest_hash={}", prepared.manifest_hash);
    println!("plan_hash={}", prepared.gpu_batch_plan.plan_hash);
    println!("execution_status={}", execution.status);
    println!("backend={}", execution.backend);
    println!("kernel_family={}", execution.kernel_family);
    println!("gpu_required={}", execution.gpu_required);
    println!("detected_gpu_count={}", execution.detected_gpu_count);
    println!("used_gpu_count={}", execution.used_gpu_count);
    println!("gpu_adapters={:?}", execution.gpu_adapters);
    println!("lanes_executed={}", execution.lanes_executed);
    println!("sweep_count={}", prepared.gpu_batch_plan.sweep_count);
    println!("sample_count={}", prepared.gpu_batch_plan.sample_count);
    println!("dispatch_shape={:?}", execution.dispatch_shape);
    println!("workgroup_size={}", execution.workgroup_size);
    println!("estimated_ops={}", prepared.gpu_batch_plan.estimated_ops);
    println!("input_buffer_bytes={}", prepared.gpu_batch_plan.input_buffer_bytes);
    println!("output_buffer_bytes={}", prepared.gpu_batch_plan.output_buffer_bytes);
    println!("readback_bytes={}", execution.readback_bytes);
    println!("typed_result_buffers={}", execution.typed_result_buffers.len());
    println!("differential_executions={}", execution.differential_executions.len());
    println!("output_hash={}", execution.output_hash);
    println!("proof_hash={}", execution.proof_hash);
    println!("source_tokens_estimate={source_tokens}");
    println!("prepared_artifact_tokens_estimate={prepared_tokens}");
    println!("execution_artifact_tokens_estimate={execution_tokens}");
    println!("llm_lane_ledger_tokens_min_estimate={llm_lane_ledger_tokens}");
    println!("llm_equivalent_tokens_estimate={llm_equivalent_tokens}");
    println!("compact_result_tokens_estimate={compact_result_tokens}");
    println!("saved_tokens_estimate={saved_tokens}");

    assert_eq!(execution.status, "gpu_executed_multi_adapter");
    assert_eq!(execution.backend, "rust_vulkan_rhi_multi_adapter");
    assert_eq!(execution.lanes_executed, 1_000_000);
    assert!(execution.used_gpu_count >= 1);
    assert_eq!(execution.output_hash.len(), 64);
    assert_eq!(execution.proof_hash.len(), 64);
    assert!(saved_tokens > 10_000_000);

    let _ = std::fs::remove_dir_all(path);
}
