//! Direct Forge agent CLI.
//!
//! This is the internal agent-OS entrypoint: ForgeSlash in, verified compact
//! projection out. MCP can wrap this path for external tools, but it is not in
//! the primary execution circuit.

#[allow(dead_code)]
#[path = "../forge_intent.rs"]
mod forge_intent;
#[allow(dead_code)]
#[path = "../forge_agent_runtime.rs"]
mod forge_agent_runtime;
#[path = "../forge_agent_tools.rs"]
mod forge_agent_tools;

use serde_json::{json, Value};
use std::io::{self, Read};

fn main() {
    match run() {
        Ok(value) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&value)
                    .unwrap_or_else(|_| "{\"error\":\"encode output\"}".to_string())
            );
        }
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(1);
        }
    }
}

fn run() -> Result<serde_json::Value, String> {
    let cli = AgentCli::parse(std::env::args().skip(1).collect())?;
    let store_path = if cli.persist && cli.command != "about" {
        Some(forge_agent_tools::resolve_store_path()?)
    } else {
        None
    };
    let mut should_persist = cli.persist && cli.command != "about";
    let mut value = match cli.command.as_str() {
        "plan" => {
            let planned = forge_agent_runtime::direct_plan_projection(&cli.source, cli.max_bytes)?;
            if let Some(ref store_path) = store_path {
                if let Some(cached) = forge_agent_runtime::lookup_cached_direct_projection(
                    store_path,
                    planned.get("intent_hash").and_then(Value::as_str),
                    "planned_no_side_effects",
                    cli.max_bytes,
                )? {
                    should_persist = false;
                    Ok(cached)
                } else {
                    Ok(planned)
                }
            } else {
                Ok(planned)
            }
        }
        "safe" | "run-safe" => {
            let planned = forge_agent_runtime::direct_plan_projection(&cli.source, cli.max_bytes)?;
            if let Some(ref store_path) = store_path {
                if let Some(cached) = forge_agent_runtime::lookup_cached_direct_projection(
                    store_path,
                    planned.get("intent_hash").and_then(Value::as_str),
                    "execute_safe",
                    cli.max_bytes,
                )? {
                    should_persist = false;
                    Ok(cached)
                } else {
                    forge_agent_runtime::direct_safe_execution_with(
                        &cli.source,
                        cli.max_bytes,
                        direct_cli_safe_step,
                    )
                }
            } else {
                forge_agent_runtime::direct_safe_execution_with(
                    &cli.source,
                    cli.max_bytes,
                    direct_cli_safe_step,
                )
            }
        }
        "approve" | "approved" => direct_cli_approved(&cli),
        "about" => Ok(forge_agent_runtime::direct_about_value()),
        other => Err(format!(
            "unknown forge_agent command '{other}'. Use: forge_agent about | forge_agent plan|safe|approve [--max-bytes N] '<ForgeSlash>'"
        )),
    }?;
    if should_persist {
        let store_path = store_path.ok_or_else(|| "missing store path for CLI persistence".to_string())?;
        let persisted = forge_agent_runtime::persist_direct_projection(&store_path, &mut value)?;
        if let Value::Object(ref mut obj) = value {
            obj.insert("cli_persisted".to_string(), persisted);
        }
    }
    Ok(value)
}

fn direct_cli_approved(cli: &AgentCli) -> Result<Value, String> {
    forge_agent_runtime::direct_approved_execution_with(
        &cli.source,
        cli.max_bytes,
        cli.approve_side_effects,
        cli.approved_intent_hash.as_deref(),
        cli.approved_policy_hash.as_deref(),
        cli.allow_run_side_effects,
        direct_cli_approved_step,
    )
}

fn direct_cli_safe_step(index: usize, step: &Value, result_budget_bytes: usize) -> Value {
    let route = step.get("route").and_then(Value::as_str).unwrap_or("");
    let command_hash = step.get("command_hash").and_then(Value::as_str).unwrap_or("");
    let args = step.get("arguments").cloned().unwrap_or_else(|| json!({}));
    let side_effect = step.get("side_effect").and_then(Value::as_bool).unwrap_or(true);
    if side_effect {
        return json!({
            "index": index,
            "route": route,
            "command_hash": command_hash,
            "status": "skipped_side_effect",
            "reason": "forge_agent safe runs only read-only and plan_only intent steps"
        });
    }
    match route {
        "run" if args.get("plan_only").and_then(Value::as_bool).unwrap_or(false) => {
            forge_agent_runtime::compact_step_result(
                index,
                route,
                command_hash,
                "executed_safe",
                json!({
                    "kind": "forge_agent_direct_plan",
                    "state": "planned",
                    "plan_only": true,
                    "arguments": args,
                    "raw_data_returned": false
                }),
                result_budget_bytes,
            )
        }
        "brain_recall" => direct_internal_tool_step(
            index,
            route,
            command_hash,
            "forge_brain_recall",
            &args,
            "executed_safe",
            result_budget_bytes,
        ),
        "brain_explain" => direct_internal_tool_step(
            index,
            route,
            command_hash,
            "forge_brain_explain",
            &args,
            "executed_safe",
            result_budget_bytes,
        ),
        "read" => direct_projection_read_step(
            index,
            route,
            command_hash,
            &args,
            result_budget_bytes,
        ),
        other => forge_agent_runtime::compact_step_result(
            index,
            route,
            command_hash,
            "skipped_direct_hostcall_not_wired",
            json!({
                "kind": "forge_agent_direct_skip",
                "route": other,
                "reason": "direct CLI hostcall is not wired yet; MCP adapter remains fallback until shared host routes move into forge_agent_runtime",
                "raw_data_returned": false
            }),
            result_budget_bytes,
        ),
    }
}

fn direct_projection_read_step(
    index: usize,
    route: &str,
    command_hash: &str,
    args: &Value,
    result_budget_bytes: usize,
) -> Value {
    match forge_agent_tools::resolve_store_path()
        .and_then(|store_path| forge_agent_runtime::direct_read_projection(&store_path, args))
    {
        Ok(value) => forge_agent_runtime::compact_step_result(
            index,
            route,
            command_hash,
            "executed_safe",
            value,
            result_budget_bytes,
        ),
        Err(err) => json!({
            "index": index,
            "route": route,
            "command_hash": command_hash,
            "status": "error",
            "raw_data_returned": false,
            "error": err
        }),
    }
}

fn direct_cli_approved_step(index: usize, step: &Value, result_budget_bytes: usize) -> Value {
    let route = step.get("route").and_then(Value::as_str).unwrap_or("");
    let command_hash = step.get("command_hash").and_then(Value::as_str).unwrap_or("");
    let args = step.get("arguments").cloned().unwrap_or_else(|| json!({}));
    let side_effect = step.get("side_effect").and_then(Value::as_bool).unwrap_or(true);
    if !side_effect {
        return direct_cli_safe_step(index, step, result_budget_bytes);
    }
    match route {
        "brain_commit" => direct_internal_tool_step(
            index,
            route,
            command_hash,
            "forge_brain_commit",
            &args,
            "executed_side_effect",
            result_budget_bytes,
        ),
        "run" => direct_run_program_step(
            index,
            route,
            command_hash,
            &args,
            result_budget_bytes,
        ),
        "create" => direct_create_program_step(
            index,
            route,
            command_hash,
            &args,
            result_budget_bytes,
        ),
        other => forge_agent_runtime::compact_step_result(
            index,
            route,
            command_hash,
            "skipped_direct_hostcall_not_wired",
            json!({
                "kind": "forge_agent_direct_skip",
                "route": other,
                "reason": "direct approved hostcall is not wired yet",
                "raw_data_returned": false
            }),
            result_budget_bytes,
        ),
    }
}

fn direct_run_program_step(
    index: usize,
    route: &str,
    command_hash: &str,
    args: &Value,
    result_budget_bytes: usize,
) -> Value {
    match forge_agent_tools::resolve_store_path()
        .and_then(|store_path| forge_agent_runtime::direct_run_program(&store_path, args, "forge_agent"))
    {
        Ok(value) => forge_agent_runtime::compact_step_result(
            index,
            route,
            command_hash,
            "executed_side_effect",
            value,
            result_budget_bytes,
        ),
        Err(err) => json!({
            "index": index,
            "route": route,
            "command_hash": command_hash,
            "status": "error",
            "raw_data_returned": false,
            "error": err
        }),
    }
}

fn direct_create_program_step(
    index: usize,
    route: &str,
    command_hash: &str,
    args: &Value,
    result_budget_bytes: usize,
) -> Value {
    match forge_agent_tools::resolve_store_path()
        .and_then(|store_path| forge_agent_runtime::direct_create_program(&store_path, args, "forge_agent"))
    {
        Ok(value) => forge_agent_runtime::compact_step_result(
            index,
            route,
            command_hash,
            "executed_side_effect",
            value,
            result_budget_bytes,
        ),
        Err(err) => json!({
            "index": index,
            "route": route,
            "command_hash": command_hash,
            "status": "error",
            "raw_data_returned": false,
            "error": err
        }),
    }
}

fn direct_internal_tool_step(
    index: usize,
    route: &str,
    command_hash: &str,
    internal_tool: &str,
    args: &Value,
    status: &str,
    result_budget_bytes: usize,
) -> Value {
    match direct_internal_tool_value(internal_tool, args) {
        Ok(value) => forge_agent_runtime::compact_step_result(
            index,
            route,
            command_hash,
            status,
            value,
            result_budget_bytes,
        ),
        Err(err) => json!({
            "index": index,
            "route": route,
            "command_hash": command_hash,
            "status": "error",
            "raw_data_returned": false,
            "error": err
        }),
    }
}

fn direct_internal_tool_value(tool: &str, args: &Value) -> Result<Value, String> {
    let store_path = forge_agent_tools::resolve_store_path()?;
    forge_agent_tools::call_internal_tool(&store_path, tool, args, None)
}

#[derive(Debug)]
struct AgentCli {
    command: String,
    source: String,
    max_bytes: usize,
    approve_side_effects: bool,
    approved_intent_hash: Option<String>,
    approved_policy_hash: Option<String>,
    allow_run_side_effects: bool,
    persist: bool,
}

impl AgentCli {
    fn parse(args: Vec<String>) -> Result<Self, String> {
        let mut command = "plan".to_string();
        let mut source_parts = Vec::<String>::new();
        let mut max_bytes = 4096usize;
        let mut approve_side_effects = false;
        let mut approved_intent_hash = None;
        let mut approved_policy_hash = None;
        let mut allow_run_side_effects = false;
        let mut persist = true;
        let mut idx = 0usize;

        if args.first().is_some_and(|arg| !arg.starts_with('-')) {
            command = args[0].clone();
            idx = 1;
        }

        while idx < args.len() {
            match args[idx].as_str() {
                "--max-bytes" => {
                    idx += 1;
                    let raw = args
                        .get(idx)
                        .ok_or_else(|| "--max-bytes requires a value".to_string())?;
                    max_bytes = raw
                        .parse::<usize>()
                        .map_err(|_| format!("invalid --max-bytes value '{raw}'"))?
                        .clamp(256, 65_536);
                }
                "--stdin" => {
                    let mut buf = String::new();
                    io::stdin()
                        .read_to_string(&mut buf)
                        .map_err(|err| format!("read stdin: {err}"))?;
                    source_parts.push(buf);
                }
                "--approve-side-effects" => {
                    approve_side_effects = true;
                }
                "--approved-intent-hash" => {
                    idx += 1;
                    approved_intent_hash = Some(
                        args.get(idx)
                            .ok_or_else(|| "--approved-intent-hash requires a value".to_string())?
                            .to_string(),
                    );
                }
                "--approved-policy-hash" => {
                    idx += 1;
                    approved_policy_hash = Some(
                        args.get(idx)
                            .ok_or_else(|| "--approved-policy-hash requires a value".to_string())?
                            .to_string(),
                    );
                }
                "--allow-run-side-effects" => {
                    allow_run_side_effects = true;
                }
                "--no-persist" => {
                    persist = false;
                }
                other => source_parts.push(other.to_string()),
            }
            idx += 1;
        }

        let source = source_parts.join(" ").trim().to_string();
        if command == "about" {
            return Ok(Self {
                command,
                source,
                max_bytes,
                approve_side_effects,
                approved_intent_hash,
                approved_policy_hash,
                allow_run_side_effects,
                persist,
            });
        }
        if source.is_empty() {
            return Err("missing ForgeSlash source; pass it as an argument or use --stdin".to_string());
        }

        Ok(Self {
            command,
            source,
            max_bytes,
            approve_side_effects,
            approved_intent_hash,
            approved_policy_hash,
            allow_run_side_effects,
            persist,
        })
    }
}
