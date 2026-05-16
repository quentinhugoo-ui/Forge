use serde::{Deserialize, Serialize};
use serde_json::{json, Value as JsonValue};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::{BufRead, BufReader, Write},
    path::Path,
};

const JOB_LEDGER_FILE: &str = "forge_job_ledger.jsonl";

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForgeJobCost {
    pub estimate_units: u64,
    pub actual_units: u64,
    pub token_estimate: u64,
    pub budget_class: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForgeJobRetry {
    pub attempt: u32,
    pub max_attempts: u32,
    pub not_before_ms: u64,
    pub leased_until_ms: u64,
    pub next_retry_ms: u64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForgeJobProof {
    pub hash: String,
    pub artifact_path: String,
    pub source_hash: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForgeUnifiedJob {
    pub id: String,
    pub kind: String,
    pub payload: JsonValue,
    pub status: String,
    pub cost: ForgeJobCost,
    pub retry: ForgeJobRetry,
    pub proof: ForgeJobProof,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForgeJobLedgerEvent {
    pub event_id: String,
    pub event_type: String,
    pub at_ms: u64,
    pub job_id: String,
    pub kind: String,
    pub status: String,
    pub job: ForgeUnifiedJob,
    pub proof_hash: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct ForgeJobLedgerSnapshot {
    pub updated_at_ms: u64,
    pub total_events: usize,
    pub proof_hash: String,
    pub jobs: Vec<ForgeUnifiedJob>,
    pub recent_events: Vec<ForgeJobLedgerEvent>,
}

impl ForgeUnifiedJob {
    pub fn new(
        id: impl Into<String>,
        kind: impl Into<String>,
        payload: JsonValue,
        status: impl Into<String>,
        cost: ForgeJobCost,
        retry: ForgeJobRetry,
        proof: ForgeJobProof,
    ) -> Self {
        let mut job = Self {
            id: id.into(),
            kind: kind.into(),
            payload,
            status: status.into(),
            cost,
            retry,
            proof,
        };
        if job.proof.hash.trim().is_empty() {
            job.proof.hash = job.compute_proof_hash();
        }
        job
    }

    #[allow(dead_code)]
    pub fn from_manifest(
        job_id: &str,
        kind: &str,
        status: &str,
        manifest: &JsonValue,
        last_modified_ms: u64,
    ) -> Self {
        let context = manifest
            .get("context_accounting")
            .or_else(|| manifest.get("contextAccounting"))
            .cloned()
            .unwrap_or(JsonValue::Null);
        let estimated_tokens = context
            .get("estimated_tokens")
            .or_else(|| context.get("estimatedTokens"))
            .and_then(JsonValue::as_u64)
            .unwrap_or(0);
        let bytes = manifest
            .get("bytes")
            .and_then(JsonValue::as_u64)
            .unwrap_or(0);
        let cost = ForgeJobCost {
            estimate_units: estimated_tokens.max(bytes.div_ceil(4096)),
            actual_units: context
                .get("exact_tokens")
                .or_else(|| context.get("exactTokens"))
                .and_then(JsonValue::as_u64)
                .unwrap_or(0),
            token_estimate: estimated_tokens,
            budget_class: if estimated_tokens > 100_000 || bytes > 50_000_000 {
                "heavy".to_string()
            } else if estimated_tokens > 8_000 || bytes > 2_000_000 {
                "medium".to_string()
            } else {
                "light".to_string()
            },
        };
        let retry = ForgeJobRetry {
            attempt: manifest
                .get("attempt")
                .or_else(|| manifest.get("attempts"))
                .and_then(JsonValue::as_u64)
                .unwrap_or(0) as u32,
            max_attempts: manifest
                .get("max_attempts")
                .or_else(|| manifest.get("maxAttempts"))
                .and_then(JsonValue::as_u64)
                .unwrap_or(1) as u32,
            not_before_ms: manifest
                .get("not_before_ms")
                .or_else(|| manifest.get("notBeforeMs"))
                .and_then(JsonValue::as_u64)
                .unwrap_or(0),
            leased_until_ms: manifest
                .get("leased_until_ms")
                .or_else(|| manifest.get("leasedUntilMs"))
                .and_then(JsonValue::as_u64)
                .unwrap_or(0),
            next_retry_ms: manifest
                .get("next_retry_ms")
                .or_else(|| manifest.get("nextRetryMs"))
                .and_then(JsonValue::as_u64)
                .unwrap_or(0),
        };
        let proof = ForgeJobProof {
            hash: manifest
                .get("proof_hash")
                .or_else(|| manifest.get("proofHash"))
                .or_else(|| manifest.get("evidence_hash"))
                .or_else(|| manifest.get("evidenceHash"))
                .or_else(|| manifest.get("strategy_hash"))
                .or_else(|| manifest.get("strategyHash"))
                .and_then(JsonValue::as_str)
                .unwrap_or("")
                .to_string(),
            artifact_path: manifest
                .get("artifact_path")
                .or_else(|| manifest.get("artifactPath"))
                .or_else(|| manifest.get("file_path"))
                .and_then(JsonValue::as_str)
                .unwrap_or("")
                .to_string(),
            source_hash: manifest
                .get("file_hash")
                .or_else(|| manifest.get("fileHash"))
                .map(JsonValue::to_string)
                .unwrap_or_default(),
        };
        Self::new(
            job_id,
            kind,
            json!({
                "title": manifest.get("title").cloned().unwrap_or(JsonValue::Null),
                "source": manifest.get("source").cloned().unwrap_or(JsonValue::Null),
                "filePath": manifest.get("file_path").cloned().unwrap_or(JsonValue::Null),
                "fileCount": manifest.get("file_count").cloned().unwrap_or(JsonValue::Null),
                "lastModifiedMs": last_modified_ms,
            }),
            status,
            cost,
            retry,
            proof,
        )
    }

    fn compute_proof_hash(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(b"forge_unified_job:v1\n");
        hasher.update(self.id.as_bytes());
        hasher.update(b"\n");
        hasher.update(self.kind.as_bytes());
        hasher.update(b"\n");
        hasher.update(self.status.as_bytes());
        hasher.update(b"\n");
        hasher.update(self.payload.to_string().as_bytes());
        format!("{:x}", hasher.finalize())
    }
}

pub fn append_job_ledger_event(
    store_path: &Path,
    event_type: &str,
    at_ms: u64,
    job: &ForgeUnifiedJob,
) -> Result<ForgeJobLedgerEvent, String> {
    fs::create_dir_all(store_path).map_err(|e| format!("create job ledger dir: {e}"))?;
    let mut event = ForgeJobLedgerEvent {
        event_id: String::new(),
        event_type: event_type.to_string(),
        at_ms,
        job_id: job.id.clone(),
        kind: job.kind.clone(),
        status: job.status.clone(),
        job: job.clone(),
        proof_hash: String::new(),
    };
    event.proof_hash = hash_json("forge_job_ledger_event:v1", &serde_json::to_value(&event).unwrap_or(JsonValue::Null));
    event.event_id = format!("fje-{}-{}", at_ms, &event.proof_hash.chars().take(16).collect::<String>());
    let path = store_path.join(JOB_LEDGER_FILE);
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| format!("open job ledger '{}': {e}", path.display()))?;
    let line = serde_json::to_string(&event).map_err(|e| format!("encode job ledger event: {e}"))?;
    writeln!(file, "{line}").map_err(|e| format!("append job ledger event: {e}"))?;
    Ok(event)
}

#[allow(dead_code)]
pub fn recover_job_ledger(store_path: &Path, recent_limit: usize) -> Result<ForgeJobLedgerSnapshot, String> {
    let path = store_path.join(JOB_LEDGER_FILE);
    if !path.exists() {
        return Ok(ForgeJobLedgerSnapshot::default());
    }
    let file = fs::File::open(&path).map_err(|e| format!("open job ledger '{}': {e}", path.display()))?;
    let reader = BufReader::new(file);
    let mut jobs = BTreeMap::<String, ForgeUnifiedJob>::new();
    let mut recent = Vec::<ForgeJobLedgerEvent>::new();
    let mut total_events = 0usize;
    let mut updated_at_ms = 0u64;
    let mut proof_chain = String::new();
    for line in reader.lines() {
        let Ok(line) = line else {
            continue;
        };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(event) = serde_json::from_str::<ForgeJobLedgerEvent>(trimmed) else {
            continue;
        };
        total_events += 1;
        updated_at_ms = updated_at_ms.max(event.at_ms);
        proof_chain = hash_text("forge_job_ledger_chain:v1", &[&proof_chain, &event.proof_hash]);
        jobs.insert(event.job_id.clone(), event.job.clone());
        recent.push(event);
        if recent.len() > recent_limit {
            recent.remove(0);
        }
    }
    let mut jobs = jobs.into_values().collect::<Vec<_>>();
    jobs.sort_by(|a, b| a.id.cmp(&b.id));
    recent.reverse();
    Ok(ForgeJobLedgerSnapshot {
        updated_at_ms,
        total_events,
        proof_hash: proof_chain,
        jobs,
        recent_events: recent,
    })
}

fn hash_json(prefix: &str, value: &JsonValue) -> String {
    hash_text(prefix, &[&value.to_string()])
}

fn hash_text(prefix: &str, parts: &[&str]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix.as_bytes());
    for part in parts {
        hasher.update(b"\n");
        hasher.update(part.as_bytes());
    }
    format!("{:x}", hasher.finalize())
}
