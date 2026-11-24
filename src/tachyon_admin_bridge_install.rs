//! Install and heal the Tachyon / SnappyMail-family `/?admin` bridge.
//!
//! Writes a root-owned (group `cpn-webmail`, mode 0640) config with the loopback panel
//! URL and a random shared token, then patches each installed `ActionsAdmin.php` so
//! `DoAdminLogin` asks the panel (`tachyon_admin_bridge`) instead of comparing against a
//! separate local admin password. Owner only; see that module for the policy.
//!
//! Operator escape hatch: create `/var/lib/cpn-webmail/admin-bridge.disabled` (heal then
//! removes the config and the PHP hook falls back to the local webmail admin login).

use crate::tachyon_admin_bridge::{BridgeConfig, config_path, load_config};
use crate::tachyon_admin_bridge_php::{
    LOGIN_BEGIN, LOGIN_END, LOGIN_VERSION, LOGOUT_BEGIN, LOGOUT_END, PHP_LOGIN_HOOK,
    PHP_LOGOUT_HOOK, PHP_SSO_HOOK, SSO_BEGIN, SSO_END,
};
use std::path::{Path, PathBuf};
use std::process::Command;

const DISABLED_MARKER: &str = "/var/lib/cpn-webmail/admin-bridge.disabled";
const BACKUP_SUFFIX: &str = ".cpn-bridge-orig";
const MAX_WALK_DEPTH: usize = 9;

/// Identifiers upstream `DoAdminLogin` must already use so the hook can reuse them.
const REQUIRED_TOKENS: &[&str] = &[
    "LoggerAuthHelper(",
    "loginErrorDelay()",
    "setAdminAuthToken()",
    "Notifications::AuthError",
    "ClientException",
    "DefaultResponse(",
    "AppData(true)",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatchOutcome {
    Patched,
    AlreadyCurrent,
    Unsupported,
}

/// Remove one injected block (any version) delimited by `begin` / `end` marker prefixes.
fn strip_block(raw: &str, begin: &str, end: &str) -> String {
    let Some(b) = raw.find(begin) else {
        return raw.to_string();
    };
    let Some(end_rel) = raw[b..].find(end) else {
        return raw.to_string();
    };
    // Start at the beginning of the BEGIN line, end after the END line.
    let line_start = raw[..b].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let end_abs = b + end_rel + end.len();
    let after = raw[end_abs..]
        .find('\n')
        .map(|i| end_abs + i + 1)
        .unwrap_or(raw.len());
    format!("{}{}", &raw[..line_start], &raw[after..])
}

/// Remove every previously injected bridge block (login, SSO, logout; any version).
pub fn strip_bridge_block(raw: &str) -> String {
    let s = strip_block(raw, LOGIN_BEGIN, LOGIN_END);
    let s = strip_block(&s, SSO_BEGIN, SSO_END);
    strip_block(&s, LOGOUT_BEGIN, LOGOUT_END)
}

/// Insert `hook` on the line after the opening brace of `fn_sig` (a `function X(` prefix).
fn insert_after_fn_open(base: &str, fn_sig: &str, hook: &str) -> Option<String> {
    let sig = base.find(fn_sig)?;
    let body_open = base[sig..].find('{')? + sig;
    let at = base[body_open..].find('\n')? + body_open + 1;
    Some(format!("{}{}{}", &base[..at], hook, &base[at..]))
}

/// Insert `hook` on the line before the first line containing `anchor` after `fn_sig`.
fn insert_before_anchor(base: &str, fn_sig: &str, anchor: &str, hook: &str) -> Option<String> {
    let sig = base.find(fn_sig)?;
    let anchor_abs = base[sig..].find(anchor)? + sig;
    let at = base[..anchor_abs].rfind('\n').map(|i| i + 1).unwrap_or(0);
    Some(format!("{}{}{}", &base[..at], hook, &base[at..]))
}

/// Return the patched source, or `None` when the login hook cannot be installed.
///
/// The login hook is required. The SSO and logout hooks are best effort: when upstream
/// layout differs they are skipped and password login still works.
pub fn patched_source(raw: &str) -> Option<(String, PatchOutcome)> {
    let base = strip_bridge_block(raw);
    let sig = base.find("function DoAdminLogin()")?;
    let body_open = base[sig..].find('{')? + sig;
    let after_open = base[body_open..].find('\n')? + body_open + 1;
    if !REQUIRED_TOKENS
        .iter()
        .all(|t| base[after_open..].contains(t))
    {
        return None;
    }
    let mut out = insert_after_fn_open(&base, "function DoAdminLogin()", PHP_LOGIN_HOOK)?;
    // Logout first: it sits below AdminAppData in upstream, but order is independent here.
    if let Some(next) = insert_after_fn_open(&out, "function DoAdminLogout()", PHP_LOGOUT_HOOK) {
        out = next;
    }
    if let Some(next) = insert_before_anchor(
        &out,
        "function AdminAppData(",
        "$aResult['Auth'] = $oActions->IsAdminLoggined(false);",
        PHP_SSO_HOOK,
    ) {
        out = next;
    }
    let outcome = if out == raw {
        PatchOutcome::AlreadyCurrent
    } else {
        PatchOutcome::Patched
    };
    Some((out, outcome))
}
fn random_token_hex() -> String {
    crate::secret_hygiene::strong_hex(32)
}

/// Rotate the bridge token when the stored one is empty, short, or a known default.
/// Does nothing when the bridge is not installed. Returns true when the token changed.
pub fn heal_weak_token() -> Result<bool, String> {
    let path = crate::tachyon_admin_bridge::config_path();
    let Some(cfg) = std::fs::read_to_string(&path)
        .ok()
        .and_then(|raw| serde_json::from_str::<BridgeConfig>(&raw).ok())
    else {
        return Ok(false);
    };
    if crate::secret_hygiene::weak_reason(&cfg.token, crate::secret_hygiene::MIN_SECRET_LEN)
        .is_none()
    {
        return Ok(false);
    }
    write_config_if_needed(&path, &desired_url())
}

fn desired_url() -> String {
    crate::panel_public_url::local_listen_base_url(crate::panel_ops_firewall::panel_listen_port())
}

/// Write the bridge config when missing or stale. Returns true when the file changed.
pub fn write_config_if_needed(path: &Path, url: &str) -> Result<bool, String> {
    let existing = std::fs::read_to_string(path)
        .ok()
        .and_then(|raw| serde_json::from_str::<BridgeConfig>(&raw).ok())
        .filter(|c| {
            c.version >= 1
                && crate::secret_hygiene::weak_reason(
                    &c.token,
                    crate::secret_hygiene::MIN_SECRET_LEN,
                )
                .is_none()
        });
    let token = existing
        .as_ref()
        .map(|c| c.token.clone())
        .unwrap_or_else(random_token_hex);
    let next = BridgeConfig {
        version: 1,
        url: url.to_string(),
        token,
    };
    if existing.as_ref() == Some(&next) {
        apply_config_permissions(path);
        return Ok(false);
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("create {}: {e}", parent.display()))?;
    }
    let json = serde_json::to_string_pretty(&next).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, format!("{json}\n")).map_err(|e| format!("write bridge config: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o640));
    }
    std::fs::rename(&tmp, path).map_err(|e| format!("install bridge config: {e}"))?;
    apply_config_permissions(path);
    Ok(true)
}

fn apply_config_permissions(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o640));
        // Group read for the PHP-FPM service user only (best effort when the group exists).
        let _ = Command::new("chown")
            .args(["root:cpn-webmail", &path.to_string_lossy()])
            .status();
    }
    #[cfg(not(unix))]
    let _ = path;
}

fn find_actions_admin_files() -> Vec<PathBuf> {
    let mut out = Vec::new();
    for root in crate::install_snappymail_lineage::lineage_docroots() {
        walk_for_actions_admin(Path::new(&root), 0, &mut out);
    }
    out.sort();
    out.dedup();
    out
}

fn walk_for_actions_admin(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth > MAX_WALK_DEPTH {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for ent in rd.flatten() {
        let path = ent.path();
        let Ok(ft) = ent.file_type() else { continue };
        if ft.is_symlink() {
            continue;
        }
        if ft.is_dir() {
            let name = ent.file_name().to_string_lossy().to_ascii_lowercase();
            if name == "data" || name == "node_modules" || name == ".git" {
                continue;
            }
            walk_for_actions_admin(&path, depth + 1, out);
        } else if ent.file_name() == "ActionsAdmin.php"
            && path.to_string_lossy().contains("libraries")
        {
            out.push(path);
        }
    }
}

fn php_lint_ok(source: &str) -> bool {
    let tmp = std::env::temp_dir().join(format!(
        "cpn-admin-bridge-lint-{}-{}.php",
        std::process::id(),
        crate::account::now_unix()
    ));
    if std::fs::write(&tmp, source).is_err() {
        return false;
    }
    let ok = Command::new("php")
        .args(["-l", &tmp.to_string_lossy()])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    let _ = std::fs::remove_file(&tmp);
    ok
}

fn patch_file(path: &Path) -> Result<PatchOutcome, String> {
    let raw = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let Some((next, outcome)) = patched_source(&raw) else {
        return Ok(PatchOutcome::Unsupported);
    };
    if outcome == PatchOutcome::AlreadyCurrent {
        return Ok(outcome);
    }
    // Never install a hook that does not parse (PHP missing counts as unverified).
    if !php_lint_ok(&next) {
        return Err(format!("php lint failed for patched {}", path.display()));
    }
    let backup = PathBuf::from(format!("{}{BACKUP_SUFFIX}", path.display()));
    if !backup.exists() {
        let _ = std::fs::write(&backup, strip_bridge_block(&raw));
    }
    let tmp = PathBuf::from(format!("{}.cpn-tmp", path.display()));
    std::fs::write(&tmp, &next).map_err(|e| format!("{}: {e}", tmp.display()))?;
    if let Ok(meta) = std::fs::metadata(path) {
        let _ = std::fs::set_permissions(&tmp, meta.permissions());
    }
    std::fs::rename(&tmp, path).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(PatchOutcome::Patched)
}

/// Remove the hook from every supported file (used by the disable path and tests).
pub fn unpatch_all() -> u32 {
    let mut n = 0;
    for path in find_actions_admin_files() {
        let Ok(raw) = std::fs::read_to_string(&path) else {
            continue;
        };
        if !raw.contains(LOGIN_BEGIN) && !raw.contains(SSO_BEGIN) && !raw.contains(LOGOUT_BEGIN) {
            continue;
        }
        if std::fs::write(&path, strip_bridge_block(&raw)).is_ok() {
            n += 1;
        }
    }
    n
}

/// True when the bridge config exists and at least one admin action file carries the hook.
pub fn bridge_active() -> bool {
    load_config().is_some()
        && find_actions_admin_files().iter().any(|p| {
            std::fs::read_to_string(p)
                .map(|raw| raw.contains(&format!("{LOGIN_BEGIN} {LOGIN_VERSION}")))
                .unwrap_or(false)
        })
}

/// Ensure webmail admin delegates to the panel owner. Safe to call from every heal.
pub fn ensure_admin_bridge() -> Result<String, String> {
    if !cfg!(unix) || !crate::install_snappymail_lineage::any_lineage_installed() {
        return Ok("skipped: no SnappyMail-family client".into());
    }
    let cfg_path = config_path();
    if Path::new(DISABLED_MARKER).is_file() {
        let _ = std::fs::remove_file(&cfg_path);
        return Ok("disabled by operator marker; local webmail admin login in effect".into());
    }
    if crate::account::load_bootstrap().is_none() {
        return Ok("skipped: no panel owner account yet".into());
    }
    let files = find_actions_admin_files();
    if files.is_empty() {
        return Ok("skipped: no ActionsAdmin.php found".into());
    }
    // Patch first: the config alone must never enable a half-installed bridge.
    let mut patched = 0u32;
    let mut current = 0u32;
    let mut errors: Vec<String> = Vec::new();
    for file in &files {
        match patch_file(file) {
            Ok(PatchOutcome::Patched) => patched += 1,
            Ok(PatchOutcome::AlreadyCurrent) => current += 1,
            Ok(PatchOutcome::Unsupported) => {}
            Err(e) => errors.push(e),
        }
    }
    if patched + current == 0 {
        let detail = if errors.is_empty() {
            "unsupported ActionsAdmin.php layout".to_string()
        } else {
            errors.join("; ")
        };
        return Err(format!("webmail admin bridge not installed: {detail}"));
    }
    let cfg_changed = write_config_if_needed(&cfg_path, &desired_url())?;
    if patched > 0 || cfg_changed {
        // Drop cached opcode for the patched PHP (best effort).
        let _ = Command::new("systemctl")
            .args(["reload", "php-fpm"])
            .status();
    }
    Ok(format!(
        "webmail admin bridge ready (patched {patched}, current {current}, config {})",
        if cfg_changed { "updated" } else { "unchanged" }
    ))
}

/// True when one admin action file lacks the current hooks (fresh upstream copy after a
/// webmail-only update, or an older hook version). Unsupported layouts never need a heal.
pub fn file_needs_heal(raw: &str) -> bool {
    matches!(patched_source(raw), Some((_, PatchOutcome::Patched)))
}

/// True when the bridge is expected to be on but something drifted: missing or stale config,
/// or any `ActionsAdmin.php` (including a brand new version directory from a webmail-only
/// update) without the current hooks. Cheap enough to poll every minute.
pub fn bridge_needs_heal() -> bool {
    if !cfg!(unix)
        || Path::new(DISABLED_MARKER).is_file()
        || !crate::install_snappymail_lineage::any_lineage_installed()
        || crate::account::load_bootstrap().is_none()
    {
        return false;
    }
    let files = find_actions_admin_files();
    if files.is_empty() {
        return false;
    }
    match load_config() {
        Some(cfg) if cfg.url == desired_url() => {}
        _ => return true,
    }
    files.iter().any(|p| {
        std::fs::read_to_string(p)
            .map(|raw| file_needs_heal(&raw))
            .unwrap_or(false)
    })
}

/// Background watcher: restores the bridge within about a minute after any webmail-only
/// update (Email > Webmail, package update, or the webmail admin self-update) replaced the
/// files the hooks live in. The same `ensure_admin_bridge` also runs from every webmail
/// heal and tip-upgrade path; this covers updates that never call CPN.
pub fn spawn_bridge_watch() {
    let _ = std::thread::Builder::new()
        .name("cpn-admin-bridge-watch".into())
        .spawn(|| {
            let mut last_err_log: u64 = 0;
            std::thread::sleep(std::time::Duration::from_secs(20));
            loop {
                if bridge_needs_heal() {
                    match ensure_admin_bridge() {
                        Ok(msg) => eprintln!("[webmail-admin-bridge] [info] re-applied: {msg}"),
                        Err(e) => {
                            let now = crate::account::now_unix();
                            if now.saturating_sub(last_err_log) > 900 {
                                last_err_log = now;
                                eprintln!("[webmail-admin-bridge] [warn] heal failed: {e}");
                            }
                        }
                    }
                }
                std::thread::sleep(std::time::Duration::from_secs(60));
            }
        });
}
#[cfg(test)]
mod tests;
