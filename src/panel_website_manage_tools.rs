//! Manage tab HTML for Git and Clone/Staging.

use crate::panel_site_clone::suggest_staging_domain;
use crate::panel_site_git::snapshot;
use crate::panel_site_staging::collect_linked_database_names;
use crate::panel_site_tools_security::site_tools_csrf_token;
use crate::panel_website_manage_ui::html_escape;
use crate::sites::{staging_sites_for, SiteRecord};

pub fn tab_git(site: &SiteRecord, username: &str) -> String {
    let csrf = html_escape(&site_tools_csrf_token(username, &site.domain));
    let domain = html_escape(&site.domain);
    let snap = html_escape(&snapshot(site));
    format!(
        r#"<div class="manage-log-panel">
  <h3>Manage Git</h3>
  <p class="manage-muted">Allowlisted git only (status, remotes, branches, pull --ff-only, push, commit, init, clone). No arbitrary shell.</p>
  <pre class="manage-log-pre">{snap}</pre>
  <div class="manage-actions-row" style="margin-top:12px;">
    <form method="post" action="/websites/git" class="inline-form">
      <input type="hidden" name="domain" value="{domain}">
      <input type="hidden" name="csrf" value="{csrf}">
      <input type="hidden" name="action" value="status">
      <button type="submit" class="manage-btn">Refresh status</button>
    </form>
    <form method="post" action="/websites/git" class="inline-form">
      <input type="hidden" name="domain" value="{domain}">
      <input type="hidden" name="csrf" value="{csrf}">
      <input type="hidden" name="action" value="pull">
      <button type="submit" class="manage-btn">Pull (ff-only)</button>
    </form>
    <form method="post" action="/websites/git" class="inline-form">
      <input type="hidden" name="domain" value="{domain}">
      <input type="hidden" name="csrf" value="{csrf}">
      <input type="hidden" name="action" value="push">
      <button type="submit" class="manage-btn">Push</button>
    </form>
    <form method="post" action="/websites/git" class="inline-form">
      <input type="hidden" name="domain" value="{domain}">
      <input type="hidden" name="csrf" value="{csrf}">
      <input type="hidden" name="action" value="init">
      <button type="submit" class="manage-btn">git init</button>
    </form>
  </div>
  <form method="post" action="/websites/git" style="margin-top:14px;display:grid;gap:8px;max-width:520px;">
    <input type="hidden" name="domain" value="{domain}">
    <input type="hidden" name="csrf" value="{csrf}">
    <input type="hidden" name="action" value="commit">
    <label class="manage-muted" for="git-msg">Commit message</label>
    <input id="git-msg" name="message" maxlength="200" required placeholder="Describe the change"
      style="min-height:36px;padding:0 10px;border-radius:8px;border:1px solid var(--m-line);background:#0b0d12;color:var(--m-ink);">
    <button type="submit" class="btn-primary">Commit (add -A)</button>
  </form>
  <form method="post" action="/websites/git" style="margin-top:14px;display:grid;gap:8px;max-width:520px;">
    <input type="hidden" name="domain" value="{domain}">
    <input type="hidden" name="csrf" value="{csrf}">
    <input type="hidden" name="action" value="clone">
    <label class="manage-muted" for="git-url">Clone remote into site workdir</label>
    <input id="git-url" name="remote_url" maxlength="512" required placeholder="https://github.com/org/repo.git"
      style="min-height:36px;padding:0 10px;border-radius:8px;border:1px solid var(--m-line);background:#0b0d12;color:var(--m-ink);">
    <button type="submit" class="manage-btn">Clone (depth 1)</button>
  </form>
</div>"#
    )
}

fn staging_relationship_html(site: &SiteRecord) -> String {
    if let Some(prod) = site.staging_of.as_deref().filter(|s| !s.is_empty()) {
        let prod_q = html_escape(prod);
        return format!(
            r#"<p class="manage-muted" style="margin:0 0 12px;">This site is <strong>staging</strong> for production <a href="/websites/manage?domain={prod_q}&amp;tab=overview">{prod_q}</a>.</p>"#
        );
    }
    let Ok(children) = staging_sites_for(&site.domain) else {
        return String::new();
    };
    if children.is_empty() {
        return String::new();
    }
    let links: Vec<String> = children
        .iter()
        .map(|s| {
            let d = html_escape(&s.domain);
            format!(
                r#"<a href="/websites/manage?domain={d}&amp;tab=overview">{d}</a>"#
            )
        })
        .collect();
    format!(
        r#"<p class="manage-muted" style="margin:0 0 12px;">Production site. Staging: {}.</p>"#,
        links.join(", ")
    )
}

pub fn tab_clone(site: &SiteRecord, username: &str) -> String {
    let csrf = html_escape(&site_tools_csrf_token(username, &site.domain));
    let domain = html_escape(&site.domain);
    let staging = html_escape(
        &suggest_staging_domain(site).unwrap_or_else(|_| format!("staging.{}", site.domain)),
    );
    let relationship = staging_relationship_html(site);
    let dbs = collect_linked_database_names(site);
    let db_line = if dbs.is_empty() {
        "No databases registered for this domain (wp-config.php may still be detected at clone time)."
            .to_string()
    } else {
        format!(
            "Linked databases detected for clone: {}.",
            html_escape(&dbs.join(", "))
        )
    };
    format!(
        r#"<div class="manage-log-panel">
  <h3>Clone / Staging</h3>
  {relationship}
  <p class="manage-muted">Creates a separate staging site (never overwrites production). Choose what to copy. MariaDB dumps use new staging users and passwords (not logged in the panel). wp-config.php is updated when detected.</p>
  <p class="manage-muted">{db_line}</p>
  <form method="post" action="/websites/clone" style="display:grid;gap:10px;max-width:560px;">
    <input type="hidden" name="domain" value="{domain}">
    <input type="hidden" name="csrf" value="{csrf}">
    <fieldset style="border:1px solid var(--m-line);border-radius:8px;padding:10px 12px;margin:0;">
      <legend class="manage-muted" style="padding:0 6px;">Clone checklist</legend>
      <input type="hidden" name="clone_files" value="0">
      <label style="display:flex;align-items:center;gap:8px;margin:4px 0;">
        <input type="checkbox" name="clone_files" value="1" checked>
        <span>Files (<code>public_html</code> and common root configs)</span>
      </label>
      <input type="hidden" name="clone_databases" value="0">
      <label style="display:flex;align-items:center;gap:8px;margin:4px 0;">
        <input type="checkbox" name="clone_databases" value="1" checked>
        <span>MariaDB databases (registry + wp-config detection)</span>
      </label>
      <input type="hidden" name="clone_plugins" value="0">
      <label style="display:flex;align-items:center;gap:8px;margin:4px 0;">
        <input type="checkbox" name="clone_plugins" value="1" checked>
        <span>Site plugins/apps and host plugin activations</span>
      </label>
      <input type="hidden" name="clone_cron" value="0">
      <label style="display:flex;align-items:center;gap:8px;margin:4px 0;">
        <input type="checkbox" name="clone_cron" value="1" checked>
        <span>Cron jobs for this site</span>
      </label>
      <input type="hidden" name="clone_docker" value="0">
      <label style="display:flex;align-items:center;gap:8px;margin:4px 0;">
        <input type="checkbox" name="clone_docker" value="1" checked>
        <span>Docker compose stacks linked to this site (new stack id + copied data)</span>
      </label>
    </fieldset>
    <label style="display:flex;align-items:center;gap:8px;">
      <input type="checkbox" name="staging" value="1" checked>
      <span>Use staging hostname (<code>{staging}</code>)</span>
    </label>
    <label class="manage-muted" for="clone-target">Or custom FQDN / subdomain label</label>
    <input id="clone-target" name="target" maxlength="253" placeholder="my-copy or copy.example.com"
      style="min-height:36px;padding:0 10px;border-radius:8px;border:1px solid var(--m-line);background:#0b0d12;color:var(--m-ink);">
    <button type="submit" class="btn-primary">Create staging clone</button>
  </form>
  <p class="manage-muted" style="margin-top:12px;">Promote staging to production (sync/push live) is not automated yet. After create, issue SSL on the staging hostname from Manage if needed. Skipped items appear in the success notice.</p>
</div>"#
    )
}
