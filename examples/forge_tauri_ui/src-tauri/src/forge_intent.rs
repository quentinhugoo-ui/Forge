use serde::Serialize;
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

pub const FORGE_SLASH_V0_GRAMMAR: &str = r#"ForgeSlash v0
Program      := Command+
Command      := "/forge" Verb Arg* | Verb Arg*
Verb         := recall | plan | create | run | project | commit | explain
Arg          := Key "=" Value
Key          := [a-z][a-z0-9_]*
Value        := quoted-string | bool | number | ref | bare-token
ref          := "@" ("latest" | "pending" | "job:" token | "program:" token | "artifact:" token)

Authority rule:
- ForgeSlash carries intent, refs, hashes and bounded parameters.
- It does not grant filesystem-wide authority; raw paths are rejected in v0.
- Execution must still pass Godel/policy gates before side effects.
"#;

pub const FORGE_SLASH_V0_EXAMPLE: &str = r#"/forge
recall scope=real_estate
run input=@latest intent="profile market data" plan_only=true
project job_id=example max_bytes=4096"#;

const INTENT_MAX_STEPS: usize = 16;
const INTENT_MAX_ARGUMENT_BYTES: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IntentVerb {
    Recall,
    Plan,
    Create,
    Run,
    Project,
    Commit,
    Explain,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "value")]
pub enum IntentValue {
    String(String),
    Bool(bool),
    Number(f64),
    Ref(String),
    Token(String),
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct IntentArg {
    pub key: String,
    pub value: IntentValue,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct IntentCommand {
    pub verb: IntentVerb,
    pub args: Vec<IntentArg>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct IntentProgram {
    pub commands: Vec<IntentCommand>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CompiledIntentProgram {
    pub intent_hash: String,
    pub step_count: usize,
    pub steps: Vec<CompiledIntentStep>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CompiledIntentStep {
    pub command_hash: String,
    pub verb: IntentVerb,
    pub route: String,
    pub arguments: Value,
    pub side_effect: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct IntentPolicyReport {
    pub ok: bool,
    pub policy_hash: String,
    pub intent_hash: String,
    pub side_effect_count: usize,
    pub checks: Vec<IntentPolicyCheck>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct IntentPolicyCheck {
    pub name: String,
    pub ok: bool,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct IntentTraceCard {
    pub trace_hash: String,
    pub status: String,
    pub intent_hash: String,
    pub policy_hash: String,
    pub command_hashes: Vec<String>,
    pub routes: Vec<String>,
    pub route_count: usize,
    pub side_effect_count: usize,
    pub argument_bytes: usize,
    pub raw_data_in_context: bool,
    pub proof_hash: Option<String>,
    pub output_hash: Option<String>,
    pub distillation_candidate: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DistillationTarget {
    NonePolicyFailed,
    ExactCache,
    VerifiedProgramAfterExecution,
    ProceduralSkillAfterExecution,
    LocalRouterExample,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DistillationAnalysis {
    pub target: DistillationTarget,
    pub reason: String,
    pub requires_execution_evidence: bool,
    pub requires_human_or_license_review: bool,
    pub rollback: String,
    pub proof_inputs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PromotionManifest {
    pub status: String,
    pub target: DistillationTarget,
    pub trace_hash: String,
    pub required_evidence: Vec<String>,
    pub ready: bool,
    pub reason: String,
    pub rollback: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SkillPromotionManifest {
    pub status: String,
    pub trace_hash: String,
    pub scope: Option<String>,
    pub required_evidence: Vec<String>,
    pub ready: bool,
    pub skill_key: Option<String>,
    pub rollback: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RouterPromotionManifest {
    pub status: String,
    pub trace_hash: String,
    pub ready: bool,
    pub required_evidence: Vec<String>,
    pub allowed_training_target: String,
    pub model_training_allowed: bool,
    pub provider_license_review_required: bool,
    pub rollback: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ForgeProjection {
    pub kind: String,
    pub intent_hash: String,
    pub policy_hash: String,
    pub trace_hash: String,
    pub status: String,
    pub route_count: usize,
    pub side_effect_count: usize,
    pub raw_data_returned: bool,
    pub bounded_preview_bytes: usize,
    pub hashes: ProjectionHashes,
    pub promotion: ProjectionPromotion,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProjectionHashes {
    pub command_hashes: Vec<String>,
    pub proof_hash: Option<String>,
    pub output_hash: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProjectionPromotion {
    pub distillation_target: DistillationTarget,
    pub program_status: String,
    pub skill_status: String,
    pub router_status: String,
}

impl IntentProgram {
    pub fn canonical_json(&self) -> String {
        serde_json::to_string(self).expect("IntentProgram serialization is infallible")
    }

    pub fn content_hash(&self) -> String {
        stable_hash("forge-intent-v0/program", self.canonical_json().as_bytes())
    }

    pub fn command_hashes(&self) -> Vec<String> {
        self.commands
            .iter()
            .map(IntentCommand::content_hash)
            .collect()
    }

    pub fn compile_v0(&self) -> CompiledIntentProgram {
        let steps: Vec<CompiledIntentStep> = self
            .commands
            .iter()
            .map(IntentCommand::compile_v0)
            .collect();
        CompiledIntentProgram {
            intent_hash: self.content_hash(),
            step_count: steps.len(),
            steps,
        }
    }
}

impl CompiledIntentProgram {
    pub fn policy_report_v0(&self) -> IntentPolicyReport {
        let mut checks = Vec::new();
        let side_effect_count = self.steps.iter().filter(|step| step.side_effect).count();
        checks.push(IntentPolicyCheck {
            name: "step_budget".to_string(),
            ok: self.step_count <= INTENT_MAX_STEPS,
            detail: format!("steps={} max={INTENT_MAX_STEPS}", self.step_count),
        });
        checks.push(IntentPolicyCheck {
            name: "argument_budget".to_string(),
            ok: self
                .steps
                .iter()
                .all(|step| route_argument_bytes(step) <= INTENT_MAX_ARGUMENT_BYTES),
            detail: format!("max_argument_bytes={INTENT_MAX_ARGUMENT_BYTES}"),
        });
        checks.push(IntentPolicyCheck {
            name: "route_allowlist".to_string(),
            ok: self.steps.iter().all(|step| route_is_allowed(&step.route)),
            detail: "routes must be existing Forge internal routes".to_string(),
        });
        checks.push(IntentPolicyCheck {
            name: "side_effect_allowlist".to_string(),
            ok: self
                .steps
                .iter()
                .all(|step| !step.side_effect || side_effect_route_is_allowed(&step.route)),
            detail: "side effects are limited to create, run and brain_commit routes".to_string(),
        });
        checks.push(IntentPolicyCheck {
            name: "bounded_refs_only".to_string(),
            ok: self.steps.iter().all(step_has_no_raw_paths),
            detail: "raw filesystem paths are rejected before execution".to_string(),
        });
        let ok = checks.iter().all(|check| check.ok);
        let policy_material = serde_json::to_string(&checks).expect("policy checks serialize");
        IntentPolicyReport {
            ok,
            policy_hash: stable_hash("forge-intent-v0/policy", policy_material.as_bytes()),
            intent_hash: self.intent_hash.clone(),
            side_effect_count,
            checks,
        }
    }

    pub fn trace_card_v0(&self, policy: &IntentPolicyReport) -> IntentTraceCard {
        let routes: Vec<String> = self.steps.iter().map(|step| step.route.clone()).collect();
        let command_hashes: Vec<String> = self
            .steps
            .iter()
            .map(|step| step.command_hash.clone())
            .collect();
        let argument_bytes = self.steps.iter().map(route_argument_bytes).sum();
        let status = if policy.ok {
            "planned_policy_ok"
        } else {
            "blocked_policy_failed"
        };
        let distillation_candidate = if policy.ok && self.steps.iter().all(|step| !step.side_effect) {
            "cache_or_projection"
        } else if policy.ok && self.steps.iter().any(|step| step.route == "run") {
            "verified_program_after_execution"
        } else if policy.ok {
            "procedural_skill_after_execution"
        } else {
            "none_policy_failed"
        };
        let material = json!({
            "intent_hash": self.intent_hash,
            "policy_hash": policy.policy_hash,
            "command_hashes": command_hashes,
            "routes": routes,
            "status": status,
            "argument_bytes": argument_bytes,
            "side_effect_count": policy.side_effect_count
        });
        let trace_hash = stable_hash(
            "forge-intent-v0/trace-card",
            serde_json::to_string(&material)
                .expect("trace material serializes")
                .as_bytes(),
        );
        IntentTraceCard {
            trace_hash,
            status: status.to_string(),
            intent_hash: self.intent_hash.clone(),
            policy_hash: policy.policy_hash.clone(),
            command_hashes,
            routes,
            route_count: self.step_count,
            side_effect_count: policy.side_effect_count,
            argument_bytes,
            raw_data_in_context: false,
            proof_hash: None,
            output_hash: None,
            distillation_candidate: distillation_candidate.to_string(),
        }
    }
}

impl IntentTraceCard {
    pub fn distillation_analysis_v0(&self) -> DistillationAnalysis {
        if self.status == "blocked_policy_failed" {
            return DistillationAnalysis {
                target: DistillationTarget::NonePolicyFailed,
                reason: "Policy failed; do not promote blocked behavior.".to_string(),
                requires_execution_evidence: false,
                requires_human_or_license_review: false,
                rollback: "fix or reject the intent before any execution".to_string(),
                proof_inputs: vec![self.intent_hash.clone(), self.policy_hash.clone()],
            };
        }
        if self.side_effect_count == 0 && self.routes.iter().all(|route| route == "read" || route.starts_with("brain_")) {
            return DistillationAnalysis {
                target: DistillationTarget::ExactCache,
                reason: "Read-only/projection intent can be replayed or cached by hashes.".to_string(),
                requires_execution_evidence: false,
                requires_human_or_license_review: false,
                rollback: "miss cache and re-run the same bounded read route".to_string(),
                proof_inputs: vec![self.trace_hash.clone(), self.intent_hash.clone(), self.policy_hash.clone()],
            };
        }
        if self.routes.iter().any(|route| route == "run" || route == "create") {
            return DistillationAnalysis {
                target: DistillationTarget::VerifiedProgramAfterExecution,
                reason: "Compute/create route should promote to a verified program only after output/proof hashes exist.".to_string(),
                requires_execution_evidence: true,
                requires_human_or_license_review: false,
                rollback: "fall back to the compiled route plan and LLM-authored intent".to_string(),
                proof_inputs: vec![self.trace_hash.clone(), self.intent_hash.clone(), self.policy_hash.clone()],
            };
        }
        if self.routes.iter().any(|route| route == "brain_commit") {
            return DistillationAnalysis {
                target: DistillationTarget::ProceduralSkillAfterExecution,
                reason: "Memory/procedural behavior can become a skill only after scoped evidence is committed.".to_string(),
                requires_execution_evidence: true,
                requires_human_or_license_review: false,
                rollback: "keep the scoped memory note unpromoted".to_string(),
                proof_inputs: vec![self.trace_hash.clone(), self.intent_hash.clone(), self.policy_hash.clone()],
            };
        }
        DistillationAnalysis {
            target: DistillationTarget::LocalRouterExample,
            reason: "Bounded successful intent can become a router example before any model training.".to_string(),
            requires_execution_evidence: true,
            requires_human_or_license_review: false,
            rollback: "drop the router example and use normal intent parsing".to_string(),
            proof_inputs: vec![self.trace_hash.clone(), self.intent_hash.clone(), self.policy_hash.clone()],
        }
    }

    pub fn promotion_manifest_v0(&self, analysis: &DistillationAnalysis) -> PromotionManifest {
        let mut required_evidence = vec!["trace_hash".to_string(), "policy_hash".to_string()];
        match analysis.target {
            DistillationTarget::NonePolicyFailed => PromotionManifest {
                status: "blocked".to_string(),
                target: analysis.target.clone(),
                trace_hash: self.trace_hash.clone(),
                required_evidence,
                ready: false,
                reason: "Policy failed; no promotion allowed.".to_string(),
                rollback: analysis.rollback.clone(),
            },
            DistillationTarget::ExactCache => PromotionManifest {
                status: "ready_exact_cache".to_string(),
                target: analysis.target.clone(),
                trace_hash: self.trace_hash.clone(),
                required_evidence,
                ready: true,
                reason: "Trace is bounded and read-only; exact replay/cache can use existing hashes.".to_string(),
                rollback: analysis.rollback.clone(),
            },
            DistillationTarget::VerifiedProgramAfterExecution => {
                required_evidence.extend([
                    "output_hash".to_string(),
                    "proof_hash".to_string(),
                    "test_vectors".to_string(),
                    "semantic_fingerprint".to_string(),
                ]);
                let ready = self.output_hash.is_some() && self.proof_hash.is_some();
                PromotionManifest {
                    status: if ready {
                        "ready_verified_program"
                    } else {
                        "pending_execution_evidence"
                    }
                    .to_string(),
                    target: analysis.target.clone(),
                    trace_hash: self.trace_hash.clone(),
                    required_evidence,
                    ready,
                    reason: "Deterministic run/create behavior can promote only with output/proof hashes and semantic tests.".to_string(),
                    rollback: analysis.rollback.clone(),
                }
            }
            DistillationTarget::ProceduralSkillAfterExecution => {
                required_evidence.extend([
                    "memory_evidence_hash".to_string(),
                    "scope".to_string(),
                    "examples".to_string(),
                ]);
                PromotionManifest {
                    status: "pending_skill_evidence".to_string(),
                    target: analysis.target.clone(),
                    trace_hash: self.trace_hash.clone(),
                    required_evidence,
                    ready: false,
                    reason: "Procedural skills require scoped evidence and examples before promotion.".to_string(),
                    rollback: analysis.rollback.clone(),
                }
            }
            DistillationTarget::LocalRouterExample => {
                required_evidence.extend(["holdout_examples".to_string(), "shadow_eval".to_string()]);
                PromotionManifest {
                    status: "pending_router_evidence".to_string(),
                    target: analysis.target.clone(),
                    trace_hash: self.trace_hash.clone(),
                    required_evidence,
                    ready: false,
                    reason: "Router examples require holdout/shadow evaluation before promotion.".to_string(),
                    rollback: analysis.rollback.clone(),
                }
            }
        }
    }

    pub fn skill_promotion_manifest_v0(&self, analysis: &DistillationAnalysis) -> SkillPromotionManifest {
        let scope = self.scope_hint();
        let mut required_evidence = vec![
            "trace_hash".to_string(),
            "policy_hash".to_string(),
            "scope".to_string(),
            "evidence_hash".to_string(),
            "examples".to_string(),
        ];
        if analysis.target != DistillationTarget::ProceduralSkillAfterExecution {
            return SkillPromotionManifest {
                status: "not_a_skill_candidate".to_string(),
                trace_hash: self.trace_hash.clone(),
                scope,
                required_evidence,
                ready: false,
                skill_key: None,
                rollback: "use the cheaper promotion target selected by distillation_analysis".to_string(),
            };
        }
        let ready = self.proof_hash.is_some() && scope.is_some();
        let skill_key = ready.then(|| {
            format!(
                "skill:{}:{}",
                scope.as_deref().unwrap_or("unknown"),
                &self.trace_hash[..12]
            )
        });
        if ready {
            required_evidence.push("human_review".to_string());
        }
        SkillPromotionManifest {
            status: if ready {
                "ready_procedural_skill"
            } else {
                "pending_skill_evidence"
            }
            .to_string(),
            trace_hash: self.trace_hash.clone(),
            scope,
            required_evidence,
            ready,
            skill_key,
            rollback: "keep the TraceCard as memory evidence; do not install a skill".to_string(),
        }
    }

    pub fn router_promotion_manifest_v0(&self, analysis: &DistillationAnalysis) -> RouterPromotionManifest {
        let required_evidence = vec![
            "trace_hash".to_string(),
            "policy_hash".to_string(),
            "holdout_traces".to_string(),
            "shadow_eval".to_string(),
            "rollback_to_llm".to_string(),
            "provider_license_review".to_string(),
        ];
        if analysis.target != DistillationTarget::LocalRouterExample {
            return RouterPromotionManifest {
                status: "not_a_router_candidate".to_string(),
                trace_hash: self.trace_hash.clone(),
                ready: false,
                required_evidence,
                allowed_training_target: "none".to_string(),
                model_training_allowed: false,
                provider_license_review_required: false,
                rollback: "use the cheaper promotion target selected by distillation_analysis".to_string(),
            };
        }
        RouterPromotionManifest {
            status: "pending_router_shadow_eval".to_string(),
            trace_hash: self.trace_hash.clone(),
            ready: false,
            required_evidence,
            allowed_training_target: "local_router_example_only".to_string(),
            model_training_allowed: false,
            provider_license_review_required: true,
            rollback: "drop the router example and fall back to LLM + ForgeSlash parser".to_string(),
        }
    }

    pub fn forge_projection_v0(
        &self,
        analysis: &DistillationAnalysis,
        promotion: &PromotionManifest,
        skill: &SkillPromotionManifest,
        router: &RouterPromotionManifest,
        bounded_preview_bytes: usize,
    ) -> ForgeProjection {
        ForgeProjection {
            kind: "forge_projection_v0".to_string(),
            intent_hash: self.intent_hash.clone(),
            policy_hash: self.policy_hash.clone(),
            trace_hash: self.trace_hash.clone(),
            status: self.status.clone(),
            route_count: self.route_count,
            side_effect_count: self.side_effect_count,
            raw_data_returned: false,
            bounded_preview_bytes,
            hashes: ProjectionHashes {
                command_hashes: self.command_hashes.clone(),
                proof_hash: self.proof_hash.clone(),
                output_hash: self.output_hash.clone(),
            },
            promotion: ProjectionPromotion {
                distillation_target: analysis.target.clone(),
                program_status: promotion.status.clone(),
                skill_status: skill.status.clone(),
                router_status: router.status.clone(),
            },
        }
    }

    fn scope_hint(&self) -> Option<String> {
        self.routes
            .iter()
            .any(|route| route.starts_with("brain_"))
            .then(|| "brain".to_string())
    }
}

impl IntentCommand {
    pub fn canonical_json(&self) -> String {
        serde_json::to_string(self).expect("IntentCommand serialization is infallible")
    }

    pub fn content_hash(&self) -> String {
        stable_hash("forge-intent-v0/command", self.canonical_json().as_bytes())
    }

    pub fn compile_v0(&self) -> CompiledIntentStep {
        let route = route_for_verb(&self.verb);
        let mut arguments = args_to_route_arguments(&self.args);
        if matches!(self.verb, IntentVerb::Plan) {
            arguments.insert("plan_only".to_string(), json!(true));
        }
        if matches!(self.verb, IntentVerb::Project) && !arguments.contains_key("kind") {
            arguments.insert("kind".to_string(), json!("artifacts"));
        }
        let side_effect = match self.verb {
            IntentVerb::Recall | IntentVerb::Plan | IntentVerb::Project | IntentVerb::Explain => false,
            IntentVerb::Create | IntentVerb::Run | IntentVerb::Commit => !arguments
                .get("plan_only")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        };
        CompiledIntentStep {
            command_hash: self.content_hash(),
            verb: self.verb.clone(),
            route: route.to_string(),
            arguments: Value::Object(arguments),
            side_effect,
        }
    }
}

pub fn parse_forge_slash_v0(source: &str) -> Result<IntentProgram, String> {
    let mut commands = Vec::new();
    let mut saw_header = false;
    for (idx, raw_line) in source.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut tokens = tokenize_line(line).map_err(|err| format!("line {}: {err}", idx + 1))?;
        if tokens.is_empty() {
            continue;
        }
        if tokens[0] == "/forge" {
            saw_header = true;
            tokens.remove(0);
            if tokens.is_empty() {
                continue;
            }
        } else if tokens[0].starts_with('/') {
            return Err(format!(
                "line {}: expected /forge or a ForgeSlash verb, got {}",
                idx + 1,
                tokens[0]
            ));
        }
        let verb = parse_verb(&tokens[0])
            .ok_or_else(|| format!("line {}: unknown ForgeSlash verb '{}'", idx + 1, tokens[0]))?;
        let args = parse_args(verb.clone(), &tokens[1..])
            .map_err(|err| format!("line {}: {err}", idx + 1))?;
        commands.push(IntentCommand { verb, args });
    }
    if commands.is_empty() {
        if saw_header {
            return Err("ForgeSlash program has /forge header but no commands".to_string());
        }
        return Err("ForgeSlash program is empty".to_string());
    }
    Ok(IntentProgram { commands })
}

fn parse_verb(token: &str) -> Option<IntentVerb> {
    match token {
        "recall" => Some(IntentVerb::Recall),
        "plan" => Some(IntentVerb::Plan),
        "create" => Some(IntentVerb::Create),
        "run" => Some(IntentVerb::Run),
        "project" => Some(IntentVerb::Project),
        "commit" => Some(IntentVerb::Commit),
        "explain" => Some(IntentVerb::Explain),
        _ => None,
    }
}

fn parse_args(verb: IntentVerb, tokens: &[String]) -> Result<Vec<IntentArg>, String> {
    let mut args = tokens
        .iter()
        .map(|token| {
            let (key, raw_value) = token
                .split_once('=')
                .ok_or_else(|| format!("argument '{token}' must use key=value"))?;
            if !is_valid_key(key) {
                return Err(format!("invalid argument key '{key}'"));
            }
            if !allowed_arg(&verb, key) {
                return Err(format!("argument '{key}' is not valid for {:?}", verb));
            }
            let value = parse_value(raw_value)?;
            validate_authority(key, &value)?;
            Ok(IntentArg {
                key: key.to_string(),
                value,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    args.sort_by(|a, b| a.key.cmp(&b.key));
    if args.windows(2).any(|pair| pair[0].key == pair[1].key) {
        return Err("duplicate argument keys are not allowed".to_string());
    }
    Ok(args)
}

fn allowed_arg(verb: &IntentVerb, key: &str) -> bool {
    let allowed: &[&str] = match verb {
        IntentVerb::Recall => &["scope", "section", "hash", "program_hash"],
        IntentVerb::Plan => &["intent", "input", "capability", "title"],
        IntentVerb::Create => &[
            "title",
            "kind",
            "program_kind",
            "domain",
            "goal",
            "intent",
            "template",
        ],
        IntentVerb::Run => &[
            "intent",
            "input",
            "job_id",
            "program_hash",
            "program",
            "capability",
            "plan_only",
            "title",
        ],
        IntentVerb::Project => &[
            "job_id",
            "program_hash",
            "run_hash",
            "format",
            "max_bytes",
            "include",
            "kind",
            "list",
            "projection_hash",
            "execution_hash",
            "trace_hash",
            "intent_hash",
            "projection_ref",
            "ref",
            "limit",
        ],
        IntentVerb::Commit => &[
            "scope",
            "section",
            "kind",
            "source",
            "confidence",
            "text",
            "observation",
            "program_hash",
        ],
        IntentVerb::Explain => &["hash", "program_hash", "memory_hash", "ref", "kind"],
    };
    allowed.contains(&key)
}

fn parse_value(raw: &str) -> Result<IntentValue, String> {
    if raw.starts_with('@') {
        validate_ref(raw)?;
        return Ok(IntentValue::Ref(raw.to_string()));
    }
    match raw {
        "true" => return Ok(IntentValue::Bool(true)),
        "false" => return Ok(IntentValue::Bool(false)),
        _ => {}
    }
    if let Ok(number) = raw.parse::<f64>() {
        if number.is_finite() {
            return Ok(IntentValue::Number(number));
        }
        return Err("numeric value must be finite".to_string());
    }
    if looks_like_raw_path(raw) {
        return Err("raw filesystem paths are not valid ForgeSlash v0 values; use refs like @latest or @job:<id>".to_string());
    }
    if raw.is_empty() {
        return Err("empty values are not allowed".to_string());
    }
    if raw.chars().any(char::is_whitespace) {
        return Ok(IntentValue::String(raw.to_string()));
    }
    Ok(IntentValue::Token(raw.to_string()))
}

fn validate_ref(raw: &str) -> Result<(), String> {
    if raw == "@latest" || raw == "@pending" {
        return Ok(());
    }
    for prefix in ["@job:", "@program:", "@artifact:"] {
        if let Some(rest) = raw.strip_prefix(prefix) {
            if !rest.is_empty() && rest.chars().all(is_ref_char) {
                return Ok(());
            }
        }
    }
    Err(format!("invalid ForgeSlash ref '{raw}'"))
}

fn validate_authority(key: &str, value: &IntentValue) -> Result<(), String> {
    if key == "input" && !matches!(value, IntentValue::Ref(_)) {
        return Err("input must be a bounded ref such as @latest, @pending or @job:<id>".to_string());
    }
    Ok(())
}

fn tokenize_line(line: &str) -> Result<Vec<String>, String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut chars = line.chars().peekable();
    let mut in_quote = false;
    while let Some(ch) = chars.next() {
        match ch {
            '"' => {
                in_quote = !in_quote;
            }
            '\\' if in_quote => {
                let escaped = chars
                    .next()
                    .ok_or_else(|| "dangling escape in quoted string".to_string())?;
                current.push(escaped);
            }
            c if c.is_whitespace() && !in_quote => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            c => current.push(c),
        }
    }
    if in_quote {
        return Err("unterminated quoted string".to_string());
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    Ok(tokens)
}

fn is_valid_key(key: &str) -> bool {
    let mut chars = key.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

fn is_ref_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.' | ':')
}

fn looks_like_raw_path(value: &str) -> bool {
    value.contains(":\\")
        || value.contains(":/")
        || value.starts_with("\\\\")
        || value.starts_with('/')
        || value.starts_with(".\\")
        || value.starts_with("../")
}

fn route_for_verb(verb: &IntentVerb) -> &'static str {
    match verb {
        IntentVerb::Recall => "brain_recall",
        IntentVerb::Plan => "run",
        IntentVerb::Create => "create",
        IntentVerb::Run => "run",
        IntentVerb::Project => "read",
        IntentVerb::Commit => "brain_commit",
        IntentVerb::Explain => "brain_explain",
    }
}

fn args_to_route_arguments(args: &[IntentArg]) -> Map<String, Value> {
    let mut out = Map::new();
    for arg in args {
        if arg.key == "input" {
            lower_input_ref(&mut out, &arg.value);
        } else {
            out.insert(arg.key.clone(), intent_value_to_json(&arg.value));
        }
    }
    out
}

fn lower_input_ref(out: &mut Map<String, Value>, value: &IntentValue) {
    match value {
        IntentValue::Ref(raw) if raw == "@latest" || raw == "@pending" => {
            out.insert("pending".to_string(), json!(true));
        }
        IntentValue::Ref(raw) if raw.starts_with("@job:") => {
            out.insert(
                "job_id".to_string(),
                json!(raw.trim_start_matches("@job:")),
            );
        }
        IntentValue::Ref(raw) if raw.starts_with("@program:") => {
            out.insert(
                "program_hash".to_string(),
                json!(raw.trim_start_matches("@program:")),
            );
        }
        IntentValue::Ref(raw) if raw.starts_with("@artifact:") => {
            out.insert(
                "artifact_ref".to_string(),
                json!(raw.trim_start_matches("@artifact:")),
            );
        }
        other => {
            out.insert("input".to_string(), intent_value_to_json(other));
        }
    }
}

fn intent_value_to_json(value: &IntentValue) -> Value {
    match value {
        IntentValue::String(value) | IntentValue::Ref(value) | IntentValue::Token(value) => json!(value),
        IntentValue::Bool(value) => json!(value),
        IntentValue::Number(value) => json!(value),
    }
}

fn stable_hash(domain: &str, bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain.as_bytes());
    hasher.update([0]);
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(&mut out, "{byte:02x}");
    }
    out
}

fn route_argument_bytes(step: &CompiledIntentStep) -> usize {
    serde_json::to_vec(&step.arguments)
        .map(|bytes| bytes.len())
        .unwrap_or(usize::MAX)
}

fn route_is_allowed(route: &str) -> bool {
    matches!(
        route,
        "brain_recall" | "run" | "create" | "read" | "brain_commit" | "brain_explain"
    )
}

fn side_effect_route_is_allowed(route: &str) -> bool {
    matches!(route, "create" | "run" | "brain_commit")
}

fn step_has_no_raw_paths(step: &CompiledIntentStep) -> bool {
    fn value_ok(value: &Value) -> bool {
        match value {
            Value::String(value) => !looks_like_raw_path(value),
            Value::Array(values) => values.iter().all(value_ok),
            Value::Object(map) => map.values().all(value_ok),
            _ => true,
        }
    }
    value_ok(&step.arguments)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_multiline_forge_slash_program() {
        let parsed = parse_forge_slash_v0(
            r#"/forge
recall scope=real_estate
run input=@latest intent="profile market data" plan_only=true
project job_id=abc123 max_bytes=4096"#,
        )
        .expect("valid ForgeSlash");

        assert_eq!(parsed.commands.len(), 3);
        assert_eq!(parsed.commands[0].verb, IntentVerb::Recall);
        assert_eq!(parsed.commands[1].verb, IntentVerb::Run);
        assert_eq!(
            parsed.commands[1]
                .args
                .iter()
                .find(|arg| arg.key == "input")
                .expect("input arg")
                .value,
            IntentValue::Ref("@latest".to_string())
        );
    }

    #[test]
    fn hashes_are_stable_across_argument_order() {
        let a = parse_forge_slash_v0(
            r#"/forge run input=@latest intent="profile market data" plan_only=true"#,
        )
        .expect("valid a");
        let b = parse_forge_slash_v0(
            r#"/forge run plan_only=true intent="profile market data" input=@latest"#,
        )
        .expect("valid b");

        assert_eq!(a.canonical_json(), b.canonical_json());
        assert_eq!(a.content_hash(), b.content_hash());
        assert_eq!(a.command_hashes(), b.command_hashes());
    }

    #[test]
    fn compiles_to_existing_internal_routes() {
        let parsed = parse_forge_slash_v0(
            r#"/forge
recall scope=real_estate
plan input=@latest intent="profile market data"
run input=@job:abc123 intent="execute mapping" plan_only=true
project job_id=abc123 max_bytes=4096
commit scope=real_estate kind=semantic observation="market source mapped"
explain hash=deadbeef"#,
        )
        .expect("valid intent");
        let compiled = parsed.compile_v0();

        assert_eq!(compiled.step_count, 6);
        assert_eq!(compiled.steps[0].route, "brain_recall");
        assert_eq!(compiled.steps[1].route, "run");
        assert_eq!(compiled.steps[1].arguments["pending"], json!(true));
        assert_eq!(compiled.steps[1].arguments["plan_only"], json!(true));
        assert_eq!(compiled.steps[2].arguments["job_id"], json!("abc123"));
        assert_eq!(compiled.steps[3].route, "read");
        assert_eq!(compiled.steps[3].arguments["kind"], json!("artifacts"));
        assert_eq!(compiled.steps[4].route, "brain_commit");
        assert_eq!(compiled.steps[5].route, "brain_explain");
    }

    #[test]
    fn policy_gate_accepts_bounded_compiled_program() {
        let parsed = parse_forge_slash_v0(FORGE_SLASH_V0_EXAMPLE).expect("valid intent");
        let compiled = parsed.compile_v0();
        let report = compiled.policy_report_v0();

        assert!(report.ok);
        assert_eq!(report.intent_hash, compiled.intent_hash);
        assert!(report.side_effect_count <= 1);
        assert!(report.policy_hash.len() == 64);
    }

    #[test]
    fn policy_gate_rejects_step_budget_overflow() {
        let source = (0..=INTENT_MAX_STEPS)
            .map(|_| "recall scope=real_estate")
            .collect::<Vec<_>>()
            .join("\n");
        let parsed = parse_forge_slash_v0(&source).expect("syntactically valid");
        let report = parsed.compile_v0().policy_report_v0();

        assert!(!report.ok);
        assert!(report
            .checks
            .iter()
            .any(|check| check.name == "step_budget" && !check.ok));
    }

    #[test]
    fn trace_card_is_stable_for_same_compiled_intent() {
        let a = parse_forge_slash_v0(
            r#"/forge run input=@latest intent="profile market data" plan_only=true"#,
        )
        .expect("valid a")
        .compile_v0();
        let b = parse_forge_slash_v0(
            r#"/forge run plan_only=true intent="profile market data" input=@latest"#,
        )
        .expect("valid b")
        .compile_v0();
        let a_policy = a.policy_report_v0();
        let b_policy = b.policy_report_v0();
        let a_trace = a.trace_card_v0(&a_policy);
        let b_trace = b.trace_card_v0(&b_policy);

        assert_eq!(a_trace.trace_hash, b_trace.trace_hash);
        assert_eq!(a_trace.status, "planned_policy_ok");
        assert!(!a_trace.raw_data_in_context);
        assert_eq!(a_trace.routes, vec!["run".to_string()]);
    }

    #[test]
    fn distillation_analyzer_prefers_programs_over_models_for_run_routes() {
        let parsed = parse_forge_slash_v0(
            r#"/forge run input=@latest intent="profile market data" plan_only=true"#,
        )
        .expect("valid intent");
        let compiled = parsed.compile_v0();
        let policy = compiled.policy_report_v0();
        let trace = compiled.trace_card_v0(&policy);
        let analysis = trace.distillation_analysis_v0();

        assert_eq!(analysis.target, DistillationTarget::VerifiedProgramAfterExecution);
        assert!(analysis.requires_execution_evidence);
        assert!(!analysis.requires_human_or_license_review);
    }

    #[test]
    fn distillation_analyzer_blocks_failed_policy() {
        let source = (0..=INTENT_MAX_STEPS)
            .map(|_| "recall scope=real_estate")
            .collect::<Vec<_>>()
            .join("\n");
        let compiled = parse_forge_slash_v0(&source)
            .expect("syntactically valid")
            .compile_v0();
        let policy = compiled.policy_report_v0();
        let trace = compiled.trace_card_v0(&policy);
        let analysis = trace.distillation_analysis_v0();

        assert_eq!(analysis.target, DistillationTarget::NonePolicyFailed);
        assert!(!analysis.requires_execution_evidence);
    }

    #[test]
    fn promotion_manifest_requires_proof_for_verified_programs() {
        let parsed = parse_forge_slash_v0(
            r#"/forge run input=@latest intent="profile market data" plan_only=true"#,
        )
        .expect("valid intent");
        let compiled = parsed.compile_v0();
        let policy = compiled.policy_report_v0();
        let trace = compiled.trace_card_v0(&policy);
        let analysis = trace.distillation_analysis_v0();
        let manifest = trace.promotion_manifest_v0(&analysis);

        assert_eq!(manifest.target, DistillationTarget::VerifiedProgramAfterExecution);
        assert!(!manifest.ready);
        assert_eq!(manifest.status, "pending_execution_evidence");
        assert!(manifest.required_evidence.contains(&"proof_hash".to_string()));
        assert!(manifest
            .required_evidence
            .contains(&"semantic_fingerprint".to_string()));
    }

    #[test]
    fn promotion_manifest_allows_exact_cache_for_read_only_trace() {
        let parsed = parse_forge_slash_v0(r#"/forge project job_id=abc123 max_bytes=4096"#)
            .expect("valid projection");
        let compiled = parsed.compile_v0();
        let policy = compiled.policy_report_v0();
        let trace = compiled.trace_card_v0(&policy);
        let analysis = trace.distillation_analysis_v0();
        let manifest = trace.promotion_manifest_v0(&analysis);

        assert_eq!(manifest.target, DistillationTarget::ExactCache);
        assert!(manifest.ready);
    }

    #[test]
    fn skill_promotion_requires_skill_evidence() {
        let parsed = parse_forge_slash_v0(
            r#"/forge commit scope=real_estate kind=procedural observation="when mapping market data, return compact hashes""#,
        )
        .expect("valid commit");
        let compiled = parsed.compile_v0();
        let policy = compiled.policy_report_v0();
        let trace = compiled.trace_card_v0(&policy);
        let analysis = trace.distillation_analysis_v0();
        let skill = trace.skill_promotion_manifest_v0(&analysis);

        assert_eq!(analysis.target, DistillationTarget::ProceduralSkillAfterExecution);
        assert_eq!(skill.status, "pending_skill_evidence");
        assert!(!skill.ready);
        assert!(skill.required_evidence.contains(&"evidence_hash".to_string()));
        assert_eq!(skill.scope, Some("brain".to_string()));
    }

    #[test]
    fn non_skill_targets_do_not_emit_skills() {
        let parsed = parse_forge_slash_v0(r#"/forge project job_id=abc123 max_bytes=4096"#)
            .expect("valid projection");
        let compiled = parsed.compile_v0();
        let policy = compiled.policy_report_v0();
        let trace = compiled.trace_card_v0(&policy);
        let analysis = trace.distillation_analysis_v0();
        let skill = trace.skill_promotion_manifest_v0(&analysis);

        assert_eq!(skill.status, "not_a_skill_candidate");
        assert!(!skill.ready);
        assert_eq!(skill.skill_key, None);
    }

    #[test]
    fn router_promotion_requires_shadow_eval_and_blocks_model_training() {
        let trace = IntentTraceCard {
            trace_hash: "trace-router".to_string(),
            status: "planned_policy_ok".to_string(),
            intent_hash: "intent".to_string(),
            policy_hash: "policy".to_string(),
            command_hashes: vec!["cmd".to_string()],
            routes: vec!["brain_explain".to_string(), "read".to_string()],
            route_count: 2,
            side_effect_count: 1,
            argument_bytes: 32,
            raw_data_in_context: false,
            proof_hash: None,
            output_hash: None,
            distillation_candidate: "local_router_example".to_string(),
        };
        let analysis = trace.distillation_analysis_v0();
        let router = trace.router_promotion_manifest_v0(&analysis);

        assert_eq!(analysis.target, DistillationTarget::LocalRouterExample);
        assert_eq!(router.status, "pending_router_shadow_eval");
        assert!(!router.ready);
        assert!(!router.model_training_allowed);
        assert!(router.provider_license_review_required);
        assert!(router.required_evidence.contains(&"shadow_eval".to_string()));
        assert!(router
            .required_evidence
            .contains(&"rollback_to_llm".to_string()));
    }

    #[test]
    fn non_router_targets_do_not_emit_router_examples() {
        let parsed = parse_forge_slash_v0(r#"/forge project job_id=abc123 max_bytes=4096"#)
            .expect("valid projection");
        let compiled = parsed.compile_v0();
        let policy = compiled.policy_report_v0();
        let trace = compiled.trace_card_v0(&policy);
        let analysis = trace.distillation_analysis_v0();
        let router = trace.router_promotion_manifest_v0(&analysis);

        assert_eq!(router.status, "not_a_router_candidate");
        assert!(!router.model_training_allowed);
    }

    #[test]
    fn forge_projection_is_compact_and_hash_only() {
        let parsed = parse_forge_slash_v0(FORGE_SLASH_V0_EXAMPLE).expect("valid example");
        let compiled = parsed.compile_v0();
        let policy = compiled.policy_report_v0();
        let trace = compiled.trace_card_v0(&policy);
        let analysis = trace.distillation_analysis_v0();
        let promotion = trace.promotion_manifest_v0(&analysis);
        let skill = trace.skill_promotion_manifest_v0(&analysis);
        let router = trace.router_promotion_manifest_v0(&analysis);
        let projection = trace.forge_projection_v0(&analysis, &promotion, &skill, &router, 4096);

        assert_eq!(projection.kind, "forge_projection_v0");
        assert!(!projection.raw_data_returned);
        assert_eq!(projection.bounded_preview_bytes, 4096);
        assert_eq!(projection.intent_hash, trace.intent_hash);
        assert_eq!(projection.hashes.command_hashes, trace.command_hashes);
    }

    #[test]
    fn rejects_duplicate_argument_keys() {
        let err = parse_forge_slash_v0("/forge run input=@latest input=@pending").unwrap_err();
        assert!(err.contains("duplicate"));
    }

    #[test]
    fn rejects_unknown_verbs_and_args() {
        let err = parse_forge_slash_v0("/forge delete everything=true").unwrap_err();
        assert!(err.contains("unknown ForgeSlash verb"));

        let err = parse_forge_slash_v0("/forge run shell=powershell").unwrap_err();
        assert!(err.contains("not valid"));
    }

    #[test]
    fn rejects_raw_paths_for_inputs() {
        let err = parse_forge_slash_v0(r#"/forge run input="C:\Users\quent\data.csv""#).unwrap_err();
        assert!(err.contains("input must be a bounded ref") || err.contains("raw filesystem paths"));
    }
}
