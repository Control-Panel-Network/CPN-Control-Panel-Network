//! Secret audit and heal. Runs on every panel start (so also after a tip upgrade) and from
//! `cpn secrets audit|heal`.
//!
//! Reports carry presence, length, entropy estimate, file-mode state, and a weak/default
//! classification only. Secret values are never printed, logged, or returned.
//!
//! Heal rules (existing strong random secrets are never touched):
//! - rotate when empty, short, repeating, low-entropy, or a known default;
//! - key material that protects stored data (MFA and panel-database keys) is audited
//!   and mode-fixed but never replaced, because replacing it would destroy that data.

use crate::paths::join_data;
use crate::secret_hygiene::{
    MIN_SECRET_LEN, WeakReason, entropy_bits_per_symbol, strong_alnum, strong_hex, weak_key_bytes,
    weak_reason, write_private,
};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

const SESSION_SECRET_FILE: &str = "panel-session.secret";
const BOOTSTRAP_TOKEN_LEN: usize = crate::paths::INSTALLER_BOOTSTRAP_TOKEN_LEN;

#[derive(Debug, Clone, Serialize)]
pub struct SecretReport {
    pub name: String,
    pub present: bool,
    pub len: usize,
    pub bits_per_symbol: f64,
    /// `None` when the secret passes every strength check (or is absent).
    pub weak: Option<String>,
    /// `Some(false)` when the file is more permissive than allowed.
    pub mode_ok: Option<bool>,
    /// False for key material whose replacement would destroy stored data.
    pub rotatable: bool,
}

impl SecretReport {
    fn absent(name: &str, rotatable: bool) -> Self {
        Self {
            name: name.into(),
            present: false,
            len: 0,
            bits_per_symbol: 0.0,
            weak: None,
            mode_ok: None,
            rotatable,
        }
    }

    fn text(name: &str, value: &str, min_len: usize, path: &Path, max_mode: u32) -> Self {
        let trimmed = value.trim();
        Self {
            name: name.into(),
            present: true,
            len: trimmed.len(),
            bits_per_symbol: round1(entropy_bits_per_symbol(trimmed.as_bytes())),
            weak: weak_reason(trimmed, min_len).map(|r| r.label().to_string()),
            mode_ok: mode_within(path, max_mode),
            rotatable: true,
        }
    }
}

fn round1(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

#[cfg(unix)]
fn mode_within(path: &Path, max_mode: u32) -> Option<bool> {
    use std::os::unix::fs::PermissionsExt;
    let mode = fs::metadata(path).ok()?.permissions().mode() & 0o777;
    Some(mode & !max_mode == 0)
}

#[cfg(not(unix))]
fn mode_within(_path: &Path, _max_mode: u32) -> Option<bool> {
    None
}

fn read_trimmed(path: &Path) -> Option<String> {
    fs::read_to_string(path).ok().map(|s| s.trim().to_string())
}

fn json_string(path: &Path, key: &str) -> Option<String> {
    let raw = fs::read_to_string(path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&raw).ok()?;
    value.get(key)?.as_str().map(str::to_string)
}

fn mfa_key_path() -> PathBuf {
    join_data("mfa").join("mfa-encryption.key")
}

fn control_secret_path() -> PathBuf {
    join_data("phpmyadmin").join("control.secret")
}

fn text_secret_files() -> Vec<(&'static str, PathBuf, usize, u32)> {
    vec![
        (
            "panel-session.secret",
            join_data(SESSION_SECRET_FILE),
            MIN_SECRET_LEN,
            0o600,
        ),
        (
            "installer-bootstrap.token",
            crate::paths::installer_bootstrap_token_path(),
            BOOTSTRAP_TOKEN_LEN,
            0o600,
        ),
        (
            "phpmyadmin control secret",
            control_secret_path(),
            MIN_SECRET_LEN,
            0o600,
        ),
    ]
}

/// Audit every known secret. Order is stable.
pub fn audit() -> Vec<SecretReport> {
    let mut out = Vec::new();
    for (name, path, min_len, max_mode) in text_secret_files() {
        out.push(match read_trimmed(&path) {
            Some(value) => SecretReport::text(name, &value, min_len, &path, max_mode),
            None => SecretReport::absent(name, true),
        });
    }
    let db_cfg = crate::panel_db::config::config_path();
    for (name, key) in [
        ("panel-db panel password", "panel_password"),
        ("panel-db mail auth password", "mail_auth_password"),
    ] {
        out.push(match json_string(&db_cfg, key) {
            Some(value) => SecretReport::text(name, &value, MIN_SECRET_LEN, &db_cfg, 0o600),
            None => SecretReport::absent(name, false),
        });
    }
    let bridge = crate::tachyon_admin_bridge::config_path();
    out.push(match json_string(&bridge, "token") {
        // The bridge file is group-readable (0640) for the webmail PHP-FPM user on purpose.
        Some(value) => SecretReport::text(
            "webmail admin bridge token",
            &value,
            MIN_SECRET_LEN,
            &bridge,
            0o640,
        ),
        None => SecretReport::absent("webmail admin bridge token", true),
    });
    for (name, path, hex_ok) in [
        (
            "panel-db encryption key",
            crate::panel_db::config::key_path(),
            false,
        ),
        ("mfa encryption key", mfa_key_path(), true),
    ] {
        out.push(match fs::read(&path) {
            Ok(bytes) => {
                let bytes = if hex_ok && bytes.len() == 64 {
                    // Legacy hex form of the same 32 byte key.
                    std::str::from_utf8(&bytes)
                        .map(|s| s.trim().as_bytes().to_vec())
                        .unwrap_or(bytes)
                } else {
                    bytes
                };
                let expected = if hex_ok && bytes.len() == 64 { 64 } else { 32 };
                SecretReport {
                    name: name.into(),
                    present: true,
                    len: bytes.len(),
                    bits_per_symbol: round1(entropy_bits_per_symbol(&bytes)),
                    weak: weak_key_bytes(&bytes, expected).map(|r| r.label().to_string()),
                    mode_ok: mode_within(&path, 0o600),
                    rotatable: false,
                }
            }
            Err(_) => SecretReport::absent(name, false),
        });
    }
    out.extend(blowfish_reports());
    out
}

fn blowfish_reports() -> Vec<SecretReport> {
    let mut out = Vec::new();
    for (idx, path) in pma_config_files().into_iter().enumerate() {
        let Ok(raw) = fs::read_to_string(&path) else {
            continue;
        };
        let name = format!("phpMyAdmin blowfish secret #{}", idx + 1);
        let literals: Vec<String> = blowfish_literals(&raw).into_iter().flatten().collect();
        match literals
            .iter()
            .find(|v| weak_reason(v, MIN_SECRET_LEN).is_some())
            .or(literals.first())
        {
            Some(value) => out.push(SecretReport::text(
                &name,
                value,
                MIN_SECRET_LEN,
                &path,
                0o644,
            )),
            None => out.push(SecretReport::absent(&name, true)),
        }
    }
    out
}

fn pma_config_files() -> Vec<PathBuf> {
    let Some(share) = crate::apps_phpmyadmin::phpmyadmin_share_dir() else {
        return Vec::new();
    };
    crate::apps_phpmyadmin_sso::phpmyadmin_config_candidates(&share)
        .into_iter()
        .filter(|p| p.is_file())
        .collect()
}

fn is_comment(trimmed: &str) -> bool {
    trimmed.starts_with("//")
        || trimmed.starts_with('#')
        || trimmed.starts_with('*')
        || trimmed.starts_with("/*")
}

fn is_blowfish_line(trimmed: &str) -> bool {
    !is_comment(trimmed)
        && ["blowfish_secret", "BlowfishSecret"].iter().any(|key| {
            trimmed.starts_with(&format!("$cfg['{key}']"))
                || trimmed.starts_with(&format!("$cfg[\"{key}\"]"))
        })
}

/// Byte range of the quoted literal on a blowfish assignment line, if it is a plain literal.
fn literal_span(line: &str) -> Option<(usize, usize)> {
    let eq = line.find('=')?;
    let rest = &line[eq + 1..];
    let open_rel = rest.find(|c: char| !c.is_whitespace())?;
    let quote = rest[open_rel..]
        .chars()
        .next()
        .filter(|c| *c == '\'' || *c == '"')?;
    let start = eq + 1 + open_rel + 1;
    let end_rel = line[start..].find(quote)?;
    Some((start, start + end_rel))
}

/// Literal values of every blowfish assignment (`None` for a non-literal PHP expression).
pub fn blowfish_literals(raw: &str) -> Vec<Option<String>> {
    raw.lines()
        .filter(|line| is_blowfish_line(line.trim_start()))
        .map(|line| literal_span(line).map(|(s, e)| line[s..e].to_string()))
        .collect()
}

/// Replace every weak blowfish literal, or append a strong one when the file has none.
/// Returns `None` when nothing needs to change. Non-literal expressions are left alone.
pub fn rewrite_blowfish(raw: &str) -> Option<String> {
    let mut changed = false;
    let mut found = false;
    let mut lines: Vec<String> = Vec::new();
    for line in raw.lines() {
        if is_blowfish_line(line.trim_start()) {
            found = true;
            if let Some((s, e)) = literal_span(line)
                && weak_reason(&line[s..e], MIN_SECRET_LEN).is_some()
            {
                lines.push(format!("{}{}{}", &line[..s], strong_alnum(32), &line[e..]));
                changed = true;
                continue;
            }
        }
        lines.push(line.to_string());
    }
    let mut out = lines.join("\n");
    if raw.ends_with('\n') {
        out.push('\n');
    }
    if !found {
        let add = format!(
            "// CPN-BLOWFISH\n$cfg['blowfish_secret'] = '{}';\n",
            strong_alnum(32)
        );
        match out.trim_end().strip_suffix("?>") {
            Some(head) => out = format!("{}\n{add}?>\n", head.trim_end()),
            None => {
                if !out.ends_with('\n') {
                    out.push('\n');
                }
                out.push_str(&add);
            }
        }
        changed = true;
    }
    changed.then_some(out)
}

/// Legacy time-derived access passwords looked like `mra<hex>` (predictable).
pub fn is_legacy_time_derived_password(value: &str) -> bool {
    value.len() <= 24
        && value
            .strip_prefix("mra")
            .is_some_and(|rest| !rest.is_empty() && rest.chars().all(|c| c.is_ascii_hexdigit()))
}

/// Rewrite a legacy access password inside an assistant `config.php`.
/// Returns the new file body and the new password, or `None` when nothing is legacy.
pub fn rewrite_legacy_access_password(raw: &str) -> Option<(String, String)> {
    let marker = "'access_password' => '";
    let start = raw.find(marker)? + marker.len();
    let end = start + raw[start..].find('\'')?;
    if !is_legacy_time_derived_password(&raw[start..end]) {
        return None;
    }
    let fresh = strong_alnum(32);
    Some((format!("{}{}{}", &raw[..start], fresh, &raw[end..]), fresh))
}

#[cfg(unix)]
fn tighten(path: &Path, max_mode: u32, notes: &mut Vec<String>, label: &str) {
    use std::os::unix::fs::PermissionsExt;
    if mode_within(path, max_mode) == Some(false)
        && fs::set_permissions(path, fs::Permissions::from_mode(max_mode)).is_ok()
    {
        notes.push(format!("{label}: permissions tightened to {max_mode:o}"));
    }
}

#[cfg(not(unix))]
fn tighten(_path: &Path, _max_mode: u32, _notes: &mut Vec<String>, _label: &str) {}

fn heal_text_file(
    name: &str,
    path: &Path,
    min_len: usize,
    notes: &mut Vec<String>,
) -> Option<WeakReason> {
    let value = read_trimmed(path)?;
    let reason = weak_reason(&value, min_len);
    if reason.is_none() {
        tighten(path, 0o600, notes, name);
    }
    reason
}

/// Rotate weak secrets and tighten file modes. Idempotent; safe on every start.
/// Messages mention names and reasons only.
pub fn heal() -> Vec<String> {
    let mut notes = Vec::new();

    let session = join_data(SESSION_SECRET_FILE);
    if let Some(reason) =
        heal_text_file("panel-session.secret", &session, MIN_SECRET_LEN, &mut notes)
    {
        match write_private(&session, strong_hex(32).as_bytes()) {
            Ok(()) => notes.push(format!(
                "panel-session.secret rotated ({}); existing panel sessions must sign in again",
                reason.label()
            )),
            Err(e) => notes.push(format!("panel-session.secret rotation failed: {e}")),
        }
    }

    let bootstrap = crate::paths::installer_bootstrap_token_path();
    if let Some(reason) = heal_text_file(
        "installer-bootstrap.token",
        &bootstrap,
        BOOTSTRAP_TOKEN_LEN,
        &mut notes,
    ) {
        // A fresh random token is minted on the next installer start.
        if fs::remove_file(&bootstrap).is_ok() {
            notes.push(format!(
                "installer-bootstrap.token removed ({})",
                reason.label()
            ));
        }
    }

    let control = control_secret_path();
    if let Some(reason) = heal_text_file(
        "phpmyadmin control secret",
        &control,
        MIN_SECRET_LEN,
        &mut notes,
    ) && fs::remove_file(&control).is_ok()
    {
        // Recreates the secret and re-applies it to the MariaDB control user.
        let outcome = crate::apps_phpmyadmin_storage::ensure_phpmyadmin_configuration_storage();
        notes.push(format!(
            "phpmyadmin control secret rotated ({}): {}",
            reason.label(),
            if outcome.is_ok() {
                "storage re-keyed"
            } else {
                "storage re-key deferred"
            }
        ));
    }

    for path in pma_config_files() {
        let Ok(raw) = fs::read_to_string(&path) else {
            continue;
        };
        if let Some(updated) = rewrite_blowfish(&raw) {
            match fs::write(&path, updated) {
                Ok(()) => {
                    notes.push("phpMyAdmin blowfish secret set to a fresh random value".into())
                }
                Err(e) => notes.push(format!("phpMyAdmin blowfish update failed: {e}")),
            }
        }
    }

    match crate::tachyon_admin_bridge_install::heal_weak_token() {
        Ok(true) => notes.push("webmail admin bridge token rotated".into()),
        Ok(false) => {}
        Err(e) => notes.push(format!("webmail admin bridge token check failed: {e}")),
    }

    for (name, path) in [
        ("panel-db.json", crate::panel_db::config::config_path()),
        ("panel-db.key", crate::panel_db::config::key_path()),
        ("mfa-encryption.key", mfa_key_path()),
    ] {
        if path.is_file() {
            tighten(&path, 0o600, &mut notes, name);
        }
    }
    notes
}

/// Heal and write the result to the panel log. Never blocks start-up on errors.
pub fn ensure_logged(reason: &str) {
    for line in heal() {
        crate::upgrade_tip_log::log_tagged_main_only(
            "secrets",
            "info",
            &format!("{reason}: {line}"),
        );
    }
}

/// Compact human-readable table (no values).
pub fn render_table(reports: &[SecretReport]) -> String {
    let mut out =
        String::from("SECRET                              PRESENT  LEN  BITS/SYM  MODE  STATUS\n");
    for r in reports {
        let mode = match r.mode_ok {
            Some(true) => "ok",
            Some(false) => "loose",
            None => "-",
        };
        let status = match (&r.weak, r.present) {
            (Some(w), _) => format!("WEAK ({w})"),
            (None, true) => "ok".to_string(),
            (None, false) => "absent".to_string(),
        };
        out.push_str(&format!(
            "{:<35} {:<8} {:<4} {:<9} {:<5} {}\n",
            r.name,
            if r.present { "yes" } else { "no" },
            r.len,
            r.bits_per_symbol,
            mode,
            status
        ));
    }
    out
}

#[cfg(test)]
#[path = "secret_hygiene_audit_tests.rs"]
mod tests;
