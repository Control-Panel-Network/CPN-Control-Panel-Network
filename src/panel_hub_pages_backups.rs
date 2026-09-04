//! Hub HTML for Backups, Settings, and Security stubs.

use crate::panel_backups::{BackupsPageQuery, backups_create_main};
use crate::panel_hub_defs::backups_hub_tiles;
use crate::panel_hubs::{feature_shell, hub_tiles_grid, not_configured_body, section_heading};
use crate::panel_ops_backup_extra::{
    BackupDestinations, BackupSchedule, list_restore_candidates, load_destinations, load_schedule,
    save_destinations, save_schedule,
};

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn backups_hub_main() -> String {
    let mut body = section_heading(
        "Backups",
        "Create, restore, schedule, and configure destinations. Paths stay concrete for the selected scope.",
    );
    body.push_str(&hub_tiles_grid("Backups", &backups_hub_tiles()));
    body
}

pub fn backups_create_page(q: BackupsPageQuery<'_>) -> String {
    format!(
        r#"{}{}"#,
        crate::panel_hubs::breadcrumb(&[
            ("Dashboard", Some("/dashboard")),
            ("Backups", Some("/backups")),
            ("Create Backup", None),
        ]),
        backups_create_main(q)
    )
}

pub fn backups_restore_page(scope: &str, domain: &str) -> String {
    let body = match list_restore_candidates(scope, domain) {
        Ok((path, files)) => {
            let mut t = format!(
                r#"<p>Archives under <code>{}</code>.</p>
                <form method="get" action="/backups/restore" class="stack-form" style="max-width:520px;">
                  <label for="scope">Scope</label>
                  <select id="scope" name="scope">
                    <option value="panel">Panel</option>
                    <option value="site">Site</option>
                    <option value="subdomain">Subdomain</option>
                  </select>
                  <label for="domain">Domain (site/subdomain)</label>
                  <input id="domain" name="domain" type="text" value="{domain}">
                  <button type="submit" class="btn-primary">List archives</button>
                </form>"#,
                html_escape(&path),
                domain = html_escape(domain),
            );
            if files.is_empty() {
                t.push_str(r#"<p class="empty-state">No archives found for this scope.</p>"#);
            } else {
                t.push_str(r#"<div class="table-wrap"><table class="data-table"><thead><tr><th>File</th><th>Size</th></tr></thead><tbody>"#);
                for (name, size) in files {
                    t.push_str(&format!(
                        "<tr><td><code>{}</code></td><td>{} bytes</td></tr>",
                        html_escape(&name),
                        size
                    ));
                }
                t.push_str("</tbody></table></div>");
                t.push_str(r#"<p class="muted">Listing is live. One-click restore extraction ships next; do not assume restore completed unless a later action confirms it.</p>"#);
            }
            t
        }
        Err(err) => not_configured_body(&err, "Select a valid scope and domain, then list again."),
    };
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Backups", Some("/backups")),
            ("Restore Backup", None),
        ],
        "Restore Backup",
        "Restore from a backup.",
        &body,
        None,
        None,
    )
}

pub fn backups_schedule_page(notice: Option<&str>, error: Option<&str>) -> String {
    let s = load_schedule();
    let checked = if s.enabled { " checked" } else { "" };
    let form = format!(
        r#"<form method="post" action="/backups/schedule/save" class="stack-form" style="max-width:520px;">
      <label><input type="checkbox" name="enabled" value="1"{checked}> Enable schedule record</label>
      <label for="cron">Cron expression</label>
      <input id="cron" name="cron" type="text" value="{cron}">
      <label for="scope">Scope</label>
      <input id="scope" name="scope" type="text" value="{scope}">
      <label for="domain">Domain</label>
      <input id="domain" name="domain" type="text" value="{domain}">
      <button type="submit" class="btn-primary">Save schedule</button>
    </form>
    <p class="muted">Saved under the CPN data dir. A systemd timer runner is the next step; this page does not pretend a timer is installed.</p>"#,
        checked = checked,
        cron = html_escape(&s.cron),
        scope = html_escape(&s.scope),
        domain = html_escape(&s.domain),
    );
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Backups", Some("/backups")),
            ("Schedule Backup", None),
        ],
        "Schedule Backup",
        "Automate backups.",
        &form,
        notice,
        error,
    )
}

pub fn save_backup_schedule(
    enabled: bool,
    cron: &str,
    scope: &str,
    domain: &str,
) -> Result<String, String> {
    save_schedule(&BackupSchedule {
        enabled,
        cron: cron.trim().to_string(),
        scope: scope.trim().to_string(),
        domain: domain.trim().to_string(),
    })?;
    Ok("Schedule saved".into())
}

pub fn backups_destinations_page(notice: Option<&str>, error: Option<&str>) -> String {
    let d = load_destinations();
    let checked = if d.local_enabled { " checked" } else { "" };
    let form = format!(
        r#"<form method="post" action="/backups/destinations/save" class="stack-form" style="max-width:560px;">
      <label><input type="checkbox" name="local_enabled" value="1"{checked}> Local archives enabled</label>
      <label for="gdrive">Google Drive note</label>
      <input id="gdrive" name="google_drive_note" type="text" value="{gdrive}">
      <label for="remote">Remote note</label>
      <input id="remote" name="remote_note" type="text" value="{remote}">
      <button type="submit" class="btn-primary">Save destinations</button>
    </form>"#,
        checked = checked,
        gdrive = html_escape(&d.google_drive_note),
        remote = html_escape(&d.remote_note),
    );
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Backups", Some("/backups")),
            ("Destinations", None),
        ],
        "Destinations",
        "Backup destinations.",
        &form,
        notice,
        error,
    )
}

pub fn save_backup_destinations(
    local_enabled: bool,
    google_drive_note: &str,
    remote_note: &str,
) -> Result<String, String> {
    save_destinations(&BackupDestinations {
        local_enabled,
        google_drive_note: google_drive_note.trim().to_string(),
        remote_note: remote_note.trim().to_string(),
    })?;
    Ok("Destinations saved".into())
}

/// Deprecated name: Settings hub lives in `panel_hub_pages_settings`.
pub fn settings_stub_page() -> String {
    crate::panel_hub_pages_settings::settings_hub_main()
}
