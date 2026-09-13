//! Site access/error log retention prefs under `/var/lib/cpn/log-retention.json`.
//!
//! Default: keep **30 days**. Optional max size (MB) truncates oversized files.
//! Applies a logrotate snippet when `/etc/logrotate.d` is writable, and best-effort
//! size truncate on save.

use crate::paths::join_data;
use crate::sites::{list_sites, site_home_from_record};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Default days to keep site access/error logs.
pub const DEFAULT_RETENTION_DAYS: u32 = 30;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogRetentionPrefs {
    /// Days to keep rotated copies (logrotate `rotate` with daily).
    #[serde(default = "default_days")]
    pub retention_days: u32,
    /// Optional max size in megabytes; files above this are truncated on apply.
    #[serde(default)]
    pub max_size_mb: Option<u32>,
}

fn default_days() -> u32 {
    DEFAULT_RETENTION_DAYS
}

impl Default for LogRetentionPrefs {
    fn default() -> Self {
        Self {
            retention_days: DEFAULT_RETENTION_DAYS,
            max_size_mb: None,
        }
    }
}

fn prefs_path() -> PathBuf {
    join_data("log-retention.json")
}

pub fn load_log_retention() -> LogRetentionPrefs {
    let path = prefs_path();
    let Ok(raw) = fs::read_to_string(&path) else {
        return LogRetentionPrefs::default();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

pub fn save_log_retention(prefs: &LogRetentionPrefs) -> Result<(), String> {
    let mut cleaned = prefs.clone();
    if cleaned.retention_days == 0 {
        cleaned.retention_days = DEFAULT_RETENTION_DAYS;
    }
    if cleaned.retention_days > 3650 {
        cleaned.retention_days = 3650;
    }
    if let Some(mb) = cleaned.max_size_mb {
        if mb == 0 {
            cleaned.max_size_mb = None;
        } else if mb > 102_400 {
            cleaned.max_size_mb = Some(102_400);
        }
    }
    let path = prefs_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Could not create data dir: {e}"))?;
    }
    let raw = serde_json::to_string_pretty(&cleaned)
        .map_err(|e| format!("Could not serialize log retention: {e}"))?;
    fs::write(&path, raw).map_err(|e| format!("Could not write log retention: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    }
    apply_log_retention(&cleaned)
}

fn site_log_files() -> Vec<PathBuf> {
    let Ok(sites) = list_sites() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for site in sites {
        let logs = site_home_from_record(&site).join("logs");
        out.push(logs.join("access.log"));
        out.push(logs.join("error.log"));
    }
    out
}

fn write_logrotate_snippet(prefs: &LogRetentionPrefs, paths: &[PathBuf]) -> Result<(), String> {
    let dest = Path::new("/etc/logrotate.d/cpn-site-logs");
    let parent = dest.parent().unwrap_or(Path::new("/etc"));
    if !parent.is_dir() {
        return Ok(());
    }
    // Best-effort: skip when not writable (non-root panel lab).
    let Ok(mut file) = fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(dest)
    else {
        return Ok(());
    };
    let mut body = String::from("# Managed by CPN Control Panel Network. Do not edit by hand.\n");
    body.push_str("# Site access/error logs under each site home logs/ directory.\n");
    if paths.is_empty() {
        body.push_str("# No sites registered yet.\n");
    } else {
        for path in paths {
            body.push_str(&format!("{}\n", path.display()));
        }
        body.push_str("{\n");
        body.push_str("  daily\n");
        body.push_str(&format!("  rotate {}\n", prefs.retention_days.max(1)));
        body.push_str("  missingok\n");
        body.push_str("  notifempty\n");
        body.push_str("  compress\n");
        body.push_str("  delaycompress\n");
        body.push_str("  copytruncate\n");
        if let Some(mb) = prefs.max_size_mb.filter(|m| *m > 0) {
            body.push_str(&format!("  maxsize {mb}M\n"));
        }
        body.push_str("}\n");
    }
    file.write_all(body.as_bytes())
        .map_err(|e| format!("Could not write logrotate snippet: {e}"))?;
    Ok(())
}

fn truncate_oversized(path: &Path, max_bytes: u64) -> Result<(), String> {
    if !path.is_file() {
        return Ok(());
    }
    let meta = fs::metadata(path).map_err(|e| format!("stat {}: {e}", path.display()))?;
    if meta.len() <= max_bytes {
        return Ok(());
    }
    // Keep the trailing window so recent lines survive.
    let keep = max_bytes.min(meta.len());
    let mut file = fs::File::open(path).map_err(|e| format!("open {}: {e}", path.display()))?;
    use std::io::{Read, Seek, SeekFrom};
    file.seek(SeekFrom::Start(meta.len() - keep))
        .map_err(|e| format!("seek {}: {e}", path.display()))?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf)
        .map_err(|e| format!("read {}: {e}", path.display()))?;
    // Drop partial first line.
    if let Some(pos) = buf.iter().position(|b| *b == b'\n') {
        buf = buf[pos + 1..].to_vec();
    }
    fs::write(path, buf).map_err(|e| format!("truncate {}: {e}", path.display()))?;
    Ok(())
}

/// Persist logrotate + optional size truncate for all registered site logs.
pub fn apply_log_retention(prefs: &LogRetentionPrefs) -> Result<(), String> {
    let paths = site_log_files();
    write_logrotate_snippet(prefs, &paths)?;
    if let Some(mb) = prefs.max_size_mb.filter(|m| *m > 0) {
        let max_bytes = u64::from(mb).saturating_mul(1024 * 1024);
        for path in &paths {
            truncate_oversized(path, max_bytes)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_thirty_days() {
        let p = LogRetentionPrefs::default();
        assert_eq!(p.retention_days, 30);
        assert!(p.max_size_mb.is_none());
    }

    #[test]
    fn roundtrip_prefs_under_temp_data() {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("cpn-log-ret-{stamp}"));
        fs::create_dir_all(&dir).unwrap();
        // Safety: only exercise serialize shape (save touches /etc optionally).
        let prefs = LogRetentionPrefs {
            retention_days: 14,
            max_size_mb: Some(50),
        };
        let raw = serde_json::to_string_pretty(&prefs).unwrap();
        let path = dir.join("log-retention.json");
        fs::write(&path, &raw).unwrap();
        let loaded: LogRetentionPrefs =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(loaded.retention_days, 14);
        assert_eq!(loaded.max_size_mb, Some(50));
        let _ = fs::remove_dir_all(&dir);
    }
}
