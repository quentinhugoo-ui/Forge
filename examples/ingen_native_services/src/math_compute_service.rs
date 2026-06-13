use std::path::Path;

use scan::{
    MemoryGovernor, MonsterMathClass, MonsterMathConstant, MonsterMathContract,
    MonsterMathOutputContract, MonsterMathSample, MonsterMathVariable, MonsterNode, Store,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NativeMathSlotSpec {
    pub name: String,
    pub required: bool,
    pub value_kind: String,
    pub accepted_content: String,
    pub forge_binding: String,
    pub validation_rule: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NativeMathClassInfo {
    pub command: String,
    pub required_slots: Vec<String>,
    pub optional_slots: Vec<String>,
    pub slot_specs: Vec<NativeMathSlotSpec>,
    pub accepted_operators: Vec<String>,
    pub classical_aliases: Vec<String>,
    pub forge_targets: Vec<String>,
    pub rejection_reasons: Vec<String>,
    pub deterministic_translation: String,
    pub compile_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NativeMathManifest {
    pub schema: String,
    pub entry_command: String,
    pub dispatch_rule: String,
    pub manifest_hash: String,
    pub classes: Vec<NativeMathClassInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NativeMathContractRun {
    pub schema: String,
    pub class: String,
    pub contract_hash: String,
    pub module_name: String,
    pub manifest_hash: String,
    pub execution_status: String,
    pub backend: String,
    pub output_name: String,
    pub output_value: f64,
    pub proof_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NativeNewComputeLlmScalarOutput {
    pub sample_name: String,
    pub output_name: String,
    pub status: String,
    pub value_bits: u64,
    pub value_text: String,
    pub expected_bits: u64,
    pub tolerance_bits: u64,
    pub abs_error_bits: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NativeNewComputeLlmTypedBuffer {
    pub name: String,
    pub forge_type: String,
    pub layout: String,
    pub element_type: String,
    pub shape: Vec<u64>,
    pub byte_len: u64,
    pub page_count: u64,
    pub buffer_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NativeNewComputeLlmScientificMetric {
    pub name: String,
    pub value_text: String,
    pub unit: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NativeNewComputeLlmResult {
    pub schema: String,
    pub class: String,
    pub module_name: String,
    pub contract_hash: String,
    pub manifest_hash: String,
    pub projection_hash: String,
    pub route_lane: String,
    pub execution_status: String,
    pub backend: String,
    pub kernel_family: String,
    pub detected_gpu_count: u32,
    pub used_gpu_count: u32,
    pub gpu_required: bool,
    pub lanes_executed: u64,
    pub dispatch_shape: [u32; 3],
    pub workgroup_size: u32,
    pub input_buffer_bytes: u64,
    pub output_buffer_bytes: u64,
    pub readback_bytes: u64,
    pub output_hash: String,
    pub proof_hash: String,
    pub scalar_outputs: Vec<NativeNewComputeLlmScalarOutput>,
    pub typed_buffers: Vec<NativeNewComputeLlmTypedBuffer>,
    pub scientific_metrics: Vec<NativeNewComputeLlmScientificMetric>,
    pub limitations: Vec<String>,
    pub compact_text: String,
}

pub fn native_math_manifest(store_path: &Path) -> Result<NativeMathManifest, String> {
    let monster = open_monster(store_path)?;
    let manifest = monster.math_capability_manifest();
    Ok(NativeMathManifest {
        schema: manifest.schema.to_string(),
        entry_command: manifest.entry_command.to_string(),
        dispatch_rule: manifest.dispatch_rule.to_string(),
        manifest_hash: manifest.manifest_hash,
        classes: manifest
            .classes
            .into_iter()
            .map(|class| NativeMathClassInfo {
                command: class.command.to_string(),
                required_slots: class
                    .required_slots
                    .into_iter()
                    .map(str::to_string)
                    .collect(),
                optional_slots: class
                    .optional_slots
                    .into_iter()
                    .map(str::to_string)
                    .collect(),
                slot_specs: class
                    .slot_specs
                    .into_iter()
                    .map(|slot| NativeMathSlotSpec {
                        name: slot.name.to_string(),
                        required: slot.required,
                        value_kind: slot.value_kind.to_string(),
                        accepted_content: slot.accepted_content.to_string(),
                        forge_binding: slot.forge_binding.to_string(),
                        validation_rule: slot.validation_rule.to_string(),
                    })
                    .collect(),
                accepted_operators: class
                    .accepted_operators
                    .into_iter()
                    .map(str::to_string)
                    .collect(),
                classical_aliases: class
                    .classical_aliases
                    .into_iter()
                    .map(str::to_string)
                    .collect(),
                forge_targets: class
                    .forge_targets
                    .into_iter()
                    .map(str::to_string)
                    .collect(),
                rejection_reasons: class
                    .rejection_reasons
                    .into_iter()
                    .map(str::to_string)
                    .collect(),
                deterministic_translation: class.deterministic_translation.to_string(),
                compile_status: class.compile_status.to_string(),
            })
            .collect(),
    })
}

pub fn run_native_numeric_rocket_contract(
    store_path: &Path,
) -> Result<NativeMathContractRun, String> {
    let monster = open_monster(store_path)?;
    let manifest_hash = monster.math_capability_manifest().manifest_hash;
    let contract = rocket_thrust_contract();
    let (compiled, prepared, execution) = monster
        .execute_math_contract(&contract)
        .map_err(|error| format!("math contract execution failed: {error:?}"))?;
    let oracle = prepared
        .route
        .plan
        .scalar_oracle_outputs
        .first()
        .ok_or_else(|| "missing scalar oracle output".to_string())?;
    Ok(NativeMathContractRun {
        schema: "ingen.native_services.math_contract_run.v1".to_string(),
        class: compiled.class.label().to_string(),
        contract_hash: compiled.contract_hash,
        module_name: compiled.module_name,
        manifest_hash,
        execution_status: execution.status.to_string(),
        backend: execution.backend.to_string(),
        output_name: oracle.output_name.clone(),
        output_value: f64::from_bits(oracle.value_bits),
        proof_hash: execution.proof_hash,
    })
}

pub fn run_native_math_contract_for_llm(
    store_path: &Path,
    contract: &MonsterMathContract,
) -> Result<NativeNewComputeLlmResult, String> {
    let monster = open_monster(store_path)?;
    let result = monster
        .execute_math_contract_for_llm(contract)
        .map_err(|error| format!("math contract execution failed: {error:?}"))?;
    Ok(NativeNewComputeLlmResult {
        schema: result.schema.to_string(),
        class: result.class,
        module_name: result.module_name,
        contract_hash: result.contract_hash,
        manifest_hash: result.manifest_hash,
        projection_hash: result.projection_hash,
        route_lane: result.route_lane,
        execution_status: result.execution_status,
        backend: result.backend,
        kernel_family: result.kernel_family,
        detected_gpu_count: result.detected_gpu_count,
        used_gpu_count: result.used_gpu_count,
        gpu_required: result.gpu_required,
        lanes_executed: result.lanes_executed,
        dispatch_shape: result.dispatch_shape,
        workgroup_size: result.workgroup_size,
        input_buffer_bytes: result.input_buffer_bytes,
        output_buffer_bytes: result.output_buffer_bytes,
        readback_bytes: result.readback_bytes,
        output_hash: result.output_hash,
        proof_hash: result.proof_hash,
        scalar_outputs: result
            .scalar_outputs
            .into_iter()
            .map(|output| NativeNewComputeLlmScalarOutput {
                sample_name: output.sample_name,
                output_name: output.output_name,
                status: output.status,
                value_bits: output.value_bits,
                value_text: output.value_text,
                expected_bits: output.expected_bits,
                tolerance_bits: output.tolerance_bits,
                abs_error_bits: output.abs_error_bits,
            })
            .collect(),
        typed_buffers: result
            .typed_buffers
            .into_iter()
            .map(|buffer| NativeNewComputeLlmTypedBuffer {
                name: buffer.name,
                forge_type: buffer.forge_type,
                layout: buffer.layout,
                element_type: buffer.element_type,
                shape: buffer.shape,
                byte_len: buffer.byte_len,
                page_count: buffer.page_count,
                buffer_hash: buffer.buffer_hash,
            })
            .collect(),
        scientific_metrics: result
            .scientific_metrics
            .into_iter()
            .map(|metric| NativeNewComputeLlmScientificMetric {
                name: metric.name,
                value_text: metric.value_text,
                unit: metric.unit,
                status: metric.status,
            })
            .collect(),
        limitations: result.limitations,
        compact_text: result.compact_text,
    })
}

fn open_monster(store_path: &Path) -> Result<MonsterNode, String> {
    let store = Store::open(store_path).map_err(|error| format!("open store: {error}"))?;
    Ok(MonsterNode::new(store, MemoryGovernor::new(8 * 1024 * 1024)))
}

fn rocket_thrust_contract() -> MonsterMathContract {
    let mut contract = MonsterMathContract::new(
        MonsterMathClass::NumericModel,
        "Compute rocket chamber thrust force from mass flow and exhaust velocity",
    );
    contract
        .variables
        .push(MonsterMathVariable::f64("mass_flow", "kg/s", 0.1, 50.0, 12.0));
    contract.variables.push(MonsterMathVariable::f64(
        "exhaust_velocity",
        "m/s",
        100.0,
        5000.0,
        2650.0,
    ));
    contract
        .constants
        .push(MonsterMathConstant::f64("pressure_delta", "N", 1200.0));
    contract.operators = vec!["*".to_string(), "+".to_string(), "finite".to_string()];
    contract
        .equations
        .push("thrust = mass_flow * exhaust_velocity + pressure_delta".to_string());
    contract.constraints.push("thrust >= 0".to_string());
    contract.samples.push(MonsterMathSample::new(
        "rocket_nozzle_nominal",
        11,
        vec![("mass_flow", 12.0), ("exhaust_velocity", 2650.0)],
        "thrust",
        33000.0,
        0.01,
    ));
    contract.validation.push("native_service_replay".to_string());
    contract.outputs.push(MonsterMathOutputContract::scalar(
        "thrust",
        "N",
        "mass_flow * exhaust_velocity + pressure_delta",
    ));
    contract
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_math_manifest_projects_monster_classes() {
        let path = scan::fresh_tmp_path("native-math", "manifest");
        let manifest = native_math_manifest(&path).unwrap();

        assert_eq!(manifest.entry_command, "/newcompute_");
        assert_eq!(manifest.classes.len(), 8);
        assert!(manifest.manifest_hash.len() == 64);
        assert!(manifest
            .classes
            .iter()
            .any(|class| class.command == "/numeric_model"
                && class.required_slots.iter().any(|slot| slot == "equations")
                && class
                    .slot_specs
                    .iter()
                    .any(|slot| slot.name == "equations"
                        && slot.value_kind == "classical_math_expression_list"
                        && slot.forge_binding == "forge_program")
                && class
                    .classical_aliases
                    .iter()
                    .any(|alias| alias == "Power->pow")));
        assert!(manifest.classes.iter().all(|class| {
            class.required_slots.len() + class.optional_slots.len() == class.slot_specs.len()
        }));
        assert!(manifest
            .classes
            .iter()
            .any(|class| class.command == "/signal_timeseries"
                && class
                    .classical_aliases
                    .iter()
                    .any(|alias| alias == "RFFT->rfft")));

        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn native_numeric_contract_runs_through_monster_core() {
        let path = scan::fresh_tmp_path("native-math", "rocket");
        let run = run_native_numeric_rocket_contract(&path).unwrap();

        assert_eq!(run.schema, "ingen.native_services.math_contract_run.v1");
        assert_eq!(run.class, "numeric_model");
        assert_eq!(run.output_name, "thrust");
        assert_eq!(run.output_value, 33000.0);
        assert!(matches!(
            run.execution_status.as_str(),
            "cpu_production_algorithm_micro_or_mini" | "gpu_executed_multi_adapter"
        ));
        assert!(run.proof_hash.len() == 64);

        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn native_math_contract_projects_llm_newcompute_result() {
        let path = scan::fresh_tmp_path("native-math", "llm-projection");
        let contract = rocket_thrust_contract();
        let run = run_native_math_contract_for_llm(&path, &contract).unwrap();

        assert_eq!(run.schema, "forge.monster.newcompute_llm_result.v1");
        assert_eq!(run.class, "numeric_model");
        assert_eq!(run.scalar_outputs.len(), 1);
        assert_eq!(run.scalar_outputs[0].output_name, "thrust");
        assert_eq!(run.scalar_outputs[0].value_text, "33000.000000000000");
        assert!(!run.typed_buffers.is_empty());
        assert_eq!(run.proof_hash.len(), 64);
        assert_eq!(run.projection_hash.len(), 64);
        assert!(run.compact_text.contains("NEWCOMPUTE_RESULT"));
        assert!(run.compact_text.contains("typed_buffers="));

        let _ = std::fs::remove_dir_all(path);
    }
}
