//! Hub HTML for Backups, Settings, and Security stubs.

use crate::backup_restore_detect::BackupFormat;
use crate::backup_restore_scan::{
    documented_upload_locations, list_restore_archives_with_fallback,
};
use crate::panel_backups::{BackupsPageQuery, backups_create_main};
use crate::panel_dashboard_activity_list::{activity_list_script, wrap_activity_table_sized};
use crate::panel_hub_defs::backups_hub_tiles;
use crate::panel_hubs::{feature_shell, hub_tiles_grid, not_configured_body, section_heading};
use crate::panel_ops_backup_extra::{
    BackupDestinations, BackupSchedule, load_destinations, load_schedule, save_destinations,
    save_schedule,
};
use crate::sites::{SiteRecord, list_sites};

/// Default page size for the Restore archive list (matches Server > Logs density).
const RESTORE_LIST_PER_PAGE: usize = 5;

fn restore_list_styles() -> &'static str {
    r#"
<style>
#activity-list-restore-archives .data-table td[data-label="Restore"] .stack-form {
  max-width:100%; width:100%; margin:0; gap:8px;
}
#activity-list-restore-archives .data-table td[data-label="Restore"] .stack-form input,
#activity-list-restore-archives .data-table td[data-label="Restore"] .stack-form select {
  max-width:100%;
}
#activity-list-restore-archives .data-table td[data-label="File"] code,
#activity-list-restore-archives .data-table td[data-label="Location"] code {
  overflow-wrap:anywhere; word-break:break-word; white-space:normal;
}
@container (max-width: 720px) {
  #activity-list-restore-archives .data-table td[data-label="Restore"] {
    display:block; grid-template-columns:1fr;
  }
  #activity-list-restore-archives .data-table td[data-label="Restore"]::before {
    display:block; margin-bottom:6px;
  }
  #activity-list-restore-archives .data-table td[data-label="Restore"] .stack-form {
    max-width:none;
  }
}
</style>
"#
}

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
    let mut out =
        String::from(r#"<option value="">Select domain (optional for recreate)</option>"#);
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
        // UI value stays `classic` (never brand a third-party panel in product HTML).
        ("classic", BackupFormat::CyberPanel.label()),
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

fn upload_locations_html() -> String {
    let mut out = String::from(
        r#"<div class="panel-card" style="margin:14px 0;padding:12px 14px;">
        <h3 style="margin:0 0 8px;font-size:1rem;">Archive upload locations</h3>
        <p class="muted" style="margin:0 0 8px;">Copy archives onto the server, then list. Preferred paths are scanned first; fallback drops are detected with provenance.</p>
        <ul style="margin:0;padding-left:1.2rem;">"#,
    );
    for (label, path) in documented_upload_locations() {
        out.push_str(&format!(
            r#"<li><strong>{}</strong>: <code>{}</code></li>"#,
            html_escape(label),
            html_escape(path)
        ));
    }
    out.push_str("</ul></div>");
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
        r#"<p>Restore a CPN archive, or import WordPress / cPanel / source control-panel backups into one or more selected entities. Prefer that site's <code>backups/</code> folder; fallback paths are also scanned.</p>
        <p class="muted">Supported: CPN <code>.tar.gz</code>; WordPress zip/tar with <code>wp-content</code> + SQL (UpdraftPlus / Duplicator / plain); cPanel <code>cpmove-*.tar.gz</code> / <code>homedir</code>+<code>mysql/</code> dump folder (imported into MariaDB); classic source control-panel archives with <code>meta.xml</code>. After listing, use <strong>Select entities</strong> to multi-select domains, subdomains, databases, and optional email/Docker/DNS. SQL restore uses the local MariaDB host database.</p>"#,
    );
    body.push_str(&upload_locations_html());
    body.push_str(&format!(
        r#"<form method="get" action="/backups/restore" class="stack-form" style="max-width:640px;">
          <label for="scope">Scope</label>
          <select id="scope" name="scope">
            <option value="site"{ss}>Site</option>
            <option value="subdomain"{su}>Subdomain</option>
            <option value="panel"{sp}>Panel (list panel + fallback archives)</option>
          </select>
          <label for="domain">Target domain (optional when recreating from archive metadata)</label>
          <select id="domain" name="domain">{opts}</select>
          <p class="muted">Leave domain empty and choose Panel scope to list fallback drops. For recreate, use a <code>backup-&lt;domain&gt;-...</code> filename or set the domain before Restore.</p>
          <button type="submit" class="btn-primary">List archives</button>
        </form>"#,
        ss = scope_sel("site"),
        su = scope_sel("subdomain"),
        sp = scope_sel("panel"),
        opts = site_options(&sites, domain),
    ));

    match list_restore_archives_with_fallback(scope, domain) {
        Ok((preferred, files)) => {
            if let Some(path) = preferred {
                body.push_str(&format!(
                    r#"<p>Preferred folder: <code>{}</code>. Also scanning fallback upload locations.</p>"#,
                    html_escape(&path)
                ));
            } else {
                body.push_str(
                    r#"<p>No preferred site folder resolved (domain optional). Showing panel + fallback archives.</p>"#,
                );
            }
            if files.is_empty() {
                body.push_str(r#"<p class="empty-state">No archives found. Copy a supported archive into a documented upload location, then list again.</p>"#);
            } else {
                let mut table = String::from(
                    r#"<div class="table-wrap"><table class="data-table"><thead><tr><th>File</th><th>Size</th><th>Location</th><th>Restore</th></tr></thead><tbody>"#,
                );
                for hit in files {
                    let target_domain = if domain.trim().is_empty() {
                        crate::backup_restore::infer_domain_from_archive_name(&hit.name)
                            .unwrap_or_default()
                    } else {
                        domain.to_string()
                    };
                    // Safe DOM id fragment: archive names are filesystem basenames.
                    let id_frag = html_escape(&hit.name).replace([' ', '.', '/', '\\', ':'], "-");
                    table.push_str(&format!(
                        r#"<tr>
                          <td data-label="File"><code>{name}</code></td>
                          <td data-label="Size">{size} bytes</td>
                          <td data-label="Location"><span class="muted">{prov}</span><br><code style="font-size:11px;">{dir}</code></td>
                          <td data-label="Restore">
                            <form method="get" action="/backups/restore/plan" class="stack-form">
                              <input type="hidden" name="scope" value="{scope}">
                              <input type="hidden" name="domain" value="{domain}">
                              <input type="hidden" name="archive" value="{name}">
                              <label class="muted" for="fmt-{id}">Format (optional)</label>
                              <select id="fmt-{id}" name="format">{formats}</select>
                              <p class="muted" style="margin:0;">Opens entity multi-select (domains, databases, optional email/Docker/DNS).</p>
                              <button type="submit" class="btn-primary">Select entities</button>
                            </form>
                          </td>
                        </tr>"#,
                        name = html_escape(&hit.name),
                        size = hit.size,
                        prov = html_escape(&hit.provenance),
                        dir = html_escape(&hit.dir.display().to_string()),
                        scope = html_escape(scope),
                        domain = html_escape(&target_domain),
                        formats = format_options("auto"),
                        id = id_frag,
                    ));
                }
                table.push_str("</tbody></table></div>");
                body.push_str(restore_list_styles());
                body.push_str(&wrap_activity_table_sized(
                    "restore-archives",
                    "Filter file, path or provenance",
                    &table,
                    RESTORE_LIST_PER_PAGE,
                ));
                body.push_str(&format!("<script>{}</script>", activity_list_script()));
            }
        }
        Err(err) => {
            body.push_str(&not_configured_body(
                &err,
                "Select a valid scope (Panel works without a domain), then list again.",
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restore_page_html_avoids_third_party_panel_brand() {
        let html = backups_restore_page("panel", "", None, None);
        let lower = html.to_ascii_lowercase();
        assert!(
            !lower.contains("cyberpanel"),
            "restore UI must not mention third-party panel brands"
        );
        assert!(lower.contains("source control-panel") || lower.contains("classic"));
        assert!(html.contains("value=\"classic\""));
        assert!(!html.contains("value=\"cyberpanel\""));
    }

    #[test]
    fn format_options_use_classic_token() {
        let html = format_options("classic");
        assert!(html.contains("value=\"classic\" selected"));
        assert!(!html.to_ascii_lowercase().contains("cyberpanel"));
    }

    #[test]
    fn restore_list_uses_stacked_activity_pattern() {
        // Empty list: no activity wrapper (empty-state only).
        let empty = backups_restore_page("panel", "", None, None);
        assert!(
            empty.contains("empty-state") || empty.contains("No archives"),
            "empty restore should show a hint"
        );
        assert!(
            empty.contains("Select entities")
                || empty.contains("entity multi-select")
                || empty.contains("Multi-select"),
            "restore copy should mention multi-entity selection"
        );
        // Markup contract when wrapping a non-empty table (unit-level, no FS scan).
        let table = r#"<div class="table-wrap"><table class="data-table"><thead><tr><th>File</th><th>Size</th><th>Location</th><th>Restore</th></tr></thead><tbody>
<tr><td data-label="File"><code>a.tar.gz</code></td><td data-label="Size">1 bytes</td><td data-label="Location">x</td><td data-label="Restore">y</td></tr>
</tbody></table></div>"#;
        let wrapped = wrap_activity_table_sized("restore-archives", "Filter", table, 5);
        assert!(wrapped.contains(r#"data-page-size="5""#));
        assert!(wrapped.contains("activity-list"));
        assert!(wrapped.contains(r#"data-label="Restore""#));
        assert!(!wrapped.contains("cyberpanel"));
    }
}
