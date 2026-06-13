use scan::{
    fresh_tmp_path, generate_rust_port_adapter, materialize_generated_rust_module,
    monster_code_template_manifest, MonsterRustPortAdapterContract,
};

struct RustPortStressCase<'a> {
    module_name: &'a str,
    domain_object: &'a str,
    request_fields: &'a str,
    response_fields: &'a str,
    backend_trait: &'a str,
    backend_method: &'a str,
    validation_rules: &'a str,
    test_success: &'a str,
    test_failure: &'a str,
    metrics: &'a str,
    connectors: &'a str,
    runtime: &'a str,
    observability: &'a str,
    dependency_policy: &'a str,
    concurrency: &'a str,
}

#[test]
fn newmodule_rust_port_adapter_real_pipeline_materializes_and_tests_project() {
    // Simulated Brain/CodeAct read: the LLM sees /newmodule_ and chooses the Rust template.
    let manifest = monster_code_template_manifest();
    assert_eq!(manifest.entry_command, "/newmodule_");
    let template = manifest
        .templates
        .iter()
        .find(|template| template.command == "/rust_port_adapter_")
        .expect("Brain template registry must expose /rust_port_adapter_");
    assert!(template.required_slots.contains(&"module_name"));
    assert!(template.required_slots.contains(&"backend_method"));
    assert!(template.required_slots.contains(&"test_failure"));
    assert!(template.optional_slots.contains(&"metrics"));
    assert!(template.optional_slots.contains(&"connectors"));
    assert!(template.optional_slots.contains(&"runtime"));
    assert!(template.optional_slots.contains(&"quality_gates"));
    assert!(template.optional_slots.contains(&"dependency_policy"));

    // Simulated user request: "he, il nous faut un outil de profil utilisateur dans projet Y".
    // The LLM fills only compact slots; it never writes the generated Rust by hand.
    let contract = MonsterRustPortAdapterContract::from_slots_pro(
        "user_profile",
        "UserProfile",
        "user_id:String, quota:u64",
        "user_id:String, display_name:String, plan:String",
        "UserProfileStore",
        "load_profile",
        "user_id non_empty, quota min 1, quota max 1000",
        "enum",
        true,
        false,
        "user_id u_1 quota 20 returns display_name Alice plan pro",
        "empty user_id returns Validation",
        "request_count:u64, validation_error_count:u64, backend_error_count:u64, latency_ms:f64",
        "state_store",
        "sync",
        "manual_enum",
        "tracing_metadata",
        "common_crates",
        "send_sync",
        "fmt,clippy,test,doc,no_unsafe,no_panic",
        "1.1.0",
        "rustdoc_full",
    )
    .unwrap();

    let generated = generate_rust_port_adapter(&contract).unwrap();
    assert_eq!(generated.files[0].path, "src/user_profile.rs");
    assert_eq!(generated.connectors, vec!["state_store".to_string()]);
    assert!(generated.cargo_dependencies.iter().any(|dep| dep.contains("tracing")));
    assert!(generated.cargo_features.iter().any(|feature| feature.contains("tracing")));
    assert!(generated.quality_gates.contains(&"clippy".to_string()));
    assert!(generated.estimated_lines >= 220);

    let project_root = fresh_tmp_path("newmodule-real-project", "project-y");
    std::fs::create_dir_all(project_root.join("src")).unwrap();
    std::fs::write(
        project_root.join("Cargo.toml"),
        r#"[package]
name = "project_y_tools"
version = "0.1.0"
edition = "2021"

[workspace]

[features]
serde = []
"#,
    )
    .unwrap();

    let receipt = materialize_generated_rust_module(&project_root, &generated).unwrap();
    assert_eq!(receipt.files.len(), 1);
    assert_eq!(receipt.files[0].path, "src/user_profile.rs");
    assert_eq!(receipt.files[0].status, "created");
    assert_eq!(receipt.integration_snippet, "pub mod user_profile;");
    assert_eq!(receipt.connectors, vec!["state_store".to_string()]);
    assert_eq!(
        receipt.public_api,
        vec![
            "run_user_profile".to_string(),
            "run_user_profile_with_ref".to_string(),
            "FORGE_TEMPLATE_COMMAND".to_string(),
            "FORGE_TEMPLATE_VERSION".to_string(),
            "FORGE_QUALITY_GATES".to_string()
        ]
    );
    assert!(receipt.cargo_dependencies.iter().any(|dep| dep.contains("tracing")));
    assert!(receipt.cargo_features.iter().any(|feature| feature.contains("tracing")));
    assert!(receipt.quality_gates.contains(&"no_unsafe".to_string()));
    assert_eq!(receipt.module_hash.len(), 64);
    assert_eq!(receipt.files[0].content_hash.len(), 64);

    // Simulated integration step: the agent applies the compact integration snippet only.
    std::fs::write(project_root.join("src").join("lib.rs"), &receipt.integration_snippet).unwrap();

    let generated_source = std::fs::read_to_string(project_root.join("src").join("user_profile.rs")).unwrap();
    assert!(generated_source.contains("pub trait UserProfileStore"));
    assert!(generated_source.contains("#![forbid(unsafe_code)]"));
    assert!(generated_source.contains("FORGE_TEMPLATE_VERSION"));
    assert!(generated_source.contains("TRACING_TARGET"));
    assert!(generated_source.contains("pub fn run_user_profile"));
    assert!(generated_source.contains("pub fn run_user_profile_with_ref"));
    assert!(generated_source.contains("pub fn backend(&self) -> &B"));
    assert!(generated_source.contains("pub fn into_backend(self) -> B"));
    assert!(generated_source.contains("pub struct MetricsSnapshot"));
    assert!(generated_source.contains("pub fn metrics_snapshot"));
    assert!(generated_source.contains("pub mod connectors"));
    assert!(generated_source.contains("STATE_STORE_PORT"));
    assert!(generated_source.contains("NEXT_TEMPLATE"));
    assert!(generated_source.contains("assert_eq!(request.user_id, \"u_1\".to_string())"));
    assert!(generated_source.contains("assert_eq!(request.quota, 20)"));
    assert!(generated_source.contains("assert_eq!(response.display_name, \"Alice\".to_string())"));
    assert!(generated_source.contains("assert_eq!(response.plan, \"pro\".to_string())"));

    let cargo_test = std::process::Command::new("cargo")
        .arg("test")
        .arg("--manifest-path")
        .arg(project_root.join("Cargo.toml"))
        .output()
        .unwrap();
    assert!(
        cargo_test.status.success(),
        "materialized project must pass cargo test\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&cargo_test.stdout),
        String::from_utf8_lossy(&cargo_test.stderr)
    );

    let unchanged = materialize_generated_rust_module(&project_root, &generated).unwrap();
    assert_eq!(unchanged.files[0].status, "unchanged");

    let _ = std::fs::remove_dir_all(project_root);
}

#[test]
fn newmodule_rust_port_adapter_stress_battery_materializes_many_professional_modules() {
    let cases = [
        RustPortStressCase {
            module_name: "risk_pricing",
            domain_object: "RiskPricing",
            request_fields: "instrument_id:String, notional:u64, volatility:f64, tenor_days:u64, stressed:bool",
            response_fields: "instrument_id:String, fair_value:f64, var_95:f64, approved:bool",
            backend_trait: "RiskPricingEngine",
            backend_method: "price_risk",
            validation_rules: "instrument_id non_empty, notional min 1, notional max 1000000000, volatility min 0.0, volatility max 5.0, tenor_days min 1, tenor_days max 36500",
            test_success: "instrument_id bond_7 notional 2500000 volatility 0.21 tenor_days 730 stressed true returns fair_value 101.75 var_95 12.5 approved true",
            test_failure: "empty instrument_id returns Validation",
            metrics: "request_count:u64, validation_error_count:u64, backend_error_count:u64, latency_ms:f64, cache_hit_count:u64",
            connectors: "state_store",
            runtime: "sync",
            observability: "metrics_metadata",
            dependency_policy: "common_crates",
            concurrency: "send_sync",
        },
        RustPortStressCase {
            module_name: "bio_variant",
            domain_object: "BioVariant",
            request_fields: "sample_id:String, gene:String, read_depth:u64, allele_fraction:f64, tumor_only:bool",
            response_fields: "sample_id:String, pathogenic:bool, confidence:f64, annotation:String",
            backend_trait: "VariantAnnotator",
            backend_method: "annotate_variant",
            validation_rules: "sample_id non_empty, gene non_empty, read_depth min 10, read_depth max 1000000, allele_fraction min 0.0, allele_fraction max 1.0",
            test_success: "sample_id s42 gene TP53 read_depth 880 allele_fraction 0.37 tumor_only false returns pathogenic true confidence 0.97 annotation likely_pathogenic",
            test_failure: "empty sample_id returns Validation",
            metrics: "request_count:u64, validation_error_count:u64, backend_error_count:u64, qc_warning_count:u64",
            connectors: "state_store",
            runtime: "sync",
            observability: "tracing_metadata",
            dependency_policy: "common_crates",
            concurrency: "send_sync",
        },
        RustPortStressCase {
            module_name: "aero_loads",
            domain_object: "AeroLoads",
            request_fields: "case_id:String, mach:f64, altitude_m:u64, angle_deg:f64, wing_area:f64, dynamic_pressure:f64",
            response_fields: "case_id:String, lift:f64, drag:f64, margin:f64",
            backend_trait: "AeroLoadSolver",
            backend_method: "solve_loads",
            validation_rules: "case_id non_empty, mach min 0.0, mach max 8.0, altitude_m min 0, altitude_m max 120000, angle_deg min -45.0, angle_deg max 45.0, wing_area min 1.0, wing_area max 10000.0, dynamic_pressure min 0.0, dynamic_pressure max 2000000.0",
            test_success: "case_id cruise_9 mach 0.82 altitude_m 11000 angle_deg 4.5 wing_area 42.0 dynamic_pressure 18000.0 returns lift 755000.0 drag 18200.0 margin 1.21",
            test_failure: "empty case_id returns Validation",
            metrics: "request_count:u64, validation_error_count:u64, backend_error_count:u64, solver_iteration_count:u64, latency_ms:f64",
            connectors: "state_store",
            runtime: "async_tower",
            observability: "local_snapshot",
            dependency_policy: "common_crates",
            concurrency: "send_sync",
        },
        RustPortStressCase {
            module_name: "chem_reactor",
            domain_object: "ChemReactor",
            request_fields: "batch_id:String, temperature_k:f64, pressure_pa:u64, catalyst_loading:f64, residence_time_s:u64",
            response_fields: "batch_id:String, conversion:f64, selectivity:f64, safe:bool",
            backend_trait: "ReactorModel",
            backend_method: "simulate_reactor",
            validation_rules: "batch_id non_empty, temperature_k min 250.0, temperature_k max 1800.0, pressure_pa min 1, pressure_pa max 100000000, catalyst_loading min 0.0, catalyst_loading max 1.0, residence_time_s min 1, residence_time_s max 100000",
            test_success: "batch_id r7 temperature_k 720.0 pressure_pa 3500000 catalyst_loading 0.18 residence_time_s 90 returns conversion 0.84 selectivity 0.91 safe true",
            test_failure: "empty batch_id returns Validation",
            metrics: "request_count:u64, validation_error_count:u64, backend_error_count:u64, safety_trip_count:u64",
            connectors: "state_store",
            runtime: "sync",
            observability: "local_snapshot",
            dependency_policy: "std_only",
            concurrency: "local",
        },
    ];

    let project_root = fresh_tmp_path("newmodule-stress-project", "professional-battery");
    std::fs::create_dir_all(project_root.join("src")).unwrap();
    std::fs::write(
        project_root.join("Cargo.toml"),
        r#"[package]
name = "professional_battery"
version = "0.1.0"
edition = "2021"

[workspace]

[features]
serde = []
tower = []
tracing = []
metrics = []
thiserror = []
"#,
    )
    .unwrap();

    let mut lib_rs = String::new();
    for case in cases {
        let contract = MonsterRustPortAdapterContract::from_slots_pro(
            case.module_name,
            case.domain_object,
            case.request_fields,
            case.response_fields,
            case.backend_trait,
            case.backend_method,
            case.validation_rules,
            "enum",
            true,
            case.runtime == "async_tower",
            case.test_success,
            case.test_failure,
            case.metrics,
            case.connectors,
            case.runtime,
            "manual_enum",
            case.observability,
            case.dependency_policy,
            case.concurrency,
            "fmt,clippy,test,doc,no_unsafe,no_panic,property_tests",
            "2.0.0-stress",
            "rustdoc_full",
        )
        .unwrap();
        let generated = generate_rust_port_adapter(&contract).unwrap();
        assert!(generated.estimated_lines >= 220);
        assert!(generated.quality_gates.contains(&"property_tests".to_string()));
        if case.dependency_policy == "common_crates" {
            assert!(!generated.cargo_features.is_empty());
        }
        if case.runtime == "async_tower" {
            assert!(generated.files[0].content.contains("tower_service::Service"));
            assert!(generated
                .cargo_dependencies
                .iter()
                .any(|dep| dep.contains("tower-service")));
        }
        if case.observability == "tracing_metadata" {
            assert!(generated.files[0].content.contains("TRACING_TARGET"));
        }
        if case.observability == "metrics_metadata" {
            assert!(generated.files[0].content.contains("METRICS_NAMESPACE"));
        }
        assert!(generated.files[0].content.contains("#![forbid(unsafe_code)]"));
        assert!(generated.files[0].content.contains("FORGE_QUALITY_GATES"));
        assert!(generated.files[0].content.contains("Validation"));
        assert!(generated.files[0].content.contains("reason: \"min\""));
        assert!(generated.files[0].content.contains("reason: \"max\""));

        let receipt = materialize_generated_rust_module(&project_root, &generated).unwrap();
        assert_eq!(receipt.files[0].status, "created");
        assert_eq!(receipt.module_hash.len(), 64);
        assert!(receipt.quality_gates.contains(&"no_panic".to_string()));
        lib_rs.push_str(&receipt.integration_snippet);
        lib_rs.push('\n');
    }

    std::fs::write(project_root.join("src").join("lib.rs"), lib_rs).unwrap();
    let cargo_test = std::process::Command::new("cargo")
        .arg("test")
        .arg("--manifest-path")
        .arg(project_root.join("Cargo.toml"))
        .output()
        .unwrap();
    assert!(
        cargo_test.status.success(),
        "stress materialized project must pass cargo test\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&cargo_test.stdout),
        String::from_utf8_lossy(&cargo_test.stderr)
    );

    let _ = std::fs::remove_dir_all(project_root);
}

#[test]
fn newmodule_rust_port_adapter_rejects_ambiguous_or_unsupported_slots() {
    let bad_runtime = MonsterRustPortAdapterContract::from_slots_pro(
        "bad_runtime",
        "BadRuntime",
        "id:String",
        "id:String",
        "BadBackend",
        "load",
        "id non_empty",
        "enum",
        false,
        false,
        "id x returns id x",
        "empty id returns Validation",
        "",
        "",
        "async_magic",
        "manual_enum",
        "local_snapshot",
        "std_only",
        "local",
        "fmt,test",
        "1",
        "rustdoc_full",
    )
    .unwrap();
    assert!(generate_rust_port_adapter(&bad_runtime).is_err());

    let bad_validation = MonsterRustPortAdapterContract::from_slots_pro(
        "bad_validation",
        "BadValidation",
        "id:String",
        "id:String",
        "BadBackend",
        "load",
        "id min 1",
        "enum",
        false,
        false,
        "id x returns id x",
        "empty id returns Validation",
        "",
        "",
        "sync",
        "manual_enum",
        "local_snapshot",
        "std_only",
        "local",
        "fmt,test",
        "1",
        "rustdoc_full",
    )
    .unwrap();
    assert!(generate_rust_port_adapter(&bad_validation).is_err());

    let bad_connector = MonsterRustPortAdapterContract::from_slots_pro(
        "bad_connector",
        "BadConnector",
        "id:String",
        "id:String",
        "BadBackend",
        "load",
        "id non_empty",
        "enum",
        false,
        false,
        "id x returns id x",
        "empty id returns Validation",
        "",
        "database_magic",
        "sync",
        "manual_enum",
        "local_snapshot",
        "std_only",
        "local",
        "fmt,test",
        "1",
        "rustdoc_full",
    );
    assert!(bad_connector.is_err());
}
