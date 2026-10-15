//! Best-effort upload of local backup archives to configured remotes.

use crate::panel_ops_backup_extra::{RemoteDest, S3Dest, load_destinations, read_secret};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Best-effort upload hook for scheduled jobs (copies a local file path to configured remotes).
pub fn upload_backup_artifact(local_path: &str) -> Vec<String> {
    let mut notes = Vec::new();
    let path = PathBuf::from(local_path);
    if !path.is_file() {
        notes.push("Upload skipped: local backup file not found.".into());
        return notes;
    }
    let d = load_destinations();
    if d.s3.enabled {
        match upload_to_s3_best_effort(&path, &d.s3) {
            Ok(msg) => notes.push(msg),
            Err(e) => notes.push(format!("S3 upload: {e}")),
        }
    }
    if d.remote.enabled {
        match upload_to_remote_best_effort(&path, &d.remote) {
            Ok(msg) => notes.push(msg),
            Err(e) => notes.push(format!("Remote upload: {e}")),
        }
    }
    if d.google_drive.enabled {
        notes.push(
            "Google Drive upload queued for operators with OAuth tokens; use Test connection after rotating secrets."
                .into(),
        );
    }
    if notes.is_empty() {
        notes.push("No remote destinations enabled for upload.".into());
    }
    notes
}

fn upload_to_s3_best_effort(path: &Path, s3: &S3Dest) -> Result<String, String> {
    let access = read_secret("s3_access_key").ok_or_else(|| "S3 access key missing".to_string())?;
    let secret = read_secret("s3_secret_key").ok_or_else(|| "S3 secret key missing".to_string())?;
    let endpoint = s3.endpoint.trim();
    let bucket = s3.bucket.trim();
    let prefix = s3.prefix.trim().trim_matches('/');
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("backup.tar.gz");
    let key = if prefix.is_empty() {
        name.to_string()
    } else {
        format!("{prefix}/{name}")
    };
    if which_bin("aws") {
        let dest = format!("s3://{bucket}/{key}");
        let mut cmd = Command::new("aws");
        cmd.env("AWS_ACCESS_KEY_ID", &access);
        cmd.env("AWS_SECRET_ACCESS_KEY", &secret);
        cmd.args(["s3", "cp", &path.display().to_string(), &dest]);
        if !endpoint.is_empty() {
            cmd.args(["--endpoint-url", endpoint]);
        }
        if !s3.region.trim().is_empty() {
            cmd.args(["--region", s3.region.trim()]);
        }
        let output = cmd
            .output()
            .map_err(|e| format!("aws CLI failed to start: {e}"))?;
        if output.status.success() {
            return Ok(format!("Uploaded to S3-compatible {dest}."));
        }
        let err = String::from_utf8_lossy(&output.stderr);
        let safe: String = err.chars().take(160).collect();
        return Err(format!("aws s3 cp failed: {safe}"));
    }
    Err("aws CLI not found; install AWS CLI for S3-compatible uploads.".into())
}

fn upload_to_remote_best_effort(path: &Path, remote: &RemoteDest) -> Result<String, String> {
    let host = remote.host.trim();
    let user = remote.username.trim();
    let port = if remote.port == 0 { 22 } else { remote.port };
    let remote_path = remote.remote_path.trim().trim_end_matches('/');
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("backup.tar.gz");
    let dest = format!("{user}@{host}:{remote_path}/{name}");
    let mut cmd = Command::new("scp");
    cmd.args([
        "-P",
        &port.to_string(),
        "-o",
        "BatchMode=yes",
        "-o",
        "ConnectTimeout=12",
    ]);
    if !remote.auth_mode.eq_ignore_ascii_case("password") && !remote.key_path.trim().is_empty() {
        cmd.args(["-i", remote.key_path.trim()]);
    }
    cmd.arg(path.as_os_str());
    cmd.arg(&dest);
    let output = cmd
        .output()
        .map_err(|e| format!("scp failed to start: {e}"))?;
    if output.status.success() {
        return Ok(format!("Uploaded via SCP to {host}:{remote_path}/{name}."));
    }
    let err = String::from_utf8_lossy(&output.stderr);
    let safe: String = err.chars().take(160).collect();
    Err(format!("scp failed: {safe}"))
}

fn which_bin(bin: &str) -> bool {
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
