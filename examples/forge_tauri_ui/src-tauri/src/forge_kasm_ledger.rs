use crate::forge_store_dir;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as JsonValue};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const FORGE_KASM_LEDGER_SCHEMA: &str = "forge.kasm.ledger.v1";
const FORGE_KASM_LEDGER_PAYLOAD_MAX_BYTES: usize = 256 * 1024;

pub fn forge_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForgeKasmLedgerRecordRequest {
    pub namespace: Option<String>,
    pub kind: String,
    pub payload: Option<JsonValue>,
    pub summary: Option<JsonValue>,
    pub cache_key: Option<String>,
    pub append_on_hit: Option<bool>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ForgeKasmLedgerRecordResult {
    pub namespace: String,
    pub kind: String,
    pub entry_hash: String,
    pub prev_hash: Option<String>,
    pub payload_hash: String,
    pub cache_key_hash: String,
    pub cache_hit: bool,
    pub timestamp_ms: u64,
    pub ledger_path: String,
    pub latest_path: String,
}

pub fn forge_kasm_hash_bytes(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let hex = digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("kasm://sha256/{hex}")
}

pub fn forge_kasm_canonical_json(value: JsonValue) -> JsonValue {
    match value {
        JsonValue::Array(items) => JsonValue::Array(
            items
                .into_iter()
                .map(forge_kasm_canonical_json)
                .collect::<Vec<_>>(),
        ),
        JsonValue::Object(map) => {
            let mut sorted = BTreeMap::new();
            for (key, value) in map {
                sorted.insert(key, forge_kasm_canonical_json(value));
            }
            let mut canonical = serde_json::Map::new();
            for (key, value) in sorted {
                canonical.insert(key, value);
            }
            JsonValue::Object(canonical)
        }
        other => other,
    }
}

pub fn forge_kasm_hash_json(value: &JsonValue) -> String {
    let canonical = forge_kasm_canonical_json(value.clone());
    let bytes = serde_json::to_vec(&canonical).unwrap_or_else(|_| b"null".to_vec());
    forge_kasm_hash_bytes(&bytes)
}

fn forge_kasm_safe_namespace(raw: Option<&str>) -> String {
    let candidate = raw
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("app");
    let safe = candidate
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    let trimmed = safe.trim_matches('_').chars().take(80).collect::<String>();
    if trimmed.is_empty() {
        "app".to_string()
    } else {
        trimmed
    }
}

fn forge_kasm_ledger_dir(namespace: &str) -> PathBuf {
    forge_store_dir()
        .join("kasm-ledger")
        .join(forge_kasm_safe_namespace(Some(namespace)))
}

fn forge_kasm_ledger_index_path(namespace: &str) -> PathBuf {
    forge_kasm_ledger_dir(namespace).join("index.json")
}

fn forge_kasm_ledger_latest_path(namespace: &str) -> PathBuf {
    forge_kasm_ledger_dir(namespace).join("latest.json")
}

fn forge_kasm_ledger_jsonl_path(namespace: &str) -> PathBuf {
    forge_kasm_ledger_dir(namespace).join("ledger.jsonl")
}

fn forge_kasm_read_json_object(path: &Path) -> serde_json::Map<String, JsonValue> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|raw| serde_json::from_str::<JsonValue>(&raw).ok())
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default()
}

fn forge_kasm_ledger_prev_hash(namespace: &str) -> Option<String> {
    forge_kasm_read_json_object(&forge_kasm_ledger_latest_path(namespace))
        .get("entryHash")
        .and_then(JsonValue::as_str)
        .map(str::to_string)
}

fn forge_kasm_ledger_cache_hit(
    namespace: &str,
    cache_key_hash: &str,
) -> Option<ForgeKasmLedgerRecordResult> {
    let index = forge_kasm_read_json_object(&forge_kasm_ledger_index_path(namespace));
    let hit = index.get(cache_key_hash)?.as_object()?;
    Some(ForgeKasmLedgerRecordResult {
        namespace: namespace.to_string(),
        kind: hit
            .get("kind")
            .and_then(JsonValue::as_str)
            .unwrap_or("action")
            .to_string(),
        entry_hash: hit
            .get("entryHash")
            .and_then(JsonValue::as_str)
            .unwrap_or_default()
            .to_string(),
        prev_hash: hit
            .get("prevHash")
            .and_then(JsonValue::as_str)
            .map(str::to_string),
        payload_hash: hit
            .get("payloadHash")
            .and_then(JsonValue::as_str)
            .unwrap_or_default()
            .to_string(),
        cache_key_hash: cache_key_hash.to_string(),
        cache_hit: true,
        timestamp_ms: hit
            .get("timestampMs")
            .and_then(JsonValue::as_u64)
            .unwrap_or(0),
        ledger_path: forge_kasm_ledger_jsonl_path(namespace).display().to_string(),
        latest_path: forge_kasm_ledger_latest_path(namespace).display().to_string(),
    })
}

#[tauri::command]
pub fn forge_kasm_record(
    request: ForgeKasmLedgerRecordRequest,
) -> Result<ForgeKasmLedgerRecordResult, String> {
    let namespace = forge_kasm_safe_namespace(request.namespace.as_deref());
    let kind = request.kind.trim().chars().take(120).collect::<String>();
    let kind = if kind.is_empty() { "action".to_string() } else { kind };
    let payload = forge_kasm_canonical_json(request.payload.unwrap_or(JsonValue::Null));
    let payload_bytes =
        serde_json::to_vec(&payload).map_err(|err| format!("kasm payload encode: {err}"))?;
    if payload_bytes.len() > FORGE_KASM_LEDGER_PAYLOAD_MAX_BYTES {
        return Err(format!(
            "kasm payload too large for UI ledger: {} bytes > {} bytes",
            payload_bytes.len(),
            FORGE_KASM_LEDGER_PAYLOAD_MAX_BYTES
        ));
    }
    let payload_hash = forge_kasm_hash_bytes(&payload_bytes);
    let cache_material = request.cache_key.unwrap_or_else(|| {
        serde_json::to_string(&json!({
            "schema": FORGE_KASM_LEDGER_SCHEMA,
            "namespace": &namespace,
            "kind": &kind,
            "payloadHash": &payload_hash,
        }))
        .unwrap_or_default()
    });
    let cache_key_hash = forge_kasm_hash_bytes(cache_material.as_bytes());
    if request.append_on_hit != Some(true) {
        if let Some(hit) = forge_kasm_ledger_cache_hit(&namespace, &cache_key_hash) {
            return Ok(hit);
        }
    }

    let dir = forge_kasm_ledger_dir(&namespace);
    std::fs::create_dir_all(&dir).map_err(|err| format!("kasm ledger mkdir: {err}"))?;
    let prev_hash = forge_kasm_ledger_prev_hash(&namespace);
    let timestamp_ms = forge_unix_ms();
    let summary = forge_kasm_canonical_json(request.summary.unwrap_or(JsonValue::Null));
    let preimage = json!({
        "schema": FORGE_KASM_LEDGER_SCHEMA,
        "namespace": &namespace,
        "kind": &kind,
        "timestampMs": timestamp_ms,
        "prevHash": &prev_hash,
        "payloadHash": &payload_hash,
        "cacheKeyHash": &cache_key_hash,
        "summary": summary,
    });
    let entry_hash = forge_kasm_hash_json(&preimage);
    let entry = json!({
        "schema": FORGE_KASM_LEDGER_SCHEMA,
        "namespace": &namespace,
        "kind": &kind,
        "timestampMs": timestamp_ms,
        "prevHash": preimage.get("prevHash").cloned().unwrap_or(JsonValue::Null),
        "entryHash": &entry_hash,
        "payloadHash": &payload_hash,
        "cacheKeyHash": &cache_key_hash,
        "summary": preimage.get("summary").cloned().unwrap_or(JsonValue::Null),
    });
    let ledger_path = forge_kasm_ledger_jsonl_path(&namespace);
    let latest_path = forge_kasm_ledger_latest_path(&namespace);
    let mut ledger = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&ledger_path)
        .map_err(|err| format!("kasm ledger open: {err}"))?;
    let entry_line = serde_json::to_vec(&entry).map_err(|err| format!("kasm ledger encode: {err}"))?;
    ledger
        .write_all(&entry_line)
        .and_then(|_| ledger.write_all(b"\n"))
        .map_err(|err| format!("kasm ledger append: {err}"))?;
    let pretty = serde_json::to_vec_pretty(&entry).map_err(|err| format!("kasm latest encode: {err}"))?;
    std::fs::write(&latest_path, pretty).map_err(|err| format!("kasm latest write: {err}"))?;

    let mut index = forge_kasm_read_json_object(&forge_kasm_ledger_index_path(&namespace));
    index.insert(
        cache_key_hash.clone(),
        json!({
            "kind": &kind,
            "entryHash": &entry_hash,
            "prevHash": &prev_hash,
            "payloadHash": &payload_hash,
            "timestampMs": timestamp_ms,
        }),
    );
    let index_value = JsonValue::Object(index);
    let index_bytes =
        serde_json::to_vec_pretty(&index_value).map_err(|err| format!("kasm index encode: {err}"))?;
    std::fs::write(forge_kasm_ledger_index_path(&namespace), index_bytes)
        .map_err(|err| format!("kasm index write: {err}"))?;

    Ok(ForgeKasmLedgerRecordResult {
        namespace,
        kind,
        entry_hash,
        prev_hash,
        payload_hash,
        cache_key_hash,
        cache_hit: false,
        timestamp_ms,
        ledger_path: ledger_path.display().to_string(),
        latest_path: latest_path.display().to_string(),
    })
}
