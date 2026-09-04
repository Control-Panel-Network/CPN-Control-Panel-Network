//! Account profile view and self-edit form fragments (used by Modify User).

use crate::account_mgmt::find_account;
use crate::account_passkeys::list_passkey_summaries;
use crate::packages::{is_panel_admin, package_for_account};
use crate::panel_hubs::{feature_shell, not_configured_body};
use crate::panel_webauthn::passkey_client_script;

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Read-only profile summary with Edit → `/account/users/modify`.
pub fn users_profile_page(
    username: &str,
    notice: Option<&str>,
    error: Option<&str>,
    _enroll_secret: Option<&str>,
    _enroll_qr_svg: Option<&str>,
    _backup_codes: Option<&[String]>,
    _generated_password: Option<&str>,
) -> String {
    let body = match find_account(username) {
        Ok((boot, _)) => {
            let pkg = package_for_account(username)
                .map(|p| format!("{} ({})", p.name, p.id))
                .unwrap_or_else(|_| "Default package".into());
            let mfa = crate::account_mfa::load_mfa(username);
            let totp = if mfa.totp_enabled {
                "Enabled"
            } else {
                "Disabled"
            };
            let passkeys = list_passkey_summaries(username).len();
            format!(
                r#"
        <div style="display:flex;justify-content:flex-end;margin-bottom:12px;">
          <a class="btn-primary" href="/account/users/modify">Edit</a>
        </div>
        <ul class="kv-list">
          <li><span>Username</span><strong>{username}</strong></li>
          <li><span>Recovery email</span><strong>{email}</strong></li>
          <li><span>Language</span><strong>{lang}</strong></li>
          <li><span>Hosting package</span><strong>{pkg}</strong></li>
          <li><span>TOTP 2FA</span><strong>{totp}</strong></li>
          <li><span>Passkeys</span><strong>{passkeys}</strong></li>
        </ul>
        <p class="muted" style="margin-top:16px;">Use Edit to change password, recovery email, username, TOTP, or passkeys.</p>"#,
                username = html_escape(&boot.username),
                email = html_escape(&boot.recovery_email),
                lang = html_escape(&boot.language),
                pkg = html_escape(&pkg),
                totp = totp,
                passkeys = passkeys,
            )
        }
        Err(err) => not_configured_body(&err, "Sign in again if this account was removed."),
    };
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Users & Plans", Some("/account/users")),
            ("View Profile", None),
        ],
        "View Profile",
        "Your account details.",
        &body,
        notice,
        error,
    )
}

/// Self-service edit forms (details, password, TOTP, Passkeys) for the signed-in user.
pub fn users_self_edit_body(
    username: &str,
    enroll_secret: Option<&str>,
    enroll_qr_svg: Option<&str>,
    backup_codes: Option<&[String]>,
    generated_password: Option<&str>,
) -> String {
    let mfa = crate::account_mfa::load_mfa(username);
    let Ok((boot, _)) = find_account(username) else {
        return not_configured_body(
            "Account not found",
            "Sign in again if this account was removed.",
        );
    };
    let lang = boot.language.clone();
    let en_sel = if lang == "en" { " selected" } else { "" };
    let es_sel = if lang == "es" { " selected" } else { "" };
    let nb_sel = if lang == "nb" { " selected" } else { "" };
    let totp_status = if mfa.totp_enabled {
        "Enabled"
    } else {
        "Disabled"
    };
    let mut body = format!(
        r#"
      <p style="margin:0 0 16px;"><a class="btn-secondary" href="/account/users/profile">Back to profile</a></p>
      <h3 style="margin:0 0 12px;">Your account</h3>
      <form method="post" action="/account/users/profile/details" class="stack-form" style="max-width:520px;display:grid;gap:12px;margin-bottom:28px;">
        <label>Username
          <input name="username" type="text" required autocomplete="username" maxlength="128" value="{username}">
        </label>
        <label>Recovery email
          <input name="recovery_email" type="email" required autocomplete="email" maxlength="254" value="{email}">
        </label>
        <label>Language
          <select name="language">
            <option value="en"{en_sel}>English</option>
            <option value="es"{es_sel}>Español</option>
            <option value="nb"{nb_sel}>Norsk</option>
          </select>
        </label>
        <button type="submit" class="btn-primary">Save details</button>
      </form>

      <form method="post" action="/account/users/profile/password" class="stack-form" style="max-width:520px;display:grid;gap:12px;margin-bottom:28px;">
        <h3 style="margin:0;">Change password</h3>
        <label>Current password
          <input name="current_password" type="password" required autocomplete="current-password" maxlength="256">
        </label>
        <label>New password (leave blank to generate)
          <input name="password" type="password" autocomplete="new-password" maxlength="256">
        </label>
        <label style="display:flex;align-items:center;gap:8px;">
          <input name="generate" type="checkbox" value="1">
          Generate a strong password
        </label>
        <button type="submit" class="btn-primary">Update password</button>
      </form>

      <div class="stack-form" style="max-width:560px;display:grid;gap:12px;margin-bottom:28px;">
        <h3 style="margin:0;">Two-factor authentication (TOTP)</h3>
        <p class="muted" style="margin:0;">Status: <strong>{totp_status}</strong>. Secrets are stored encrypted under the CPN data directory.</p>"#,
        username = html_escape(&boot.username),
        email = html_escape(&boot.recovery_email),
        en_sel = en_sel,
        es_sel = es_sel,
        nb_sel = nb_sel,
        totp_status = totp_status,
    );

    if let Some(password) = generated_password {
        body.push_str(&format!(
            r#"<p class="panel-notice ok" role="status"><strong>Generated password</strong> (copy now): <code style="user-select:all;">{pw}</code></p>"#,
            pw = html_escape(password)
        ));
    }
    if let Some(codes) = backup_codes {
        body.push_str(
            r#"<p class="panel-notice ok" role="status"><strong>Backup codes</strong> (copy now; each works once):</p><ul>"#,
        );
        for code in codes {
            body.push_str(&format!(
                r#"<li><code style="user-select:all;">{c}</code></li>"#,
                c = html_escape(code)
            ));
        }
        body.push_str("</ul>");
    }
    if let (Some(secret), Some(svg)) = (enroll_secret, enroll_qr_svg) {
        body.push_str(&format!(
            r#"
        <div style="display:grid;gap:10px;padding:12px;border:1px solid var(--border,#334155);border-radius:10px;">
          <p style="margin:0;">Scan this QR with your authenticator app, or enter the secret manually.</p>
          <div style="background:#fff;padding:8px;border-radius:8px;width:fit-content;">{svg}</div>
          <p style="margin:0;"><strong>Secret:</strong> <code style="user-select:all;">{secret}</code></p>
          <form method="post" action="/account/users/profile/totp/confirm" class="stack-form" style="display:grid;gap:10px;">
            <label>Authenticator code
              <input name="code" type="text" inputmode="numeric" pattern="[0-9]{{6}}" maxlength="6" required autocomplete="one-time-code">
            </label>
            <button type="submit" class="btn-primary">Confirm and enable</button>
          </form>
        </div>"#,
            svg = svg,
            secret = html_escape(secret),
        ));
    } else if mfa.totp_enabled {
        body.push_str(
            r#"
        <form method="post" action="/account/users/profile/totp/disable" class="stack-form" style="display:grid;gap:10px;">
          <label>Current password
            <input name="current_password" type="password" required autocomplete="current-password" maxlength="256">
          </label>
          <label>Authenticator or backup code
            <input name="code" type="text" required autocomplete="one-time-code" maxlength="32">
          </label>
          <button type="submit" class="btn-secondary">Disable TOTP</button>
        </form>"#,
        );
    } else {
        body.push_str(
            r#"
        <form method="post" action="/account/users/profile/totp/begin">
          <button type="submit" class="btn-primary">Enable TOTP</button>
        </form>"#,
        );
    }
    body.push_str("</div>");

    // Passkeys
    body.push_str(
        r#"
      <div class="stack-form" style="max-width:560px;display:grid;gap:12px;margin-bottom:28px;">
        <h3 style="margin:0;">Passkeys (WebAuthn)</h3>
        <p class="muted" style="margin:0;">Register a platform or security-key passkey for passwordless sign-in. Credentials are stored under the CPN data directory.</p>"#,
    );
    let keys = list_passkey_summaries(username);
    if keys.is_empty() {
        body.push_str(r#"<p class="empty-state">No passkeys registered yet.</p>"#);
    } else {
        body.push_str(r#"<div class="table-wrap"><table class="data-table"><thead><tr><th>Label</th><th>Created</th><th></th></tr></thead><tbody>"#);
        for (id, label, created, _) in &keys {
            body.push_str(&format!(
                r#"<tr>
              <td><strong>{label}</strong></td>
              <td>{created}</td>
              <td>
                <form method="post" action="/account/users/profile/passkey/delete" style="display:inline;"
                      onsubmit="return confirm('Remove this passkey?');">
                  <input type="hidden" name="id" value="{id}">
                  <button type="submit" class="linkish" style="background:none;border:0;color:#d92d20;font-weight:600;cursor:pointer;padding:0;">Remove</button>
                </form>
              </td>
            </tr>"#,
                label = html_escape(label),
                created = created,
                id = html_escape(id),
            ));
        }
        body.push_str("</tbody></table></div>");
    }
    body.push_str(
        r#"
        <label>Label (optional)
          <input id="cpn-passkey-label" type="text" maxlength="64" placeholder="Laptop / YubiKey">
        </label>
        <button type="button" class="btn-primary" onclick="cpnRegisterPasskey()">Register passkey</button>
        <p id="cpn-passkey-status" class="muted" role="status"></p>
      </div>
      <script>"#,
    );
    body.push_str(passkey_client_script());
    body.push_str("</script>");
    body
}

/// Modify page: self-edit for everyone; other-user admin tools when viewer is admin.
pub fn users_modify_page(
    viewer: &str,
    notice: Option<&str>,
    error: Option<&str>,
    enroll_secret: Option<&str>,
    enroll_qr_svg: Option<&str>,
    backup_codes: Option<&[String]>,
    generated_password: Option<&str>,
) -> String {
    let mut body = users_self_edit_body(
        viewer,
        enroll_secret,
        enroll_qr_svg,
        backup_codes,
        generated_password,
    );
    if is_panel_admin(viewer) {
        body.push_str(&admin_other_users_section());
    }
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Users & Plans", Some("/account/users")),
            ("Modify User", None),
        ],
        "Modify User",
        "Edit your account, or manage other panel users.",
        &body,
        notice,
        error,
    )
}

fn admin_other_users_section() -> String {
    use crate::account_mgmt::list_accounts;
    let accounts = list_accounts().unwrap_or_default();
    let mut options = String::new();
    for acct in &accounts {
        if is_panel_admin(&acct.username) {
            continue;
        }
        options.push_str(&format!(
            r#"<option value="{u}">{u}</option>"#,
            u = html_escape(&acct.username),
        ));
    }
    let select_inner = if options.is_empty() {
        r#"<option value="">No non-admin accounts</option>"#.to_string()
    } else {
        options
    };
    format!(
        r#"
      <hr style="margin:28px 0;border:0;border-top:1px solid var(--border,#334155);">
      <h3 style="margin:0 0 12px;">Other accounts (admin)</h3>
      <form method="post" action="/account/users/password" class="stack-form" style="max-width:520px;display:grid;gap:12px;margin-bottom:28px;">
        <h4 style="margin:0;">Reset password</h4>
        <label>Username
          <select name="username" required>{select_inner}</select>
        </label>
        <label>New password (leave blank to generate)
          <input name="password" type="password" autocomplete="new-password" maxlength="256">
        </label>
        <label style="display:flex;align-items:center;gap:8px;">
          <input name="generate" type="checkbox" value="1">
          Generate a strong password
        </label>
        <button type="submit" class="btn-primary">Reset password</button>
      </form>
      <form method="post" action="/account/users/delete" class="stack-form" style="max-width:520px;display:grid;gap:12px;"
            onsubmit="return confirm('Delete this panel account? This cannot be undone.');">
        <h4 style="margin:0;">Delete user</h4>
        <label>Username
          <select name="username" required>{select_inner}</select>
        </label>
        <button type="submit" class="btn-secondary">Delete user</button>
      </form>
      <p class="muted">The bootstrap admin account cannot be deleted from this screen.</p>"#
    )
}
