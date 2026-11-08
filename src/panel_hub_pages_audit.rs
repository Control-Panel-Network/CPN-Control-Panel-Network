//! Settings > Audit trail policy page (retention, opt-out, webhook, export links).

use crate::panel_audit_policy::{
    AuditPolicy, DEFAULT_AUDIT_RETENTION_DAYS, load_audit_policy, opt_outable_categories,
};
use crate::panel_audit_types::AuditCategory;
use crate::panel_hubs::feature_shell;

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn audit_settings_page(notice: Option<&str>, error: Option<&str>) -> String {
    let prefs = load_audit_policy();
    audit_settings_page_with(&prefs, notice, error)
}

pub fn audit_settings_page_with(
    prefs: &AuditPolicy,
    notice: Option<&str>,
    error: Option<&str>,
) -> String {
    let notice_html = notice
        .map(|n| format!(r#"<p class="notice ok">{}</p>"#, html_escape(n)))
        .unwrap_or_default();
    let error_html = error
        .map(|n| format!(r#"<p class="notice error">{}</p>"#, html_escape(n)))
        .unwrap_or_default();
    let disabled: std::collections::HashSet<_> = prefs
        .disabled_categories
        .iter()
        .map(|s| s.to_ascii_lowercase())
        .collect();
    let mut opt_out_rows = String::new();
    for cat in opt_outable_categories() {
        let checked = if disabled.contains(cat.as_str()) {
            " checked"
        } else {
            ""
        };
        opt_out_rows.push_str(&format!(
            r#"<label style="display:block;margin:6px 0;"><input type="checkbox" name="disable_{id}" value="1"{checked}> Disable non-essential category <code>{id}</code></label>"#,
            id = cat.as_str(),
            checked = checked,
        ));
    }
    let alerts_checked = if prefs.alerts_enabled { " checked" } else { "" };
    let essential: String = [
        AuditCategory::Auth,
        AuditCategory::Identity,
        AuditCategory::Accounts,
        AuditCategory::ApiTokens,
        AuditCategory::Firewall,
        AuditCategory::Updates,
        AuditCategory::Cli,
        AuditCategory::Migrate,
        AuditCategory::Repair,
        AuditCategory::Backup,
        AuditCategory::Settings,
    ]
    .iter()
    .map(|c| format!("<code>{}</code>", c.as_str()))
    .collect::<Vec<_>>()
    .join(", ");
    let body = format!(
        r#"{notice_html}{error_html}
<article class="section-card" style="max-width:760px;">
  <h2>Panel audit trail</h2>
  <p class="muted">Append-only security activity log at <code>/var/lib/cpn/audit/audit.jsonl</code> (legacy path <code>panel-actions.jsonl</code> is kept compatible). Default retention is <strong>{default_days} days</strong>. Security-essential categories cannot be disabled.</p>
  <p class="muted">Locked on: {essential}</p>
  <form method="post" action="/settings/audit" class="stack-form">
    <label for="retention_days">Retention (days)</label>
    <input id="retention_days" name="retention_days" type="number" min="1" max="3650" required value="{days}">
    <label for="webhook_url">Optional webhook URL (HTTPS, or http://127.0.0.1 for local SIEM)</label>
    <input id="webhook_url" name="webhook_url" type="url" maxlength="500" placeholder="https://example.invalid/hooks/cpn-audit" value="{webhook}">
    <p class="muted">Payloads are redacted JSON audit rows only (no passwords, tokens, or private keys).</p>
    <label style="display:block;margin:10px 0;"><input type="checkbox" name="alerts_enabled" value="1"{alerts_checked}> Owner alerts for key security events</label>
    <h3>Non-essential opt-out</h3>
    <p class="muted">Optional categories you may silence. Essential security categories stay on.</p>
    {opt_out}
    <button type="submit" class="btn-primary">Save audit policy</button>
  </form>
</article>
<article class="section-card" style="max-width:760px;margin-top:16px;">
  <h2>Export</h2>
  <p class="muted">Download the newest audit rows (admin only). Secrets are redacted at write time.</p>
  <p><a class="btn-secondary" href="/settings/audit/export?format=jsonl">Download JSONL</a>
  <a class="btn-secondary" href="/settings/audit/export?format=csv" style="margin-left:8px;">Download CSV</a></p>
  <p class="muted">Also view live rows under <a href="/server/logs">Server &gt; Logs</a>.</p>
</article>"#,
        notice_html = notice_html,
        error_html = error_html,
        default_days = DEFAULT_AUDIT_RETENTION_DAYS,
        days = prefs.retention_days,
        webhook = html_escape(&prefs.webhook_url),
        alerts_checked = alerts_checked,
        essential = essential,
        opt_out = opt_out_rows,
    );
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Settings", Some("/settings")),
            ("Audit trail", None),
        ],
        "Audit trail",
        "Retention, opt-out, export, and webhook forward",
        &body,
        None,
        None,
    )
}
