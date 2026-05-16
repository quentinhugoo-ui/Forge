use crate::forge_kasm_ledger::{
    forge_kasm_canonical_json, forge_kasm_hash_bytes, forge_kasm_hash_json, forge_kasm_record,
    forge_unix_ms, ForgeKasmLedgerRecordRequest,
};
use serde_json::{json, Value as JsonValue};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

pub fn forge_program_run_cached_with<F>(
    store_path: PathBuf,
    args: JsonValue,
    run_tool: F,
) -> Result<JsonValue, String>
where
    F: FnOnce(PathBuf, &str, JsonValue) -> Result<JsonValue, String>,
{
    let (cache_key_hash, cache_material) = forge_program_run_cache_material(&args)?;
    if let Some((cached, result_hash, result_path)) =
        forge_program_read_cached_response(&store_path, &cache_key_hash)
    {
        let ledger = forge_kasm_record(ForgeKasmLedgerRecordRequest {
            namespace: Some("program-run".to_string()),
            kind: "program.run.hit".to_string(),
            payload: Some(forge_program_run_cache_summary(&cache_material, &cache_key_hash)),
            summary: Some(json!({
                "cache": "hit",
                "resultHash": result_hash,
                "rawInputsStored": false,
            })),
            cache_key: Some(cache_key_hash.clone()),
            append_on_hit: Some(false),
        })
        .ok();
        let ledger_hash = ledger.as_ref().map(|item| item.entry_hash.as_str());
        let meta =
            forge_program_run_cache_meta(true, &cache_key_hash, &result_hash, ledger_hash, &result_path);
        return Ok(forge_program_annotate_cache(cached, meta));
    }

    let response = run_tool(store_path.clone(), "run", args)?;
    let result_hash = forge_kasm_hash_json(&response);
    let ledger = forge_kasm_record(ForgeKasmLedgerRecordRequest {
        namespace: Some("program-run".to_string()),
        kind: "program.run.store".to_string(),
        payload: Some(json!({
            "request": forge_program_run_cache_summary(&cache_material, &cache_key_hash),
            "resultHash": result_hash,
        })),
        summary: Some(json!({
            "cache": "miss",
            "resultHash": result_hash,
            "rawInputsStored": false,
        })),
        cache_key: Some(cache_key_hash.clone()),
        append_on_hit: Some(false),
    })
    .ok();
    let ledger_hash = ledger.as_ref().map(|item| item.entry_hash.as_str());
    let result_path = forge_program_write_cached_response(
        &store_path,
        &cache_key_hash,
        &cache_material,
        &response,
        &result_hash,
        ledger_hash,
    )?;
    let meta =
        forge_program_run_cache_meta(false, &cache_key_hash, &result_hash, ledger_hash, &result_path);
    Ok(forge_program_annotate_cache(response, meta))
}

fn forge_program_run_cache_dir(store_path: &Path) -> PathBuf {
    store_path
        .join("kasm-ledger")
        .join("program-run")
        .join("results")
}

fn forge_program_cache_hash_hex(hash: &str) -> String {
    let safe = hash
        .rsplit('/')
        .next()
        .unwrap_or(hash)
        .chars()
        .filter(|ch| ch.is_ascii_hexdigit())
        .take(80)
        .collect::<String>();
    if safe.is_empty() {
        "unknown".to_string()
    } else {
        safe
    }
}

fn forge_program_run_cache_path(store_path: &Path, cache_key_hash: &str) -> PathBuf {
    forge_program_run_cache_dir(store_path).join(format!(
        "{}.json",
        forge_program_cache_hash_hex(cache_key_hash)
    ))
}

fn forge_program_file_fingerprint(path: &str, role: &str) -> Result<JsonValue, String> {
    let path_hash = forge_kasm_hash_bytes(path.as_bytes());
    let file_path = PathBuf::from(path);
    let metadata = std::fs::metadata(&file_path)
        .map_err(|err| format!("read input metadata '{}': {err}", file_path.display()))?;
    if !metadata.is_file() {
        return Err(format!("program input is not a file: {}", file_path.display()));
    }
    let modified_ms = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0);
    let mut file = std::fs::File::open(&file_path)
        .map_err(|err| format!("open input '{}': {err}", file_path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 1024 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|err| format!("hash input '{}': {err}", file_path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    let file_hash = format!(
        "kasm://sha256/{}",
        hasher
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    );
    Ok(json!({
        "role": role,
        "pathHash": path_hash,
        "fileHash": file_hash,
        "bytes": metadata.len(),
        "modifiedMs": modified_ms,
    }))
}

fn forge_program_run_cache_material(args: &JsonValue) -> Result<(String, JsonValue), String> {
    let inputs = args
        .get("inputs")
        .and_then(JsonValue::as_array)
        .cloned()
        .unwrap_or_default();
    let mut input_fingerprints = Vec::new();
    for input in inputs {
        let role = input
            .get("role")
            .and_then(JsonValue::as_str)
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("data");
        if let Some(path) = input
            .get("path")
            .and_then(JsonValue::as_str)
            .filter(|value| !value.trim().is_empty())
        {
            input_fingerprints.push(forge_program_file_fingerprint(path, role)?);
        } else {
            input_fingerprints.push(json!({
                "role": role,
                "inlineInputHash": forge_kasm_hash_json(&input),
            }));
        }
    }
    let program_inline_hash = args
        .get("program")
        .and_then(JsonValue::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(|value| forge_kasm_hash_bytes(value.as_bytes()));
    let title_hash = args
        .get("title")
        .and_then(JsonValue::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(|value| forge_kasm_hash_bytes(value.as_bytes()));
    let material = forge_kasm_canonical_json(json!({
        "schema": "forge.program.run.cache.v1",
        "programHash": args.get("program_hash").and_then(JsonValue::as_str).unwrap_or(""),
        "programInlineHash": program_inline_hash,
        "titleHash": title_hash,
        "dryRun": args.get("dry_run").and_then(JsonValue::as_bool).unwrap_or(false),
        "planOnly": args.get("plan_only").and_then(JsonValue::as_bool).unwrap_or(false),
        "inputs": input_fingerprints,
    }));
    let cache_key_hash = forge_kasm_hash_json(&material);
    Ok((cache_key_hash, material))
}

fn forge_program_run_cache_summary(cache_material: &JsonValue, cache_key_hash: &str) -> JsonValue {
    let inputs = cache_material
        .get("inputs")
        .and_then(JsonValue::as_array)
        .cloned()
        .unwrap_or_default();
    json!({
        "cacheKeyHash": cache_key_hash,
        "programHash": cache_material.get("programHash").cloned().unwrap_or(JsonValue::Null),
        "programInlineHash": cache_material.get("programInlineHash").cloned().unwrap_or(JsonValue::Null),
        "inputCount": inputs.len(),
        "inputHashes": inputs
            .iter()
            .filter_map(|input| input.get("fileHash").or_else(|| input.get("inlineInputHash")).cloned())
            .collect::<Vec<_>>(),
        "dryRun": cache_material.get("dryRun").cloned().unwrap_or_else(|| json!(false)),
        "planOnly": cache_material.get("planOnly").cloned().unwrap_or_else(|| json!(false)),
        "rawInputsStored": false,
    })
}

fn forge_program_run_cache_meta(
    cache_hit: bool,
    cache_key_hash: &str,
    result_hash: &str,
    ledger_hash: Option<&str>,
    result_path: &Path,
) -> JsonValue {
    json!({
        "namespace": "program-run",
        "cacheHit": cache_hit,
        "cacheKeyHash": cache_key_hash,
        "resultHash": result_hash,
        "ledgerHash": ledger_hash.unwrap_or(""),
        "resultPath": result_path.display().to_string(),
        "contentAddressed": true,
        "rawInputsStored": false,
    })
}

fn forge_program_annotate_cache(mut response: JsonValue, meta: JsonValue) -> JsonValue {
    if let Some(obj) = response.as_object_mut() {
        obj.insert("kasmCache".to_string(), meta.clone());
        if let Some(summary) = obj.get_mut("summary").and_then(JsonValue::as_object_mut) {
            summary.insert("kasmCache".to_string(), meta);
        }
        response
    } else {
        json!({
            "result": response,
            "kasmCache": meta,
        })
    }
}

fn forge_program_read_cached_response(
    store_path: &Path,
    cache_key_hash: &str,
) -> Option<(JsonValue, String, PathBuf)> {
    let path = forge_program_run_cache_path(store_path, cache_key_hash);
    let raw = std::fs::read_to_string(&path).ok()?;
    let value = serde_json::from_str::<JsonValue>(&raw).ok()?;
    if value.get("cacheKeyHash").and_then(JsonValue::as_str) != Some(cache_key_hash) {
        return None;
    }
    let response = value.get("response")?.clone();
    let result_hash = value
        .get("resultHash")
        .and_then(JsonValue::as_str)
        .unwrap_or("")
        .to_string();
    Some((response, result_hash, path))
}

fn forge_program_write_cached_response(
    store_path: &Path,
    cache_key_hash: &str,
    cache_material: &JsonValue,
    response: &JsonValue,
    result_hash: &str,
    ledger_hash: Option<&str>,
) -> Result<PathBuf, String> {
    let dir = forge_program_run_cache_dir(store_path);
    std::fs::create_dir_all(&dir).map_err(|err| format!("program cache mkdir: {err}"))?;
    let path = forge_program_run_cache_path(store_path, cache_key_hash);
    let payload = json!({
        "schema": "forge.program.run.cache.v1",
        "cacheKeyHash": cache_key_hash,
        "resultHash": result_hash,
        "ledgerHash": ledger_hash.unwrap_or(""),
        "storedAtMs": forge_unix_ms(),
        "cacheMaterial": cache_material,
        "response": response,
    });
    let bytes = serde_json::to_vec_pretty(&payload)
        .map_err(|err| format!("program cache encode: {err}"))?;
    std::fs::write(&path, bytes).map_err(|err| format!("program cache write: {err}"))?;
    Ok(path)
}
