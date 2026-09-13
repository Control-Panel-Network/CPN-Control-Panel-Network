//! Manage tab HTML for Git and Clone/Staging.

use crate::panel_site_clone::suggest_staging_domain;
use crate::panel_site_git::snapshot;
use crate::panel_site_tools_security::site_tools_csrf_token;
use crate::panel_website_manage_ui::html_escape;
use crate::sites::SiteRecord;

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

pub fn tab_clone(site: &SiteRecord, username: &str) -> String {
    let csrf = html_escape(&site_tools_csrf_token(username, &site.domain));
    let domain = html_escape(&site.domain);
    let staging = html_escape(
        &suggest_staging_domain(site).unwrap_or_else(|_| format!("staging.{}", site.domain)),
    );
    format!(
        r#"<div class="manage-log-panel">
  <h3>Clone / Staging</h3>
  <p class="manage-muted">Creates a new site under the parent home, copies <code>public_html</code> files, and registers it. Databases are not cloned.</p>
  <form method="post" action="/websites/clone" style="display:grid;gap:10px;max-width:520px;">
    <input type="hidden" name="domain" value="{domain}">
    <input type="hidden" name="csrf" value="{csrf}">
    <label style="display:flex;align-items:center;gap:8px;">
      <input type="checkbox" name="staging" value="1" checked>
      <span>Use staging slot (<code>{staging}</code>)</span>
    </label>
    <label class="manage-muted" for="clone-target">Or custom FQDN / subdomain label</label>
    <input id="clone-target" name="target" maxlength="253" placeholder="my-copy or copy.example.com"
      style="min-height:36px;padding:0 10px;border-radius:8px;border:1px solid var(--m-line);background:#0b0d12;color:var(--m-ink);">
    <button type="submit" class="btn-primary">Clone files to new site</button>
  </form>
  <p class="manage-muted" style="margin-top:12px;">After clone, open the new site Manage page to finish SSL or import a database dump if needed.</p>
</div>"#
    )
}
