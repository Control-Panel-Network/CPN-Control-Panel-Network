//! Server hub page for migrating another hosting panel into CPN.

use crate::foreign_panel::catalog::CATALOG;
use crate::foreign_panel::{detect_at, host_root, load_report};

fn esc(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn panel_migrate_page(notice: Option<&str>, error: Option<&str>) -> String {
    let root = host_root();
    let found = detect_at(&root);
    let mut body = String::from(
        r#"<h2>Migrate another panel</h2>
<p>CPN can copy websites, databases, mail, and DNS from a panel already installed on this server, then remove that panel's programs and the duplicate files. Shared services CPN still needs (MariaDB, the web server, mail) stay installed. A site that already lives in the CPN docroot is kept in place.</p>
<p class="muted">Mail, DNS, and panel users are best-effort. Users are listed in the verify report and are not created as CPN accounts. Database import needs the MariaDB client. Large copies belong on <code>cpn panel-migrate</code> if this page times out.</p>"#,
    );
    if let Some(notice) = notice {
        body.push_str(&format!("<p class=\"notice\">{}</p>", esc(notice)));
    }
    if let Some(error) = error {
        body.push_str(&format!("<p class=\"error\">{}</p>", esc(error)));
    }
    body.push_str(&format!(
        "<p>Search root: <code>{}</code></p>",
        esc(&root.to_string_lossy())
    ));
    if found.is_empty() {
        body.push_str("<p>No other hosting panels were found.</p><ul>");
        for spec in CATALOG {
            body.push_str(&format!(
                "<li><code>{}</code> {}</li>",
                esc(spec.id),
                esc(spec.name)
            ));
        }
        body.push_str("</ul>");
        return body;
    }
    body.push_str("<table class=\"data\"><thead><tr><th>Panel</th><th>Websites</th><th>Status</th><th></th></tr></thead><tbody>");
    for row in &found {
        let status = if row.inventory.mapped {
            format!("{} site(s)", row.inventory.sites.len())
        } else {
            esc(&row.inventory.note)
        };
        let report = load_report(&row.id).ok();
        let verified = report.as_ref().map(|item| item.verified).unwrap_or(false);
        body.push_str(&format!(
            "<tr><td>{} <code>{}</code></td><td>{}</td><td>{}</td><td>",
            esc(&row.name),
            esc(&row.id),
            status,
            if verified {
                "Copy verified"
            } else if report.is_some() {
                "Copy needs attention"
            } else {
                "Not copied"
            }
        ));
        if row.inventory.mapped && !row.inventory.sites.is_empty() {
            body.push_str(&format!(
                r#"<form method="post" action="/server/panel-migrate/apply" class="inline-form">
<input type="hidden" name="id" value="{id}">
<label><input type="checkbox" name="port_ok" value="1"> Port OK</label>
<label>Type MIGRATE <input name="confirm" required autocomplete="off"></label>
<button type="submit">Copy into CPN</button>
</form>"#,
                id = esc(&row.id)
            ));
            if verified {
                body.push_str(&format!(
                    r#"<form method="post" action="/server/panel-migrate/wipe" class="inline-form">
<input type="hidden" name="id" value="{id}">
<label>Type REMOVE <input name="confirm" required autocomplete="off"></label>
<button type="submit">Remove panel</button>
</form>"#,
                    id = esc(&row.id)
                ));
            }
        }
        body.push_str("</td></tr>");
        if !row.inventory.sites.is_empty() {
            body.push_str("<tr><td colspan=\"4\"><ul>");
            for site in &row.inventory.sites {
                body.push_str(&format!(
                    "<li><code>{}</code> {} (databases {}, mail {}, DNS {})</li>",
                    esc(&site.domain),
                    esc(&site.docroot),
                    site.databases.len(),
                    site.mail_dirs.len(),
                    site.zone_files.len()
                ));
            }
            body.push_str("</ul></td></tr>");
        }
        if let Some(report) = report
            && !report.delete_paths.is_empty()
        {
            body.push_str("<tr><td colspan=\"4\"><p>Paths removed only after REMOVE:</p><ul>");
            for path in &report.delete_paths {
                body.push_str(&format!("<li><code>{}</code></li>", esc(path)));
            }
            body.push_str("</ul></td></tr>");
        }
    }
    body.push_str("</tbody></table>");
    body
}
