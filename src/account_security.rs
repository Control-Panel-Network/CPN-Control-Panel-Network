//! First-login security gates: forced password change and mandatory admin 2FA.

use crate::account::{PanelBootstrap, load_bootstrap, write_account_file};
use crate::account_mfa::totp_enabled_for;
use crate::account_mgmt::find_account;
use crate::account_passkeys::has_passkeys;
use crate::panel_webauthn::passkey_client_script;
use std::fs;

/// True when the account must change password before using the full panel.
pub fn must_change_password(username: &str) -> bool {
    find_account(username)
        .map(|(boot, _)| boot.must_change_password)
        .unwrap_or(false)
}

/// True when MFA (TOTP or at least one passkey) is required and not yet enrolled.
pub fn needs_mfa_enrollment(username: &str) -> bool {
    let requires = find_account(username)
        .map(|(boot, _)| boot.totp_required)
        .unwrap_or(false);
    if !requires {
        return false;
    }
    !(totp_enabled_for(username) || has_passkeys(username))
}

/// Clear the must-change-password flag after a successful forced change.
pub fn clear_must_change_password(username: &str) -> Result<(), String> {
    let (mut boot, path) = find_account(username)?;
    if !boot.must_change_password {
        return Ok(());
    }
    boot.must_change_password = false;
    write_account_file(&path, &boot)
}

/// Ensure bootstrap admin has `totp_required` for upgrades (migration hook).
pub fn ensure_account_security_flags_migrated() -> Result<(), String> {
    if let Some(mut boot) = load_bootstrap() {
        let mut dirty = false;
        if !boot.totp_required {
            boot.totp_required = true;
            dirty = true;
        }
        if dirty {
            crate::account::persist_bootstrap(&boot)?;
        }
    }
    let dir = crate::account::accounts_dir();
    if !dir.is_dir() {
        return Ok(());
    }
    let entries =
        fs::read_dir(&dir).map_err(|err| format!("Could not read {}: {err}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|err| format!("Could not read account entry: {err}"))?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let Ok(raw) = fs::read_to_string(&path) else {
            continue;
        };
        let Ok(boot) = serde_json::from_str::<PanelBootstrap>(&raw) else {
            continue;
        };
        // Touch parse path so older JSON without new flags stays loadable.
        let _ = boot.must_change_password;
        let _ = boot.totp_required;
        let _ = write_account_file(&path, &boot);
    }
    Ok(())
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Theme-aware backup codes panel with Copy to clipboard and Download (.txt).
/// `hint` is the parenthetical after "Backup codes", e.g. "store securely; shown once".
pub fn backup_codes_panel_html(codes: &[String], hint: &str) -> String {
    let mut html = format!(
        r#"<div class="mfa-codes-panel" data-mfa-codes>
  <div class="mfa-codes-head">
    <p class="mfa-codes-label"><strong>Backup codes</strong> ({hint}):</p>
    <div class="mfa-codes-actions">
      <button type="button" class="btn-secondary mfa-codes-copy">Copy to clipboard</button>
      <button type="button" class="btn-secondary mfa-codes-download">Download</button>
    </div>
  </div>
  <ul>"#,
        hint = html_escape(hint),
    );
    for code in codes {
        html.push_str(&format!("<li><code>{}</code></li>", html_escape(code)));
    }
    html.push_str(
        r#"</ul>
  <p class="muted mfa-codes-status" role="status" aria-live="polite"></p>
</div>
<script>
(function () {
  var panel = document.querySelector("[data-mfa-codes]");
  if (!panel || panel.getAttribute("data-mfa-wired") === "1") return;
  panel.setAttribute("data-mfa-wired", "1");
  var statusEl = panel.querySelector(".mfa-codes-status");
  var copyBtn = panel.querySelector(".mfa-codes-copy");
  var dlBtn = panel.querySelector(".mfa-codes-download");
  function codesText() {
    return Array.prototype.map.call(panel.querySelectorAll("ul code"), function (el) {
      return (el.textContent || "").trim();
    }).filter(Boolean).join("\n");
  }
  function setStatus(msg) {
    if (!statusEl) return;
    statusEl.textContent = msg || "";
  }
  function copyFallback(text) {
    var ta = document.createElement("textarea");
    ta.value = text;
    ta.setAttribute("readonly", "");
    ta.style.position = "fixed";
    ta.style.left = "-9999px";
    document.body.appendChild(ta);
    ta.select();
    var ok = false;
    try { ok = document.execCommand("copy"); } catch (e) { ok = false; }
    document.body.removeChild(ta);
    return ok;
  }
  function copyCodes() {
    var text = codesText();
    if (!text) { setStatus("No codes to copy."); return; }
    function done() { setStatus("Copied to clipboard."); }
    function fail() {
      if (copyFallback(text)) done();
      else setStatus("Copy failed. Select the codes manually.");
    }
    if (navigator.clipboard && navigator.clipboard.writeText) {
      navigator.clipboard.writeText(text).then(done).catch(fail);
    } else {
      fail();
    }
  }
  function downloadCodes() {
    var text = codesText();
    if (!text) { setStatus("No codes to download."); return; }
    var blob = new Blob([text + "\n"], { type: "text/plain;charset=utf-8" });
    var url = URL.createObjectURL(blob);
    var a = document.createElement("a");
    a.href = url;
    a.download = "cpn-backup-codes.txt";
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    setTimeout(function () { URL.revokeObjectURL(url); }, 1000);
    setStatus("Download started (cpn-backup-codes.txt).");
  }
  if (copyBtn) copyBtn.addEventListener("click", copyCodes);
  if (dlBtn) dlBtn.addEventListener("click", downloadCodes);
})();
</script>"#,
    );
    html
}

/// HTML main body for forced password change (no current-password field).
pub fn change_password_gate_main(notice: Option<&str>, error: Option<&str>) -> String {
    let notice_html = notice
        .map(|n| {
            format!(
                r#"<p class="notice" style="color:#18864b;">{n}</p>"#,
                n = html_escape(n)
            )
        })
        .unwrap_or_default();
    let error_html = error
        .map(|e| {
            format!(
                r#"<p class="error" style="color:#b42318;">{e}</p>"#,
                e = html_escape(e)
            )
        })
        .unwrap_or_default();
    format!(
        r#"
<section class="panel-card" style="max-width:520px;">
  <p class="eyebrow">SECURITY</p>
  <h1>Change your password</h1>
  <p class="muted">Your account requires a new password before you can use the panel.</p>
  {notice_html}
  {error_html}
  <form method="post" action="/account/security/change-password" class="stack-form" style="display:grid;gap:12px;margin-top:16px;">
    <label>New password
      <input type="password" name="password" required autocomplete="new-password" minlength="12">
    </label>
    <label>Confirm password
      <input type="password" name="password_confirm" required autocomplete="new-password" minlength="12">
    </label>
    <button type="submit" class="btn-primary">Save new password</button>
  </form>
</section>"#
    )
}

fn enroll_passkey_section_html() -> String {
    format!(
        r#"
  <div id="cpn-passkey-enroll" data-redirect="/account/users/modify?notice=Passkey+registered" class="stack-form mfa-passkey-section">
    <h2 style="margin:0;font-size:1.1rem;">Passkey</h2>
    <p class="muted" style="margin:0;">Register a platform or security-key passkey instead of TOTP. Completing either path returns you to Modify User so you can add more factors.</p>
    <p class="muted" style="margin:0;">On loopback labs, open the panel as <code>http://localhost</code> with your panel port (not <code>127.0.0.1</code>) so the browser can create the credential. Use Windows Hello or a FIDO2 security key (touch; a key PIN is optional but recommended).</p>
    <label>Label (optional)
      <input id="cpn-passkey-label" type="text" maxlength="64" placeholder="Laptop / YubiKey" autocomplete="off">
    </label>
    <button type="button" class="btn-secondary" onclick="cpnRegisterPasskey()">Register passkey</button>
    <p id="cpn-passkey-status" class="muted" role="status"></p>
  </div>
  <script>{script}</script>"#,
        script = passkey_client_script(),
    )
}

/// HTML main body for mandatory 2FA enrollment.
pub fn enroll_mfa_gate_main(
    notice: Option<&str>,
    error: Option<&str>,
    enroll_secret: Option<&str>,
    enroll_qr_svg: Option<&str>,
    backup_codes: Option<&[String]>,
) -> String {
    let notice_html = notice
        .map(|n| {
            format!(
                r#"<p class="notice" style="color:#18864b;">{n}</p>"#,
                n = html_escape(n)
            )
        })
        .unwrap_or_default();
    let error_html = error
        .map(|e| {
            format!(
                r#"<p class="error" style="color:#b42318;">{e}</p>"#,
                e = html_escape(e)
            )
        })
        .unwrap_or_default();

    let mut body = format!(
        r#"
<section class="panel-card" style="max-width:640px;">
  <p class="eyebrow">SECURITY</p>
  <h1>Enable two-factor authentication</h1>
  <p class="muted">Panel administrators must enroll TOTP (authenticator app) or a passkey before using the dashboard and other operational areas.</p>
  <p class="muted"><a href="/settings">Settings</a> (version, design, setup wizard, connect, site and error messages, log retention, and change port) stay available so you can finish panel configuration while enrollment is pending.</p>
  {notice_html}
  {error_html}
"#
    );

    if let Some(codes) = backup_codes {
        body.push_str(&backup_codes_panel_html(
            codes,
            "store securely; shown once",
        ));
        body.push_str(
            r#"<p><a class="btn-primary" href="/account/users/modify">Continue to Modify User</a></p>"#,
        );
    } else if let (Some(secret), Some(qr)) = (enroll_secret, enroll_qr_svg) {
        body.push_str(&format!(
            r#"
  <h2 style="margin:16px 0 8px;font-size:1.1rem;">Authenticator app (TOTP)</h2>
  <div class="mfa-totp-setup" style="margin:16px 0;">
    <div class="mfa-qr-wrap">{qr}</div>
    <p class="mfa-secret">Secret: <code style="user-select:all;">{secret}</code></p>
    <form method="post" action="/account/security/enroll-2fa/confirm" class="stack-form" style="display:grid;gap:12px;margin-top:0;max-width:100%;">
      <label>Authenticator code
        <input type="text" name="code" required autocomplete="one-time-code" inputmode="numeric" pattern="[0-9 ]*" maxlength="12">
      </label>
      <button type="submit" class="btn-primary">Confirm TOTP</button>
    </form>
  </div>"#,
            qr = qr,
            secret = html_escape(secret),
        ));
        body.push_str(&enroll_passkey_section_html());
    } else {
        body.push_str(
            r#"
  <h2 style="margin:16px 0 8px;font-size:1.1rem;">Authenticator app (TOTP)</h2>
  <form method="post" action="/account/security/enroll-2fa/begin" style="margin:16px 0;">
    <button type="submit" class="btn-primary">Start TOTP enrollment</button>
  </form>"#,
        );
        body.push_str(&enroll_passkey_section_html());
    }

    body.push_str("</section>");
    body
}

/// Nav keys that remain reachable while MFA enrollment is still pending.
///
/// Password-change remains a hard gate for every area except the security pages.
/// Settings hub and all settings children use `active = "settings"` in `panel_shell`.
pub fn mfa_enrollment_allows_nav(active: &str) -> bool {
    matches!(active, "account-security" | "settings")
}

/// If a gate applies, return (active_nav_key, title, main_html).
pub fn security_gate_override(
    username: &str,
    active: &str,
) -> Option<(&'static str, &'static str, String)> {
    // Gate pages themselves use active = "account-security".
    if active == "account-security" {
        return None;
    }
    if must_change_password(username) {
        return Some((
            "account-security",
            "Change password",
            change_password_gate_main(None, None),
        ));
    }
    if needs_mfa_enrollment(username) {
        if mfa_enrollment_allows_nav(active) {
            return None;
        }
        return Some((
            "account-security",
            "Enable 2FA",
            enroll_mfa_gate_main(None, None, None, None, None),
        ));
    }
    None
}

/// Preferred post-login path when gates apply.
pub fn post_login_security_path(username: &str) -> Option<&'static str> {
    if must_change_password(username) {
        return Some("/account/security/change-password");
    }
    if needs_mfa_enrollment(username) {
        return Some("/account/security/enroll-2fa");
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::{default_password_policy, with_test_data_dir};
    use crate::account_mgmt::create_account;

    #[test]
    fn generated_account_requires_password_change_and_mfa() {
        with_test_data_dir(|| {
            unsafe {
                std::env::set_var("CPN_RESERVED_USERNAMES_OFFLINE", "1");
            }
            let created = create_account(
                "panelowner",
                None,
                true,
                "owner@example.com",
                default_password_policy(),
                "en",
            )
            .expect("create");
            assert!(created.generated_password.is_some());
            assert!(must_change_password("panelowner"));
            assert!(needs_mfa_enrollment("panelowner"));
            clear_must_change_password("panelowner").unwrap();
            assert!(!must_change_password("panelowner"));
            assert!(needs_mfa_enrollment("panelowner"));
            unsafe {
                std::env::remove_var("CPN_RESERVED_USERNAMES_OFFLINE");
            }
        });
    }

    #[test]
    fn password_change_still_gates_settings() {
        with_test_data_dir(|| {
            unsafe {
                std::env::set_var("CPN_RESERVED_USERNAMES_OFFLINE", "1");
            }
            let created = create_account(
                "pwdgate",
                None,
                true,
                "pwdgate@example.com",
                default_password_policy(),
                "en",
            )
            .expect("create");
            assert!(created.generated_password.is_some());
            assert!(must_change_password("pwdgate"));
            let gated = security_gate_override("pwdgate", "settings");
            assert!(
                gated.is_some(),
                "forced password change must still block Settings"
            );
            assert_eq!(gated.unwrap().1, "Change password");
            unsafe {
                std::env::remove_var("CPN_RESERVED_USERNAMES_OFFLINE");
            }
        });
    }

    #[test]
    fn mfa_gate_allows_settings_but_not_dashboard() {
        assert!(mfa_enrollment_allows_nav("settings"));
        assert!(mfa_enrollment_allows_nav("account-security"));
        assert!(!mfa_enrollment_allows_nav("dashboard"));
        assert!(!mfa_enrollment_allows_nav("websites"));
        assert!(!mfa_enrollment_allows_nav("server"));
    }

    #[test]
    fn security_gate_skips_settings_while_mfa_pending() {
        with_test_data_dir(|| {
            unsafe {
                std::env::set_var("CPN_RESERVED_USERNAMES_OFFLINE", "1");
            }
            let created = create_account(
                "settingsgate",
                None,
                true,
                "settingsgate@example.com",
                default_password_policy(),
                "en",
            )
            .expect("create");
            assert!(created.generated_password.is_some());
            clear_must_change_password("settingsgate").unwrap();
            assert!(needs_mfa_enrollment("settingsgate"));
            assert!(
                security_gate_override("settingsgate", "settings").is_none(),
                "Settings must stay open while MFA enrollment is pending"
            );
            let dashboard = security_gate_override("settingsgate", "dashboard");
            assert!(
                dashboard.is_some(),
                "Dashboard must stay behind the MFA enroll gate"
            );
            assert_eq!(dashboard.unwrap().1, "Enable 2FA");
            unsafe {
                std::env::remove_var("CPN_RESERVED_USERNAMES_OFFLINE");
            }
        });
    }

    #[test]
    fn enroll_gate_offers_passkey_alongside_totp() {
        let start = enroll_mfa_gate_main(None, None, None, None, None);
        assert!(
            start.contains("href=\"/settings\""),
            "enroll page must link to Settings while MFA is pending"
        );
        assert!(
            start.contains("Register passkey"),
            "start view must offer passkey enrollment"
        );
        assert!(
            start.contains("cpnRegisterPasskey"),
            "start view must include passkey client script"
        );
        assert!(
            start.contains("data-redirect=\"/account/users/modify?notice=Passkey+registered\""),
            "enroll gate passkey returns to Modify User"
        );

        let mid = enroll_mfa_gate_main(
            None,
            None,
            Some("TESTSECRET"),
            Some("<svg xmlns=\"http://www.w3.org/2000/svg\"></svg>"),
            None,
        );
        assert!(
            mid.contains("Confirm TOTP") && mid.contains("Register passkey"),
            "TOTP-in-progress view must still offer passkey"
        );

        let done = enroll_mfa_gate_main(
            Some("ok"),
            None,
            None,
            None,
            Some(&[String::from("AAAA-BBBB")]),
        );
        assert!(
            done.contains("Continue to Modify User"),
            "backup-codes view must continue to Modify User"
        );
        assert!(
            done.contains("href=\"/account/users/modify\""),
            "backup-codes continue link must target Modify User"
        );
        assert!(
            done.contains("Copy to clipboard") && done.contains("Download"),
            "backup-codes view must offer copy and download"
        );
        assert!(
            done.contains("cpn-backup-codes.txt"),
            "download must use cpn-backup-codes.txt filename"
        );
        assert!(
            done.contains("mfa-codes-panel") && done.contains("AAAA-BBBB"),
            "backup codes must render in theme-aware panel"
        );
        assert!(
            !done.contains("Register passkey"),
            "backup-codes view is TOTP-complete; no passkey CTA needed"
        );
    }
}
