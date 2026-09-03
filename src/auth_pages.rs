use crate::account::{account_public_from_disk, load_bootstrap};
use crate::model::InstallerStatus;

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn panel_login_html(status: &InstallerStatus) -> String {
    let account = status.account.clone().or_else(account_public_from_disk);
    let username = account
        .as_ref()
        .map(|value| value.username.as_str())
        .unwrap_or("admin");
    let email = account
        .as_ref()
        .map(|value| value.recovery_email.as_str())
        .unwrap_or("");
    let configured = account
        .as_ref()
        .map(|value| value.configured)
        .unwrap_or(false);
    let note = if configured {
        format!(
            "Cuenta inicial lista para <strong>{}</strong>. El panel Next.js completo usará estos datos cuando la autenticación esté conectada.",
            html_escape(username)
        )
    } else {
        "Todavía no hay una cuenta inicial. Completa el instalador primero.".into()
    };
    format!(
        r#"<!DOCTYPE html>
<html lang="es">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Inicio de sesión · CPN Panel</title>
  <style>
    body {{ margin:0; font-family:"Segoe UI",system-ui,sans-serif; background:#f5f5f7; color:#1d1d1f; }}
    main {{ min-height:100vh; display:grid; place-items:center; padding:48px 20px; }}
    .card {{ width:min(100%,420px); background:#fff; border:1px solid #e0e0e0; border-radius:18px; padding:28px; }}
    h1 {{ margin:0 0 8px; font-size:1.7rem; }}
    p {{ color:#6e6e73; line-height:1.5; }}
    label {{ display:block; margin:14px 0 6px; font-weight:600; font-size:.92rem; }}
    input {{ width:100%; box-sizing:border-box; border:1px solid #d0d5dd; border-radius:10px; padding:11px 12px; font:inherit; }}
    button {{ margin-top:18px; width:100%; border:0; border-radius:999px; padding:12px 16px; background:#0066cc; color:#fff; font-weight:700; cursor:pointer; }}
    .row {{ display:flex; justify-content:space-between; align-items:center; gap:12px; }}
    a {{ color:#0066cc; text-decoration:none; font-size:.92rem; }}
    .hint {{ margin-top:14px; font-size:.9rem; }}
  </style>
</head>
<body>
  <main>
    <section class="card">
      <p style="color:#0066cc;font-size:12px;font-weight:700;letter-spacing:.08em;margin:0 0 8px;">CPN PANEL</p>
      <h1>Iniciar sesión</h1>
      <p class="hint">{note}</p>
      <form method="post" action="/login" autocomplete="on">
        <label for="username">Usuario</label>
        <input id="username" name="username" value="{username}" required>
        <label for="password">Contraseña</label>
        <input id="password" name="password" type="password" required>
        <div class="row">
          <span></span>
          <a href="/forgot-password">¿Olvidaste la contraseña?</a>
        </div>
        <button type="submit">Entrar</button>
      </form>
      <p class="hint">Este formulario usa POST. La autenticación completa del panel se conectará a <code>/var/lib/cpn/panel-bootstrap.json</code>.</p>
      <p class="hint">Correo de recuperación configurado: <strong>{email}</strong></p>
    </section>
  </main>
</body>
</html>"#,
        username = html_escape(username),
        email = html_escape(if email.is_empty() {
            "(no configurado)"
        } else {
            email
        }),
        note = note,
    )
}

pub fn forgot_password_html() -> String {
    let boot = load_bootstrap();
    let email = boot
        .as_ref()
        .map(|value| value.recovery_email.as_str())
        .unwrap_or("");
    let username = boot
        .as_ref()
        .map(|value| value.username.as_str())
        .unwrap_or("admin");
    let masked = if email.is_empty() {
        "No hay correo de recuperación todavía.".to_string()
    } else {
        mask_email(email)
    };
    format!(
        r#"<!DOCTYPE html>
<html lang="es">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Contraseña olvidada · CPN Panel</title>
  <style>
    body {{ margin:0; font-family:"Segoe UI",system-ui,sans-serif; background:#f5f5f7; color:#1d1d1f; }}
    main {{ min-height:100vh; display:grid; place-items:center; padding:48px 20px; }}
    .card {{ width:min(100%,420px); background:#fff; border:1px solid #e0e0e0; border-radius:18px; padding:28px; }}
    h1 {{ margin:0 0 8px; font-size:1.7rem; }}
    p {{ color:#6e6e73; line-height:1.5; }}
    a {{ color:#0066cc; text-decoration:none; }}
  </style>
</head>
<body>
  <main>
    <section class="card">
      <p style="color:#0066cc;font-size:12px;font-weight:700;letter-spacing:.08em;margin:0 0 8px;">CPN PANEL</p>
      <h1>Contraseña olvidada</h1>
      <p>Punto de entrada para restablecer la contraseña de la cuenta <strong>{username}</strong>.</p>
      <p>Correo de recuperación registrado: <strong>{masked}</strong></p>
      <p>El envío real de correo se conectará cuando el panel tenga SMTP configurado. Mientras tanto, un operador con acceso root puede restablecer la cuenta desde el servidor usando el bootstrap en <code>/var/lib/cpn/panel-bootstrap.json</code>.</p>
      <p><a href="/login">Volver al inicio de sesión</a></p>
    </section>
  </main>
</body>
</html>"#,
        username = html_escape(username),
        masked = html_escape(&masked),
    )
}

fn mask_email(email: &str) -> String {
    let Some((local, domain)) = email.split_once('@') else {
        return "***".into();
    };
    let visible = local.chars().next().unwrap_or('*');
    format!("{visible}***@{domain}")
}
