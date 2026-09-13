//! Settings > Log retention page (admin).

use crate::panel_hubs::feature_shell;
use crate::panel_log_retention::{DEFAULT_RETENTION_DAYS, LogRetentionPrefs, load_log_retention};

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn log_retention_settings_page(notice: Option<&str>, error: Option<&str>) -> String {
    let prefs = load_log_retention();
    log_retention_settings_page_with(&prefs, notice, error)
}

pub fn log_retention_settings_page_with(
    prefs: &LogRetentionPrefs,
    notice: Option<&str>,
    error: Option<&str>,
) -> String {
    let notice_html = notice
        .map(|n| format!(r#"<p class="notice ok">{}</p>"#, html_escape(n)))
        .unwrap_or_default();
    let error_html = error
        .map(|n| format!(r#"<p class="notice error">{}</p>"#, html_escape(n)))
        .unwrap_or_default();
    let max_size = prefs.max_size_mb.map(|n| n.to_string()).unwrap_or_default();
    let body = format!(
        r#"{notice_html}{error_html}
<article class="section-card" style="max-width:720px;">
  <h2>Site log retention</h2>
  <p class="muted">Controls how long each website keeps <code>logs/access.log</code> and <code>logs/error.log</code> under its site home. Default is <strong>{default_days} days</strong>. Parent and subdomain homes stay isolated; there is no aggregate all-logs view.</p>
  <form method="post" action="/settings/logs" class="stack-form">
    <label for="retention_days">Retention (days)</label>
    <input id="retention_days" name="retention_days" type="number" min="1" max="3650" required value="{days}">
    <label for="max_size_mb">Max size per log file (MB, optional)</label>
    <input id="max_size_mb" name="max_size_mb" type="number" min="0" max="102400" placeholder="Leave blank for no size cap" value="{max_size}">
    <p class="muted">On save, CPN writes <code>/etc/logrotate.d/cpn-site-logs</code> when writable (daily rotate + copytruncate) and immediately truncates files over the max size.</p>
    <button type="submit" class="btn-primary">Save log retention</button>
  </form>
</article>
<p class="muted">Prefs file: <code>/var/lib/cpn/log-retention.json</code> (mode 600).</p>"#,
        notice_html = notice_html,
        error_html = error_html,
        default_days = DEFAULT_RETENTION_DAYS,
        days = prefs.retention_days,
        max_size = html_escape(&max_size),
    );
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Settings", Some("/settings")),
            ("Log retention", None),
        ],
        "Log retention",
        "Access and error log keep window",
        &body,
        None,
        None,
    )
}
