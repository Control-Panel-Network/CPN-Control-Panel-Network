//! Restore plan page: multi-entity selection for one archive.

use crate::backup_restore::plan_restore_entities;
use crate::backup_restore_detect::BackupFormat;
use crate::backup_restore_entities::RestoreEntity;
use crate::panel_hubs::feature_shell;

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn format_options(selected: &str) -> String {
    let items = [
        ("auto", "Auto-detect"),
        ("cpn", BackupFormat::Cpn.label()),
        ("wordpress", BackupFormat::WordPress.label()),
        ("cpanel", BackupFormat::Cpanel.label()),
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

fn entity_rows(entities: &[RestoreEntity]) -> String {
    if entities.is_empty() {
        return r#"<p class="empty-state">No entities discovered. You can still confirm and restore with format defaults.</p>"#.into();
    }
    let mut out = String::from(
        r#"<div class="restore-entity-list" id="restore-entity-list">
        <div class="restore-entity-toolbar">
          <label><input type="checkbox" id="restore-select-all" checked> Select all default entities</label>
        </div>
        <ul class="restore-entity-cards">"#,
    );
    for ent in entities {
        let checked = if ent.default_selected { " checked" } else { "" };
        let badge = if ent.needs_extra_confirm {
            r#" <span class="muted">(needs optional confirm)</span>"#
        } else {
            ""
        };
        out.push_str(&format!(
            r#"<li class="restore-entity-card">
              <label class="restore-entity-label">
                <input type="checkbox" class="restore-entity-cb" name="entity" value="{id}" data-extra="{extra}"{checked}>
                <span>
                  <strong>{label}</strong>{badge}
                  <span class="muted" style="display:block;font-weight:400;">{detail}</span>
                  <span class="muted" style="display:block;font-size:12px;">id: <code>{id}</code></span>
                </span>
              </label>
            </li>"#,
            id = html_escape(&ent.id),
            label = html_escape(&ent.label),
            detail = html_escape(&ent.detail),
            badge = badge,
            checked = checked,
            extra = if ent.needs_extra_confirm { "1" } else { "0" },
        ));
    }
    out.push_str("</ul></div>");
    out
}

fn plan_styles() -> &'static str {
    r#"
<style>
.restore-entity-list { margin:12px 0; }
.restore-entity-toolbar { margin:0 0 10px; }
.restore-entity-cards {
  list-style:none; margin:0; padding:0; display:flex; flex-direction:column; gap:8px;
}
.restore-entity-card {
  border:1px solid var(--border, #d0d7de); border-radius:8px; padding:10px 12px;
  background:var(--panel-card-bg, #fff);
}
.restore-entity-label {
  display:flex; align-items:flex-start; gap:10px; font-weight:600; margin:0;
}
.restore-entity-label input { margin-top:4px; flex:0 0 auto; }
.restore-status-list {
  list-style:none; margin:10px 0 0; padding:0; display:flex; flex-direction:column; gap:6px;
}
.restore-status-item {
  border:1px solid var(--border, #d0d7de); border-radius:8px; padding:8px 10px;
}
.restore-status-ok { border-left:4px solid #1a7f37; }
.restore-status-skipped { border-left:4px solid #9a6700; }
.restore-status-failed { border-left:4px solid #cf222e; }
.restore-status-noted { border-left:4px solid #0969da; }
@media (max-width: 720px) {
  .restore-entity-label { flex-direction:row; }
}
</style>
"#
}

fn select_all_script() -> &'static str {
    r#"
(function(){
  var all = document.getElementById('restore-select-all');
  var form = document.getElementById('restore-plan-form');
  var hidden = document.getElementById('restore-entities-hidden');
  function syncHidden(){
    if (!hidden) return;
    var vals = [];
    document.querySelectorAll('.restore-entity-cb:checked').forEach(function(cb){
      vals.push(cb.value);
    });
    hidden.value = vals.join(',');
  }
  if (all) {
    all.addEventListener('change', function(){
      document.querySelectorAll('.restore-entity-cb').forEach(function(cb){
        if (cb.getAttribute('data-extra') === '1') {
          if (!all.checked) cb.checked = false;
          return;
        }
        cb.checked = all.checked;
      });
      syncHidden();
    });
  }
  document.querySelectorAll('.restore-entity-cb').forEach(function(cb){
    cb.addEventListener('change', syncHidden);
  });
  if (form) {
    form.addEventListener('submit', syncHidden);
  }
  syncHidden();
})();
"#
}

/// HTML for `/backups/restore/plan` (entity multi-select).
pub fn backups_restore_plan_page(
    scope: &str,
    domain: &str,
    archive: &str,
    format: &str,
    db_name: &str,
    notice: Option<&str>,
    error: Option<&str>,
) -> String {
    let mut body = String::from(plan_styles());
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
    body.push_str(&format!(
        r#"<p>Choose which entities to restore from <code>{}</code>. Use Select all for defaults; optional email / Docker / DNS stay off until you tick them and confirm.</p>
        <p><a class="btn-secondary" href="/backups/restore?scope={}&amp;domain={}">Back to archive list</a></p>"#,
        html_escape(archive),
        html_escape(scope),
        html_escape(domain),
    ));

    match plan_restore_entities(scope, domain, archive) {
        Ok((detected, entities, name)) => {
            body.push_str(&format!(
                r#"<p>Detected format: <strong>{}</strong> ({}){}</p>"#,
                html_escape(detected.format.label()),
                html_escape(detected.confidence),
                if detected.has_sql {
                    " · SQL dumps present"
                } else {
                    ""
                },
            ));
            if !detected.notes.is_empty() {
                body.push_str("<ul>");
                for note in &detected.notes {
                    body.push_str(&format!("<li class=\"muted\">{}</li>", html_escape(note)));
                }
                body.push_str("</ul>");
            }
            let confirm_js = "return confirm('Restore the selected entities? Website files and databases may change. Missing domains are created only when those confirmations are checked.');";
            body.push_str(&format!(
                r#"<form method="post" action="/backups/restore/run" class="stack-form" id="restore-plan-form" style="max-width:720px;">
                  <input type="hidden" name="scope" value="{scope}">
                  <input type="hidden" name="domain" value="{domain}">
                  <input type="hidden" name="archive" value="{archive}">
                  <input type="hidden" name="entities" id="restore-entities-hidden" value="">
                  <label for="fmt-plan">Format</label>
                  <select id="fmt-plan" name="format">{formats}</select>
                  <label for="db-plan">Target DB (WordPress, optional)</label>
                  <input id="db-plan" name="db_name" type="text" value="{db}" placeholder="optional">
                  {entities}
                  <label><input type="checkbox" name="create_domain_if_missing" value="1"> Create domain / subdomain if missing</label>
                  <label><input type="checkbox" name="confirm_create_domain" value="1"> Confirm create missing domain(s)</label>
                  <label><input type="checkbox" name="confirm_overwrite_files" value="1" required> Confirm overwrite website files</label>
                  <label><input type="checkbox" name="confirm_import_databases" value="1"> Confirm import databases / SQL</label>
                  <label><input type="checkbox" name="confirm_optional_entities" value="1"> Confirm optional email / Docker / DNS / panel-config</label>
                  <label><input type="checkbox" name="confirm_users_acl_packages" value="1"> Confirm Users / ACL / Packages merge (may create accounts and grants; does not wipe MFA; source passwords are not reused)</label>
                  <button type="submit" class="btn-primary" onclick="{confirm_js}">Restore selected</button>
                </form>
                <script>{script}</script>"#,
                scope = html_escape(scope),
                domain = html_escape(domain),
                archive = html_escape(&name),
                formats = format_options(if format.trim().is_empty() { "auto" } else { format }),
                db = html_escape(db_name),
                entities = entity_rows(&entities),
                confirm_js = confirm_js,
                script = select_all_script(),
            ));
        }
        Err(err) => {
            body.push_str(&format!(
                r#"<p class="panel-notice error" role="status">{}</p>"#,
                html_escape(&err)
            ));
        }
    }

    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Backups", Some("/backups")),
            ("Restore Backup", Some("/backups/restore")),
            ("Select entities", None),
        ],
        "Select restore entities",
        "Multi-select domains, databases, and optional payloads from one archive.",
        &body,
        None,
        None,
    )
}

/// Render stacked per-entity status cards (used after restore redirect via notice encoding).
pub fn entity_status_html(rows: &[(String, String, String, String)]) -> String {
    if rows.is_empty() {
        return String::new();
    }
    let mut out = String::from(r#"<ul class="restore-status-list">"#);
    for (id, label, status, detail) in rows {
        let cls = match status.as_str() {
            "ok" => "restore-status-ok",
            "failed" => "restore-status-failed",
            "noted" => "restore-status-noted",
            _ => "restore-status-skipped",
        };
        out.push_str(&format!(
            r#"<li class="restore-status-item {cls}"><strong>{label}</strong> · {status}<br><span class="muted">{detail}</span><br><code>{id}</code></li>"#,
            cls = cls,
            label = html_escape(label),
            status = html_escape(status),
            detail = html_escape(detail),
            id = html_escape(id),
        ));
    }
    out.push_str("</ul>");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backup_restore_entities::{EntityKind, RestoreEntity};

    #[test]
    fn entity_rows_include_checkboxes_and_select_all() {
        let ents = vec![
            RestoreEntity {
                id: "website".into(),
                kind: EntityKind::WebsiteFiles,
                label: "Website files".into(),
                detail: "docroot".into(),
                needs_extra_confirm: false,
                default_selected: true,
            },
            RestoreEntity {
                id: "db:news_cms".into(),
                kind: EntityKind::Database,
                label: "Database news_cms".into(),
                detail: "sql".into(),
                needs_extra_confirm: false,
                default_selected: true,
            },
            RestoreEntity {
                id: "email".into(),
                kind: EntityKind::Email,
                label: "Email".into(),
                detail: "vmail".into(),
                needs_extra_confirm: true,
                default_selected: false,
            },
        ];
        let html = entity_rows(&ents);
        assert!(html.contains("name=\"entity\""));
        assert!(html.contains("value=\"db:news_cms\""));
        assert!(html.contains("restore-select-all"));
        assert!(!html.to_ascii_lowercase().contains("cyberpanel"));
    }
}
