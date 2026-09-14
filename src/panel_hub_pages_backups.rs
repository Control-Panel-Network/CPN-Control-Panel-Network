//! Hub HTML for Backups, Settings, and Security stubs.

use crate::backup_restore_detect::BackupFormat;
use crate::panel_backups::{BackupsPageQuery, backups_create_main};
use crate::panel_hub_defs::backups_hub_tiles;
use crate::panel_hubs::{feature_shell, hub_tiles_grid, not_configured_body, section_heading};
use crate::panel_ops_backup_extra::{
    BackupDestinations, BackupSchedule, list_restore_candidates, load_destinations, load_schedule,
    save_destinations, save_schedule,
};
use crate::sites::{SiteRecord, list_sites};

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

fn site_options(sites: &[SiteRecord], selected: &str) -> String {
    let mut out = String::from(r#"<option value="">Select domain</option>"#);
    for site in sites {
        let sel = if site.domain == selected {
            " selected"
        } else {
            ""
        };
        out.push_str(&format!(
            r#"<option value="{domain}"{sel}>{domain}</option>"#,
            domain = html_escape(&site.domain),
            sel = sel,
        ));
    }
    out
}

fn format_options(selected: &str) -> String {
    let items = [
        ("auto", "Auto-detect"),
        ("cpn", BackupFormat::Cpn.label()),
        ("wordpress", BackupFormat::WordPress.label()),
        ("cpanel", BackupFormat::Cpanel.label()),
        ("cyberpanel", BackupFormat::CyberPanel.label()),
    ];
    let mut out = String::new();
    for (value, label) in items {
        let sel = if selected == value { " selected" } else { "" };
        out.push_str(&format!(
            r#"<option value="{value}"{sel}>{label}</option>"#,
            value = html_escape(value),
            sel = sel,
            label = html_escape(label),
        ));
    }
    out
}

pub fn backups_restore_page(
    scope: &str,
    domain: &str,
    notice: Option<&str>,
    error: Option<&str>,
) -> String {
    let sites = list_sites().unwrap_or_default();
    let scope_sel = |v: &str| if scope == v { " selected" } else { "" };
    let mut body = String::new();
    if let Some(msg) = notice.filter(|m| !m.is_empty()) {
        body.push_str(&format!(
            r#"<p class="panel-notice ok" role="status">{}</p>"#,
            html_escape(msg)
        ));
    }
    if let Some(msg) = error.filter(|m| !m.is_empty()) {
        body.push_str(&format!(
            r#"<p class="panel-notice error" role="status">{}</p>"#,
            html_escape(msg)
        ));
    }
    body.push_str(
        r#"<p>Restore a CPN archive, or import WordPress / cPanel / CyberPanel source backups into a chosen site. Place the archive under that site's <code>backups/</code> folder first.</p>
        <p class="muted">Supported: CPN <code>.tar.gz</code>; WordPress zip/tar with <code>wp-content</code> + SQL (UpdraftPlus / Duplicator / plain); cPanel <code>cpmove-*.tar.gz</code> / <code>homedir</code>+<code>mysql/</code> dump folder (imported into MariaDB); CyberPanel classic with <code>meta.xml</code>. SQL restore uses the local MariaDB host database (not Oracle MySQL). Email import is best-effort only. CPN is not CyberPanel; CyberPanel is a supported source format only.</p>"#,
    );
    body.push_str(&format!(
        r#"<form method="get" action="/backups/restore" class="stack-form" style="max-width:560px;">
          <label for="scope">Scope</label>
          <select id="scope" name="scope">
            <option value="site"{ss}>Site</option>
            <option value="subdomain"{su}>Subdomain</option>
            <option value="panel"{sp}>Panel (list panel archives)</option>
          </select>
          <label for="domain">Target domain (required for restore)</label>
          <select id="domain" name="domain">{opts}</select>
          <button type="submit" class="btn-primary">List archives</button>
        </form>"#,
        ss = scope_sel("site"),
        su = scope_sel("subdomain"),
        sp = scope_sel("panel"),
        opts = site_options(&sites, domain),
    ));

    match list_restore_candidates(scope, domain) {
        Ok((path, files)) => {
            body.push_str(&format!(
                r#"<p>Archives under <code>{}</code>.</p>"#,
                html_escape(&path)
            ));
            if files.is_empty() {
                body.push_str(r#"<p class="empty-state">No archives found for this scope. Copy a supported archive into this folder, then list again.</p>"#);
            } else {
                body.push_str(r#"<div class="table-wrap"><table class="data-table"><thead><tr><th>File</th><th>Size</th><th>Restore</th></tr></thead><tbody>"#);
                for (name, size) in files {
                    body.push_str(&format!(
                        r#"<tr>
                          <td><code>{name}</code></td>
                          <td>{size} bytes</td>
                          <td>
                            <form method="post" action="/backups/restore/run" class="stack-form" style="margin:0;gap:6px;">
                              <input type="hidden" name="scope" value="{scope}">
                              <input type="hidden" name="domain" value="{domain}">
                              <input type="hidden" name="archive" value="{name}">
                              <label class="muted" for="fmt-{name}">Format</label>
                              <select id="fmt-{name}" name="format">{formats}</select>
                              <label class="muted" for="db-{name}">Target DB (WordPress, optional)</label>
                              <input id="db-{name}" name="db_name" type="text" placeholder="optional" style="max-width:160px;">
                              <button type="submit" class="btn-primary" onclick="return confirm('Restore this archive into the selected domain? Website files may be overwritten.');">Restore</button>
                            </form>
                          </td>
                        </tr>"#,
                        name = html_escape(&name),
                        size = size,
                        scope = html_escape(scope),
                        domain = html_escape(domain),
                        formats = format_options("auto"),
                    ));
                }
                body.push_str("</tbody></table></div>");
            }
        }
        Err(err) => {
            body.push_str(&not_configured_body(
                &err,
                "Select a valid scope and target domain, then list again.",
            ));
        }
    }

    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Backups", Some("/backups")),
            ("Restore Backup", None),
        ],
        "Restore Backup",
        "Restore or import into a site.",
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
