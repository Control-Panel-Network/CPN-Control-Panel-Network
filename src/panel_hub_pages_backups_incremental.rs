//! Incremental (restic/borg) backup jobs UI.

use crate::panel_hubs::feature_shell;
use crate::panel_ops_backup_incremental::{
    list_restic_snapshots, load_jobs, run_incremental_job, upsert_job,
};
use std::time::{SystemTime, UNIX_EPOCH};

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn flash(notice: Option<&str>, error: Option<&str>) -> String {
    let mut out = String::new();
    if let Some(n) = notice.filter(|s| !s.is_empty()) {
        out.push_str(&format!(
            r#"<p class="notice notice-ok">{}</p>"#,
            html_escape(n)
        ));
    }
    if let Some(e) = error.filter(|s| !s.is_empty()) {
        out.push_str(&format!(
            r#"<p class="notice notice-err">{}</p>"#,
            html_escape(e)
        ));
    }
    out
}

fn format_last_run(unix: u64) -> String {
    if unix == 0 {
        return "never".into();
    }
    // Norwegian/European display: dd/mm/yyyy (UTC epoch rendered as date digits only).
    let secs = unix as i64;
    let days = secs / 86_400;
    // Approximate calendar from Unix day count is enough for operator status; full TZ on Linux lab.
    let _ = days;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    if now >= unix {
        format!("unix {unix} (see job log; schedule uses 24h clock)")
    } else {
        format!("unix {unix}")
    }
}

pub fn backups_incremental_page(notice: Option<&str>, error: Option<&str>) -> String {
    let jobs = load_jobs();
    let mut rows = String::from(
        r#"<div class="table-wrap"><table class="data-table"><thead><tr>
        <th>Id</th><th>Engine</th><th>Source</th><th>Destination</th><th>Cron</th><th>Last</th><th>Status</th><th></th>
        </tr></thead><tbody>"#,
    );
    if jobs.is_empty() {
        rows.push_str(r#"<tr><td colspan="8" class="muted">No incremental jobs yet.</td></tr>"#);
    } else {
        for j in &jobs {
            rows.push_str(&format!(
                r#"<tr>
                <td><code>{id}</code></td>
                <td>{engine}</td>
                <td><code>{source}</code></td>
                <td>{dest}</td>
                <td><code>{cron}</code></td>
                <td>{last}</td>
                <td>{status}</td>
                <td>
                  <form method="post" action="/backups/incremental/run" class="inline-form" style="display:inline;">
                    <input type="hidden" name="id" value="{id}">
                    <button type="submit" class="btn-secondary">Run now</button>
                  </form>
                </td>
                </tr>"#,
                id = html_escape(&j.id),
                engine = html_escape(&j.engine),
                source = html_escape(&j.source),
                dest = html_escape(&j.destination),
                cron = html_escape(&j.schedule_cron),
                last = html_escape(&format_last_run(j.last_run_unix)),
                status = html_escape(&j.last_status),
            ));
        }
    }
    rows.push_str("</tbody></table></div>");

    let snapshots = match list_restic_snapshots("local") {
        Ok(text) => format!(
            r#"<h2 style="font-size:16px;margin:18px 0 8px;">Local restic snapshots</h2>
            <pre class="manage-log-pre" style="white-space:pre-wrap;max-height:240px;overflow:auto;">{}</pre>"#,
            html_escape(&text)
        ),
        Err(err) => format!(
            r#"<p class="muted" style="margin-top:14px;">Snapshots: {}</p>"#,
            html_escape(&err)
        ),
    };

    let form = r#"<h2 style="font-size:16px;margin:18px 0 8px;">Add or update job</h2>
    <form method="post" action="/backups/incremental/save" class="stack-form" style="max-width:640px;display:grid;gap:10px;">
      <label>Job id <input type="text" name="id" required maxlength="64" placeholder="nightly-home" style="width:100%;"></label>
      <label>Engine
        <select name="engine" style="width:100%;">
          <option value="restic">restic</option>
          <option value="borg">borg</option>
        </select>
      </label>
      <label>Source path <input type="text" name="source" required placeholder="/home/example.com" style="width:100%;"></label>
      <label>Destination
        <select name="destination" style="width:100%;">
          <option value="local">local</option>
          <option value="s3">s3</option>
          <option value="sftp">sftp</option>
        </select>
      </label>
      <label>Cron (24h) <input type="text" name="schedule_cron" value="0 2 * * *" style="width:100%;"></label>
      <label><input type="checkbox" name="enabled" value="1" checked> Enabled</label>
      <button type="submit" class="btn-primary">Save job</button>
    </form>
    <p class="muted">Uses restic (preferred) or borg. S3/SFTP credentials come from Backup Destinations (mode 600). Restore: list snapshots above, then restore via restic CLI or Create/Restore for full archives.</p>"#;

    let body = format!(
        "{flash}{rows}{form}{snapshots}",
        flash = flash(notice, error),
        rows = rows,
        form = form,
        snapshots = snapshots
    );
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Backups", Some("/backups")),
            ("Incremental", None),
        ],
        "Incremental backups",
        "restic or borg jobs to local, S3-compatible, or SFTP destinations.",
        &body,
        None,
        None,
    )
}

pub fn save_incremental_form(
    id: &str,
    engine: &str,
    source: &str,
    destination: &str,
    schedule_cron: &str,
    enabled: bool,
) -> Result<String, String> {
    let id = id.trim();
    if id.is_empty() {
        return Err("Job id is required.".into());
    }
    let source = source.trim();
    if source.is_empty() {
        return Err("Source path is required.".into());
    }
    let mut existing = load_jobs()
        .into_iter()
        .find(|j| j.id == id)
        .unwrap_or_default();
    existing.id = id.to_string();
    existing.engine = if engine.trim().is_empty() {
        "restic".into()
    } else {
        engine.trim().to_string()
    };
    existing.source = source.to_string();
    existing.destination = destination.trim().to_string();
    existing.schedule_cron = if schedule_cron.trim().is_empty() {
        "0 2 * * *".into()
    } else {
        schedule_cron.trim().to_string()
    };
    existing.enabled = enabled;
    upsert_job(existing)
}

pub fn run_incremental_form(id: &str) -> Result<String, String> {
    run_incremental_job(id.trim())
}
