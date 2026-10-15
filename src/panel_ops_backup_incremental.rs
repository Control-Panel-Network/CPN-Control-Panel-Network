//! Incremental backups via restic (preferred) or borg, targeting local/S3/SFTP.

use crate::panel_ops_backup_extra::load_destinations;
use crate::paths::join_data;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

// Re-export secret reader through a thin local helper (avoid exposing secrets module-wide).
mod secrets {
    use crate::paths::join_data;
    use std::fs;
    pub fn read(name: &str) -> Option<String> {
        fs::read_to_string(join_data("backup-secrets").join(name))
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct IncrementalJob {
    pub id: String,
    pub engine: String,
    pub source: String,
    pub destination: String,
    pub schedule_cron: String,
    pub enabled: bool,
    pub last_run_unix: u64,
    pub last_status: String,
}

fn jobs_path() -> PathBuf {
    join_data("backup-incremental-jobs.json")
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn load_jobs() -> Vec<IncrementalJob> {
    let Ok(raw) = fs::read_to_string(jobs_path()) else {
        return Vec::new();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

pub fn save_jobs(jobs: &[IncrementalJob]) -> Result<(), String> {
    if let Some(parent) = jobs_path().parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Cannot create data dir: {e}"))?;
    }
    let raw = serde_json::to_string_pretty(jobs).map_err(|e| e.to_string())?;
    fs::write(jobs_path(), raw).map_err(|e| format!("Cannot save incremental jobs: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(jobs_path(), fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

pub fn upsert_job(job: IncrementalJob) -> Result<String, String> {
    let mut jobs = load_jobs();
    if let Some(existing) = jobs.iter_mut().find(|j| j.id == job.id) {
        *existing = job;
    } else {
        jobs.push(job);
    }
    save_jobs(&jobs)?;
    Ok("Incremental backup job saved.".into())
}

fn which(bin: &str) -> bool {
    #[cfg(windows)]
    {
        Command::new("where")
            .arg(bin)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
    #[cfg(not(windows))]
    {
        Command::new("sh")
            .args(["-c", &format!("command -v {bin} >/dev/null 2>&1")])
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
}

fn restic_repo_for_destination(dest: &str) -> Result<(String, Vec<(String, String)>), String> {
    let d = load_destinations();
    match dest.trim().to_ascii_lowercase().as_str() {
        "local" => {
            let repo = join_data("restic-repo");
            fs::create_dir_all(&repo).map_err(|e| format!("Cannot create restic repo: {e}"))?;
            Ok((format!("local:{}", repo.display()), Vec::new()))
        }
        "s3" => {
            if !d.s3.enabled {
                return Err("S3 destination is not enabled.".into());
            }
            let access = secrets::read("s3_access_key").ok_or("S3 access key missing")?;
            let secret = secrets::read("s3_secret_key").ok_or("S3 secret key missing")?;
            let endpoint = d.s3.endpoint.trim().trim_end_matches('/');
            let bucket = d.s3.bucket.trim();
            let prefix = d.s3.prefix.trim().trim_matches('/');
            let path = if prefix.is_empty() {
                format!("s3:{endpoint}/{bucket}")
            } else {
                format!("s3:{endpoint}/{bucket}/{prefix}")
            };
            Ok((
                path,
                vec![
                    ("AWS_ACCESS_KEY_ID".into(), access),
                    ("AWS_SECRET_ACCESS_KEY".into(), secret),
                ],
            ))
        }
        "sftp" | "remote" => {
            if !d.remote.enabled {
                return Err("Remote destination is not enabled.".into());
            }
            let host = d.remote.host.trim();
            let user = d.remote.username.trim();
            let path = d.remote.remote_path.trim().trim_end_matches('/');
            Ok((format!("sftp:{user}@{host}:{path}"), Vec::new()))
        }
        other => Err(format!(
            "Unknown destination `{other}` (use local, s3, or sftp)."
        )),
    }
}

pub fn run_incremental_job(job_id: &str) -> Result<String, String> {
    let mut jobs = load_jobs();
    let job = jobs
        .iter_mut()
        .find(|j| j.id == job_id)
        .ok_or_else(|| "Incremental job not found.".to_string())?;
    let engine = job.engine.to_ascii_lowercase();
    let source = job.source.clone();
    let dest = job.destination.clone();
    if engine == "restic" || engine.is_empty() {
        if !which("restic") {
            return Err("restic is not installed on this host.".into());
        }
        let (repo, envs) = restic_repo_for_destination(&dest)?;
        let password = secrets::read("restic_password").unwrap_or_else(|| {
            let generated = format!("cpn-{}", now_unix());
            let _ = fs::create_dir_all(join_data("backup-secrets"));
            let _ = fs::write(
                join_data("backup-secrets").join("restic_password"),
                &generated,
            );
            generated
        });
        let mut init = Command::new("restic");
        init.args(["-r", &repo, "snapshots"]);
        init.env("RESTIC_PASSWORD", &password);
        for (k, v) in &envs {
            init.env(k, v);
        }
        let _ = init.output();
        let mut cmd = Command::new("restic");
        cmd.args(["-r", &repo, "backup", &source]);
        cmd.env("RESTIC_PASSWORD", &password);
        for (k, v) in &envs {
            cmd.env(k, v);
        }
        let output = cmd
            .output()
            .map_err(|e| format!("restic backup failed to start: {e}"))?;
        job.last_run_unix = now_unix();
        if output.status.success() {
            job.last_status = "ok".into();
            save_jobs(&jobs)?;
            return Ok(format!("restic backup of `{source}` completed."));
        }
        let err = String::from_utf8_lossy(&output.stderr);
        job.last_status = "error".into();
        save_jobs(&jobs)?;
        return Err(format!(
            "restic backup failed: {}",
            err.chars().take(180).collect::<String>()
        ));
    }
    if engine == "borg" {
        if !which("borg") {
            return Err("borg is not installed on this host.".into());
        }
        return Err("borg jobs are configured; install borg and set BORG_REPO via destination notes for restore UI.".into());
    }
    Err("Unsupported engine. Use restic or borg.".into())
}

pub fn list_restic_snapshots(destination: &str) -> Result<String, String> {
    if !which("restic") {
        return Err("restic is not installed.".into());
    }
    let (repo, envs) = restic_repo_for_destination(destination)?;
    let password = secrets::read("restic_password").ok_or("restic password missing")?;
    let mut cmd = Command::new("restic");
    cmd.args(["-r", &repo, "snapshots"]);
    cmd.env("RESTIC_PASSWORD", password);
    for (k, v) in &envs {
        cmd.env(k, v);
    }
    let output = cmd
        .output()
        .map_err(|e| format!("restic snapshots failed: {e}"))?;
    let text = String::from_utf8_lossy(&output.stdout).to_string();
    if text.trim().is_empty() {
        Ok("(no snapshots)".into())
    } else {
        Ok(text.chars().take(8000).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;

    #[test]
    fn job_roundtrip() {
        with_test_data_dir(|| {
            let job = IncrementalJob {
                id: "nightly".into(),
                engine: "restic".into(),
                source: "/home".into(),
                destination: "local".into(),
                schedule_cron: "0 2 * * *".into(),
                enabled: true,
                last_run_unix: 0,
                last_status: String::new(),
            };
            upsert_job(job).unwrap();
            let loaded = load_jobs();
            assert_eq!(loaded.len(), 1);
            assert_eq!(loaded[0].id, "nightly");
        });
    }
}
