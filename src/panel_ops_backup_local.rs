//! Local storage path helpers and OneDrive OAuth secret hooks.

use crate::panel_ops_backup_extra::{load_destinations, read_secret};
use std::path::PathBuf;

fn write_secret(name: &str, value: &str) -> Result<(), String> {
    crate::panel_ops_backup_extra::write_dest_secret(name, value)
}

pub fn set_onedrive_secrets(client_secret: &str, refresh_token: &str) -> Result<(), String> {
    if !client_secret.trim().is_empty() {
        write_secret("onedrive_client_secret", client_secret.trim())?;
    }
    if !refresh_token.trim().is_empty() {
        write_secret("onedrive_refresh_token", refresh_token.trim())?;
    }
    Ok(())
}

pub fn test_onedrive() -> Result<String, String> {
    let d = load_destinations();
    if d.onedrive.client_id.trim().is_empty() {
        return Err("OneDrive (Microsoft Graph) client ID is required.".into());
    }
    if read_secret("onedrive_client_secret").is_none() {
        return Err("OneDrive client secret is not saved.".into());
    }
    if read_secret("onedrive_refresh_token").is_none() {
        return Err("OneDrive refresh token is not saved.".into());
    }
    Ok(
        "OneDrive OAuth credentials are present. Upload runs on the next scheduled backup when enabled."
            .into(),
    )
}

/// Default or operator-chosen local archive directory for panel-scope backups.
pub fn effective_local_backup_dir() -> PathBuf {
    let d = load_destinations();
    let custom = d.local_path.trim();
    if custom.is_empty() {
        return crate::paths::panel_backups_dir();
    }
    PathBuf::from(custom)
}

/// Validate and normalize an absolute local storage path (refuse system roots).
pub fn validate_local_backup_path(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(String::new());
    }
    let path = PathBuf::from(trimmed);
    let display = path.to_string_lossy().replace('\\', "/");
    // Hosts are Linux; accept `/...` even when unit tests run on Windows.
    let absolute = path.is_absolute() || display.starts_with('/');
    if !absolute {
        return Err(
            "Local storage path must be absolute (for example /home/cpn-panel/backups).".into(),
        );
    }
    if display == "/" {
        return Err("Local storage path cannot be the filesystem root.".into());
    }
    let blocked_prefixes = [
        "/bin/", "/boot/", "/dev/", "/etc/", "/lib/", "/lib64/", "/proc/", "/root/", "/run/",
        "/sbin/", "/sys/", "/usr/",
    ];
    for prefix in blocked_prefixes {
        if display.starts_with(prefix) || display == prefix.trim_end_matches('/') {
            return Err(format!("Local storage path `{trimmed}` is not allowed."));
        }
    }
    Ok(trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;
    use crate::panel_ops_backup_extra::{
        destinations_path_for_test, load_destinations, save_destinations, set_s3_secrets,
    };
    use std::fs;

    #[test]
    fn validate_local_path_rules() {
        assert!(validate_local_backup_path("").unwrap().is_empty());
        assert!(validate_local_backup_path("relative/path").is_err());
        assert!(validate_local_backup_path("/").is_err());
        assert!(validate_local_backup_path("/etc/passwd").is_err());
        assert_eq!(
            validate_local_backup_path("/home/cpn-panel/backups").unwrap(),
            "/home/cpn-panel/backups"
        );
    }

    #[test]
    fn destinations_roundtrip_and_legacy_defaults() {
        with_test_data_dir(|| {
            let legacy = r#"{
              "local_enabled": true,
              "google_drive_note": "legacy gdrive",
              "remote_note": "legacy remote"
            }"#;
            fs::write(destinations_path_for_test(), legacy).unwrap();
            let loaded = load_destinations();
            assert!(loaded.local_enabled);
            assert_eq!(loaded.google_drive.note, "legacy gdrive");
            assert_eq!(loaded.remote.note, "legacy remote");

            let mut d = loaded;
            d.s3.enabled = true;
            d.s3.bucket = "cpn-backups".into();
            d.s3.endpoint = "https://s3.example.com".into();
            d.onedrive.enabled = true;
            d.onedrive.client_id = "app-id".into();
            set_s3_secrets("AKIA_TEST", "SECRET_TEST").unwrap();
            set_onedrive_secrets("od-secret", "od-refresh").unwrap();
            save_destinations(&d).unwrap();
            let again = load_destinations();
            assert!(again.s3.enabled);
            assert_eq!(again.s3.bucket, "cpn-backups");
            assert!(again.s3.access_key_set);
            assert!(again.s3.secret_key_set);
            assert!(again.onedrive.enabled);
            assert!(again.onedrive.client_secret_set);
            assert!(again.onedrive.refresh_token_set);
            let raw = fs::read_to_string(destinations_path_for_test()).unwrap();
            assert!(!raw.contains("AKIA_TEST"));
            assert!(!raw.contains("SECRET_TEST"));
            assert!(!raw.contains("od-secret"));
            assert!(!raw.contains("od-refresh"));
        });
    }
}
