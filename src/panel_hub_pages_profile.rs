//! Self-service account profile HTML (edit details, password, TOTP).

use crate::account_mgmt::find_account;
use crate::packages::package_for_account;
use crate::panel_hubs::{feature_shell, not_configured_body};

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn users_profile_page(
    username: &str,
    notice: Option<&str>,
    error: Option<&str>,
    enroll_secret: Option<&str>,
    enroll_qr_svg: Option<&str>,
    backup_codes: Option<&[String]>,
    generated_password: Option<&str>,
) -> String {
    let mfa = crate::account_mfa::load_mfa(username);
    let (detail, package_note) = match find_account(username) {
        Ok((boot, _)) => {
            let pkg = package_for_account(username)
                .map(|p| format!("{} ({})", p.name, p.id))
                .unwrap_or_else(|_| "Default package".into());
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
        <p class="muted">Hosting package: <strong>{pkg}</strong></p>

        <form method="post" action="/account/users/profile/details" class="stack-form" style="max-width:520px;display:grid;gap:12px;margin-bottom:28px;">
          <h3 style="margin:0;">Account details</h3>
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
                pkg = html_escape(&pkg),
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

            body.push_str(
                r#"
        </div>

        <div class="stack-form" style="max-width:560px;display:grid;gap:8px;">
          <h3 style="margin:0;">Passkeys (WebAuthn)</h3>
          <p class="muted" style="margin:0;">Passkey register, list, and sign-in are planned next. TOTP 2FA above is fully available in this release.</p>
        </div>"#,
            );
            (body, String::new())
        }
        Err(err) => (
            not_configured_body(&err, "Sign in again if this account was removed."),
            String::new(),
        ),
    };
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Users & Plans", Some("/account/users")),
            ("Account profile", None),
        ],
        "Account profile",
        "Edit your account, password, and two-factor authentication.",
        &format!("{detail}{package_note}"),
        notice,
        error,
    )
}
