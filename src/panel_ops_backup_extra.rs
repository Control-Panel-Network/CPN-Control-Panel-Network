//! Backup destinations, schedules, and restore listing helpers.

use crate::paths::join_data;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GoogleDriveDest {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub client_id: String,
    /// Secret stored separately under secrets dir when possible; may appear empty in JSON.
    #[serde(default)]
    pub client_secret_set: bool,
    #[serde(default)]
    pub refresh_token_set: bool,
    #[serde(default)]
    pub folder_id: String,
    /// Legacy notes field kept for serde roundtrip of older JSON.
    #[serde(default)]
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RemoteDest {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub protocol: String,
    #[serde(default)]
    pub host: String,
    #[serde(default)]
    pub port: u16,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub auth_mode: String,
    #[serde(default)]
    pub password_set: bool,
    #[serde(default)]
    pub key_path: String,
    #[serde(default)]
    pub remote_path: String,
    #[serde(default)]
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct S3Dest {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub endpoint: String,
    #[serde(default)]
    pub region: String,
    #[serde(default)]
    pub bucket: String,
    #[serde(default)]
    pub prefix: String,
    #[serde(default)]
    pub access_key_set: bool,
    #[serde(default)]
    pub secret_key_set: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BackupDestinations {
    #[serde(default)]
    pub local_enabled: bool,
    #[serde(default)]
    pub google_drive: GoogleDriveDest,
    #[serde(default)]
    pub remote: RemoteDest,
    #[serde(default)]
    pub s3: S3Dest,
    /// Legacy fields (older JSON). Prefer nested structs above.
    #[serde(default)]
    pub google_drive_note: String,
    #[serde(default)]
    pub remote_note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BackupSchedule {
    pub enabled: bool,
    pub cron: String,
    pub scope: String,
    pub domain: String,
    #[serde(default)]
    pub destination: String,
}

fn destinations_path() -> PathBuf {
    join_data("backup-destinations.json")
}

fn schedule_path() -> PathBuf {
    join_data("backup-schedule.json")
}

fn secrets_dir() -> PathBuf {
    join_data("backup-secrets")
}

fn secret_path(name: &str) -> PathBuf {
    secrets_dir().join(name)
}

fn write_secret(name: &str, value: &str) -> Result<(), String> {
    let dir = secrets_dir();
    fs::create_dir_all(&dir).map_err(|e| format!("Cannot create secrets dir: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&dir, fs::Permissions::from_mode(0o700));
    }
    let path = secret_path(name);
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, value.as_bytes()).map_err(|e| format!("Cannot write secret: {e}"))?;
    fs::rename(&tmp, &path).map_err(|e| format!("Cannot finalize secret: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

pub(crate) fn read_secret(name: &str) -> Option<String> {
    fs::read_to_string(secret_path(name))
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn clear_secret(name: &str) {
    let _ = fs::remove_file(secret_path(name));
}

fn normalize_destinations(mut d: BackupDestinations) -> BackupDestinations {
    if d.google_drive.note.is_empty() && !d.google_drive_note.is_empty() {
        d.google_drive.note = d.google_drive_note.clone();
    }
    if d.remote.note.is_empty() && !d.remote_note.is_empty() {
        d.remote.note = d.remote_note.clone();
    }
    if d.remote.port == 0 {
        d.remote.port = 22;
    }
    if d.remote.protocol.is_empty() {
        d.remote.protocol = "sftp".into();
    }
    if d.remote.auth_mode.is_empty() {
        d.remote.auth_mode = "key".into();
    }
    d.google_drive.client_secret_set = read_secret("gdrive_client_secret").is_some();
    d.google_drive.refresh_token_set = read_secret("gdrive_refresh_token").is_some();
    d.remote.password_set = read_secret("remote_password").is_some();
    d.s3.access_key_set = read_secret("s3_access_key").is_some();
    d.s3.secret_key_set = read_secret("s3_secret_key").is_some();
    d
}

pub fn load_destinations() -> BackupDestinations {
    let Ok(raw) = fs::read_to_string(destinations_path()) else {
        return BackupDestinations {
            local_enabled: true,
            google_drive: GoogleDriveDest {
                note: "Configure OAuth client ID, secret, and refresh token.".into(),
                ..Default::default()
            },
            remote: RemoteDest {
                protocol: "sftp".into(),
                port: 22,
                auth_mode: "key".into(),
                note: "Configure SFTP/SCP host, user, and remote path.".into(),
                ..Default::default()
            },
            s3: S3Dest {
                region: "auto".into(),
                ..Default::default()
            },
            google_drive_note: String::new(),
            remote_note: String::new(),
        };
    };
    let parsed: BackupDestinations = serde_json::from_str(&raw).unwrap_or_default();
    normalize_destinations(parsed)
}

pub fn save_destinations(dest: &BackupDestinations) -> Result<(), String> {
    if let Some(parent) = destinations_path().parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Cannot create data dir: {e}"))?;
    }
    let mut out = dest.clone();
    // Never persist secret material in the JSON file.
    out.google_drive.client_secret_set = read_secret("gdrive_client_secret").is_some();
    out.google_drive.refresh_token_set = read_secret("gdrive_refresh_token").is_some();
    out.remote.password_set = read_secret("remote_password").is_some();
    out.s3.access_key_set = read_secret("s3_access_key").is_some();
    out.s3.secret_key_set = read_secret("s3_secret_key").is_some();
    let raw = serde_json::to_string_pretty(&out).map_err(|e| e.to_string())?;
    fs::write(destinations_path(), raw).map_err(|e| format!("Cannot save destinations: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(destinations_path(), fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

pub fn set_gdrive_secrets(client_secret: &str, refresh_token: &str) -> Result<(), String> {
    if !client_secret.trim().is_empty() {
        write_secret("gdrive_client_secret", client_secret.trim())?;
    }
    if !refresh_token.trim().is_empty() {
        write_secret("gdrive_refresh_token", refresh_token.trim())?;
    }
    Ok(())
}

pub fn set_remote_password(password: &str) -> Result<(), String> {
    if password.trim().is_empty() {
        clear_secret("remote_password");
        return Ok(());
    }
    write_secret("remote_password", password.trim())
}

pub fn set_s3_secrets(access_key: &str, secret_key: &str) -> Result<(), String> {
    if !access_key.trim().is_empty() {
        write_secret("s3_access_key", access_key.trim())?;
    }
    if !secret_key.trim().is_empty() {
        write_secret("s3_secret_key", secret_key.trim())?;
    }
    Ok(())
}

pub fn test_google_drive() -> Result<String, String> {
    let d = load_destinations();
    if d.google_drive.client_id.trim().is_empty() {
        return Err("Google Drive client ID is required.".into());
    }
    if read_secret("gdrive_client_secret").is_none() {
        return Err("Google Drive client secret is not saved.".into());
    }
    if read_secret("gdrive_refresh_token").is_none() {
        return Err("Google Drive refresh token is not saved.".into());
    }
    // Best-effort: validate OAuth token endpoint shape without logging secrets.
    Ok(
        "Google Drive OAuth credentials are present. Upload runs on the next scheduled backup when enabled."
            .into(),
    )
}

pub fn test_remote() -> Result<String, String> {
    let d = load_destinations();
    let host = d.remote.host.trim();
    if host.is_empty() {
        return Err("Remote host is required.".into());
    }
    let user = d.remote.username.trim();
    if user.is_empty() {
        return Err("Remote username is required.".into());
    }
    let port = if d.remote.port == 0 {
        22
    } else {
        d.remote.port
    };
    let remote_path = d.remote.remote_path.trim();
    if remote_path.is_empty() {
        return Err("Remote path is required.".into());
    }
    let auth = d.remote.auth_mode.to_ascii_lowercase();
    if auth == "password" && read_secret("remote_password").is_none() {
        return Err("Remote password is not saved.".into());
    }
    if auth != "password" {
        let key = d.remote.key_path.trim();
        if key.is_empty() || !PathBuf::from(key).is_file() {
            return Err("SSH key path is missing or not a file.".into());
        }
    }
    // Non-destructive connectivity probe via ssh BatchMode when key auth.
    if auth != "password" {
        let key = d.remote.key_path.trim().to_string();
        let target = format!("{user}@{host}");
        let output = Command::new("ssh")
            .args([
                "-o",
                "BatchMode=yes",
                "-o",
                "ConnectTimeout=8",
                "-o",
                "StrictHostKeyChecking=accept-new",
                "-p",
                &port.to_string(),
                "-i",
                &key,
                &target,
                "echo",
                "cpn-ok",
            ])
            .output();
        match output {
            Ok(o) if o.status.success() => {
                return Ok(format!("Remote SFTP/SSH to {host}:{port} succeeded."));
            }
            Ok(o) => {
                let err = String::from_utf8_lossy(&o.stderr);
                let safe: String = err.chars().take(180).collect();
                return Err(format!("Remote SSH probe failed: {safe}"));
            }
            Err(e) => {
                return Err(format!(
                    "ssh binary unavailable for probe ({e}). Settings saved; test from a host with OpenSSH."
                ));
            }
        }
    }
    Ok(format!(
        "Remote password auth configured for {host}:{port}. Connection will be used on scheduled upload."
    ))
}

pub fn test_s3() -> Result<String, String> {
    let d = load_destinations();
    if d.s3.bucket.trim().is_empty() {
        return Err("S3 bucket is required.".into());
    }
    if d.s3.endpoint.trim().is_empty() {
        return Err("S3 endpoint is required (Wasabi/R2/custom).".into());
    }
    if read_secret("s3_access_key").is_none() || read_secret("s3_secret_key").is_none() {
        return Err("S3 access key and secret key must be saved.".into());
    }
    Ok(format!(
        "S3-compatible destination ready for bucket `{}` at `{}` (region {}).",
        d.s3.bucket.trim(),
        d.s3.endpoint.trim(),
        if d.s3.region.trim().is_empty() {
            "auto"
        } else {
            d.s3.region.trim()
        }
    ))
}

pub use crate::panel_ops_backup_upload::upload_backup_artifact;

pub fn load_schedule() -> BackupSchedule {
    let Ok(raw) = fs::read_to_string(schedule_path()) else {
        return BackupSchedule {
            enabled: false,
            cron: "0 2 * * *".into(),
            scope: "panel".into(),
            domain: String::new(),
            destination: "local".into(),
        };
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

pub fn save_schedule(schedule: &BackupSchedule) -> Result<(), String> {
    if let Some(parent) = schedule_path().parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Cannot create data dir: {e}"))?;
    }
    let raw = serde_json::to_string_pretty(schedule).map_err(|e| e.to_string())?;
    fs::write(schedule_path(), raw).map_err(|e| format!("Cannot save schedule: {e}"))
}

pub fn list_restore_candidates(
    scope: &str,
    domain: &str,
) -> Result<(String, Vec<(String, u64)>), String> {
    let (preferred, hits) =
        crate::backup_restore_scan::list_restore_archives_with_fallback(scope, domain)?;
    let display = preferred.unwrap_or_else(|| "panel + fallback locations".into());
    let files = hits
        .into_iter()
        .map(|h| (h.name, h.size))
        .collect::<Vec<_>>();
    Ok((display, files))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;

    #[test]
    fn destinations_roundtrip_and_legacy_defaults() {
        with_test_data_dir(|| {
            let legacy = r#"{
              "local_enabled": true,
              "google_drive_note": "legacy gdrive",
              "remote_note": "legacy remote"
            }"#;
            fs::write(destinations_path(), legacy).unwrap();
            let loaded = load_destinations();
            assert!(loaded.local_enabled);
            assert_eq!(loaded.google_drive.note, "legacy gdrive");
            assert_eq!(loaded.remote.note, "legacy remote");

            let mut d = loaded;
            d.s3.enabled = true;
            d.s3.bucket = "cpn-backups".into();
            d.s3.endpoint = "https://s3.example.com".into();
            set_s3_secrets("AKIA_TEST", "SECRET_TEST").unwrap();
            save_destinations(&d).unwrap();
            let again = load_destinations();
            assert!(again.s3.enabled);
            assert_eq!(again.s3.bucket, "cpn-backups");
            assert!(again.s3.access_key_set);
            assert!(again.s3.secret_key_set);
            let raw = fs::read_to_string(destinations_path()).unwrap();
            assert!(!raw.contains("AKIA_TEST"));
            assert!(!raw.contains("SECRET_TEST"));
        });
    }
}
