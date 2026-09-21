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

/// `can_manage`: panel admin only; non-admins get read-only version info.
pub fn version_management_page(can_manage: bool) -> String {
    let existing = detect_existing_install(RUNNING_VERSION);
    let installed = html_escape(&existing.package_version);
    let running = html_escape(RUNNING_VERSION);
    let source_block = if can_manage {
        r#"<div id="cpn-version-source" class="stack-form" style="margin-top:18px;max-width:640px;">
  <h3 style="margin:0 0 10px;">Update source</h3>
  <p class="muted" style="margin:0 0 12px;">Default is the official CPN repo. Point to your fork (owner/repo) for lab builds. Token is optional for public repos.</p>
  <label for="cpn-source-repo">GitHub repo (owner/repo)
    <input id="cpn-source-repo" type="text" autocomplete="off" spellcheck="false"
      placeholder="Control-Panel-Network/CPN-Control-Panel-Network"
      style="display:block;width:100%;margin-top:6px;box-sizing:border-box;padding:8px 10px;" />
  </label>
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
    <button type="button" class="btn-primary" id="cpn-version-upgrade-latest">Upgrade to latest</button>
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
    <p id="cpn-version-progress-label" class="muted" style="margin-top:8px;" role="status"></p>
  </div>
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
        r#"<ul class="kv-list">
  <li><span>Running</span><strong id="cpn-version-running">{running}</strong></li>
  <li><span>Installed package</span><strong id="cpn-version-installed">{installed}</strong></li>
  <li><span>Your source</span><strong id="cpn-version-source-tip">-</strong></li>
  <li><span>Upstream official</span><strong id="cpn-version-upstream-tip">-</strong></li>
  <li><span>Latest (configured source)</span><strong id="cpn-version-latest">-</strong></li>
  <li><span>Manifest</span><strong>{manifest}</strong></li>
</ul>
<p id="cpn-version-status" class="muted" role="status">Checking for updates...</p>
<div id="cpn-version-details" class="muted"></div>
<div class="stack-form" style="margin-top:16px;max-width:560px;">
  <button type="button" class="btn-primary" id="cpn-version-refresh">Check for updates</button>
</div>
{source_block}
{manage_block}
<p class="muted" style="margin-top:18px;">
  Package ops can run from this page when you are the panel admin and the installer service runs as root.
  CLI remains available: <code>sudo cpn-installer --upgrade</code> / <code>--repair</code> / <code>--downgrade --to X.Y.Z --yes</code>.
  Release assets: <a href="https://github.com/Control-Panel-Network/CPN-Control-Panel-Network/releases" target="_blank" rel="noopener noreferrer">GitHub Releases</a>.
</p>
{script}"#,
        running = running,
        installed = installed,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_page_uses_searchable_picker() {
        let html = version_management_page(true);
        assert!(html.contains("cpn-version-source-tip"));
        assert!(html.contains("cpn-source-repo"));
        assert!(html.contains("cpn-version-search"));
        assert!(html.contains("Type to search tags"));
        assert!(!html.contains("id=\"cpn-version-select\""));
        assert!(html.contains("Upgrade to latest"));
        assert!(html.contains("startRetryCountdown"));
        assert!(html.contains("data-retry-after"));
        assert!(!html.contains('\u{2014}'));
        assert!(!html.contains('\u{2013}'));
        assert!(!html.to_lowercase().contains("cyberpanel"));
    }
}
