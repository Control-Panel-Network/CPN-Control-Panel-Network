use crate::model::InstallerStatus;

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn status_html_page(status: &InstallerStatus) -> String {
    let pretty = serde_json::to_string_pretty(status).unwrap_or_else(|_| "{}".into());
    let phase = html_escape(status.phase);
    let message = html_escape(&status.message);
    let progress = status.progress;
    let server = status
        .selected_server
        .map(|value| value.label().to_string())
        .unwrap_or_else(|| "ninguno".into());
    let mail = status
        .selected_mail
        .map(|value| value.label().to_string())
        .unwrap_or_else(|| "ninguno".into());
    let error = status
        .error
        .as_deref()
        .map(html_escape)
        .unwrap_or_else(|| "ninguno".into());
    let json = html_escape(&pretty);
    format!(
        r#"<!DOCTYPE html>
<html lang="es">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Estado técnico · CPN</title>
  <style>
    :root {{ color-scheme: light; }}
    body {{ margin: 0; font-family: "Segoe UI", system-ui, sans-serif; background: #f7f8fa; color: #111827; }}
    main {{ max-width: 920px; margin: 0 auto; padding: 48px 20px; }}
    h1 {{ font-size: 2rem; margin: 0 0 8px; letter-spacing: -0.03em; }}
    .eyebrow {{ color: #0f766e; font-size: 0.75rem; font-weight: 700; letter-spacing: 0.08em; }}
    .card {{ background: #fff; border: 1px solid #e5e7eb; border-radius: 16px; padding: 24px; margin-top: 24px; }}
    .grid {{ display: grid; gap: 12px; grid-template-columns: repeat(auto-fit, minmax(180px, 1fr)); }}
    .label {{ color: #667085; font-size: 0.85rem; }}
    .value {{ font-size: 1.05rem; font-weight: 600; word-break: break-word; }}
    pre {{ margin: 0; overflow: auto; background: #0b1220; color: #e5e7eb; border-radius: 12px; padding: 16px; font-size: 0.9rem; line-height: 1.45; }}
    code {{ background: #eef2f7; padding: 2px 6px; border-radius: 6px; }}
  </style>
</head>
<body>
  <main>
    <p class="eyebrow">CPN INSTALADOR</p>
    <h1>Estado técnico</h1>
    <p>Resumen legible del instalador. Los clientes API pueden pedir JSON en <code>/api/status</code>.</p>
    <section class="card grid">
      <div><div class="label">Fase</div><div class="value">{phase}</div></div>
      <div><div class="label">Progreso</div><div class="value">{progress}%</div></div>
      <div><div class="label">Servidor</div><div class="value">{server}</div></div>
      <div><div class="label">Correo</div><div class="value">{mail}</div></div>
      <div style="grid-column: 1 / -1"><div class="label">Mensaje</div><div class="value">{message}</div></div>
      <div style="grid-column: 1 / -1"><div class="label">Error</div><div class="value">{error}</div></div>
    </section>
    <section class="card" style="margin-top: 16px;">
      <div class="label" style="margin-bottom: 10px;">JSON</div>
      <pre>{json}</pre>
    </section>
  </main>
</body>
</html>"#
    )
}
