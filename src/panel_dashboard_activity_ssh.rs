//! Activity Board SSH logs panel and security review snooze UI.

use crate::panel_dashboard_activity_list::wrap_activity_table;
use crate::panel_ops_activity::{SshSecurityAnalysis, recent_ssh_logs};
use crate::panel_user_prefs::{
    SSH_SECURITY_REVIEW_SNOOZE_DEFAULT_DAYS, format_epoch_dd_mm_yyyy,
    ssh_security_review_snooze_until,
};

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn log_table(rows: &[crate::panel_ops_activity::ActivityLogRow], empty: &str) -> String {
    if rows.is_empty() {
        return format!(r#"<p class="empty-state">{e}</p>"#, e = html_escape(empty));
    }
    let mut t = String::from(
        r#"<div class="table-wrap"><table class="data-table"><thead><tr><th>Timestamp</th><th>Message</th></tr></thead><tbody>"#,
    );
    for row in rows {
        t.push_str(&format!(
            r#"<tr><td><time>{ts}</time></td><td><code>{msg}</code></td></tr>"#,
            ts = html_escape(&row.timestamp),
            msg = html_escape(&row.message),
        ));
    }
    t.push_str("</tbody></table></div>");
    t
}

fn ssh_security_review_card(analysis: &SshSecurityAnalysis) -> String {
    let tips: String = analysis
        .tips
        .iter()
        .map(|t| {
            format!(
                r#"<div class="activity-tip">{tip}</div>"#,
                tip = html_escape(t)
            )
        })
        .collect();
    let alert_title = if analysis.alert_count > 0 {
        format!("Security notices ({n})", n = analysis.alert_count)
    } else {
        "SSH security review".into()
    };
    format!(
        r#"<div class="activity-sec-box">
          <h4>{alert_title}</h4>
          <div class="activity-sec-card">
            <strong>SSH security best practices</strong>
            <span class="badge-info">INFO</span>
            <p style="margin:8px 0 0;font-size:13px;">While no immediate compromise is claimed from this sample, consider the hardening tips below.</p>
            <div class="activity-sec-meta">
              <div><span>Status</span><strong>{status}</strong></div>
              <div><span>Logs analyzed</span><strong>{logs}</strong></div>
              <div><span>Firewall</span><strong>{fw}</strong></div>
            </div>
            <div class="activity-sec-meta" style="margin-top:8px;">
              <div><span>Failed logins (sample)</span><strong>{failed}</strong></div>
              <div><span>Accepted logins (sample)</span><strong>{ok}</strong></div>
              <div><span>Manage</span><strong><a href="/security/ssh">SSH settings</a></strong></div>
            </div>
            {tips}
            <div class="activity-sec-actions">
              <form method="post" action="/dashboard/activity/ssh-security-review/snooze">
                <input type="hidden" name="days" value="{days}">
                <button type="submit">Hide for 1 month</button>
              </form>
              <p class="muted">Snoozes this banner only (max {days} days). Tips still apply; hiding does not mean the advice is wrong.</p>
            </div>
          </div>
        </div>"#,
        alert_title = html_escape(&alert_title),
        status = html_escape(&analysis.status_label),
        logs = analysis.logs_analyzed,
        fw = html_escape(&analysis.firewall_label),
        failed = analysis.failed_logins,
        ok = analysis.accepted_logins,
        tips = tips,
        days = SSH_SECURITY_REVIEW_SNOOZE_DEFAULT_DAYS,
    )
}

fn ssh_security_review_snoozed_note(until_epoch: i64) -> String {
    let until = format_epoch_dd_mm_yyyy(until_epoch);
    format!(
        r#"<div class="activity-sec-snoozed">
          SSH security review is hidden until <strong>{until}</strong>.
          Analysis still runs; this only snoozes the banner.
          <a href="/security/ssh">Show again from SSH settings</a>
          or
          <form method="post" action="/dashboard/activity/ssh-security-review/show" style="display:inline;">
            <button type="submit" style="background:none;border:none;padding:0;color:inherit;font:inherit;font-weight:700;text-decoration:underline;cursor:pointer;">show again now</button>
          </form>.
        </div>"#,
        until = html_escape(&until),
    )
}

pub(crate) fn ssh_logs_panel(username: &str, analysis: &SshSecurityAnalysis) -> String {
    let (rows, _analyzed) = recent_ssh_logs(200);
    let review = match ssh_security_review_snooze_until(username) {
        Some(until) => ssh_security_review_snoozed_note(until),
        None => ssh_security_review_card(analysis),
    };
    let table = wrap_activity_table(
        "ssh-logs",
        "Filter timestamp or message",
        &log_table(
            &rows,
            "No SSH log lines available (need readable auth logs or journalctl).",
        ),
    );
    format!(
        r#"<div class="activity-panel-head">
          <h3>SSH Security Analysis</h3>
          <a class="btn-secondary" href="/dashboard?activity=ssh-logs#activity-ssh-logs" style="min-height:36px;padding:0 12px;border-radius:999px;border:1px solid var(--hairline);display:inline-flex;align-items:center;text-decoration:none;font-weight:700;font-size:13px;">Refresh analysis</a>
        </div>
        {review}
        <h3 style="margin:0 0 8px;font-size:15px;">Recent SSH Logs</h3>
        {table}"#,
        review = review,
        table = table,
    )
}
