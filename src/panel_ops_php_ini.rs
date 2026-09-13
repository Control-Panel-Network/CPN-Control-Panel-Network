//! Read/write host php.ini with backup, and restart PHP services.

use crate::panel_ops_php_ext::{PhpPackageFamily, list_php_versions};
use crate::paths::default_data_dir;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

/// Basic php.ini keys shown on the Configurations Basic Settings tab.
pub const BASIC_BOOL_KEYS: &[&str] = &[
    "display_errors",
    "file_uploads",
    "allow_url_fopen",
    "allow_url_include",
];

pub const BASIC_VALUE_KEYS: &[&str] = &[
    "memory_limit",
    "max_execution_time",
    "upload_max_filesize",
    "post_max_size",
    "max_input_time",
];

#[derive(Debug, Clone)]
pub struct PhpIniTarget {
    pub branch: String,
    pub ini_path: PathBuf,
    pub binary: String,
    pub family: PhpPackageFamily,
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn branch_to_lsphp_prefix(branch: &str) -> Option<&'static str> {
    match branch.trim() {
        "8.5" => Some("lsphp85"),
        "8.4" => Some("lsphp84"),
        "8.3" => Some("lsphp83"),
        "8.2" => Some("lsphp82"),
        _ => None,
    }
}

fn php_ini_from_binary(bin: &str) -> Option<PathBuf> {
    let out = Command::new(bin).args(["-i"]).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("Loaded Configuration File =>") {
            let p = rest.trim();
            if !p.is_empty() && p != "(none)" {
                return Some(PathBuf::from(p));
            }
        }
    }
    None
}

/// Resolve the writable php.ini for a managed PHP branch.
pub fn resolve_php_ini_target(branch: &str) -> Result<PhpIniTarget, String> {
    let branch = branch.trim();
    if branch.is_empty() {
        return Err("PHP version is required".into());
    }
    let versions = list_php_versions();
    let matched = versions.iter().find(|v| v.branch == branch);
    if let Some(v) = matched
        && v.family == PhpPackageFamily::LiteSpeed
        && let Some(prefix) = branch_to_lsphp_prefix(branch)
    {
        let bin = format!("/usr/local/lsws/{prefix}/bin/lsphp");
        if Path::new(&bin).is_file() {
            let ini = php_ini_from_binary(&bin).or_else(|| {
                let candidate =
                    PathBuf::from(format!("/usr/local/lsws/{prefix}/etc/php.ini"));
                candidate.is_file().then_some(candidate)
            });
            if let Some(ini_path) = ini {
                return Ok(PhpIniTarget {
                    branch: branch.to_string(),
                    ini_path,
                    binary: bin,
                    family: PhpPackageFamily::LiteSpeed,
                });
            }
        }
    }

    let bin = "php".to_string();
    let ini_path = php_ini_from_binary(&bin)
        .or_else(|| {
            [
                "/etc/php.ini",
                "/etc/php/php.ini",
                "/etc/php8/php.ini",
            ]
            .iter()
            .map(PathBuf::from)
            .find(|p| p.is_file())
        })
        .ok_or_else(|| {
            format!("Could not locate php.ini for PHP {branch}. Install php-cli first.")
        })?;
    Ok(PhpIniTarget {
        branch: branch.to_string(),
        ini_path,
        binary: bin,
        family: PhpPackageFamily::Modular,
    })
}

pub fn read_php_ini_text(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|e| format!("Cannot read {}: {e}", path.display()))
}

fn parse_ini_value(raw: &str, key: &str) -> Option<String> {
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with(';') || trimmed.starts_with('#') {
            continue;
        }
        let Some((k, v)) = trimmed.split_once('=') else {
            continue;
        };
        if k.trim() == key {
            let mut val = v.trim().to_string();
            if (val.starts_with('"') && val.ends_with('"'))
                || (val.starts_with('\'') && val.ends_with('\''))
            {
                val = val[1..val.len() - 1].to_string();
            }
            return Some(val);
        }
    }
    None
}

pub fn read_basic_setting(raw: &str, key: &str) -> Option<String> {
    parse_ini_value(raw, key)
}

pub fn is_ini_truthy(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "on" | "true" | "yes"
    )
}

fn set_ini_directive(raw: &str, key: &str, value: &str) -> String {
    let mut out = String::with_capacity(raw.len() + 64);
    let mut replaced = false;
    for line in raw.lines() {
        let trimmed = line.trim();
        let is_assign = !trimmed.is_empty()
            && !trimmed.starts_with(';')
            && !trimmed.starts_with('#')
            && trimmed
                .split_once('=')
                .map(|(k, _)| k.trim() == key)
                .unwrap_or(false);
        if is_assign {
            out.push_str(&format!("{key} = {value}\n"));
            replaced = true;
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    if !replaced {
        out.push_str(&format!("\n; Added by CPN PHP Configurations\n{key} = {value}\n"));
    }
    out
}

fn backup_php_ini(path: &Path) -> Result<PathBuf, String> {
    let backup_dir = default_data_dir().join("php-ini-backups");
    fs::create_dir_all(&backup_dir)
        .map_err(|e| format!("Cannot create backup dir {}: {e}", backup_dir.display()))?;
    let name = format!(
        "php.ini.{}.{}.bak",
        path.file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("php.ini")
            .replace(['/', '\\'], "_"),
        now_unix()
    );
    let dest = backup_dir.join(name);
    fs::copy(path, &dest).map_err(|e| {
        format!(
            "Cannot backup {} to {}: {e}",
            path.display(),
            dest.display()
        )
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&dest, fs::Permissions::from_mode(0o600));
    }
    Ok(dest)
}

/// Write full php.ini content after creating a backup under `/var/lib/cpn/php-ini-backups/`.
pub fn write_php_ini_with_backup(path: &Path, content: &str) -> Result<PathBuf, String> {
    if !path.is_file() {
        return Err(format!("php.ini not found: {}", path.display()));
    }
    let backup = backup_php_ini(path)?;
    fs::write(path, content).map_err(|e| format!("Cannot write {}: {e}", path.display()))?;
    Ok(backup)
}

/// Apply basic settings onto an existing php.ini (with backup).
pub fn apply_basic_settings(
    path: &Path,
    bools: &[(String, bool)],
    values: &[(String, String)],
) -> Result<PathBuf, String> {
    let mut raw = read_php_ini_text(path)?;
    for (key, on) in bools {
        if !BASIC_BOOL_KEYS.contains(&key.as_str()) {
            return Err(format!("Unsupported php.ini key: {key}"));
        }
        let v = if *on { "On" } else { "Off" };
        raw = set_ini_directive(&raw, key, v);
    }
    for (key, value) in values {
        if !BASIC_VALUE_KEYS.contains(&key.as_str()) {
            return Err(format!("Unsupported php.ini key: {key}"));
        }
        let cleaned = value.trim();
        if cleaned.is_empty() || cleaned.len() > 32 {
            return Err(format!("Invalid value for {key}"));
        }
        if cleaned.chars().any(|c| {
            !(c.is_ascii_alphanumeric() || c == 'M' || c == 'G' || c == 'K' || c == '.')
        }) {
            return Err(format!("Invalid characters in {key}"));
        }
        raw = set_ini_directive(&raw, key, cleaned);
    }
    write_php_ini_with_backup(path, &raw)
}

/// Restart host PHP (php-fpm and/or LiteSpeed) so php.ini changes apply.
pub fn restart_php_services(family: PhpPackageFamily) -> Result<String, String> {
    let mut notes = Vec::new();
    if family == PhpPackageFamily::Modular || Path::new("/usr/lib/systemd/system/php-fpm.service").exists()
    {
        let status = Command::new("systemctl")
            .args(["restart", "php-fpm"])
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .output();
        match status {
            Ok(out) if out.status.success() => notes.push("php-fpm restarted".to_string()),
            Ok(out) => {
                let err = String::from_utf8_lossy(&out.stderr);
                notes.push(format!(
                    "php-fpm restart: {}",
                    err.lines().next().unwrap_or("failed").trim()
                ));
            }
            Err(e) => notes.push(format!("php-fpm restart error: {e}")),
        }
    }
    if family == PhpPackageFamily::LiteSpeed {
        for unit in ["lshttpd", "lsws"] {
            let status = Command::new("systemctl")
                .args(["try-restart", unit])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
            if status.map(|s| s.success()).unwrap_or(false) {
                notes.push(format!("{unit} restarted"));
                break;
            }
        }
    }
    if notes.is_empty() {
        Ok("No PHP service restart was required".into())
    } else {
        Ok(notes.join("; "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_directive_replaces_and_appends() {
        let raw = "memory_limit = 128M\n; comment\n";
        let updated = set_ini_directive(raw, "memory_limit", "256M");
        assert!(updated.contains("memory_limit = 256M"));
        let added = set_ini_directive(raw, "display_errors", "Off");
        assert!(added.contains("display_errors = Off"));
    }

    #[test]
    fn truthy_parser() {
        assert!(is_ini_truthy("On"));
        assert!(!is_ini_truthy("Off"));
    }
}
