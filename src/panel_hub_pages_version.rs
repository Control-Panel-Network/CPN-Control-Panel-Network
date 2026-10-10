//! Version Management settings page (searchable release picker).

use crate::manifest::detect_existing_install;
use crate::panel_hub_pages_version_script::version_page_script;
use crate::panel_hub_pages_version_source_script::version_source_script;
use crate::panel_hubs::feature_shell;

const RUNNING_VERSION: &str = env!("CARGO_PKG_VERSION");

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn norm_ver(value: &str) -> String {
    value
        .trim()
        .trim_start_matches('v')
        .trim_start_matches('V')
        .to_string()
}

/// `can_manage`: panel admin only; non-admins get read-only version info.
pub fn version_management_page(can_manage: bool) -> String {
    let existing = detect_existing_install(RUNNING_VERSION);
    let installed = html_escape(&existing.package_version);
    let running = html_escape(RUNNING_VERSION);
    let versions_match = norm_ver(&existing.package_version) == norm_ver(RUNNING_VERSION);
    let current_row_hidden = if versions_match { "" } else { " hidden" };
    let split_row_hidden = if versions_match { " hidden" } else { "" };
    let diverge_note = if versions_match {
        String::new()
    } else if cmp_ver_ahead(RUNNING_VERSION, &existing.package_version) {
        "Running binary is ahead of packaged RPM/DEB. Upgrade the package to match the running binary."
            .to_string()
    } else {
        "Packaged install differs from the running binary.".to_string()
    };
    let diverge_hidden = if diverge_note.is_empty() {
        " hidden"
    } else {
        ""
    };
    let diverge_note_esc = html_escape(&diverge_note);
    let source_block = if can_manage {
        r#"<div id="cpn-version-source" class="stack-form" style="margin-top:18px;max-width:640px;">
  <h3 style="margin:0 0 10px;">Update source</h3>
  <p class="muted" style="margin:0 0 12px;">Default is the official CPN repo. Point to your fork (owner/repo) for lab builds. Token is optional for public repos.</p>
  <label for="cpn-source-repo">GitHub repo (owner/repo)
    <input id="cpn-source-repo" type="text" autocomplete="off" spellcheck="false"
      placeholder="Control-Panel-Network/CPN-Control-Panel-Network"
      style="display:block;width:100%;margin-top:6px;box-sizing:border-box;padding:8px 10px;" />
  </label>
  <label for="cpn-source-branch" style="margin-top:12px;display:block;">Branch for latest commits
    <select id="cpn-source-branch" data-default-branch="stable"
      style="display:block;width:100%;margin-top:6px;box-sizing:border-box;padding:8px 10px;">
      <option value="stable" selected>stable (production: releases and stable commits)</option>
      <option value="dev">dev (pre-release testing, lab only)</option>
    </select>
  </label>
  <p id="cpn-source-branch-help" class="muted" style="margin:6px 0 0;font-size:13px;line-height:1.45;">
    Production servers should stay on <code>stable</code> (published releases and stable commits).
    Pick <code>dev</code> only on lab or pre-release hosts: dev commits can break, be rebased, or lag a hotfix.
    <strong>Upgrade to latest commits</strong>, the Update source row, and CLI <code>--to tip</code> follow the saved branch.
    Other branches of the configured repo are listed when GitHub answers; they are lab only.
  </p>
  <p id="cpn-source-branch-warning" class="muted" role="status" style="display:none;margin:8px 0 0;padding:8px 10px;border:1px solid #fb923c;border-radius:8px;color:#fb923c;font-size:13px;"></p>
  <label for="cpn-source-token" style="margin-top:12px;display:block;">GitHub token (optional)
    <input id="cpn-source-token" type="password" autocomplete="new-password"
      placeholder="Leave blank to keep existing token"
      style="display:block;width:100%;margin-top:6px;box-sizing:border-box;padding:8px 10px;" />
  </label>
  <label style="margin-top:10px;display:flex;gap:8px;align-items:center;">
    <input id="cpn-source-clear-token" type="checkbox" />
    <span>Clear stored GitHub token</span>
  </label>
  <div style="margin-top:12px;">
    <button type="button" class="btn-primary" id="cpn-source-save">Save update source</button>
  </div>
  <p id="cpn-source-status" class="muted" style="margin-top:10px;" role="status"></p>
</div>"#
    } else {
        ""
    };
    let manage_block = if can_manage {
        r#"<div id="cpn-version-ops" class="stack-form" style="margin-top:18px;max-width:640px;">
  <label for="cpn-version-search">Release / tag
    <input id="cpn-version-search" type="search" autocomplete="off" spellcheck="false"
      placeholder="Type to search tags (example: 0.2.6-alpha)"
      style="display:block;width:100%;margin-top:6px;box-sizing:border-box;padding:8px 10px;"
      aria-autocomplete="list" aria-controls="cpn-version-results" aria-expanded="false" />
  </label>
  <ul id="cpn-version-results" role="listbox"
    style="display:none;list-style:none;margin:4px 0 0;padding:0;max-height:220px;overflow:auto;border:1px solid var(--cpn-border, #334155);border-radius:8px;background:var(--cpn-surface, #0f172a);"></ul>
  <p class="muted" style="margin-top:8px;">Selected: <strong id="cpn-version-selected-label">-</strong></p>
  <div style="display:flex;flex-wrap:wrap;gap:8px;margin-top:12px;">
    <button type="button" class="btn-primary" id="cpn-version-upgrade-latest">Upgrade to latest release</button>
    <button type="button" class="btn-primary" id="cpn-version-upgrade-stable">Upgrade to latest commits</button>
    <button type="button" class="btn-primary" id="cpn-version-apply">Apply selected version</button>
    <button type="button" class="btn-primary" id="cpn-version-repair">Repair selected</button>
  </div>
  <div id="cpn-version-confirm" class="muted" style="display:none;margin-top:14px;padding:12px;border:1px solid var(--cpn-border, #334155);border-radius:8px;">
    <p id="cpn-version-confirm-text" style="margin:0 0 10px;"></p>
    <div style="display:flex;flex-wrap:wrap;gap:8px;">
      <button type="button" class="btn-primary" id="cpn-version-confirm-go">Confirm</button>
      <button type="button" id="cpn-version-confirm-cancel">Cancel</button>
    </div>
  </div>
  <div id="cpn-version-progress-wrap" style="display:none;margin-top:16px;">
    <div style="height:10px;background:rgba(148,163,184,0.25);border-radius:999px;overflow:hidden;">
      <div id="cpn-version-progress-bar" style="height:100%;width:0%;background:var(--cpn-accent, #2563eb);transition:width 0.2s ease;"></div>
    </div>
    <div style="display:flex;justify-content:space-between;align-items:baseline;gap:12px;flex-wrap:wrap;">
      <p id="cpn-version-progress-label" class="muted" style="margin-top:8px;flex:1 1 auto;" role="status"></p>
      <p id="cpn-version-progress-eta" class="muted" style="margin-top:8px;white-space:nowrap;display:none;" aria-live="polite" title="Estimated time remaining"></p>
    </div>
  </div>
  <details id="cpn-version-log" class="cpn-ssh-log">
    <summary>Installer log <span id="cpn-version-log-state" class="cpn-ssh-log-state" aria-live="polite"></span></summary>
    <div class="cpn-ssh-log-toolbar">
      <span class="cpn-ssh-log-hint">Times shown in your local time (dd/mm/yyyy, 24h). This tail is the current or last job; the full persistent log is <code>/var/log/cpn/panel.log</code>.</span>
      <span class="cpn-ssh-log-actions">
        <button type="button" id="cpn-version-log-copy" class="cpn-ssh-log-btn" title="Copy the installer log to the clipboard">Copy log</button>
        <a href="/server/logs/panel" class="cpn-ssh-log-btn" title="Open the persistent Main Log (Server &gt; Logs &gt; Panel)">Open Main Log</a>
      </span>
    </div>
    <pre id="cpn-version-log-pre" class="cpn-ssh-log-pre" aria-live="polite"></pre>
  </details>
  <p id="cpn-version-op-error" class="muted" style="margin-top:10px;color:#f87171;" role="alert"></p>
</div>"#
    } else {
        r#"<p class="muted" style="margin-top:16px;">Only the panel admin can upgrade, downgrade, or repair from this page.</p>"#
    };
    let mut script = version_page_script(can_manage);
    if can_manage {
        script.push_str(&version_source_script());
    }
    let body = format!(
        r#"<style>
.version-card-toolbar {{
  display:flex; flex-wrap:wrap; align-items:flex-start; justify-content:space-between;
  gap:12px; margin:0 0 8px;
}}
.version-card-toolbar .version-card-lede {{
  margin:0; flex:1 1 220px; max-width:520px; line-height:1.45;
}}
.version-card-toolbar #cpn-version-refresh {{
  flex:0 0 auto; margin-left:auto;
}}
.version-kv {{ list-style:none; padding:0; margin:0; }}
.version-kv > li {{
  display:grid; grid-template-columns:minmax(140px,180px) minmax(0,1fr);
  gap:12px 20px; align-items:start; padding:12px 0;
  border-top:1px solid var(--hairline,#2a2f3a); font-size:14px;
}}
/* display:grid must not override the HTML hidden attribute */
.version-kv > li[hidden] {{ display:none !important; }}
.version-kv > li:not([hidden]) {{ border-top:1px solid var(--hairline,#2a2f3a); }}
.version-kv > li:not([hidden]):first-child,
.version-kv > li[hidden] + li:not([hidden]) {{ border-top:0; }}
.version-kv .kv-label {{ color:var(--muted,#98a2b3); font-weight:500; padding-top:2px; }}
.version-kv .kv-value {{
  min-width:0; text-align:right; justify-self:stretch;
  overflow-wrap:anywhere; word-break:break-word; line-height:1.45;
}}
.version-kv .kv-value strong {{ font-weight:700; }}
.version-kv .kv-meta {{ display:block; margin-top:4px; color:var(--muted,#98a2b3); font-weight:400; font-size:13px; }}
.version-kv .kv-value[data-update-state="behind"],
.version-kv strong[data-update-state="behind"] {{ color:#fb923c; }}
.version-kv .kv-value[data-update-state="stale"],
.version-kv strong[data-update-state="stale"] {{ color:#f87171; }}
.version-kv .kv-value[data-update-state="current"],
.version-kv strong[data-update-state="current"] {{ color:#4ade80; }}
.version-kv .kv-hint {{ color:var(--muted,#98a2b3); font-size:13px; line-height:1.45; }}
.cpn-ssh-log {{
  margin-top:14px; max-width:640px;
  background:#0b1220; color:#d1fae5; border:1px solid #1f2937;
  border-radius:8px; padding:0;
}}
.cpn-ssh-log summary {{
  cursor:pointer; padding:10px 12px; color:#e2e8f0; font-weight:600;
  font-family:ui-monospace,SFMono-Regular,Menlo,Consolas,monospace; font-size:13px;
}}
.cpn-ssh-log-pre {{
  margin:0; padding:10px 12px 12px; max-height:280px; overflow:auto;
  font-family:ui-monospace,SFMono-Regular,Menlo,Consolas,monospace; font-size:12px;
  line-height:1.45; white-space:pre-wrap; word-break:break-word; color:#d1fae5;
}}
.cpn-ssh-log-pre .ssh-fail {{ color:#fca5a5; }}
.cpn-ssh-log-pre .ssh-note {{ color:#94a3b8; font-style:italic; }}
.cpn-ssh-log-pre .ssh-done {{ color:#4ade80; font-weight:700; }}
.cpn-ssh-log-pre .ssh-time {{ color:#64748b; }}
.cpn-ssh-log-state {{ font-weight:400; font-size:12px; margin-left:8px; color:#94a3b8; }}
.cpn-ssh-log-state[data-state="done"] {{ color:#4ade80; }}
.cpn-ssh-log-state[data-state="failed"] {{ color:#fca5a5; }}
.cpn-ssh-log-state[data-state="running"] {{ color:#fbbf24; }}
.cpn-ssh-log-toolbar {{
  display:flex; flex-wrap:wrap; gap:8px 12px; align-items:center; justify-content:space-between;
  padding:0 12px 8px; border-bottom:1px solid #1f2937;
}}
.cpn-ssh-log-hint {{ color:#94a3b8; font-size:12px; line-height:1.4; flex:1 1 260px; }}
.cpn-ssh-log-hint code {{ color:#cbd5e1; font-size:12px; }}
.cpn-ssh-log-actions {{ display:flex; gap:8px; flex:0 0 auto; }}
.cpn-ssh-log-btn {{
  display:inline-block; padding:5px 10px; border-radius:6px; border:1px solid #334155;
  background:#111827; color:#e2e8f0; font-size:12px; text-decoration:none; cursor:pointer;
  font-family:inherit; line-height:1.2;
}}
.cpn-ssh-log-btn:hover {{ background:#1f2937; }}
.cpn-ssh-log-btn[data-copied="1"] {{ border-color:#4ade80; color:#4ade80; }}
@media (max-width:640px) {{
  .version-kv > li {{ grid-template-columns:1fr; gap:4px; }}
  .version-kv .kv-value {{ text-align:left; }}
  .version-card-toolbar #cpn-version-refresh {{ margin-left:0; }}
}}
</style>
<div class="version-card-toolbar">
  <p class="muted version-card-lede">
    <strong>Running</strong> is the executing panel binary.
    <strong>Installed package</strong> is the RPM/DEB/manifest on disk (can lag after a hot-deploy).
    When they match, CPN shows one <strong>Current</strong> row.
  </p>
  <button type="button" class="btn-primary" id="cpn-version-refresh">Refresh</button>
</div>
<ul class="kv-list version-kv" id="cpn-version-summary">
  <li id="cpn-version-row-current"{current_row_hidden}>
    <span class="kv-label">Current</span>
    <span class="kv-value">
      <strong id="cpn-version-current">{running}</strong>
      <span class="kv-meta" id="cpn-version-current-sha"></span>
      <span class="kv-meta" id="cpn-version-current-date"></span>
      <span class="kv-meta" id="cpn-version-current-installed-at"></span>
    </span>
  </li>
  <li id="cpn-version-row-running"{split_row_hidden}>
    <span class="kv-label">Running</span>
    <span class="kv-value">
      <strong id="cpn-version-running">{running}</strong>
      <span class="kv-meta" id="cpn-version-running-date"></span>
      <span class="kv-meta" id="cpn-version-running-sha"></span>
    </span>
  </li>
  <li id="cpn-version-row-installed"{split_row_hidden}>
    <span class="kv-label">Installed package</span>
    <span class="kv-value">
      <strong id="cpn-version-installed">{installed}</strong>
      <span class="kv-meta" id="cpn-version-installed-date"></span>
      <span class="kv-meta" id="cpn-version-installed-at"></span>
    </span>
  </li>
  <li id="cpn-version-row-diverge"{diverge_hidden}>
    <span class="kv-label">Note</span>
    <span class="kv-value kv-hint" id="cpn-version-diverge">{diverge_note_esc}</span>
  </li>
  <li>
    <span class="kv-label">Latest available</span>
    <span class="kv-value">
      <strong id="cpn-version-latest">Loading...</strong>
      <span class="kv-meta" id="cpn-version-status" role="status">Checking for updates...</span>
    </span>
  </li>
  <li>
    <span class="kv-label">Update source</span>
    <span class="kv-value">
      <strong id="cpn-version-source-tip">Loading...</strong>
      <span class="kv-meta" id="cpn-version-upstream-tip"></span>
      <span class="kv-meta" id="cpn-version-stable-tip"></span>
    </span>
  </li>
  <li>
    <span class="kv-label">Manifest</span>
    <span class="kv-value"><strong>{manifest}</strong></span>
  </li>
  <li id="cpn-version-row-repo" hidden>
    <span class="kv-label">Configured repo</span>
    <span class="kv-value" id="cpn-version-repo">-</span>
  </li>
  <li id="cpn-version-row-source" hidden>
    <span class="kv-label">Package source</span>
    <span class="kv-value" id="cpn-version-pkg-source">-</span>
  </li>
  <li id="cpn-version-row-note" hidden>
    <span class="kv-label">Details</span>
    <span class="kv-value muted" id="cpn-version-note">-</span>
  </li>
</ul>
{source_block}
{manage_block}
<p class="muted" style="margin-top:14px;max-width:640px;">
  CPN supports the <strong>latest two published releases</strong> only (current release plus the previous release).
  The searchable picker lists <strong>all GitHub Releases with installable assets</strong> so you can upgrade or downgrade later.
  Older tags remain selectable for lab use, but they are outside support. Prefer upgrade to the newest release when one exists.
  When the saved branch (default <code>stable</code>) advances without a new tag, use <strong>Upgrade to latest commits</strong> (commit path: GitHub Actions binaries when present, otherwise a source build using cargo from PATH, rustup, or /home/cpn). Failures are written to the installer log below, Main Log, and Error logs.
  Choose the branch under <strong>Update source</strong>: <code>stable</code> for production, <code>dev</code> for lab and pre-release testing.
</p>
<p class="muted" style="margin-top:18px;">
  Package ops can run from this page when you are the panel admin and the installer service runs as root.
  CLI remains available: <code>sudo cpn-installer --upgrade</code> / <code>--repair</code> / <code>--downgrade --to X.Y.Z --yes</code>.
  Commit targets: <code>--to stable</code>, <code>--to dev</code>, <code>--to branch:&lt;name&gt;</code>, <code>--to &lt;branch&gt;@&lt;sha&gt;</code>, or <code>--to tip</code> for the saved branch.
  Release assets: <a href="https://github.com/Control-Panel-Network/CPN-Control-Panel-Network/releases" target="_blank" rel="noopener noreferrer">GitHub Releases</a>.
</p>
{script}"#,
        running = running,
        installed = installed,
        current_row_hidden = current_row_hidden,
        split_row_hidden = split_row_hidden,
        diverge_hidden = diverge_hidden,
        diverge_note_esc = diverge_note_esc,
        manifest = if existing.has_manifest {
            "present"
        } else {
            "missing"
        },
        source_block = source_block,
        manage_block = manage_block,
        script = script,
    );
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Settings", Some("/settings")),
            ("Version Management", None),
        ],
        "Version Management",
        "Update CPN",
        &body,
        None,
        None,
    )
}

fn cmp_ver_ahead(a: &str, b: &str) -> bool {
    use crate::releases::compare_versions;
    use std::cmp::Ordering;
    compare_versions(a, b) == Ordering::Greater
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_page_uses_searchable_picker() {
        let html = version_management_page(true);
        assert!(html.contains("cpn-version-source-tip"));
        assert!(html.contains("cpn-source-repo"));
        assert!(html.contains("id=\"cpn-source-branch\""));
        assert!(html.contains("<option value=\"stable\" selected>"));
        assert!(html.contains("<option value=\"dev\">"));
        assert!(html.contains("pre-release testing, lab only"));
        assert!(html.contains("cpn-source-branch-warning"));
        assert!(html.contains("Production servers should stay on <code>stable</code>"));
        assert!(html.contains("--to dev"));
        assert!(html.contains("/api/version-branches"));
        assert!(html.contains("cpn-version-search"));
        assert!(html.contains("Type to search tags"));
        assert!(!html.contains("id=\"cpn-version-select\""));
        assert!(html.contains("Upgrade to latest release"));
        assert!(html.contains("Upgrade to latest commits"));
        assert!(html.contains("cpn-version-stable-tip"));
        assert!(html.contains("latest two published releases"));
        assert!(html.contains("all GitHub Releases with installable assets"));
        assert!(html.contains("cpn-version-running-date"));
        assert!(html.contains("cpn-version-installed-at"));
        assert!(html.contains("version-kv"));
        assert!(html.contains("cpn-version-row-repo"));
        assert!(html.contains("cpn-version-row-current"));
        assert!(html.contains("cpn-version-row-diverge"));
        assert!(html.contains("Latest available"));
        assert!(html.contains(">Refresh<"));
        assert!(html.contains("version-card-toolbar"));
        assert!(html.contains("can lag after a hot-deploy"));
        assert!(html.contains("li[hidden]"));
        assert!(html.contains("cpn-version-log-pre"));
        assert!(html.contains("Installer log"));
        assert!(html.contains("id=\"cpn-version-log-copy\""));
        assert!(html.contains("href=\"/server/logs/panel\""));
        assert!(html.contains("/var/log/cpn/panel.log"));
        assert!(html.contains("cpn-version-log-state"));
        assert!(html.contains("dd/mm/yyyy, 24h"));
        assert!(html.contains("data-update-state=\"stale\""));
        assert!(html.contains("startRetryCountdown"));
        assert!(html.contains("data-retry-after"));
        assert!(html.contains("cpnVersionCache"));
        assert!(!html.contains("cpn-version-row-latest-tag"));
        assert!(!html.contains("cpn-version-row-commit"));
        assert!(!html.contains('\u{2014}'));
        assert!(!html.contains('\u{2013}'));
        assert!(!html.to_lowercase().contains("cyberpanel"));
    }

    #[test]
    fn read_only_users_get_no_branch_picker() {
        let html = version_management_page(false);
        assert!(!html.contains("id=\"cpn-source-branch\""));
        assert!(!html.contains("id=\"cpn-source-repo\""));
        assert!(html.contains("Only the panel admin can upgrade"));
        assert!(!html.contains('\u{2014}'));
    }
}
