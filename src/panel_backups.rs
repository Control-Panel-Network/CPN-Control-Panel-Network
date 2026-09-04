//! Backups panel page HTML (selective chooser).

use crate::backups::{BackupScope, is_subdomain_site, list_backup_files, resolve_archive_dir};
use crate::paths::{legacy_panel_backups_dir, panel_backups_dir};
use crate::service_detect::detect_database;
use crate::sites::{SiteRecord, list_sites};

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn section_heading(title: &str, blurb: &str) -> String {
    format!(
        r#"
      <div class="dashboard-heading">
        <div>
          <p class="eyebrow">CPN PANEL</p>
          <h1>{title}</h1>
          <p>{blurb}</p>
        </div>
      </div>"#,
        title = html_escape(title),
        blurb = html_escape(blurb),
    )
}

fn notice_block(kind: &str, message: Option<&str>) -> String {
    let Some(message) = message.filter(|value| !value.is_empty()) else {
        return String::new();
    };
    let class = if kind == "error" {
        "panel-notice error"
    } else {
        "panel-notice ok"
    };
    format!(
        r#"<p class="{class}" role="status">{msg}</p>"#,
        msg = html_escape(message)
    )
}

fn backup_rows(files: &[(String, u64)], empty_msg: &str) -> String {
    if files.is_empty() {
        return format!(
            r#"<p class="empty-state">{msg}</p>"#,
            msg = html_escape(empty_msg)
        );
    }
    let mut rows = String::from(
        r#"<div class="table-wrap"><table class="data-table">
      <thead><tr><th>File</th><th>Size</th></tr></thead><tbody>"#,
    );
    for (name, size) in files {
        rows.push_str(&format!(
            r#"<tr><td><code>{name}</code></td><td>{size} bytes</td></tr>"#,
            name = html_escape(name),
            size = size,
        ));
    }
    rows.push_str("</tbody></table></div>");
    rows
}

fn site_options(sites: &[SiteRecord], selected: &str, subdomains_only: bool) -> String {
    let mut out = String::from(r#"<option value="">Select domain</option>"#);
    for site in sites {
        let is_sub = is_subdomain_site(&site.domain);
        if subdomains_only && !is_sub {
            continue;
        }
        if !subdomains_only && is_sub {
            continue;
        }
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

fn checkbox(name: &str, label: &str, checked: bool, disabled: bool, hint: &str) -> String {
    let c = if checked { " checked" } else { "" };
    let d = if disabled { " disabled" } else { "" };
    let hint_html = if hint.is_empty() {
        String::new()
    } else {
        format!(
            r#"<span class="muted" style="display:block;font-weight:400;">{h}</span>"#,
            h = html_escape(hint)
        )
    };
    format!(
        r#"<label style="display:flex;align-items:flex-start;gap:10px;font-weight:600;">
          <input type="checkbox" name="{name}" value="1"{c}{d} style="margin-top:4px;">
          <span>{label}{hint}</span>
        </label>"#,
        name = html_escape(name),
        c = c,
        d = d,
        label = html_escape(label),
        hint = hint_html,
    )
}

pub struct BackupsPageQuery<'a> {
    pub notice: Option<&'a str>,
    pub error: Option<&'a str>,
    pub scope: &'a str,
    pub domain: &'a str,
}

pub fn backups_create_main(q: BackupsPageQuery<'_>) -> String {
    let sites = list_sites().unwrap_or_default();
    let scope = BackupScope::parse(q.scope).unwrap_or(BackupScope::Panel);
    let domain = q.domain.trim();
    let archive_dir = resolve_archive_dir(scope, domain).ok();
    let files = archive_dir
        .as_ref()
        .map(|(dir, _)| list_backup_files(dir))
        .unwrap_or_default();
    let path_display =
        archive_dir
            .as_ref()
            .map(|(_, p)| p.clone())
            .unwrap_or_else(|| match scope {
                BackupScope::Panel => panel_backups_dir().display().to_string(),
                BackupScope::Site => "/home/<domain>/backups/".into(),
                BackupScope::Subdomain => "/home/<parent>/<sub.fqdn>/backups/".into(),
            });
    let path_blurb = if path_display.contains('<') {
        "<p>Choose a website or subdomain to see where archives are saved.</p>".to_string()
    } else {
        format!(
            "<p>Saved in <code>{}</code>.</p>",
            html_escape(&path_display)
        )
    };
    let legacy = legacy_panel_backups_dir();
    let migrate_note = if scope == BackupScope::Panel
        && legacy.is_dir()
        && legacy != panel_backups_dir()
    {
        format!(
            r#"<p class="muted">Older archives may still exist under <code>{}</code>. New panel backups use <code>{}</code>.</p>"#,
            html_escape(&legacy.display().to_string()),
            html_escape(&panel_backups_dir().display().to_string()),
        )
    } else {
        String::new()
    };
    let db = detect_database();
    let db_available = db.listening_3306 || db.service_label != "Not detected";
    let scope_panel = if scope == BackupScope::Panel {
        " checked"
    } else {
        ""
    };
    let scope_site = if scope == BackupScope::Site {
        " checked"
    } else {
        ""
    };
    let scope_sub = if scope == BackupScope::Subdomain {
        " checked"
    } else {
        ""
    };
    let domain_block = match scope {
        BackupScope::Panel => String::new(),
        BackupScope::Site => format!(
            r#"<label for="domain">Website domain</label>
        <select id="domain" name="domain">{opts}</select>"#,
            opts = site_options(&sites, domain, false),
        ),
        BackupScope::Subdomain => format!(
            r#"<label for="domain">Subdomain</label>
        <select id="domain" name="domain">{opts}</select>"#,
            opts = site_options(&sites, domain, true),
        ),
    };
    let content_checks = match scope {
        BackupScope::Panel => format!(
            "{}{}",
            checkbox(
                "panel_config",
                "Panel config",
                true,
                false,
                "Bootstrap, accounts, site records, preferences, SMTP when present."
            ),
            checkbox(
                "databases",
                "Databases",
                false,
                !db_available,
                if db_available {
                    "mysqldump of local MariaDB/MySQL when available."
                } else {
                    "No local database detected yet."
                }
            ),
        ),
        BackupScope::Site | BackupScope::Subdomain => format!(
            "{}{}{}{}{}",
            checkbox("website_files", "Website files (docroot)", true, false, ""),
            checkbox("backups_folder", "Backups folder", false, false, ""),
            checkbox("plugins_folder", "Plugins folder", false, false, ""),
            checkbox(
                "databases",
                "Databases",
                false,
                !db_available,
                if db_available {
                    "mysqldump when a local DB exists."
                } else {
                    "No local database detected yet."
                }
            ),
            checkbox(
                "ftp",
                "FTP content / users",
                false,
                true,
                "Not implemented yet (honest stub)."
            ),
        ),
    };
    let domain_get = if scope == BackupScope::Panel {
        String::new()
    } else {
        format!(
            r#"{domain_block}
          <button type="submit" class="btn-secondary" style="min-height:40px;padding:0 14px;border:0;border-radius:999px;background:#f2f4f7;color:#344054;font-weight:700;cursor:pointer;">Refresh list</button>"#
        )
    };
    let domain_post = match scope {
        BackupScope::Panel => String::new(),
        BackupScope::Site => format!(
            r#"<label for="run-domain">Website domain</label>
        <select id="run-domain" name="domain" required>{opts}</select>"#,
            opts = site_options(&sites, domain, false),
        ),
        BackupScope::Subdomain => format!(
            r#"<label for="run-domain">Subdomain</label>
        <select id="run-domain" name="domain" required>{opts}</select>"#,
            opts = site_options(&sites, domain, true),
        ),
    };
    format!(
        r#"{heading}
      {ok}
      {err}
      <article class="section-card">
        <h2>Selective backup</h2>
        {path_blurb}
        {migrate}
        <form method="get" action="/backups" class="stack-form" style="max-width:560px;">
          <fieldset style="border:1px solid #eeeef0;border-radius:12px;padding:12px 14px;">
            <legend style="font-weight:700;padding:0 6px;">Scope</legend>
            <label style="display:block;margin:6px 0;"><input type="radio" name="scope" value="panel"{sp} onchange="this.form.submit()"> Panel config</label>
            <label style="display:block;margin:6px 0;"><input type="radio" name="scope" value="site"{ss} onchange="this.form.submit()"> Website domain</label>
            <label style="display:block;margin:6px 0;"><input type="radio" name="scope" value="subdomain"{su} onchange="this.form.submit()"> Subdomain</label>
          </fieldset>
          {domain_get}
        </form>
        <form method="post" action="/backups/run" class="stack-form" style="margin-top:18px;max-width:560px;">
          <input type="hidden" name="scope" value="{scope_val}">
          {domain_post}
          <fieldset style="border:1px solid #eeeef0;border-radius:12px;padding:12px 14px;">
            <legend style="font-weight:700;padding:0 6px;">Contents</legend>
            {checks}
          </fieldset>
          <button type="submit" class="btn-primary">Run backup</button>
        </form>
      </article>
      <article class="section-card" style="margin-top:22px;">
        <h2>Archives in this scope</h2>
        <p class="muted">Archives for this scope.</p>
        {rows}
      </article>"#,
        heading = section_heading(
            "Create Backup",
            "Choose scope and contents; archives live under /home for the selected scope.",
        ),
        ok = notice_block("ok", q.notice),
        err = notice_block("error", q.error),
        path_blurb = path_blurb,
        migrate = migrate_note,
        sp = scope_panel,
        ss = scope_site,
        su = scope_sub,
        domain_get = domain_get,
        scope_val = scope.as_str(),
        domain_post = domain_post,
        checks = content_checks,
        rows = backup_rows(
            &files,
            "No archives yet for this scope. Run a backup above.",
        ),
    )
}
