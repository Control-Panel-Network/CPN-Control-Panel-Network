//! SnappyMail operator defaults: Markdown, AllowStyles, Sieve domain, admin password sync.

use crate::install_webmail_runtime::SNAPPYMAIL_DATA_DIR;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

const MARKER_DEFAULTS: &str = "_data_/_default_/.cpn-user-defaults-v1";

/// Apply IMAP/SMTP/Sieve domain defaults, Markdown + AllowStyles, and heal Actions.php baselines.
pub fn ensure_snappymail_operator_defaults() -> Result<(), String> {
    let _ = ensure_domain_sieve_enabled();
    let _ = ensure_actions_php_defaults();
    let _ = ensure_user_settings_defaults();
    let _ = chown_snappy_data();
    Ok(())
}

/// Set SnappyMail `/?admin` password to match the given plaintext (CPN / mailbox flow).
///
/// Product intent: one password for panel mail ops and SnappyMail admin. Call from
/// `/email/password` and when the CPN panel account password changes.
pub fn sync_snappymail_admin_password(password: &str) -> Result<(), String> {
    if password.len() < 8 {
        return Err("SnappyMail admin password must be at least 8 characters".into());
    }
    let ini = Path::new(SNAPPYMAIL_DATA_DIR).join("_data_/_default_/configs/application.ini");
    if !ini.is_file() {
        return Ok(());
    }
    let hash = php_password_hash(password)?;
    let raw = std::fs::read_to_string(&ini).map_err(|e| e.to_string())?;
    let updated = replace_ini_value(&raw, "admin_password", &hash);
    if updated != raw {
        std::fs::write(&ini, updated).map_err(|e| e.to_string())?;
    }
    // Drop one-time plaintext so only the bcrypt hash is authoritative.
    let txt = Path::new(SNAPPYMAIL_DATA_DIR).join("_data_/_default_/admin_password.txt");
    if txt.is_file() {
        let _ = std::fs::remove_file(&txt);
    }
    let _ = chown_snappy_data();
    Ok(())
}

fn php_password_hash(password: &str) -> Result<String, String> {
    // Avoid shell quoting: pass password via env to php -r.
    let output = Command::new("php")
        .args([
            "-r",
            "echo password_hash(getenv('CPN_SNAPPY_PASS'), PASSWORD_DEFAULT);",
        ])
        .env("CPN_SNAPPY_PASS", password)
        .output()
        .map_err(|e| format!("php password_hash failed to start: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "php password_hash failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let hash = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if !hash.starts_with("$2y$") && !hash.starts_with("$2a$") && !hash.starts_with("$argon") {
        return Err("php password_hash returned unexpected output".into());
    }
    Ok(hash)
}

fn replace_ini_value(raw: &str, key: &str, value: &str) -> String {
    let prefix = format!("{key} = ");
    let mut out = String::with_capacity(raw.len() + value.len());
    let mut replaced = false;
    for line in raw.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with(&prefix) || trimmed.starts_with(&format!("{key}=")) {
            out.push_str(&format!("{key} = \"{value}\"\n"));
            replaced = true;
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    if !replaced {
        // Append under [security] if present.
        if let Some(idx) = out.find("[security]") {
            let insert_at = out[idx..]
                .find('\n')
                .map(|n| idx + n + 1)
                .unwrap_or(out.len());
            out.insert_str(insert_at, &format!("{key} = \"{value}\"\n"));
        } else {
            out.push_str(&format!("\n[security]\n{key} = \"{value}\"\n"));
        }
    }
    out
}

/// Enable ManageSieve on local SnappyMail domain profiles (127.0.0.1:4190).
fn ensure_domain_sieve_enabled() -> Result<(), String> {
    let domains = Path::new(SNAPPYMAIL_DATA_DIR).join("_data_/_default_/domains");
    if !domains.is_dir() {
        return Ok(());
    }
    let entries: Vec<PathBuf> = std::fs::read_dir(&domains)
        .map_err(|e| e.to_string())?
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.extension().and_then(|e| e.to_str()) == Some("json")
                && !p
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .eq_ignore_ascii_case("gmail.com.json")
                && !p
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .eq_ignore_ascii_case("hotmail.com.json")
                && !p
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .eq_ignore_ascii_case("yahoo.com.json")
        })
        .collect();
    for path in entries {
        let raw = match std::fs::read_to_string(&path) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let Ok(mut data) = serde_json::from_str::<Value>(&raw) else {
            continue;
        };
        let sieve = data.as_object_mut().map(|o| {
            o.entry("Sieve".to_string())
                .or_insert_with(|| Value::Object(Default::default()))
        });
        if let Some(Value::Object(obj)) = sieve {
            obj.insert("enabled".into(), Value::Bool(true));
            obj.insert("host".into(), Value::String("127.0.0.1".into()));
            obj.insert("port".into(), serde_json::json!(4190));
            obj.insert("shortLogin".into(), Value::Bool(true));
            obj.insert("lowerLogin".into(), Value::Bool(true));
            obj.insert("sasl".into(), serde_json::json!(["PLAIN", "LOGIN"]));
        }
        if let Ok(pretty) = serde_json::to_string_pretty(&data) {
            let _ = std::fs::write(&path, format!("{pretty}\n"));
        }
    }
    Ok(())
}

/// Patch installed Actions.php so new sessions default markdown + AllowStyles On.
fn ensure_actions_php_defaults() -> Result<(), String> {
    let root = Path::new("/opt/cpn-webmail/snappymail");
    if !root.is_dir() {
        return Ok(());
    }
    let output = Command::new("bash")
        .args([
            "-c",
            r#"find /opt/cpn-webmail/snappymail -path '*/libraries/RainLoop/Actions.php' -type f 2>/dev/null"#,
        ])
        .output()
        .map_err(|e| e.to_string())?;
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let path = Path::new(line.trim());
        if !path.is_file() {
            continue;
        }
        let Ok(raw) = std::fs::read_to_string(path) else {
            continue;
        };
        let updated = raw
            .replace("'AllowStyles' => false,", "'AllowStyles' => true,")
            .replace("'markdown' => false,", "'markdown' => true,");
        if updated != raw {
            let _ = std::fs::write(path, updated);
        }
    }
    Ok(())
}

/// Seed or migrate per-account settings JSON so Markdown and AllowStyles are On.
///
/// After the marker exists, only accounts missing those keys get defaults (user toggles stick).
fn ensure_user_settings_defaults() -> Result<(), String> {
    let storage = Path::new(SNAPPYMAIL_DATA_DIR).join("_data_/_default_/storage");
    if !storage.is_dir() {
        return Ok(());
    }
    let marker = Path::new(SNAPPYMAIL_DATA_DIR).join(MARKER_DEFAULTS);
    let force_once = !marker.is_file();
    walk_settings(&storage, force_once)?;
    if force_once {
        if let Some(parent) = marker.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(
            &marker,
            "cpn snappymail user defaults v1: markdown + AllowStyles\n",
        );
    }
    Ok(())
}

fn walk_settings(dir: &Path, force: bool) -> Result<(), String> {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return Ok(()),
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_settings(&path, force)?;
            continue;
        }
        if path.file_name().and_then(|n| n.to_str()) != Some("settings") {
            continue;
        }
        patch_settings_file(&path, force)?;
    }
    Ok(())
}

fn patch_settings_file(path: &Path, force: bool) -> Result<(), String> {
    let raw = std::fs::read_to_string(path).unwrap_or_else(|_| "{}".into());
    let mut data: Value = serde_json::from_str(&raw).unwrap_or_else(|_| serde_json::json!({}));
    let Some(obj) = data.as_object_mut() else {
        return Ok(());
    };
    let mut changed = false;
    if force || !obj.contains_key("markdown") {
        obj.insert("markdown".into(), Value::Bool(true));
        changed = true;
    }
    if force || !obj.contains_key("AllowStyles") {
        obj.insert("AllowStyles".into(), Value::Bool(true));
        changed = true;
    }
    if changed {
        let pretty = serde_json::to_string(&data).map_err(|e| e.to_string())?;
        std::fs::write(path, pretty).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn chown_snappy_data() -> Result<(), String> {
    let _ = Command::new("chown")
        .args([
            "-R",
            "cpn-webmail:cpn-webmail",
            &format!("{SNAPPYMAIL_DATA_DIR}_data_"),
        ])
        .status();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::replace_ini_value;

    #[test]
    fn replaces_admin_password_line() {
        let raw = "[security]\nadmin_login = \"admin\"\nadmin_password = \"old\"\n";
        let out = replace_ini_value(raw, "admin_password", "$2y$10$abc");
        assert!(out.contains("admin_password = \"$2y$10$abc\""));
        assert!(out.contains("admin_login = \"admin\""));
    }
}
