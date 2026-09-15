use crate::account::load_bootstrap;
use crate::auth_i18n::PANEL_I18N_SCRIPT;
use crate::model::InstallerStatus;
use crate::panel_brand::brand_favicon_links;

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn normalize_panel_locale(raw: &str) -> &'static str {
    let value = raw.trim().to_lowercase();
    if value.starts_with("es") {
        "es"
    } else if value.starts_with("nb") || value == "no" || value.starts_with("nn") {
        "nb"
    } else {
        "en"
    }
}

fn resolve_initial_locale(status: &InstallerStatus) -> &'static str {
    if let Some(boot) = load_bootstrap() {
        return normalize_panel_locale(&boot.language);
    }
    normalize_panel_locale(&status.language)
}

fn shared_auth_styles() -> &'static str {
    r#"
    body { margin:0; font-family:"Segoe UI",system-ui,sans-serif; background:#f5f5f7; color:#1d1d1f; }
    main { min-height:100vh; display:grid; place-items:center; padding:48px 20px; }
    .card { width:min(100%,420px); background:#fff; border:1px solid #e0e0e0; border-radius:22px; padding:32px; position:relative; box-shadow:0 18px 50px rgba(16,24,40,.08); }
    h1 { margin:0 0 8px; font-size:1.7rem; }
    p { color:#6e6e73; line-height:1.5; }
    label { display:block; margin:14px 0 6px; font-weight:600; font-size:.92rem; }
    input { width:100%; box-sizing:border-box; border:1px solid #d0d5dd; border-radius:10px; padding:11px 12px; font:inherit; }
    input:focus { outline:3px solid rgba(0,102,204,.16); border-color:#0066cc; }
    button { margin-top:18px; width:100%; border:0; border-radius:999px; padding:12px 16px; background:#0066cc; color:#fff; font-weight:700; cursor:pointer; }
    button:disabled { opacity:.55; cursor:not-allowed; }
    .btn-secondary { margin-top:8px; width:100%; border:1px solid #d0d5dd; border-radius:999px; padding:12px 16px; background:#fff; color:#1d1d1f; font-weight:700; cursor:pointer; }
    .btn-secondary:disabled { opacity:.55; cursor:not-allowed; }
    .row { display:flex; justify-content:space-between; align-items:center; gap:12px; }
    .remember { display:flex; align-items:center; gap:8px; margin:14px 0 0; font-weight:600; font-size:.92rem; }
    .remember input { width:auto; margin:0; }
    a { color:#0066cc; text-decoration:none; font-size:.92rem; }
    .hint { margin-top:14px; font-size:.9rem; }
    .error {
      margin:0 0 14px; padding:12px 14px; border-radius:10px; background:#fef2f2;
      border:1px solid #fecaca; color:#991b1b; font-size:.95rem; font-weight:650; line-height:1.45;
    }
    .notice {
      margin:0 0 14px; padding:12px 14px; border-radius:10px; background:#fffbeb;
      border:1px solid #fde68a; color:#92400e; font-size:.92rem; font-weight:600; line-height:1.45;
    }
    .notice[data-ready="1"] {
      background:#ecfdf5; border-color:#a7f3d0; color:#065f46;
    }
    .passkey-status { margin-top:10px; font-size:.9rem; color:#475467; min-height:1.2em; }
    .lang-host { position:absolute; top:20px; right:20px; }
    .lang { display:flex; align-items:center; gap:7px; margin:0; border:1px solid #d0d5dd; border-radius:999px; padding:7px 10px; background:#fff; color:#344054; font-weight:650; font-size:.8rem; }
    .lang select { min-width:84px; border:0; padding:0; font:inherit; background:#fff; color:#1d1d1f; cursor:pointer; outline:0; }
    .lang-label { position:absolute; width:1px; height:1px; overflow:hidden; clip:rect(0 0 0 0); }
    .brand-logo { display:block; width:190px; height:72px; margin:-8px 0 10px; object-fit:contain; object-position:left center; }
    .password-wrap { position:relative; }
    .password-wrap input { padding-right:48px; }
    .password-toggle { position:absolute; top:50%; right:5px; width:38px; height:38px; margin:0; padding:0; border-radius:8px; background:transparent; color:#475467; transform:translateY(-50%); }
    .password-toggle:hover { background:#f2f4f7; }
    .command-box { margin:22px 0; border:1px solid #b8d8fa; border-radius:14px; padding:16px; background:#f4f9ff; }
    .command-box code { display:block; margin-top:8px; border-radius:9px; padding:12px; background:#101828; color:#f8fafc; font:600 .9rem ui-monospace,SFMono-Regular,Consolas,monospace; user-select:all; }
    @media (max-width:520px) { .card { padding:24px; } .lang-host { position:static; display:flex; justify-content:flex-end; margin-bottom:12px; } .brand-logo { width:160px; } }
"#
}

pub fn panel_login_html(
    status: &InstallerStatus,
    error: Option<&str>,
    next: Option<&str>,
) -> String {
    let gate = crate::login_service_gate::evaluate_login_services();
    panel_login_html_with_gate(status, error, next, &gate)
}

pub fn panel_login_html_with_gate(
    status: &InstallerStatus,
    error: Option<&str>,
    next: Option<&str>,
    gate: &crate::login_service_gate::LoginServiceStatus,
) -> String {
    let initial_locale = resolve_initial_locale(status);
    let safe_next = next.and_then(crate::login_next::sanitize_login_next);
    let mut action_q = String::new();
    if let Some(token) = status
        .panel_login_url
        .as_ref()
        .and_then(|url| url.split("token=").nth(1))
    {
        action_q.push_str("?token=");
        action_q.push_str(&html_escape(token));
    }
    if let Some(ref n) = safe_next {
        action_q.push(if action_q.is_empty() { '?' } else { '&' });
        action_q.push_str("next=");
        action_q.push_str(&html_escape(n));
    }
    let next_hidden = match safe_next.as_deref() {
        Some(n) => format!(
            r#"<input type="hidden" name="next" value="{val}">"#,
            val = html_escape(n)
        ),
        None => String::new(),
    };
    let services_block = if !gate.ready {
        format!(
            r#"<p class="notice" id="cpn-login-services" role="status" data-ready="0">{msg}</p>"#,
            msg = html_escape(&gate.message)
        )
    } else if !gate.warnings.is_empty() {
        format!(
            r#"<p class="notice" id="cpn-login-services" role="status" data-ready="1">{msg}</p>"#,
            msg = html_escape(gate.warnings.first().map(String::as_str).unwrap_or(""))
        )
    } else {
        r#"<p class="notice" id="cpn-login-services" role="status" data-ready="1" hidden></p>"#.into()
    };
    let error_block = match error {
        Some(message) if !message.is_empty() => format!(
            r#"<p class="error" id="i18n-login-error" role="alert">{msg}</p>"#,
            msg = html_escape(message)
        ),
        _ => r#"<p class="error" id="i18n-login-error" role="alert" hidden></p>"#.into(),
    };
    let disabled = if gate.ready { "" } else { " disabled" };
    let poll_script = r#"
(function(){
  var notice=document.getElementById('cpn-login-services');
  var form=document.querySelector('form[action*="/login"]');
  var submit=document.getElementById('i18n-submit');
  var passkey=document.getElementById('i18n-passkey');
  function applyReady(ready,message){
    if(notice){
      if(message){ notice.hidden=false; notice.textContent=message; }
      else { notice.hidden=true; notice.textContent=''; }
      notice.setAttribute('data-ready', ready ? '1' : '0');
    }
    if(submit) submit.disabled=!ready;
    if(passkey) passkey.disabled=!ready;
    if(form){
      Array.prototype.forEach.call(form.querySelectorAll('input'), function(el){
        if(el.type==='hidden') return;
        el.disabled=!ready;
      });
    }
  }
  async function poll(){
    try{
      var res=await fetch('/api/login/services',{credentials:'same-origin',headers:{'Accept':'application/json'}});
      var data=await res.json().catch(function(){return {};});
      var ready=!!data.ready;
      var msg=ready ? ((data.warnings&&data.warnings[0])||'') : (data.message||'');
      applyReady(ready,msg);
      if(ready && notice && notice.getAttribute('data-was-blocked')==='1'){
        location.reload();
        return;
      }
      if(!ready && notice) notice.setAttribute('data-was-blocked','1');
    }catch(e){}
    setTimeout(poll, 4000);
  }
  if(notice && notice.getAttribute('data-ready')==='0'){
    notice.setAttribute('data-was-blocked','1');
  }
  setTimeout(poll, 4000);
})();
"#;

    format!(
        r#"<!DOCTYPE html>
<html lang="{locale}" data-initial-locale="{locale}">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Sign in · CPN Panel</title>
  {favicons}
  <style>{styles}</style>
</head>
<body data-page="login"{error_attr}{next_attr}{gate_attr}>
  <main>
    <section class="card">
      <div id="cpn-lang-host" class="lang-host"></div>
      <img class="brand-logo" src="/cpn-logo.png" alt="CPN Control Panel Network">
      <h1 id="i18n-title">Sign in</h1>
      {services_block}
      {error_block}
      <form method="post" action="/login{action_q}" autocomplete="on">
        {next_hidden}
        <label for="username" id="i18n-username">Username</label>
        <input id="username" name="username" value="" autocomplete="username" required{disabled}>
        <label for="password" id="i18n-password">Password</label>
        <div class="password-wrap">
          <input id="password" name="password" type="password" value="" autocomplete="current-password" required{disabled}>
          <button class="password-toggle" type="button" aria-label="Show password" onclick="cpnTogglePassword(this)">👁</button>
        </div>
        <label class="remember" for="remember_me">
          <input id="remember_me" name="remember_me" type="checkbox" value="1"{disabled}>
          <span id="i18n-remember">Remember me</span>
        </label>
        <div class="row">
          <span></span>
          <a id="i18n-forgot" href="/forgot-password">Reset password</a>
        </div>
        <button id="i18n-submit" type="submit"{disabled}>Sign in</button>
      </form>
      <p id="i18n-or" class="hint" style="margin-top:18px;text-align:center;">or</p>
      <button id="i18n-passkey" class="btn-secondary" type="button" onclick="cpnLoginPasskey()"{disabled}>Sign in with passkey</button>
      <p id="cpn-passkey-login-status" class="passkey-status" role="status"></p>
    </section>
  </main>
  <script>{passkey_script}</script>
  <script>{poll_script}</script>
  {script}
</body>
</html>"#,
        locale = initial_locale,
        favicons = brand_favicon_links(),
        styles = shared_auth_styles(),
        action_q = action_q,
        next_hidden = next_hidden,
        services_block = services_block,
        error_block = error_block,
        disabled = disabled,
        error_attr = if error.is_some() {
            r#" data-login-error="1""#
        } else {
            ""
        },
        next_attr = match safe_next.as_deref() {
            Some(n) => format!(r#" data-login-next="{}""#, html_escape(n)),
            None => String::new(),
        },
        gate_attr = if gate.ready {
            r#" data-services-ready="1""#
        } else {
            r#" data-services-ready="0""#
        },
        passkey_script = crate::panel_webauthn::passkey_client_script(),
        poll_script = poll_script,
        script = PANEL_I18N_SCRIPT,
    )
}

#[derive(Debug, Clone, Copy)]
pub struct MfaPageOptions {
    pub totp_available: bool,
    pub passkey_available: bool,
}

pub fn panel_mfa_html(
    status: &InstallerStatus,
    error: Option<&str>,
    next: Option<&str>,
    options: MfaPageOptions,
) -> String {
    let initial_locale = resolve_initial_locale(status);
    let safe_next = next.and_then(crate::login_next::sanitize_login_next);
    let next_hidden = match safe_next.as_deref() {
        Some(n) => format!(
            r#"<input type="hidden" name="next" value="{val}">"#,
            val = html_escape(n)
        ),
        None => String::new(),
    };
    let back_href = crate::login_next::login_location(safe_next.as_deref());
    let error_block = match error {
        Some(message) if !message.is_empty() => format!(
            r#"<p class="error" id="i18n-login-error" role="alert">{msg}</p>"#,
            msg = html_escape(message)
        ),
        _ => r#"<p class="error" id="i18n-login-error" role="alert" hidden></p>"#.into(),
    };
    let intro = if options.totp_available && options.passkey_available {
        "Enter the 6-digit code from your authenticator app, a one-time backup code, or sign in with a passkey."
    } else if options.passkey_available {
        "Confirm sign-in with a registered passkey for this account."
    } else {
        "Enter the 6-digit code from your authenticator app, or a one-time backup code."
    };
    let totp_form = if options.totp_available {
        format!(
            r#"<form method="post" action="/login/2fa" autocomplete="off">
        {next_hidden}
        <label for="code">Authenticator code</label>
        <input id="code" name="code" type="text" inputmode="numeric" autocomplete="one-time-code" required maxlength="32" autofocus>
        <button type="submit">Verify</button>
      </form>"#,
            next_hidden = next_hidden
        )
    } else {
        String::new()
    };
    let passkey_block = if options.passkey_available {
        let or_line = if options.totp_available {
            r#"<p id="i18n-or" class="hint" style="margin-top:18px;text-align:center;">or</p>"#
        } else {
            ""
        };
        format!(
            r#"{or_line}
      <button id="i18n-passkey" class="btn-secondary" type="button" onclick="cpnMfaPasskey()">Sign in with passkey</button>
      <p id="cpn-passkey-login-status" class="passkey-status" role="status"></p>"#,
            or_line = or_line
        )
    } else {
        String::new()
    };
    let passkey_script = if options.passkey_available {
        crate::panel_webauthn::passkey_client_script()
    } else {
        ""
    };
    format!(
        r#"<!DOCTYPE html>
<html lang="{locale}" data-initial-locale="{locale}">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Two-factor · CPN Panel</title>
  {favicons}
  <style>{styles}</style>
</head>
<body data-page="login-mfa"{next_attr}>
  <main>
    <section class="card">
      <div id="cpn-lang-host" class="lang-host"></div>
      <p id="i18n-brand" style="color:#0066cc;font-size:12px;font-weight:700;letter-spacing:.08em;margin:0 0 8px;">CPN PANEL</p>
      <h1>Two-factor authentication</h1>
      <p class="hint">{intro}</p>
      {error_block}
      {totp_form}
      {passkey_block}
      <p class="hint"><a href="{back_href}">Back to sign in</a></p>
    </section>
  </main>
  <script>{passkey_script}</script>
  {script}
</body>
</html>"#,
        locale = initial_locale,
        favicons = brand_favicon_links(),
        styles = shared_auth_styles(),
        intro = html_escape(intro),
        error_block = error_block,
        totp_form = totp_form,
        passkey_block = passkey_block,
        back_href = html_escape(&back_href),
        next_attr = match safe_next.as_deref() {
            Some(n) => format!(r#" data-login-next="{}""#, html_escape(n)),
            None => String::new(),
        },
        passkey_script = passkey_script,
        script = PANEL_I18N_SCRIPT,
    )
}

pub fn forgot_password_html() -> String {
    let boot = load_bootstrap();
    let initial_locale = boot
        .as_ref()
        .map(|value| normalize_panel_locale(&value.language))
        .unwrap_or("en");
    format!(
        r#"<!DOCTYPE html>
<html lang="{locale}" data-initial-locale="{locale}">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Forgot password · CPN Panel</title>
  {favicons}
  <style>{styles}</style>
</head>
<body data-page="forgot">
  <main>
    <section class="card">
      <div id="cpn-lang-host" class="lang-host"></div>
      <img class="brand-logo" src="/cpn-logo.png" alt="CPN Control Panel Network">
      <h1 id="i18n-title">Forgot password</h1>
      <p class="hint" id="i18n-forgot-intro">Enter your username or email. If a matching account exists, we email a one-time reset link when SMTP or local Postfix is available.</p>
      <form method="post" action="/forgot-password" autocomplete="on">
        <label for="account" id="i18n-forgot-account">Username/Email</label>
        <input id="account" name="account" type="text" autocomplete="username" required>
        <button id="i18n-forgot-submit" type="submit">Request reset</button>
      </form>
      <p class="hint" id="i18n-forgot-smtp">Reset emails use configured SMTP when present, otherwise local Postfix if it is running. The message includes a time-limited link to set a new password.</p>
      <div class="command-box">
        <strong id="i18n-forgot-cli">If email cannot be delivered, a server operator can run:</strong>
        <code>sudo cpn password</code>
      </div>
      <p><a id="i18n-forgot-back" href="/login">Back to sign in</a></p>
    </section>
  </main>
  {script}
</body>
</html>"#,
        locale = initial_locale,
        favicons = brand_favicon_links(),
        styles = shared_auth_styles(),
        script = PANEL_I18N_SCRIPT,
    )
}

pub fn forgot_password_ack_html() -> String {
    let boot = load_bootstrap();
    let initial_locale = boot
        .as_ref()
        .map(|value| normalize_panel_locale(&value.language))
        .unwrap_or("en");
    format!(
        r#"<!DOCTYPE html>
<html lang="{locale}" data-initial-locale="{locale}">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Forgot password · CPN Panel</title>
  {favicons}
  <style>{styles}</style>
</head>
<body data-page="forgot-ack">
  <main>
    <section class="card">
      <div id="cpn-lang-host" class="lang-host"></div>
      <p id="i18n-brand" style="color:#0066cc;font-size:12px;font-weight:700;letter-spacing:.08em;margin:0 0 8px;">CPN PANEL</p>
      <h1 id="i18n-title">Check your inbox</h1>
      <p class="hint" id="i18n-forgot-ack"></p>
      <p class="hint" id="i18n-forgot-smtp"></p>
      <p><a id="i18n-forgot-back" href="/login">Back to sign in</a></p>
    </section>
  </main>
  {script}
</body>
</html>"#,
        locale = initial_locale,
        favicons = brand_favicon_links(),
        styles = shared_auth_styles(),
        script = PANEL_I18N_SCRIPT,
    )
}

pub fn reset_password_html(token: &str, error: Option<&str>) -> String {
    let boot = load_bootstrap();
    let initial_locale = boot
        .as_ref()
        .map(|value| normalize_panel_locale(&value.language))
        .unwrap_or("en");
    let error_block = match error {
        Some(message) if !message.is_empty() => format!(
            r#"<p class="error" id="i18n-reset-error" role="alert">{msg}</p>"#,
            msg = html_escape(message)
        ),
        _ => r#"<p class="error" id="i18n-reset-error" role="alert" hidden></p>"#.into(),
    };
    format!(
        r#"<!DOCTYPE html>
<html lang="{locale}" data-initial-locale="{locale}">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Reset password · CPN Panel</title>
  {favicons}
  <style>{styles}</style>
</head>
<body data-page="reset-password"{error_attr}>
  <main>
    <section class="card">
      <div id="cpn-lang-host" class="lang-host"></div>
      <p id="i18n-brand" style="color:#0066cc;font-size:12px;font-weight:700;letter-spacing:.08em;margin:0 0 8px;">CPN PANEL</p>
      <h1 id="i18n-title">Choose a new password</h1>
      <p class="hint" id="i18n-reset-intro"></p>
      {error_block}
      <form method="post" action="/reset-password" autocomplete="on">
        <input type="hidden" name="token" value="{token}">
        <label for="password" id="i18n-reset-password">New password</label>
        <input id="password" name="password" type="password" autocomplete="new-password" required>
        <label for="password_confirm" id="i18n-reset-confirm">Confirm password</label>
        <input id="password_confirm" name="password_confirm" type="password" autocomplete="new-password" required>
        <button id="i18n-reset-submit" type="submit">Save new password</button>
      </form>
      <p><a id="i18n-forgot-back" href="/login">Back to sign in</a></p>
    </section>
  </main>
  {script}
</body>
</html>"#,
        locale = initial_locale,
        favicons = brand_favicon_links(),
        styles = shared_auth_styles(),
        token = html_escape(token),
        error_block = error_block,
        error_attr = if error.is_some() {
            r#" data-reset-error="1""#
        } else {
            ""
        },
        script = PANEL_I18N_SCRIPT,
    )
}

pub fn reset_password_invalid_html(message: &str) -> String {
    let boot = load_bootstrap();
    let initial_locale = boot
        .as_ref()
        .map(|value| normalize_panel_locale(&value.language))
        .unwrap_or("en");
    format!(
        r#"<!DOCTYPE html>
<html lang="{locale}" data-initial-locale="{locale}">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Reset password · CPN Panel</title>
  {favicons}
  <style>{styles}</style>
</head>
<body data-page="reset-invalid">
  <main>
    <section class="card">
      <div id="cpn-lang-host" class="lang-host"></div>
      <p id="i18n-brand" style="color:#0066cc;font-size:12px;font-weight:700;letter-spacing:.08em;margin:0 0 8px;">CPN PANEL</p>
      <h1 id="i18n-title">Reset link unavailable</h1>
      <p class="error" role="alert">{msg}</p>
      <p class="hint" id="i18n-reset-invalid-hint"></p>
      <p><a id="i18n-forgot-back" href="/forgot-password">Request a new reset</a></p>
    </section>
  </main>
  {script}
</body>
</html>"#,
        locale = initial_locale,
        favicons = brand_favicon_links(),
        styles = shared_auth_styles(),
        msg = html_escape(message),
        script = PANEL_I18N_SCRIPT,
    )
}

pub fn installer_token_required_html() -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="en" data-initial-locale="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>CPN Installer</title>
  {favicons}
  <style>{styles}</style>
</head>
<body data-page="token">
  <main>
    <section class="card" style="width:min(100%,480px)">
      <div id="cpn-lang-host" class="lang-host"></div>
      <h1 id="i18n-auth-title">Open the installer URL with its token</h1>
      <p id="i18n-auth-body">Installation is not finished yet. Use the full URL printed in the installer console, including the ?token=... query parameter.</p>
      <p><a id="i18n-auth-login" href="/login">If installation already finished, open panel login.</a></p>
    </section>
  </main>
  {script}
</body>
</html>"#,
        favicons = brand_favicon_links(),
        styles = shared_auth_styles(),
        script = PANEL_I18N_SCRIPT,
    )
}

#[cfg(test)]
mod tests {
    use super::{forgot_password_html, panel_login_html, panel_mfa_html, MfaPageOptions};
    use crate::model::InstallerStatus;

    #[test]
    fn login_has_logo_and_password_visibility_control() {
        let html = panel_login_html(&InstallerStatus::default(), None, None);
        assert!(html.contains("/cpn-logo.png"));
        assert!(html.contains("/favicon.ico"));
        assert!(html.contains("cpnTogglePassword"));
        assert!(html.contains("cpn-login-services"));
        assert!(html.contains("/api/login/services"));
    }

    #[test]
    fn mfa_page_shows_passkey_when_available() {
        let html = panel_mfa_html(
            &InstallerStatus::default(),
            None,
            None,
            MfaPageOptions {
                totp_available: true,
                passkey_available: true,
            },
        );
        assert!(html.contains("cpnMfaPasskey"));
        assert!(html.contains("Sign in with passkey"));
        assert!(html.contains("Authenticator code"));
        assert!(html.contains("id=\"i18n-login-error\""));
    }

    #[test]
    fn recovery_offers_email_form_and_cli_fallback() {
        let html = forgot_password_html();
        assert!(html.contains("action=\"/forgot-password\""));
        assert!(html.contains("name=\"account\""));
        assert!(html.contains("sudo cpn password"));
        assert!(
            !html.contains("For security, password recovery is performed directly on the server.")
        );
    }
}
