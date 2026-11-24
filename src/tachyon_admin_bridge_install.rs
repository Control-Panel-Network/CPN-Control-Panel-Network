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
use rand::Rng;
use std::path::{Path, PathBuf};
use std::process::Command;

const MARKER_BEGIN: &str = "// CPN-ADMIN-BRIDGE-BEGIN";
const MARKER_END: &str = "// CPN-ADMIN-BRIDGE-END";
const BRIDGE_VERSION_TAG: &str = "v1";
const DISABLED_MARKER: &str = "/var/lib/cpn-webmail/admin-bridge.disabled";
const BACKUP_SUFFIX: &str = ".cpn-bridge-orig";
const MAX_WALK_DEPTH: usize = 9;

/// PHP injected at the top of `DoAdminLogin()` (tabs match upstream style).
const PHP_HOOK: &str = r##"		// CPN-ADMIN-BRIDGE-BEGIN v1
		// Webmail admin sign-in is delegated to the CPN panel owner account (owner only).
		// Mailbox IMAP login is unchanged. Create /var/lib/cpn-webmail/admin-bridge.disabled
		// and re-run CPN webmail heal to restore the local admin_login / admin_password.
		$cpnCfgPath = '/var/lib/cpn-webmail/admin-bridge.json';
		if (@\file_exists($cpnCfgPath)) {
			$cpnLogin = \trim((string) $this->GetActionParam('Login', ''));
			$cpnPass = (string) $this->GetActionParam('Password', '');
			$cpnTotp = (string) $this->GetActionParam('TOTP', '');
			$cpnOk = false;
			$cpnCfg = \json_decode((string) @\file_get_contents($cpnCfgPath), true);
			$cpnUrl = \is_array($cpnCfg) ? (string) ($cpnCfg['url'] ?? '') : '';
			$cpnToken = \is_array($cpnCfg) ? (string) ($cpnCfg['token'] ?? '') : '';
			$cpnHost = \strtolower((string) \parse_url($cpnUrl, PHP_URL_HOST));
			if (\strlen($cpnLogin) && \strlen($cpnPass) && \strlen($cpnToken)
				&& $this->Config()->Get('security', 'allow_admin_panel', true)
				&& \in_array($cpnHost, ['127.0.0.1', 'localhost', '[::1]', '::1'], true)
			) {
				$cpnBody = \json_encode(['username' => $cpnLogin, 'password' => $cpnPass, 'totp' => $cpnTotp], JSON_UNESCAPED_SLASHES);
				$cpnEndpoint = \rtrim($cpnUrl, '/') . '/api/internal/tachyon-admin/verify';
				$cpnHeaders = ['Content-Type: application/json', 'Accept: application/json', 'X-CPN-Bridge-Token: ' . $cpnToken];
				$cpnRaw = false;
				if (\function_exists('curl_init')) {
					$cpnCh = \curl_init($cpnEndpoint);
					\curl_setopt_array($cpnCh, [
						CURLOPT_POST => true,
						CURLOPT_POSTFIELDS => $cpnBody,
						CURLOPT_HTTPHEADER => $cpnHeaders,
						CURLOPT_RETURNTRANSFER => true,
						CURLOPT_TIMEOUT => 10,
						CURLOPT_CONNECTTIMEOUT => 3,
						CURLOPT_FOLLOWLOCATION => false,
						CURLOPT_SSL_VERIFYPEER => false,
						CURLOPT_SSL_VERIFYHOST => 0,
					]);
					$cpnRaw = \curl_exec($cpnCh);
					unset($cpnCh);
				} else {
					$cpnCtx = \stream_context_create([
						'http' => ['method' => 'POST', 'header' => \implode("\r\n", $cpnHeaders), 'content' => $cpnBody, 'timeout' => 10, 'ignore_errors' => true, 'follow_location' => 0],
						'ssl' => ['verify_peer' => false, 'verify_peer_name' => false],
					]);
					$cpnRaw = @\file_get_contents($cpnEndpoint, false, $cpnCtx);
				}
				$cpnJson = \is_string($cpnRaw) ? \json_decode($cpnRaw, true) : null;
				$cpnOk = \is_array($cpnJson) && true === ($cpnJson['ok'] ?? false);
			}
			unset($cpnPass, $cpnBody, $cpnToken, $cpnCfg);
			if (!$cpnOk) {
				$this->LoggerAuthHelper(null, $cpnLogin, true);
				$this->loginErrorDelay();
				throw new ClientException(Notifications::AuthError);
			}
			$sToken = $this->setAdminAuthToken();
			return $this->DefaultResponse($sToken ? $this->AppData(true) : false);
		}
		// CPN-ADMIN-BRIDGE-END
"##;

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

/// Remove a previously injected bridge block (any version).
pub fn strip_bridge_block(raw: &str) -> String {
    let Some(begin) = raw.find(MARKER_BEGIN) else {
        return raw.to_string();
    };
    let Some(end_rel) = raw[begin..].find(MARKER_END) else {
        return raw.to_string();
    };
    // Start at the beginning of the BEGIN line, end after the END line.
    let line_start = raw[..begin].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let end_abs = begin + end_rel + MARKER_END.len();
    let after = raw[end_abs..]
        .find('\n')
        .map(|i| end_abs + i + 1)
        .unwrap_or(raw.len());
    format!("{}{}", &raw[..line_start], &raw[after..])
}

/// Return the patched source, or `None` when the file layout is not supported.
pub fn patched_source(raw: &str) -> Option<(String, PatchOutcome)> {
    if raw.contains(&format!("{MARKER_BEGIN} {BRIDGE_VERSION_TAG}")) {
        return Some((raw.to_string(), PatchOutcome::AlreadyCurrent));
    }
    let base = strip_bridge_block(raw);
    let sig = base.find("function DoAdminLogin()")?;
    let body_open = base[sig..].find('{')? + sig;
    // Hook goes on the line right after the opening brace.
    let insert_at = base[body_open..].find('\n')? + body_open + 1;
    let fn_body = &base[insert_at..];
    if !REQUIRED_TOKENS.iter().all(|t| fn_body.contains(t)) {
        return None;
    }
    let mut out = String::with_capacity(base.len() + PHP_HOOK.len());
    out.push_str(&base[..insert_at]);
    out.push_str(PHP_HOOK);
    out.push_str(&base[insert_at..]);
    Some((out, PatchOutcome::Patched))
}

fn random_token_hex() -> String {
    let bytes: [u8; 32] = rand::rng().random();
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn desired_url() -> String {
    crate::panel_public_url::local_listen_base_url(crate::panel_ops_firewall::panel_listen_port())
}

/// Write the bridge config when missing or stale. Returns true when the file changed.
pub fn write_config_if_needed(path: &Path, url: &str) -> Result<bool, String> {
    let existing = std::fs::read_to_string(path)
        .ok()
        .and_then(|raw| serde_json::from_str::<BridgeConfig>(&raw).ok())
        .filter(|c| c.version >= 1 && c.token.len() >= 32);
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
        if !raw.contains(MARKER_BEGIN) {
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
                .map(|raw| raw.contains(&format!("{MARKER_BEGIN} {BRIDGE_VERSION_TAG}")))
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

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "<?php\nnamespace Tachyon;\nuse MailSo\\Exceptions\\ClientException;\nclass ActionsAdmin\n{\n\tpublic function DoAdminLogin() : array\n\t{\n\t\t$sLogin = trim($this->GetActionParam('Login', ''));\n\t\tif (!ok) {\n\t\t\t$this->LoggerAuthHelper(null, $sLogin, true);\n\t\t\t$this->loginErrorDelay();\n\t\t\tthrow new ClientException(Notifications::AuthError);\n\t\t}\n\t\t$sToken = $this->setAdminAuthToken();\n\t\treturn $this->DefaultResponse($sToken ? $this->AppData(true) : false);\n\t}\n\n\tpublic function DoAdminLogout() : array\n\t{\n\t\treturn [];\n\t}\n}\n";

    #[test]
    fn hook_is_injected_inside_do_admin_login_only_once() {
        let (out, outcome) = patched_source(SAMPLE).expect("supported");
        assert_eq!(outcome, PatchOutcome::Patched);
        assert!(out.contains("CPN-ADMIN-BRIDGE-BEGIN v1"));
        assert!(out.contains("/api/internal/tachyon-admin/verify"));
        let hook_pos = out.find("CPN-ADMIN-BRIDGE-BEGIN").unwrap();
        let login_pos = out.find("function DoAdminLogin()").unwrap();
        let logout_pos = out.find("function DoAdminLogout()").unwrap();
        assert!(login_pos < hook_pos && hook_pos < logout_pos);
        // Original local check stays below the hook as the fallback when the config is absent.
        assert!(out.contains("$sLogin = trim("));
        let (again, outcome2) = patched_source(&out).unwrap();
        assert_eq!(outcome2, PatchOutcome::AlreadyCurrent);
        assert_eq!(again, out);
    }

    #[test]
    fn strip_restores_original() {
        let (out, _) = patched_source(SAMPLE).unwrap();
        assert_eq!(strip_bridge_block(&out), SAMPLE);
    }

    #[test]
    fn unsupported_layout_is_not_patched() {
        assert!(patched_source("<?php\nclass X { function Other() {} }\n").is_none());
        let no_tokens = "<?php\nclass X {\n\tpublic function DoAdminLogin() : array\n\t{\n\t\treturn [];\n\t}\n}\n";
        assert!(patched_source(no_tokens).is_none());
    }

    #[test]
    fn hook_fails_closed_and_never_logs_password() {
        let hook = PHP_HOOK;
        // Config present but unreadable or invalid must deny, not fall through to local auth.
        assert!(hook.contains("if (!$cpnOk)"));
        assert!(hook.contains("throw new ClientException(Notifications::AuthError)"));
        // Only loopback targets are allowed (TLS verify is off for loopback only).
        assert!(hook.contains("'127.0.0.1', 'localhost'"));
        assert!(hook.contains("unset($cpnPass"));
        assert!(!hook.contains("error_log"));
        assert!(!hook.contains("curl_close"));
    }

    #[test]
    fn config_roundtrip_keeps_token_and_updates_url() {
        let dir = std::env::temp_dir().join(format!(
            "cpn-bridge-cfg-{}-{}",
            std::process::id(),
            crate::account::now_unix()
        ));
        let path = dir.join("admin-bridge.json");
        assert!(write_config_if_needed(&path, "http://127.0.0.1:2086").unwrap());
        let first: BridgeConfig =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(first.token.len(), 64);
        assert!(!write_config_if_needed(&path, "http://127.0.0.1:2086").unwrap());
        assert!(write_config_if_needed(&path, "http://127.0.0.1:2090").unwrap());
        let second: BridgeConfig =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(first.token, second.token);
        assert_eq!(second.url, "http://127.0.0.1:2090");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o640);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
