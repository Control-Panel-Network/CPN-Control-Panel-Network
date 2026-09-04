use crate::account::load_bootstrap;
use crate::auth_i18n::PANEL_I18N_SCRIPT;
use crate::model::InstallerStatus;

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
    .card { width:min(100%,420px); background:#fff; border:1px solid #e0e0e0; border-radius:18px; padding:28px; position:relative; }
    h1 { margin:0 0 8px; font-size:1.7rem; }
    p { color:#6e6e73; line-height:1.5; }
    label { display:block; margin:14px 0 6px; font-weight:600; font-size:.92rem; }
    input { width:100%; box-sizing:border-box; border:1px solid #d0d5dd; border-radius:10px; padding:11px 12px; font:inherit; }
    button { margin-top:18px; width:100%; border:0; border-radius:999px; padding:12px 16px; background:#0066cc; color:#fff; font-weight:700; cursor:pointer; }
    .row { display:flex; justify-content:space-between; align-items:center; gap:12px; }
    .remember { display:flex; align-items:center; gap:8px; margin:14px 0 0; font-weight:600; font-size:.92rem; }
    .remember input { width:auto; margin:0; }
    a { color:#0066cc; text-decoration:none; font-size:.92rem; }
    .hint { margin-top:14px; font-size:.9rem; }
    .error {
      margin:0 0 14px; padding:10px 12px; border-radius:10px; background:#fef2f2;
      border:1px solid #fecaca; color:#b91c1c; font-size:.92rem; line-height:1.4;
    }
    .lang-host { position:absolute; top:16px; right:16px; }
    .lang { display:flex; flex-direction:column; gap:4px; align-items:flex-end; margin:0; font-weight:600; font-size:.8rem; color:#6e6e73; }
    .lang select { min-width:120px; border:1px solid #d0d5dd; border-radius:8px; padding:6px 8px; font:inherit; background:#fff; color:#1d1d1f; }
    .lang-label { font-size:.75rem; letter-spacing:.02em; }
"#
}

pub fn panel_login_html(status: &InstallerStatus, error: Option<&str>) -> String {
    let initial_locale = resolve_initial_locale(status);
    let token_q = status
        .panel_login_url
        .as_ref()
        .and_then(|url| url.split("token=").nth(1))
        .map(|value| format!("?token={}", html_escape(value)))
        .unwrap_or_default();
    let error_block = match error {
        Some(message) if !message.is_empty() => format!(
            r#"<p class="error" id="i18n-login-error" role="alert">{msg}</p>"#,
            msg = html_escape(message)
        ),
        _ => r#"<p class="error" id="i18n-login-error" role="alert" hidden></p>"#.into(),
    };

    format!(
        r#"<!DOCTYPE html>
<html lang="{locale}" data-initial-locale="{locale}">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Sign in · CPN Panel</title>
  <style>{styles}</style>
</head>
<body data-page="login"{error_attr}>
  <main>
    <section class="card">
      <div id="cpn-lang-host" class="lang-host"></div>
      <p id="i18n-brand" style="color:#0066cc;font-size:12px;font-weight:700;letter-spacing:.08em;margin:0 0 8px;">CPN PANEL</p>
      <h1 id="i18n-title">Sign in</h1>
      {error_block}
      <form method="post" action="/login{token_q}" autocomplete="on">
        <label for="username" id="i18n-username">Username</label>
        <input id="username" name="username" value="" autocomplete="username" required>
        <label for="password" id="i18n-password">Password</label>
        <input id="password" name="password" type="password" value="" autocomplete="current-password" required>
        <label class="remember" for="remember_me">
          <input id="remember_me" name="remember_me" type="checkbox" value="1">
          <span id="i18n-remember">Remember me</span>
        </label>
        <div class="row">
          <span></span>
          <a id="i18n-forgot" href="/forgot-password">Forgot password?</a>
        </div>
        <button id="i18n-submit" type="submit">Sign in</button>
      </form>
      <p class="hint" style="margin-top:18px;text-align:center;">or</p>
      <button type="button" onclick="cpnLoginPasskey()" style="margin-top:8px;width:100%;border:1px solid #d0d5dd;border-radius:999px;padding:12px 16px;background:#fff;color:#1d1d1f;font-weight:700;cursor:pointer;">Sign in with passkey</button>
      <p id="cpn-passkey-login-status" class="hint" role="status"></p>
    </section>
  </main>
  <script>{passkey_script}</script>
  {script}
</body>
</html>"#,
        locale = initial_locale,
        styles = shared_auth_styles(),
        token_q = token_q,
        error_block = error_block,
        error_attr = if error.is_some() {
            r#" data-login-error="1""#
        } else {
            ""
        },
        passkey_script = crate::panel_webauthn::passkey_client_script(),
        script = PANEL_I18N_SCRIPT,
    )
}

pub fn panel_mfa_html(status: &InstallerStatus, error: Option<&str>) -> String {
    let initial_locale = resolve_initial_locale(status);
    let error_block = match error {
        Some(message) if !message.is_empty() => format!(
            r#"<p class="error" role="alert">{msg}</p>"#,
            msg = html_escape(message)
        ),
        _ => String::new(),
    };
    format!(
        r#"<!DOCTYPE html>
<html lang="{locale}" data-initial-locale="{locale}">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Two-factor · CPN Panel</title>
  <style>{styles}</style>
</head>
<body data-page="login-mfa">
  <main>
    <section class="card">
      <div id="cpn-lang-host" class="lang-host"></div>
      <p id="i18n-brand" style="color:#0066cc;font-size:12px;font-weight:700;letter-spacing:.08em;margin:0 0 8px;">CPN PANEL</p>
      <h1>Two-factor authentication</h1>
      <p class="hint">Enter the 6-digit code from your authenticator app, or a one-time backup code.</p>
      {error_block}
      <form method="post" action="/login/2fa" autocomplete="off">
        <label for="code">Authenticator code</label>
        <input id="code" name="code" type="text" inputmode="numeric" autocomplete="one-time-code" required maxlength="32" autofocus>
        <button type="submit">Verify</button>
      </form>
      <p class="hint"><a href="/login">Back to sign in</a></p>
    </section>
  </main>
  {script}
</body>
</html>"#,
        locale = initial_locale,
        styles = shared_auth_styles(),
        error_block = error_block,
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
  <style>{styles}</style>
</head>
<body data-page="forgot">
  <main>
    <section class="card">
      <div id="cpn-lang-host" class="lang-host"></div>
      <p id="i18n-brand" style="color:#0066cc;font-size:12px;font-weight:700;letter-spacing:.08em;margin:0 0 8px;">CPN PANEL</p>
      <h1 id="i18n-title">Forgot password</h1>
      <p class="hint" id="i18n-forgot-intro"></p>
      <form method="post" action="/forgot-password" autocomplete="on">
        <label for="account" id="i18n-forgot-account">Username/Email</label>
        <input id="account" name="account" type="text" autocomplete="username" required>
        <button id="i18n-forgot-submit" type="submit">Request reset</button>
      </form>
      <p class="hint" id="i18n-forgot-smtp"></p>
      <p><a id="i18n-forgot-back" href="/login">Back to sign in</a></p>
    </section>
  </main>
  {script}
</body>
</html>"#,
        locale = initial_locale,
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
        styles = shared_auth_styles(),
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
        styles = shared_auth_styles(),
        script = PANEL_I18N_SCRIPT,
    )
}
