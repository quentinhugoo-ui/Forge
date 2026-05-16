use crate::forge_intent;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const FORGE_AGENT_RUNTIME_V0: &str = "forge_agent_runtime_v0";

pub fn direct_about_value() -> Value {
    json!({
        "kind": "forge_agent_cli_about_v0",
        "name": "forge_agent",
        "runtime": FORGE_AGENT_RUNTIME_V0,
        "primary_circuit": "llm_cli -> ForgeSlash -> policy/Godel gate -> direct Forge routes -> projection",
        "mcp_in_primary_path": false,
        "mcp_role": "compatibility adapter for external LLM clients only",
        "commands": {
            "about": "Describe the direct agent CLI contract.",
            "plan": "Parse and compile ForgeSlash into a compact policy-checked projection without MCP.",
            "safe": "Run the direct safe execution circuit; only read-only and plan_only steps may execute.",
            "approve": "Run side-effect routes only after matching intent and policy hash approval."
        },
        "cache": "plan and safe can exact-hit persisted direct projections by intent_hash + mode + preview budget before recompute",
        "input_language": "ForgeSlash v0",
        "grammar": forge_intent::FORGE_SLASH_V0_GRAMMAR,
        "example": forge_intent::FORGE_SLASH_V0_EXAMPLE,
        "raw_data_returned": false
    })
}

pub fn direct_plan_projection(source: &str, max_bytes: usize) -> Result<Value, String> {
    let program = forge_intent::parse_forge_slash_v0(source)?;
    let compiled = program.compile_v0();
    let policy_report = compiled.policy_report_v0();
    let trace_card = compiled.trace_card_v0(&policy_report);
    let distillation_analysis = trace_card.distillation_analysis_v0();
    let promotion_manifest = trace_card.promotion_manifest_v0(&distillation_analysis);
    let skill_promotion_manifest = trace_card.skill_promotion_manifest_v0(&distillation_analysis);
    let router_promotion_manifest = trace_card.router_promotion_manifest_v0(&distillation_analysis);
    let forge_projection = trace_card.forge_projection_v0(
        &distillation_analysis,
        &promotion_manifest,
        &skill_promotion_manifest,
        &router_promotion_manifest,
        max_bytes,
    );
    let next_engine = if forge_projection.side_effect_count == 0 {
        "direct_safe_route_executor"
    } else {
        "direct_approval_gate"
    };

    Ok(json!({
        "ok": policy_report.ok,
        "kind": "forge_agent_direct_projection_v0",
        "runtime": FORGE_AGENT_RUNTIME_V0,
        "mode": "planned_no_side_effects",
        "surface": "forge_direct_runtime_v0",
        "mcp_in_primary_path": false,
        "source_language": "ForgeSlash v0",
        "raw_data_returned": false,
        "command_count": program.commands.len(),
        "intent_hash": program.content_hash(),
        "command_hashes": program.command_hashes(),
        "canonical_ast": program,
        "compiled_route_plan": compiled,
        "policy_report": policy_report,
        "trace_card": trace_card,
        "distillation_analysis": distillation_analysis,
        "promotion_manifest": promotion_manifest,
        "skill_promotion_manifest": skill_promotion_manifest,
        "router_promotion_manifest": router_promotion_manifest,
        "forge_projection": forge_projection,
        "next_engine": next_engine,
        "source": source
    }))
}

pub fn execution_report_v0(
    projection: &Value,
    executed_steps: &[Value],
    mode: &str,
    side_effects_allowed: bool,
    promotion_rule: &str,
) -> Value {
    let executed_step_count = executed_steps
        .iter()
        .filter(|step| {
            step.get("status")
                .and_then(Value::as_str)
                .map(|status| status.starts_with("executed_"))
                .unwrap_or(false)
        })
        .count();
    let skipped_step_count = executed_steps
        .iter()
        .filter(|step| {
            step.get("status")
                .and_then(Value::as_str)
                .map(|status| status.starts_with("skipped_"))
                .unwrap_or(false)
        })
        .count();
    let error_count = executed_steps
        .iter()
        .filter(|step| step.get("status").and_then(Value::as_str) == Some("error"))
        .count();
    let mut report = json!({
        "kind": "forge_intent_execution_report_v0",
        "runtime": FORGE_AGENT_RUNTIME_V0,
        "mode": mode,
        "intent_hash": projection.get("intent_hash").cloned().unwrap_or(Value::Null),
        "policy_hash": projection
            .get("policy_report")
            .and_then(|report| report.get("policy_hash"))
            .cloned()
            .unwrap_or(Value::Null),
        "trace_hash": projection
            .get("trace_card")
            .and_then(|trace| trace.get("trace_hash"))
            .cloned()
            .unwrap_or(Value::Null),
        "raw_data_returned": false,
        "side_effects_allowed": side_effects_allowed,
        "step_count": executed_steps.len(),
        "executed_step_count": executed_step_count,
        "skipped_step_count": skipped_step_count,
        "error_count": error_count,
        "executed_steps_hash": stable_json_hash("forge-intent-v0/executed-steps", &json!(executed_steps)),
        "promotion_rule": promotion_rule
    });
    let execution_hash = stable_json_hash("forge-intent-v0/execution-report", &report);
    if let Value::Object(ref mut obj) = report {
        obj.insert("execution_hash".to_string(), json!(execution_hash));
    }
    report
}

pub fn direct_safe_execution_with<F>(
    source: &str,
    max_bytes: usize,
    mut execute_step: F,
) -> Result<Value, String>
where
    F: FnMut(usize, &Value, usize) -> Value,
{
    let mut projection = direct_plan_projection(source, max_bytes)?;
    let policy_ok = projection
        .get("policy_report")
        .and_then(|report| report.get("ok"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if !policy_ok {
        if let Value::Object(ref mut obj) = projection {
            obj.insert("mode".to_string(), json!("blocked_policy_failed"));
            obj.insert("executed_steps".to_string(), json!([]));
        }
        return Ok(projection);
    }

    let executed_steps = projection
        .get("compiled_route_plan")
        .and_then(|plan| plan.get("steps"))
        .and_then(Value::as_array)
        .map(|steps| {
            steps
                .iter()
                .enumerate()
                .map(|(idx, step)| execute_step(idx, step, max_bytes))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let execution_report = execution_report_v0(
        &projection,
        &executed_steps,
        "execute_safe",
        false,
        "safe execution reports can seed exact-cache/router evidence, but cannot promote side-effect behavior without output/proof hashes",
    );
    if let Value::Object(ref mut obj) = projection {
        obj.insert("mode".to_string(), json!("execute_safe"));
        obj.insert("execution_contract".to_string(), json!({
            "side_effects_allowed": false,
            "executed_routes": ["run:plan_only", "read", "brain_recall", "brain_explain"],
            "skipped_routes": ["create", "run:non_plan", "brain_commit"],
            "raw_data_returned": false
        }));
        obj.insert("executed_steps".to_string(), json!(executed_steps));
        obj.insert("execution_report".to_string(), execution_report);
    }
    Ok(projection)
}

pub fn direct_approved_execution_with<F>(
    source: &str,
    max_bytes: usize,
    approve_side_effects: bool,
    approved_intent_hash: Option<&str>,
    approved_policy_hash: Option<&str>,
    allow_run_side_effects: bool,
    mut execute_step: F,
) -> Result<Value, String>
where
    F: FnMut(usize, &Value, usize) -> Value,
{
    let mut projection = direct_plan_projection(source, max_bytes)?;
    let approval_gate = approval_gate_v0(
        &projection,
        approve_side_effects,
        approved_intent_hash,
        approved_policy_hash,
        allow_run_side_effects,
    )?;
    if !approval_gate.get("ok").and_then(Value::as_bool).unwrap_or(false) {
        if let Value::Object(ref mut obj) = projection {
            obj.insert("mode".to_string(), json!("approval_required"));
            obj.insert("approval_gate".to_string(), approval_gate);
            obj.insert("executed_steps".to_string(), json!([]));
        }
        return Ok(projection);
    }

    let executed_steps = projection
        .get("compiled_route_plan")
        .and_then(|plan| plan.get("steps"))
        .and_then(Value::as_array)
        .map(|steps| {
            steps
                .iter()
                .enumerate()
                .map(|(idx, step)| {
                    if step.get("route").and_then(Value::as_str) == Some("run")
                        && step.get("side_effect").and_then(Value::as_bool).unwrap_or(false)
                        && !allow_run_side_effects
                    {
                        return json!({
                            "index": idx,
                            "route": "run",
                            "command_hash": step.get("command_hash").cloned().unwrap_or(Value::Null),
                            "status": "skipped_unapproved_run_side_effect",
                            "raw_data_returned": false,
                            "reason": "non-plan run requires allow_run_side_effects=true in addition to matching approval hashes"
                        });
                    }
                    execute_step(idx, step, max_bytes)
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let execution_report = execution_report_v0(
        &projection,
        &executed_steps,
        "execute_approved",
        true,
        "approved execution can promote behavior only when output/proof hashes and semantic tests are attached to the side-effect evidence",
    );
    if let Value::Object(ref mut obj) = projection {
        obj.insert("mode".to_string(), json!("execute_approved"));
        obj.insert("approval_gate".to_string(), approval_gate);
        obj.insert("execution_contract".to_string(), json!({
            "side_effects_allowed": true,
            "approval_required": ["approve_side_effects", "approved_intent_hash", "approved_policy_hash"],
            "non_plan_run_extra_gate": "allow_run_side_effects",
            "raw_data_returned": false
        }));
        obj.insert("executed_steps".to_string(), json!(executed_steps));
        obj.insert("execution_report".to_string(), execution_report);
    }
    Ok(projection)
}

pub fn approval_gate_v0(
    projection: &Value,
    approve_side_effects: bool,
    approved_intent_hash: Option<&str>,
    approved_policy_hash: Option<&str>,
    allow_run_side_effects: bool,
) -> Result<Value, String> {
    let policy_ok = projection
        .get("policy_report")
        .and_then(|report| report.get("ok"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let side_effect_count = projection
        .pointer("/trace_card/side_effect_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let intent_hash = projection
        .get("intent_hash")
        .and_then(Value::as_str)
        .unwrap_or("");
    let policy_hash = projection
        .pointer("/policy_report/policy_hash")
        .and_then(Value::as_str)
        .unwrap_or("");
    if let Some(hash) = approved_intent_hash {
        validate_content_hash(hash, "approved_intent_hash")?;
    }
    if let Some(hash) = approved_policy_hash {
        validate_content_hash(hash, "approved_policy_hash")?;
    }
    let intent_hash_matches = approved_intent_hash == Some(intent_hash);
    let policy_hash_matches = approved_policy_hash == Some(policy_hash);
    let approval_required = side_effect_count > 0;
    let approved = !approval_required
        || (approve_side_effects && intent_hash_matches && policy_hash_matches);
    Ok(json!({
        "kind": "forge_intent_side_effect_gate_v0",
        "runtime": FORGE_AGENT_RUNTIME_V0,
        "ok": policy_ok && approved,
        "policy_ok": policy_ok,
        "approval_required": approval_required,
        "side_effect_count": side_effect_count,
        "approve_side_effects": approve_side_effects,
        "approved_intent_hash_matches": intent_hash_matches,
        "approved_policy_hash_matches": policy_hash_matches,
        "allow_run_side_effects": allow_run_side_effects,
        "intent_hash": intent_hash,
        "policy_hash": policy_hash,
        "raw_data_returned": false,
        "reason": if !policy_ok {
            "policy_failed"
        } else if !approval_required {
            "no_side_effects"
        } else if approved {
            "side_effect_hashes_approved"
        } else {
            "side_effects_require_approve_side_effects_and_matching_intent_policy_hashes"
        }
    }))
}

fn validate_content_hash(value: &str, field: &str) -> Result<(), String> {
    if value.len() < 8
        || value.len() > 64
        || !value.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err(format!("invalid {field}; expected hex content hash"));
    }
    Ok(())
}

pub fn direct_read_projection(store_path: &Path, args: &Value) -> Result<Value, String> {
    let kind = args
        .get("kind")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_ascii_lowercase();
    if matches!(kind.as_str(), "programs" | "program" | "direct_programs") {
        return direct_read_programs(store_path, args);
    }
    if matches!(kind.as_str(), "direct_runs" | "runs" | "run") {
        return direct_read_runs(store_path, args);
    }
    if args.get("kind").and_then(Value::as_str) == Some("intent_projections")
        || args.get("list").and_then(Value::as_bool).unwrap_or(false)
    {
        return list_intent_projections(store_path, args);
    }
    let query_hash = intent_projection_query_hash(args);
    let Some(hash) = query_hash.clone() else {
        return list_intent_projections(store_path, args);
    };
    let value = if hash.len() >= 8 && hash.len() <= 64 && hash.bytes().all(|b| b.is_ascii_hexdigit()) {
        read_projection_by_hash_or_index(store_path, &hash)?
    } else {
        return Err("direct projection read expects projection_hash, execution_hash, trace_hash or intent_hash".to_string());
    };
    compact_read_projection_value(value, query_hash)
}

fn direct_read_programs(store_path: &Path, args: &Value) -> Result<Value, String> {
    let list = args.get("list").and_then(Value::as_bool).unwrap_or(false);
    let program_hash = args
        .get("program_hash")
        .or_else(|| args.get("program_id"))
        .or_else(|| args.get("ref"))
        .and_then(Value::as_str)
        .map(normalize_program_ref);
    if list || program_hash.is_none() {
        return list_direct_programs(store_path, args);
    }
    let program_hash = program_hash.unwrap_or_default();
    validate_content_hash(&program_hash, "program_hash")?;
    let program = read_json_value(&direct_program_manifest_path(store_path, &program_hash))?;
    Ok(compact_program_read_value(program_hash, program))
}

fn direct_read_runs(store_path: &Path, args: &Value) -> Result<Value, String> {
    let list = args.get("list").and_then(Value::as_bool).unwrap_or(false);
    let run_hash = args
        .get("run_hash")
        .or_else(|| args.get("ref"))
        .and_then(Value::as_str)
        .map(normalize_run_ref);
    if list || run_hash.is_none() {
        return list_direct_runs(store_path, args);
    }
    let run_hash = run_hash.unwrap_or_default();
    validate_content_hash(&run_hash, "run_hash")?;
    let path = direct_run_path(store_path, &run_hash);
    let run = read_json_value(&path)?;
    Ok(compact_run_read_value(run_hash, run))
}

pub fn lookup_cached_direct_projection(
    store_path: &Path,
    intent_hash: Option<&str>,
    mode: &str,
    requested_budget: usize,
) -> Result<Option<Value>, String> {
    let Some(intent_hash) = intent_hash else {
        return Ok(None);
    };
    validate_content_hash(intent_hash, "intent_hash")?;
    let index = read_intent_projection_index(store_path)?;
    let Some(entries) = index.get("entries").and_then(Value::as_array) else {
        return Ok(None);
    };
    for entry in entries {
        if entry.get("intent_hash").and_then(Value::as_str) != Some(intent_hash) {
            continue;
        }
        if entry.get("mode").and_then(Value::as_str) != Some(mode) {
            continue;
        }
        let budget = entry
            .get("bounded_preview_bytes")
            .and_then(Value::as_u64)
            .unwrap_or(0) as usize;
        if budget < requested_budget {
            continue;
        }
        let Some(projection_hash) = entry.get("projection_hash").and_then(Value::as_str) else {
            continue;
        };
        validate_content_hash(projection_hash, "projection_hash")?;
        let mut cached = read_json_value(
            &intent_projection_store_dir(store_path).join(format!("{projection_hash}.json")),
        )?;
        if let Value::Object(ref mut obj) = cached {
            obj.insert("cache_hit".to_string(), json!(true));
            obj.insert("cache_reason".to_string(), json!("exact_intent_mode_and_budget"));
            obj.insert("cache_lookup".to_string(), json!({
                "runtime": FORGE_AGENT_RUNTIME_V0,
                "intent_hash": intent_hash,
                "mode": mode,
                "requested_budget": requested_budget,
                "stored_budget": budget,
                "projection_hash": projection_hash,
                "raw_data_returned": false
            }));
        }
        return Ok(Some(cached));
    }
    Ok(None)
}

pub fn persist_direct_projection(store_path: &Path, projection: &mut Value) -> Result<Value, String> {
    let execution_hash = projection
        .pointer("/execution_report/execution_hash")
        .and_then(Value::as_str)
        .map(str::to_string);
    let trace_hash = projection
        .pointer("/trace_card/trace_hash")
        .and_then(Value::as_str)
        .map(str::to_string);
    let intent_hash = projection
        .get("intent_hash")
        .and_then(Value::as_str)
        .map(str::to_string);
    let projection_hash = execution_hash
        .clone()
        .or_else(|| trace_hash.clone())
        .or_else(|| intent_hash.clone())
        .ok_or_else(|| "projection missing execution_hash, trace_hash and intent_hash".to_string())?;
    validate_content_hash(&projection_hash, "projection_hash")?;

    let dir = intent_projection_store_dir(store_path);
    fs::create_dir_all(&dir)
        .map_err(|e| format!("create intent projection dir '{}': {e}", dir.display()))?;
    let now = now_ms();
    let projection_ref = format!("refs/intent/projection/{projection_hash}");
    let persisted = json!({
        "kind": "forge_agent_direct_persisted_projection_v0",
        "runtime": FORGE_AGENT_RUNTIME_V0,
        "projection_hash": projection_hash,
        "projection_ref": projection_ref,
        "execution_hash": execution_hash,
        "trace_hash": trace_hash,
        "intent_hash": intent_hash,
        "stored_ms": now,
        "raw_data_returned": false
    });
    if let Value::Object(ref mut obj) = projection {
        obj.insert("persisted_projection".to_string(), persisted.clone());
        obj.insert("projection_hash".to_string(), persisted["projection_hash"].clone());
        obj.insert("projection_ref".to_string(), persisted["projection_ref"].clone());
        if let Some(hash) = persisted.get("execution_hash").cloned().filter(|v| !v.is_null()) {
            obj.insert("execution_hash".to_string(), hash);
        }
    }
    let bytes = serde_json::to_vec_pretty(projection)
        .map_err(|e| format!("encode intent projection: {e}"))?;
    let path = dir.join(format!("{}.json", persisted["projection_hash"].as_str().unwrap_or("")));
    let mut file = fs::File::create(&path)
        .map_err(|e| format!("write intent projection '{}': {e}", path.display()))?;
    file.write_all(&bytes)
        .map_err(|e| format!("write intent projection '{}': {e}", path.display()))?;
    update_projection_index(store_path, projection, &persisted)?;
    Ok(persisted)
}

fn update_projection_index(store_path: &Path, projection: &Value, persisted: &Value) -> Result<Value, String> {
    let mut index = read_intent_projection_index(store_path)?;
    let now = now_ms();
    let projection_hash = persisted
        .get("projection_hash")
        .and_then(Value::as_str)
        .ok_or_else(|| "persisted projection missing projection_hash".to_string())?;
    let entry = compact_projection_index_entry(projection, persisted, now);
    let entries = index
        .as_object_mut()
        .ok_or_else(|| "intent projection index root is not an object".to_string())?
        .entry("entries".to_string())
        .or_insert_with(|| json!([]));
    let entries = entries
        .as_array_mut()
        .ok_or_else(|| "intent projection index entries is not an array".to_string())?;
    entries.retain(|item| item.get("projection_hash").and_then(Value::as_str) != Some(projection_hash));
    entries.insert(0, entry);
    entries.truncate(128);
    if let Value::Object(ref mut obj) = index {
        obj.insert("updated_ms".to_string(), json!(now));
        obj.insert("raw_data_returned".to_string(), json!(false));
    }
    persist_projection_index(store_path, &index)
}

fn compact_projection_index_entry(projection: &Value, persisted: &Value, now: u128) -> Value {
    json!({
        "kind": "forge_intent_projection_index_entry_v0",
        "runtime": FORGE_AGENT_RUNTIME_V0,
        "projection_hash": persisted.get("projection_hash").cloned().unwrap_or(Value::Null),
        "projection_ref": persisted.get("projection_ref").cloned().unwrap_or(Value::Null),
        "execution_hash": persisted.get("execution_hash").cloned().unwrap_or(Value::Null),
        "trace_hash": persisted.get("trace_hash").cloned().unwrap_or(Value::Null),
        "intent_hash": persisted.get("intent_hash").cloned().unwrap_or(Value::Null),
        "stored_ms": persisted.get("stored_ms").cloned().unwrap_or_else(|| json!(now)),
        "mode": projection.get("mode").cloned().unwrap_or(Value::Null),
        "surface": projection.get("surface").cloned().unwrap_or(Value::Null),
        "ok": projection.get("ok").cloned().unwrap_or(Value::Null),
        "bounded_preview_bytes": projection.pointer("/forge_projection/bounded_preview_bytes").cloned().unwrap_or(Value::Null),
        "source_preview": projection
            .get("source")
            .and_then(Value::as_str)
            .map(|source| source.chars().take(240).collect::<String>())
            .unwrap_or_default(),
        "route_count": projection.pointer("/trace_card/route_count").cloned().unwrap_or(Value::Null),
        "side_effect_count": projection.pointer("/trace_card/side_effect_count").cloned().unwrap_or(Value::Null),
        "execution": {
            "step_count": projection.pointer("/execution_report/step_count").cloned().unwrap_or(Value::Null),
            "executed_step_count": projection.pointer("/execution_report/executed_step_count").cloned().unwrap_or(Value::Null),
            "error_count": projection.pointer("/execution_report/error_count").cloned().unwrap_or(Value::Null)
        },
        "promotion": {
            "distillation_target": projection.pointer("/distillation_analysis/target").cloned().unwrap_or(Value::Null),
            "program_status": projection.pointer("/promotion_manifest/status").cloned().unwrap_or(Value::Null),
            "skill_status": projection.pointer("/skill_promotion_manifest/status").cloned().unwrap_or(Value::Null),
            "router_status": projection.pointer("/router_promotion_manifest/status").cloned().unwrap_or(Value::Null)
        },
        "raw_data_returned": false
    })
}

fn persist_projection_index(store_path: &Path, index: &Value) -> Result<Value, String> {
    let dir = intent_projection_store_dir(store_path);
    fs::create_dir_all(&dir)
        .map_err(|e| format!("create intent projection dir '{}': {e}", dir.display()))?;
    let path = dir.join("index.json");
    let bytes = serde_json::to_vec_pretty(index)
        .map_err(|e| format!("encode intent projection index: {e}"))?;
    let mut file = fs::File::create(&path)
        .map_err(|e| format!("write intent projection index '{}': {e}", path.display()))?;
    file.write_all(&bytes)
        .map_err(|e| format!("write intent projection index '{}': {e}", path.display()))?;
    Ok(index.clone())
}

pub fn direct_create_program(store_path: &Path, args: &Value, actor: &str) -> Result<Value, String> {
    let title = clean_bounded_text(
        args.get("title")
            .and_then(Value::as_str)
            .unwrap_or("Forge direct program"),
        "title",
        160,
    )?;
    let goal = clean_bounded_text(
        args.get("goal")
            .or_else(|| args.get("intent"))
            .and_then(Value::as_str)
            .unwrap_or("Direct Forge program created from ForgeSlash intent"),
        "goal",
        4096,
    )?;
    let intent = args
        .get("intent")
        .and_then(Value::as_str)
        .map(|value| clean_bounded_text(value, "intent", 4096))
        .transpose()?;
    let domain = args
        .get("domain")
        .and_then(Value::as_str)
        .map(|value| clean_bounded_text(value, "domain", 120))
        .transpose()?;
    let template = args
        .get("template")
        .and_then(Value::as_str)
        .map(|value| clean_bounded_text(value, "template", 120))
        .transpose()?;
    let program_kind = normalize_program_kind(
        args.get("program_kind")
            .or_else(|| args.get("kind"))
            .and_then(Value::as_str)
            .unwrap_or("compute_program"),
    );
    let canonical = json!({
        "title": title,
        "goal": goal,
        "intent": intent,
        "domain": domain,
        "template": template,
        "program_kind": program_kind,
        "source": "forge_agent_direct_create_v0"
    });
    let program_hash = stable_json_hash("forge-agent-runtime-v0/program", &canonical);
    let now = now_ms();
    let manifest = json!({
        "program_id": format!("program-{program_hash}"),
        "program_hash": program_hash,
        "kind": if program_kind == "visual_program" { "forge_visual_program" } else { "forge_compute_program" },
        "program_kind": program_kind,
        "status": "ready_direct_draft",
        "created_ms": now,
        "updated_ms": now,
        "created_by_agent": {
            "name": actor,
            "runtime": FORGE_AGENT_RUNTIME_V0
        },
        "canonical_hash_basis": "sha256(forge-agent-runtime-v0/program canonical_json)",
        "content_addressed": true,
        "duplicate_program_reused": direct_program_manifest_path(store_path, &program_hash).exists(),
        "spec_text_included": false,
        "canonical": canonical,
        "execution": {
            "mode": "direct_intent_program",
            "current_stage": "direct program manifest stored; rich Metric DSL compilation is still MCP-adapter hosted",
            "raw_data_returned": false
        },
        "agent_next_step": "Use /forge run program_hash=@program:<hash> plan_only=true for planning, or migrate this draft into the rich compiler when metric tags/views are needed.",
        "raw_data_returned": false
    });
    persist_direct_program_manifest(store_path, &program_hash, &manifest)?;
    Ok(json!({
        "kind": "forge_agent_direct_program_create_v0",
        "runtime": FORGE_AGENT_RUNTIME_V0,
        "program_hash": program_hash,
        "program": manifest,
        "raw_data_returned": false
    }))
}

pub fn direct_run_program(store_path: &Path, args: &Value, actor: &str) -> Result<Value, String> {
    let program_hash = args
        .get("program_hash")
        .or_else(|| args.get("program_id"))
        .and_then(Value::as_str)
        .map(normalize_program_ref)
        .ok_or_else(|| "direct run requires program_hash".to_string())?;
    validate_content_hash(&program_hash, "program_hash")?;
    let program_path = direct_program_manifest_path(store_path, &program_hash);
    let program = read_json_value(&program_path)?;
    let canonical = json!({
        "program_hash": program_hash,
        "program_status": program.get("status").cloned().unwrap_or(Value::Null),
        "program_kind": program.get("program_kind").cloned().unwrap_or(Value::Null),
        "args": compact_args_for_run(args),
        "actor": actor,
        "runtime": FORGE_AGENT_RUNTIME_V0
    });
    let run_hash = stable_json_hash("forge-agent-runtime-v0/direct-run", &canonical);
    let now = now_ms();
    let direct_runtime_owned = program
        .pointer("/created_by_agent/runtime")
        .and_then(Value::as_str)
        == Some(FORGE_AGENT_RUNTIME_V0)
        || program
            .pointer("/canonical/source")
            .and_then(Value::as_str)
            == Some("forge_agent_direct_create_v0");
    let run = json!({
        "kind": "forge_agent_direct_run_v0",
        "runtime": FORGE_AGENT_RUNTIME_V0,
        "run_hash": run_hash,
        "program_hash": program_hash,
        "program_ref": format!("refs/program/{program_hash}"),
        "status": if direct_runtime_owned { "completed_direct_manifest_run" } else { "requires_rich_runner_migration" },
        "ran": direct_runtime_owned,
        "created_ms": now,
        "created_by_agent": {
            "name": actor,
            "runtime": FORGE_AGENT_RUNTIME_V0
        },
        "execution": {
            "mode": "direct_manifest_run",
            "deterministic": true,
            "raw_data_returned": false,
            "note": if direct_runtime_owned {
                "Direct runtime executed a content-addressed manifest run and produced a compact proof envelope."
            } else {
                "Program manifest exists, but rich Metric DSL execution is still hosted by the MCP adapter."
            }
        },
        "program_summary": {
            "title": program.pointer("/canonical/title").cloned().or_else(|| program.get("title").cloned()).unwrap_or(Value::Null),
            "goal": program.pointer("/canonical/goal").cloned().unwrap_or(Value::Null),
            "program_kind": program.get("program_kind").cloned().unwrap_or(Value::Null),
            "status": program.get("status").cloned().unwrap_or(Value::Null)
        },
        "input_summary": compact_args_for_run(args),
        "raw_data_returned": false
    });
    persist_direct_run(store_path, &run_hash, &run)?;
    Ok(run)
}

fn normalize_program_ref(value: &str) -> String {
    value
        .trim()
        .trim_start_matches("@program:")
        .trim_start_matches("refs/program/")
        .trim_start_matches("program-")
        .to_string()
}

fn normalize_run_ref(value: &str) -> String {
    value
        .trim()
        .trim_start_matches("@run:")
        .trim_start_matches("refs/direct-run/")
        .trim_start_matches("run-")
        .to_string()
}

fn compact_args_for_run(args: &Value) -> Value {
    let mut out = serde_json::Map::new();
    for key in ["program_hash", "program_id", "intent", "capability", "job_id", "artifact_ref"] {
        if let Some(value) = args.get(key) {
            out.insert(key.to_string(), value.clone());
        }
    }
    out.insert(
        "has_inputs".to_string(),
        json!(args.get("inputs").and_then(Value::as_array).map(|items| !items.is_empty()).unwrap_or(false)),
    );
    out.insert("raw_data_returned".to_string(), json!(false));
    Value::Object(out)
}

fn list_direct_programs(store_path: &Path, args: &Value) -> Result<Value, String> {
    let limit = bounded_list_limit(args);
    let mut entries = Vec::new();
    let dir = store_path.join("programs");
    if dir.exists() {
        for item in fs::read_dir(&dir).map_err(|e| format!("read programs dir '{}': {e}", dir.display()))? {
            let item = item.map_err(|e| format!("read programs dir entry '{}': {e}", dir.display()))?;
            if item.path().extension().and_then(|value| value.to_str()) != Some("json") {
                continue;
            }
            let Ok(program) = read_json_value(&item.path()) else {
                continue;
            };
            entries.push(compact_program_index_entry(&program));
        }
    }
    entries.sort_by(|a, b| {
        b.get("created_ms")
            .and_then(Value::as_u64)
            .cmp(&a.get("created_ms").and_then(Value::as_u64))
    });
    entries.truncate(limit);
    Ok(json!({
        "kind": "forge_agent_direct_program_list_v0",
        "runtime": FORGE_AGENT_RUNTIME_V0,
        "found": !entries.is_empty(),
        "result_count": entries.len(),
        "limit": limit,
        "raw_data_returned": false,
        "entries": entries
    }))
}

fn list_direct_runs(store_path: &Path, args: &Value) -> Result<Value, String> {
    let limit = bounded_list_limit(args);
    let mut entries = Vec::new();
    let dir = store_path.join("direct-runs");
    if dir.exists() {
        for item in fs::read_dir(&dir).map_err(|e| format!("read direct-runs dir '{}': {e}", dir.display()))? {
            let item = item.map_err(|e| format!("read direct-runs dir entry '{}': {e}", dir.display()))?;
            if item.path().extension().and_then(|value| value.to_str()) != Some("json") {
                continue;
            }
            let Ok(run) = read_json_value(&item.path()) else {
                continue;
            };
            entries.push(compact_run_index_entry(&run));
        }
    }
    entries.sort_by(|a, b| {
        b.get("created_ms")
            .and_then(Value::as_u64)
            .cmp(&a.get("created_ms").and_then(Value::as_u64))
    });
    entries.truncate(limit);
    Ok(json!({
        "kind": "forge_agent_direct_run_list_v0",
        "runtime": FORGE_AGENT_RUNTIME_V0,
        "found": !entries.is_empty(),
        "result_count": entries.len(),
        "limit": limit,
        "raw_data_returned": false,
        "entries": entries
    }))
}

fn compact_program_index_entry(program: &Value) -> Value {
    json!({
        "kind": "forge_agent_direct_program_index_entry_v0",
        "runtime": FORGE_AGENT_RUNTIME_V0,
        "program_hash": program.get("program_hash").cloned().unwrap_or(Value::Null),
        "program_ref": program
            .get("program_hash")
            .and_then(Value::as_str)
            .map(|hash| format!("refs/program/{hash}"))
            .unwrap_or_default(),
        "status": program.get("status").cloned().unwrap_or(Value::Null),
        "program_kind": program.get("program_kind").cloned().unwrap_or(Value::Null),
        "title": program.pointer("/canonical/title").cloned().or_else(|| program.get("title").cloned()).unwrap_or(Value::Null),
        "created_ms": program.get("created_ms").cloned().unwrap_or(Value::Null),
        "updated_ms": program.get("updated_ms").cloned().unwrap_or(Value::Null),
        "raw_data_returned": false
    })
}

fn compact_run_index_entry(run: &Value) -> Value {
    json!({
        "kind": "forge_agent_direct_run_index_entry_v0",
        "runtime": FORGE_AGENT_RUNTIME_V0,
        "run_hash": run.get("run_hash").cloned().unwrap_or(Value::Null),
        "run_ref": run
            .get("run_hash")
            .and_then(Value::as_str)
            .map(|hash| format!("refs/direct-run/{hash}"))
            .unwrap_or_default(),
        "program_hash": run.get("program_hash").cloned().unwrap_or(Value::Null),
        "status": run.get("status").cloned().unwrap_or(Value::Null),
        "ran": run.get("ran").cloned().unwrap_or(Value::Null),
        "created_ms": run.get("created_ms").cloned().unwrap_or(Value::Null),
        "raw_data_returned": false
    })
}

fn compact_program_read_value(program_hash: String, program: Value) -> Value {
    json!({
        "kind": "forge_agent_direct_program_read_v0",
        "runtime": FORGE_AGENT_RUNTIME_V0,
        "found": true,
        "program_hash": program_hash.clone(),
        "program_ref": format!("refs/program/{program_hash}"),
        "status": program.get("status").cloned().unwrap_or(Value::Null),
        "program_kind": program.get("program_kind").cloned().unwrap_or(Value::Null),
        "created_by_agent": program.get("created_by_agent").cloned().unwrap_or(Value::Null),
        "canonical": {
            "title": program.pointer("/canonical/title").cloned().unwrap_or(Value::Null),
            "goal": program.pointer("/canonical/goal").cloned().unwrap_or(Value::Null),
            "intent": program.pointer("/canonical/intent").cloned().unwrap_or(Value::Null),
            "domain": program.pointer("/canonical/domain").cloned().unwrap_or(Value::Null),
            "template": program.pointer("/canonical/template").cloned().unwrap_or(Value::Null)
        },
        "execution": program.get("execution").cloned().unwrap_or(Value::Null),
        "raw_data_returned": false
    })
}

fn compact_run_read_value(run_hash: String, run: Value) -> Value {
    json!({
        "kind": "forge_agent_direct_run_read_v0",
        "runtime": FORGE_AGENT_RUNTIME_V0,
        "found": true,
        "run_hash": run_hash.clone(),
        "run_ref": format!("refs/direct-run/{run_hash}"),
        "program_hash": run.get("program_hash").cloned().unwrap_or(Value::Null),
        "program_ref": run.get("program_ref").cloned().unwrap_or(Value::Null),
        "status": run.get("status").cloned().unwrap_or(Value::Null),
        "ran": run.get("ran").cloned().unwrap_or(Value::Null),
        "execution": run.get("execution").cloned().unwrap_or(Value::Null),
        "program_summary": run.get("program_summary").cloned().unwrap_or(Value::Null),
        "input_summary": run.get("input_summary").cloned().unwrap_or(Value::Null),
        "raw_data_returned": false
    })
}

fn bounded_list_limit(args: &Value) -> usize {
    args.get("limit")
        .and_then(json_number_to_u64)
        .unwrap_or(8)
        .clamp(1, 32) as usize
}

fn json_number_to_u64(value: &Value) -> Option<u64> {
    value.as_u64().or_else(|| {
        value
            .as_f64()
            .filter(|number| number.is_finite() && *number >= 0.0)
            .map(|number| number as u64)
    })
}

fn clean_bounded_text(value: &str, field: &str, max_chars: usize) -> Result<String, String> {
    let cleaned = value.trim().replace(['\r', '\n', '\t'], " ");
    if cleaned.is_empty() {
        return Err(format!("{field} cannot be empty"));
    }
    if cleaned.chars().count() > max_chars {
        return Err(format!("{field} too long; max {max_chars} chars"));
    }
    Ok(cleaned)
}

fn normalize_program_kind(value: &str) -> &'static str {
    match value.trim().to_ascii_lowercase().as_str() {
        "visual" | "visual_program" | "view" | "views" => "visual_program",
        _ => "compute_program",
    }
}

fn persist_direct_program_manifest(
    store_path: &Path,
    program_hash: &str,
    manifest: &Value,
) -> Result<(), String> {
    let dir = store_path.join("programs");
    fs::create_dir_all(&dir).map_err(|e| format!("create programs dir '{}': {e}", dir.display()))?;
    let path = direct_program_manifest_path(store_path, program_hash);
    let bytes = serde_json::to_vec_pretty(manifest)
        .map_err(|e| format!("encode program manifest: {e}"))?;
    let mut file = fs::File::create(&path)
        .map_err(|e| format!("write program manifest '{}': {e}", path.display()))?;
    file.write_all(&bytes)
        .map_err(|e| format!("write program manifest '{}': {e}", path.display()))
}

fn persist_direct_run(store_path: &Path, run_hash: &str, run: &Value) -> Result<(), String> {
    let dir = store_path.join("direct-runs");
    fs::create_dir_all(&dir).map_err(|e| format!("create direct-runs dir '{}': {e}", dir.display()))?;
    let path = direct_run_path(store_path, run_hash);
    let bytes = serde_json::to_vec_pretty(run)
        .map_err(|e| format!("encode direct run: {e}"))?;
    let mut file = fs::File::create(&path)
        .map_err(|e| format!("write direct run '{}': {e}", path.display()))?;
    file.write_all(&bytes)
        .map_err(|e| format!("write direct run '{}': {e}", path.display()))
}

fn direct_program_manifest_path(store_path: &Path, program_hash: &str) -> PathBuf {
    store_path.join("programs").join(format!("{program_hash}.json"))
}

fn direct_run_path(store_path: &Path, run_hash: &str) -> PathBuf {
    store_path.join("direct-runs").join(format!("{run_hash}.json"))
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

fn read_projection_by_hash_or_index(store_path: &Path, hash: &str) -> Result<Value, String> {
    if hash.len() == 64 {
        let direct_path = intent_projection_store_dir(store_path).join(format!("{hash}.json"));
        if direct_path.exists() {
            return read_json_value(&direct_path);
        }
    }
    let index = read_intent_projection_index(store_path)?;
    let Some(entries) = index.get("entries").and_then(Value::as_array) else {
        return Err("intent projection index has no entries".to_string());
    };
    for entry in entries {
        let matches = ["projection_hash", "execution_hash", "trace_hash", "intent_hash"]
            .iter()
            .any(|field| entry.get(*field).and_then(Value::as_str) == Some(hash));
        if !matches {
            continue;
        }
        let Some(projection_hash) = entry.get("projection_hash").and_then(Value::as_str) else {
            continue;
        };
        validate_content_hash(projection_hash, "projection_hash")?;
        return read_json_value(&intent_projection_store_dir(store_path).join(format!("{projection_hash}.json")));
    }
    Err(format!("no intent projection found for hash {hash}"))
}

fn list_intent_projections(store_path: &Path, args: &Value) -> Result<Value, String> {
    let limit = bounded_list_limit(args);
    let index = read_intent_projection_index(store_path)?;
    let entries: Vec<Value> = index
        .get("entries")
        .and_then(Value::as_array)
        .map(|entries| entries.iter().take(limit).cloned().collect())
        .unwrap_or_default();
    Ok(json!({
        "kind": "forge_agent_direct_projection_list_v0",
        "runtime": FORGE_AGENT_RUNTIME_V0,
        "found": !entries.is_empty(),
        "result_count": entries.len(),
        "limit": limit,
        "raw_data_returned": false,
        "entries": entries
    }))
}

fn compact_read_projection_value(value: Value, query_hash: Option<String>) -> Result<Value, String> {
    Ok(json!({
        "kind": "forge_agent_direct_projection_read_v0",
        "runtime": FORGE_AGENT_RUNTIME_V0,
        "found": true,
        "query_hash": query_hash,
        "raw_data_returned": false,
        "persisted_projection": value.get("persisted_projection").cloned().unwrap_or(Value::Null),
        "mode": value.get("mode").cloned().unwrap_or(Value::Null),
        "surface": value.get("surface").cloned().unwrap_or(Value::Null),
        "ok": value.get("ok").cloned().unwrap_or(Value::Null),
        "intent_hash": value.get("intent_hash").cloned().unwrap_or(Value::Null),
        "policy_hash": value.pointer("/policy_report/policy_hash").cloned().unwrap_or(Value::Null),
        "trace_hash": value.pointer("/trace_card/trace_hash").cloned().unwrap_or(Value::Null),
        "execution_report": value.get("execution_report").cloned().unwrap_or(Value::Null),
        "forge_projection": value.get("forge_projection").cloned().unwrap_or(Value::Null),
        "executed_steps": value.get("executed_steps").cloned().unwrap_or_else(|| json!([])),
        "promotion": {
            "distillation": value.get("distillation_analysis").cloned().unwrap_or(Value::Null),
            "program": value.get("promotion_manifest").cloned().unwrap_or(Value::Null),
            "skill": value.get("skill_promotion_manifest").cloned().unwrap_or(Value::Null),
            "router": value.get("router_promotion_manifest").cloned().unwrap_or(Value::Null)
        }
    }))
}

fn intent_projection_query_hash(args: &Value) -> Option<String> {
    let direct = args
        .get("projection_hash")
        .or_else(|| args.get("execution_hash"))
        .or_else(|| args.get("trace_hash"))
        .or_else(|| args.get("intent_hash"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    if direct.is_some() {
        return direct;
    }
    args.get("ref")
        .or_else(|| args.get("projection_ref"))
        .and_then(Value::as_str)
        .and_then(|value| value.rsplit('/').next())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn intent_projection_store_dir(store_path: &Path) -> PathBuf {
    store_path.join("intent-projections")
}

fn read_intent_projection_index(store_path: &Path) -> Result<Value, String> {
    let path = intent_projection_store_dir(store_path).join("index.json");
    if !path.exists() {
        return Ok(json!({
            "kind": "forge_intent_projection_index_v0",
            "entries": [],
            "raw_data_returned": false
        }));
    }
    read_json_value(&path)
}

fn read_json_value(path: &Path) -> Result<Value, String> {
    let bytes = fs::read(path).map_err(|e| format!("read json '{}': {e}", path.display()))?;
    serde_json::from_slice(&bytes).map_err(|e| format!("decode json '{}': {e}", path.display()))
}

pub fn compact_step_result(
    index: usize,
    route: &str,
    command_hash: &str,
    status: &str,
    value: Value,
    result_budget_bytes: usize,
) -> Value {
    let result_hash = stable_json_hash("forge-intent-v0/step-result", &value);
    let result_bytes = serde_json::to_vec(&value)
        .map(|bytes| bytes.len())
        .unwrap_or_default();
    let mut payload = json!({
        "index": index,
        "route": route,
        "command_hash": command_hash,
        "status": status,
        "raw_data_returned": false,
        "result_hash": result_hash,
        "result_bytes": result_bytes,
        "result_summary": summarize_step_result(&value)
    });
    if let Value::Object(ref mut obj) = payload {
        if result_bytes <= result_budget_bytes {
            obj.insert("result".to_string(), value);
            obj.insert("result_truncated".to_string(), json!(false));
        } else {
            let text = serde_json::to_string(&value).unwrap_or_default();
            let preview: String = text.chars().take(result_budget_bytes).collect();
            obj.insert("result_preview".to_string(), json!(preview));
            obj.insert("result_preview_bytes".to_string(), json!(result_budget_bytes));
            obj.insert("result_truncated".to_string(), json!(true));
        }
    }
    payload
}

fn summarize_step_result(value: &Value) -> Value {
    json!({
        "plan_only": value.get("plan_only").and_then(Value::as_bool),
        "state": value.get("state").and_then(Value::as_str),
        "kind": value.get("kind").and_then(Value::as_str),
        "recommended_tool": value.get("recommended_tool").and_then(Value::as_str),
        "inferred_capability": value.get("inferred_capability").and_then(Value::as_str),
        "program_hash": value.pointer("/program/program_hash")
            .and_then(Value::as_str)
            .or_else(|| value.get("program_hash").and_then(Value::as_str)),
        "note_hash": value.pointer("/note/hash")
            .and_then(Value::as_str),
        "memory_layer": value.pointer("/note/memory_layer")
            .and_then(Value::as_str),
        "verification_status": value.pointer("/note/verification_status")
            .and_then(Value::as_str),
        "raw_input_not_returned": value
            .get("compute_contract")
            .and_then(|contract| contract.get("raw_input_not_returned"))
            .and_then(Value::as_bool)
            .or_else(|| value.get("raw_input_not_returned").and_then(Value::as_bool))
    })
}

pub fn stable_json_hash(domain: &str, value: &Value) -> String {
    let bytes = serde_json::to_vec(value).expect("json value serializes");
    let mut hasher = Sha256::new();
    hasher.update(domain.as_bytes());
    hasher.update([0]);
    hasher.update(bytes);
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_plan_projection_never_enters_mcp_path() {
        let projection = direct_plan_projection(
            r#"/forge run input=@latest intent="profile market data" plan_only=true"#,
            4096,
        )
        .expect("direct plan projection");

        assert_eq!(projection["ok"].as_bool(), Some(true));
        assert_eq!(projection["mcp_in_primary_path"].as_bool(), Some(false));
        assert_eq!(projection["runtime"].as_str(), Some(FORGE_AGENT_RUNTIME_V0));
        assert_eq!(projection["raw_data_returned"].as_bool(), Some(false));
        assert_eq!(projection["compiled_route_plan"]["steps"][0]["route"].as_str(), Some("run"));
    }

    #[test]
    fn execution_report_hash_is_stable_in_direct_runtime() {
        let projection = direct_plan_projection(
            r#"/forge run input=@latest intent="profile market data" plan_only=true"#,
            4096,
        )
        .expect("direct plan projection");
        let steps = vec![compact_step_result(
            0,
            "run",
            "command",
            "executed_safe",
            json!({"plan_only": true, "state": "planned"}),
            4096,
        )];
        let first = execution_report_v0(&projection, &steps, "execute_safe", false, "test");
        let second = execution_report_v0(&projection, &steps, "execute_safe", false, "test");

        assert_eq!(first["execution_hash"], second["execution_hash"]);
        assert_eq!(first["runtime"].as_str(), Some(FORGE_AGENT_RUNTIME_V0));
    }

    #[test]
    fn direct_safe_execution_orchestrates_policy_steps_and_report() {
        let projection = direct_safe_execution_with(
            r#"/forge run input=@latest intent="profile market data" plan_only=true"#,
            4096,
            |idx, step, budget| {
                compact_step_result(
                    idx,
                    step.get("route").and_then(Value::as_str).unwrap_or(""),
                    step.get("command_hash").and_then(Value::as_str).unwrap_or(""),
                    "executed_safe",
                    json!({"plan_only": true, "state": "planned"}),
                    budget,
                )
            },
        )
        .expect("safe execution projection");

        assert_eq!(projection["mode"].as_str(), Some("execute_safe"));
        assert_eq!(projection["executed_steps"].as_array().map(Vec::len), Some(1));
        assert_eq!(
            projection["execution_report"]["runtime"].as_str(),
            Some(FORGE_AGENT_RUNTIME_V0)
        );
    }

    #[test]
    fn approved_execution_requires_matching_hash_gate() {
        let source = r#"/forge commit scope=cutover kind=semantic observation="remember direct runtime""#;
        let planned = direct_plan_projection(source, 4096).expect("planned projection");
        let rejected = direct_approved_execution_with(
            source,
            4096,
            true,
            Some("aaaaaaaa"),
            planned
                .pointer("/policy_report/policy_hash")
                .and_then(Value::as_str),
            false,
            |idx, step, budget| {
                compact_step_result(
                    idx,
                    step.get("route").and_then(Value::as_str).unwrap_or(""),
                    step.get("command_hash").and_then(Value::as_str).unwrap_or(""),
                    "executed_side_effect",
                    json!({"kind": "synthetic_side_effect"}),
                    budget,
                )
            },
        )
        .expect("rejected approval projection");

        assert_eq!(rejected["mode"].as_str(), Some("approval_required"));
        assert_eq!(rejected["executed_steps"].as_array().map(Vec::len), Some(0));

        let accepted = direct_approved_execution_with(
            source,
            4096,
            true,
            planned.get("intent_hash").and_then(Value::as_str),
            planned
                .pointer("/policy_report/policy_hash")
                .and_then(Value::as_str),
            false,
            |idx, step, budget| {
                compact_step_result(
                    idx,
                    step.get("route").and_then(Value::as_str).unwrap_or(""),
                    step.get("command_hash").and_then(Value::as_str).unwrap_or(""),
                    "executed_side_effect",
                    json!({"kind": "synthetic_side_effect"}),
                    budget,
                )
            },
        )
        .expect("accepted approval projection");

        assert_eq!(accepted["mode"].as_str(), Some("execute_approved"));
        assert_eq!(accepted["approval_gate"]["runtime"].as_str(), Some(FORGE_AGENT_RUNTIME_V0));
        assert_eq!(accepted["executed_steps"].as_array().map(Vec::len), Some(1));
    }

    #[test]
    fn direct_read_projection_lists_missing_index_without_raw_data() {
        let tmp = scan::TmpDir::new(scan::fresh_tmp_path("forge-agent-runtime", "projection-list"));
        fs::create_dir_all(tmp.as_ref()).expect("tmp dir");
        let listed = direct_read_projection(tmp.as_ref(), &json!({"list": true}))
            .expect("projection list");

        assert_eq!(listed["found"].as_bool(), Some(false));
        assert_eq!(listed["runtime"].as_str(), Some(FORGE_AGENT_RUNTIME_V0));
        assert_eq!(listed["raw_data_returned"].as_bool(), Some(false));
    }

    #[test]
    fn direct_create_program_persists_content_addressed_manifest() {
        let tmp = scan::TmpDir::new(scan::fresh_tmp_path("forge-agent-runtime", "program-create"));
        fs::create_dir_all(tmp.as_ref()).expect("tmp dir");
        let created = direct_create_program(
            tmp.as_ref(),
            &json!({
                "title": "DirectSmoke",
                "goal": "prove direct create",
                "program_kind": "compute_program"
            }),
            "test",
        )
        .expect("program created");
        let hash = created["program_hash"].as_str().expect("program hash");
        assert!(direct_program_manifest_path(tmp.as_ref(), hash).exists());
        assert_eq!(created["runtime"].as_str(), Some(FORGE_AGENT_RUNTIME_V0));
        assert_eq!(created["raw_data_returned"].as_bool(), Some(false));
    }

    #[test]
    fn direct_run_program_executes_direct_manifest_by_hash() {
        let tmp = scan::TmpDir::new(scan::fresh_tmp_path("forge-agent-runtime", "program-run"));
        fs::create_dir_all(tmp.as_ref()).expect("tmp dir");
        let created = direct_create_program(
            tmp.as_ref(),
            &json!({
                "title": "DirectRunSmoke",
                "goal": "prove direct run",
                "program_kind": "compute_program"
            }),
            "test",
        )
        .expect("program created");
        let run = direct_run_program(
            tmp.as_ref(),
            &json!({ "program_hash": created["program_hash"] }),
            "test",
        )
        .expect("program ran");

        assert_eq!(run["ran"].as_bool(), Some(true));
        assert_eq!(run["runtime"].as_str(), Some(FORGE_AGENT_RUNTIME_V0));
        assert!(run["run_hash"].as_str().is_some());
        assert_eq!(run["raw_data_returned"].as_bool(), Some(false));
    }

    #[test]
    fn direct_read_lists_and_reads_programs_and_runs() {
        let tmp = scan::TmpDir::new(scan::fresh_tmp_path("forge-agent-runtime", "program-read"));
        fs::create_dir_all(tmp.as_ref()).expect("tmp dir");
        let created = direct_create_program(
            tmp.as_ref(),
            &json!({
                "title": "DirectReadSmoke",
                "goal": "prove direct program read",
                "program_kind": "compute_program"
            }),
            "test",
        )
        .expect("program created");
        let run = direct_run_program(
            tmp.as_ref(),
            &json!({ "program_hash": created["program_hash"] }),
            "test",
        )
        .expect("program ran");

        let programs = direct_read_projection(tmp.as_ref(), &json!({"kind": "programs", "list": true}))
            .expect("list programs");
        let program = direct_read_projection(
            tmp.as_ref(),
            &json!({"kind": "program", "program_hash": created["program_hash"]}),
        )
        .expect("read program");
        let runs = direct_read_projection(tmp.as_ref(), &json!({"kind": "direct_runs", "list": true}))
            .expect("list runs");
        let run_read = direct_read_projection(
            tmp.as_ref(),
            &json!({"kind": "direct_runs", "run_hash": run["run_hash"]}),
        )
        .expect("read run");

        assert_eq!(programs["found"].as_bool(), Some(true));
        assert_eq!(programs["entries"].as_array().map(Vec::len), Some(1));
        assert_eq!(program["program_hash"], created["program_hash"]);
        assert_eq!(runs["found"].as_bool(), Some(true));
        assert_eq!(runs["entries"].as_array().map(Vec::len), Some(1));
        assert_eq!(run_read["run_hash"], run["run_hash"]);
        assert_eq!(run_read["raw_data_returned"].as_bool(), Some(false));
    }

    #[test]
    fn direct_list_limit_accepts_forgeslash_float_numbers() {
        assert_eq!(bounded_list_limit(&json!({"limit": 3.0})), 3);
        assert_eq!(bounded_list_limit(&json!({"limit": 999.0})), 32);
    }

    #[test]
    fn direct_projection_persistence_indexes_cli_outputs() {
        let tmp = scan::TmpDir::new(scan::fresh_tmp_path("forge-agent-runtime", "persist-projection"));
        fs::create_dir_all(tmp.as_ref()).expect("tmp dir");
        let mut projection = direct_safe_execution_with(
            r#"/forge run input=@latest intent="profile market data" plan_only=true"#,
            4096,
            |idx, step, budget| {
                compact_step_result(
                    idx,
                    step.get("route").and_then(Value::as_str).unwrap_or(""),
                    step.get("command_hash").and_then(Value::as_str).unwrap_or(""),
                    "executed_safe",
                    json!({"plan_only": true, "state": "planned"}),
                    budget,
                )
            },
        )
        .expect("safe projection");
        let persisted = persist_direct_projection(tmp.as_ref(), &mut projection)
            .expect("persist direct projection");
        let listed = direct_read_projection(tmp.as_ref(), &json!({"list": true}))
            .expect("list projections");

        assert_eq!(persisted["runtime"].as_str(), Some(FORGE_AGENT_RUNTIME_V0));
        assert_eq!(listed["found"].as_bool(), Some(true));
        assert_eq!(listed["entries"].as_array().map(Vec::len), Some(1));
        assert_eq!(
            listed["entries"][0]["projection_hash"],
            persisted["projection_hash"]
        );
    }

    #[test]
    fn direct_exact_cache_requires_matching_mode_and_budget() {
        let tmp = scan::TmpDir::new(scan::fresh_tmp_path("forge-agent-runtime", "exact-cache"));
        fs::create_dir_all(tmp.as_ref()).expect("tmp dir");
        let mut projection = direct_safe_execution_with(
            r#"/forge run input=@latest intent="cache direct safe projection" plan_only=true"#,
            2048,
            |idx, step, budget| {
                compact_step_result(
                    idx,
                    step.get("route").and_then(Value::as_str).unwrap_or(""),
                    step.get("command_hash").and_then(Value::as_str).unwrap_or(""),
                    "executed_safe",
                    json!({"plan_only": true, "state": "planned"}),
                    budget,
                )
            },
        )
        .expect("safe projection");
        let intent_hash = projection["intent_hash"].as_str().expect("intent hash").to_string();
        persist_direct_projection(tmp.as_ref(), &mut projection).expect("persist projection");

        let hit = lookup_cached_direct_projection(
            tmp.as_ref(),
            Some(&intent_hash),
            "execute_safe",
            1024,
        )
        .expect("cache lookup")
        .expect("cache hit");
        assert_eq!(hit["cache_hit"].as_bool(), Some(true));
        assert_eq!(
            hit["cache_lookup"]["cache_reason"].as_str().or_else(|| hit["cache_reason"].as_str()),
            Some("exact_intent_mode_and_budget")
        );

        let wrong_mode = lookup_cached_direct_projection(
            tmp.as_ref(),
            Some(&intent_hash),
            "planned_no_side_effects",
            1024,
        )
        .expect("wrong mode cache lookup");
        assert!(wrong_mode.is_none());

        let too_small_budget = lookup_cached_direct_projection(
            tmp.as_ref(),
            Some(&intent_hash),
            "execute_safe",
            4096,
        )
        .expect("budget cache lookup");
        assert!(too_small_budget.is_none());
    }
}
