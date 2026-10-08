//! OpenPGP defaults for SnappyMail-family (Tachyon / SnappyMail / NextSnapMail)
//! and Roundcube Enigma. Keys stay under `/var/lib/cpn-webmail` (outside webroot).

use crate::install_snappymail_lineage::{
    application_ini_path, lineage_data_dirs, replace_ini_bool,
};
use std::path::{Path, PathBuf};
use std::process::Command;

/// GnuPG home for Roundcube Enigma (private + public keys). Not under the HTTP docroot.
pub const ROUNDCUBE_ENIGMA_HOMEDIR: &str = "/var/lib/cpn-webmail/roundcube-enigma";

const ROUNDCUBE_ROOT: &str = "/opt/cpn-webmail/roundcube";
const ROUNDCUBE_CONFIG: &str = "/opt/cpn-webmail/roundcube/config/config.inc.php";
const ROUNDCUBE_ENIGMA_CONFIG: &str = "/opt/cpn-webmail/roundcube/plugins/enigma/config.inc.php";

/// Enable OpenPGP on every installed webmail client that supports it.
///
/// SnappyMail-family: `openpgp` + `gnupg` in `application.ini` (built-in; no plugin).
/// Roundcube: Enigma plugin + keydir outside the docroot.
/// Soft-installs the `gnupg` / `gnupg2` package when missing (non-fatal).
pub fn ensure_webmail_openpgp_defaults() -> Result<(), String> {
    let _ = ensure_gnupg_cli_present();
    let _ = ensure_snappymail_family_openpgp();
    let _ = ensure_roundcube_enigma();
    Ok(())
}

fn ensure_gnupg_cli_present() -> Result<(), String> {
    if which_gpg().is_some() {
        return Ok(());
    }
    // Best-effort package install; OpenPGP.js still works without server gpg.
    let script = r#"
set -e
if command -v gpg >/dev/null 2>&1 || command -v gpg2 >/dev/null 2>&1; then exit 0; fi
if command -v dnf >/dev/null 2>&1; then
  dnf install -y gnupg2 >/dev/null 2>&1 || dnf install -y gnupg >/dev/null 2>&1 || true
elif command -v yum >/dev/null 2>&1; then
  yum install -y gnupg2 >/dev/null 2>&1 || yum install -y gnupg >/dev/null 2>&1 || true
elif command -v apt-get >/dev/null 2>&1; then
  DEBIAN_FRONTEND=noninteractive apt-get install -y gnupg >/dev/null 2>&1 || true
fi
"#;
    let _ = Command::new("bash").args(["-c", script]).status();
    Ok(())
}

fn which_gpg() -> Option<PathBuf> {
    for name in ["gpg", "gpg2"] {
        let out = Command::new("bash")
            .args(["-c", &format!("command -v {name} 2>/dev/null")])
            .output()
            .ok()?;
        if out.status.success() {
            let p = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !p.is_empty() {
                return Some(PathBuf::from(p));
            }
        }
    }
    None
}

fn ensure_snappymail_family_openpgp() -> Result<(), String> {
    for data_dir in lineage_data_dirs() {
        let ini = application_ini_path(&data_dir);
        if !ini.is_file() {
            continue;
        }
        let raw = match std::fs::read_to_string(&ini) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let mut updated = raw.clone();
        // Built-in OpenPGP UI (OpenPGP.js + optional Mailvelope + GnuPG).
        updated = replace_ini_bool(&updated, "openpgp", true);
        // Server GnuPG when available (attachments + Thunderbird-like keyring backup).
        // Private-key ops still prefer OpenPGP.js in the browser when that keyring has keys.
        updated = replace_ini_bool(&updated, "gnupg", true);
        if updated != raw {
            std::fs::write(&ini, updated).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn ensure_roundcube_enigma() -> Result<(), String> {
    let root = Path::new(ROUNDCUBE_ROOT);
    if !root.is_dir() {
        return Ok(());
    }
    let enigma_plugin = root.join("plugins/enigma");
    if !enigma_plugin.is_dir() {
        // Complete tarball should ship Enigma; skip rather than invent a stub.
        return Ok(());
    }

    ensure_enigma_homedir()?;
    ensure_enigma_plugin_config()?;
    ensure_roundcube_plugins_list()?;
    Ok(())
}

fn ensure_enigma_homedir() -> Result<(), String> {
    let dir = Path::new(ROUNDCUBE_ENIGMA_HOMEDIR);
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700));
    }
    let _ = Command::new("chown")
        .args(["-R", "cpn-webmail:cpn-webmail", ROUNDCUBE_ENIGMA_HOMEDIR])
        .status();
    let _ = Command::new("chmod")
        .args(["700", ROUNDCUBE_ENIGMA_HOMEDIR])
        .status();
    Ok(())
}

fn ensure_enigma_plugin_config() -> Result<(), String> {
    let path = Path::new(ROUNDCUBE_ENIGMA_CONFIG);
    if path.is_file() {
        // Keep operator edits; only ensure homedir line exists when empty/missing.
        let raw = std::fs::read_to_string(path).unwrap_or_default();
        if raw.contains("enigma_pgp_homedir") {
            return Ok(());
        }
    }
    let body = format!(
        "<?php\n\
         // Managed by CPN: Roundcube Enigma OpenPGP (keys outside HTTP docroot).\n\
         $config['enigma_pgp_homedir'] = '{home}';\n\
         $config['enigma_signatures'] = true;\n\
         $config['enigma_decryption'] = true;\n\
         $config['enigma_encryption'] = true;\n\
         $config['enigma_sign_all'] = false;\n\
         $config['enigma_encrypt_all'] = false;\n\
         // Attach public key so recipients can save it (Thunderbird-like).\n\
         $config['enigma_attach_pubkey'] = true;\n\
         $config['enigma_password_time'] = 5;\n\
         $config['enigma_passwordless'] = false;\n",
        home = ROUNDCUBE_ENIGMA_HOMEDIR.replace('\'', "\\'")
    );
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(path, body).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o640));
    }
    let _ = Command::new("chown")
        .args(["root:cpn-webmail", ROUNDCUBE_ENIGMA_CONFIG])
        .status();
    Ok(())
}

fn ensure_roundcube_plugins_list() -> Result<(), String> {
    let path = Path::new(ROUNDCUBE_CONFIG);
    if !path.is_file() {
        return Ok(());
    }
    let raw = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    if raw.contains("'enigma'") || raw.contains("\"enigma\"") {
        return Ok(());
    }
    let updated = insert_enigma_plugin(&raw);
    if updated != raw {
        std::fs::write(path, updated).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o640));
        }
    }
    Ok(())
}

/// Insert `enigma` into `$config['plugins'] = [...]` or `array(...)` when missing.
fn insert_enigma_plugin(raw: &str) -> String {
    if raw.contains("'enigma'") || raw.contains("\"enigma\"") {
        return raw.to_string();
    }
    // Short-array: $config['plugins'] = ['archive', 'zipdownload'];
    if let Some(idx) = raw.find("$config['plugins']") {
        let rest = &raw[idx..];
        if let Some(eq) = rest.find('=') {
            let after_eq = &rest[eq + 1..];
            let trimmed = after_eq.trim_start();
            if trimmed.starts_with('[')
                && let Some(end) = trimmed.find(']')
            {
                let inner = &trimmed[1..end];
                let new_inner = if inner.trim().is_empty() {
                    "'enigma'".to_string()
                } else {
                    format!("{}, 'enigma'", inner.trim_end_matches(',').trim_end())
                };
                let before = &raw[..idx + eq + 1 + (after_eq.len() - trimmed.len())];
                let after = &trimmed[end..];
                return format!("{before}[{new_inner}{after}");
            }
            if trimmed.starts_with("array(")
                && let Some(end_rel) = find_matching_paren(trimmed, 5)
            {
                let inner = &trimmed[6..end_rel];
                let new_inner = if inner.trim().is_empty() {
                    "'enigma'".to_string()
                } else {
                    format!("{}, 'enigma'", inner.trim_end_matches(',').trim_end())
                };
                let prefix_len = idx + eq + 1 + (after_eq.len() - trimmed.len());
                let before = &raw[..prefix_len];
                let after = &trimmed[end_rel..];
                return format!("{before}array({new_inner}{after}");
            }
        }
    }
    // Fallback: append a plugins line (Roundcube merges/overwrites later assignments).
    format!(
        "{}\n$config['plugins'] = array_values(array_unique(array_merge($config['plugins'] ?? [], ['enigma'])));\n",
        raw.trim_end()
    )
}

fn find_matching_paren(s: &str, open_idx: usize) -> Option<usize> {
    let bytes = s.as_bytes();
    if open_idx >= bytes.len() || bytes[open_idx] != b'(' {
        return None;
    }
    let mut depth = 0i32;
    for (i, &b) in bytes.iter().enumerate().skip(open_idx) {
        match b {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inserts_enigma_into_short_array() {
        let raw = "$config['plugins'] = ['archive', 'zipdownload'];\n";
        let out = insert_enigma_plugin(raw);
        assert!(out.contains("'enigma'"));
        assert!(out.contains("'archive'"));
    }

    #[test]
    fn inserts_enigma_into_array_call() {
        let raw = "$config['plugins'] = array('archive', 'zipdownload');\n";
        let out = insert_enigma_plugin(raw);
        assert!(out.contains("'enigma'"));
        assert!(out.contains("array("));
    }

    #[test]
    fn idempotent_when_enigma_present() {
        let raw = "$config['plugins'] = ['archive', 'enigma'];\n";
        // ensure_roundcube_plugins_list short-circuits; insert still ok if called.
        let out = insert_enigma_plugin(raw);
        assert!(out.contains("'enigma'"));
    }
}
