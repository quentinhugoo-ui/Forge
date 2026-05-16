use serde_json::{json, Value as JsonValue};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

pub fn forge_mcp_jobs_dir_candidate() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("FORGE_JOBS_DIR") {
        let path = PathBuf::from(path);
        if !path.as_os_str().is_empty() {
            return Some(path);
        }
    }
    if let Some(path) = std::env::var_os("FORGE_STORE_DIR") {
        let store = PathBuf::from(path);
        if !store.as_os_str().is_empty() {
            return Some(store.join("jobs"));
        }
    }
    None
}

pub fn forge_job_mirror_dirs(primary_jobs_dir: &Path) -> Vec<PathBuf> {
    let mut dirs = vec![primary_jobs_dir.to_path_buf()];
    if let Some(mirror) = forge_mcp_jobs_dir_candidate() {
        if mirror != primary_jobs_dir {
            dirs.push(mirror);
        }
    }
    dirs
}

pub fn write_forge_job_manifest_to_dirs(
    job_id: &str,
    manifest_bytes: &[u8],
    job_dirs: &[PathBuf],
) -> Result<(), String> {
    for jobs_dir in job_dirs {
        std::fs::create_dir_all(jobs_dir)
            .map_err(|e| format!("create Forge jobs dir '{}': {e}", jobs_dir.display()))?;
        let manifest_path = jobs_dir.join(format!("{job_id}.json"));
        let manifest_tmp = manifest_path.with_extension("json.tmp");
        std::fs::write(&manifest_tmp, manifest_bytes)
            .map_err(|e| format!("write Forge job manifest tmp '{}': {e}", manifest_tmp.display()))?;
        std::fs::rename(&manifest_tmp, &manifest_path)
            .map_err(|e| format!("commit Forge job manifest '{}': {e}", manifest_path.display()))?;
    }
    Ok(())
}

pub fn write_forge_job_log_to_dirs(
    job_id: &str,
    log_text: &str,
    job_dirs: &[PathBuf],
) -> Result<(), String> {
    for jobs_dir in job_dirs {
        std::fs::create_dir_all(jobs_dir)
            .map_err(|e| format!("create Forge jobs dir '{}': {e}", jobs_dir.display()))?;
        let log_path = jobs_dir.join(format!("{job_id}.log"));
        std::fs::write(&log_path, log_text)
            .map_err(|e| format!("write Forge job log '{}': {e}", log_path.display()))?;
    }
    Ok(())
}

pub fn read_forge_job_manifest_from_dirs(
    job_id: &str,
    job_dirs: &[PathBuf],
) -> Result<(PathBuf, Vec<u8>), String> {
    let mut last_err = None;
    for jobs_dir in job_dirs {
        let manifest_path = jobs_dir.join(format!("{job_id}.json"));
        match std::fs::read(&manifest_path) {
            Ok(bytes) => return Ok((manifest_path, bytes)),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => last_err = Some(format!("read Forge job manifest '{}': {err}", manifest_path.display())),
        }
    }
    Err(last_err.unwrap_or_else(|| {
        format!("Forge job manifest '{job_id}' was not found in any protected jobs directory")
    }))
}

pub fn read_forge_job_log_from_dirs(job_id: &str, job_dirs: &[PathBuf]) -> Result<String, String> {
    let mut last_err = None;
    for jobs_dir in job_dirs {
        let log_path = jobs_dir.join(format!("{job_id}.log"));
        match std::fs::read_to_string(&log_path) {
            Ok(text) => return Ok(text),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => last_err = Some(format!("read Forge job log '{}': {err}", log_path.display())),
        }
    }
    if let Some(err) = last_err {
        Err(err)
    } else {
        Ok(String::new())
    }
}

pub fn read_forge_job_log_tail_from_dirs(
    job_id: &str,
    job_dirs: &[PathBuf],
    cursor: u64,
    max_bytes: u64,
) -> Result<JsonValue, String> {
    let mut last_err = None;
    let bounded_max = max_bytes.clamp(1, 262_144) as usize;
    for jobs_dir in job_dirs {
        let log_path = jobs_dir.join(format!("{job_id}.log"));
        let metadata = match std::fs::metadata(&log_path) {
            Ok(metadata) => metadata,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => continue,
            Err(err) => {
                last_err = Some(format!("stat Forge job log '{}': {err}", log_path.display()));
                continue;
            }
        };
        let size = metadata.len();
        let start = if cursor <= size { cursor } else { 0 };
        let mut file = std::fs::File::open(&log_path)
            .map_err(|err| format!("open Forge job log '{}': {err}", log_path.display()))?;
        file.seek(SeekFrom::Start(start))
            .map_err(|err| format!("seek Forge job log '{}': {err}", log_path.display()))?;
        let remaining = size.saturating_sub(start).min(bounded_max as u64) as usize;
        let mut bytes = vec![0_u8; remaining];
        let read = file
            .read(&mut bytes)
            .map_err(|err| format!("read Forge job log '{}': {err}", log_path.display()))?;
        bytes.truncate(read);
        let text = String::from_utf8_lossy(&bytes).to_string();
        return Ok(json!({
            "text": text,
            "cursor": start + read as u64,
            "size": size,
            "reset": start == 0 && cursor > size,
        }));
    }
    if let Some(err) = last_err {
        Err(err)
    } else {
        Ok(json!({
            "text": "",
            "cursor": 0_u64,
            "size": 0_u64,
            "reset": cursor > 0,
        }))
    }
}
