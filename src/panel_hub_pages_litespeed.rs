//! Server pages: Open OLS / Open OLSE WebAdmin and LiteSpeed plan/version management.

use crate::litespeed_stack::{
    LiteSpeedKind, OWNED_PLANS, STORE_OWNED_LSWS, STORE_SUPPORT, apply_serial, detect_kind,
    detect_version_label, downgrade_openlitespeed_to, litespeed_enterprise_installed, load_config,
    mask_serial, openlitespeed_installed, service_unit_hint, set_selected_tier, set_webadmin_url,
    upgrade_openlitespeed_packages, webadmin_reachable, webadmin_url,
};
use crate::panel_hubs::{feature_shell, not_configured_body};

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn flash(kind: &str, msg: Option<&str>) -> String {
    let Some(text) = msg.filter(|s| !s.trim().is_empty()) else {
        return String::new();
    };
    let cls = if kind == "ok" {
        "notice-ok"
    } else {
        "notice-error"
    };
    format!(
        r#"<p class="{cls}" role="status">{msg}</p>"#,
        cls = cls,
        msg = html_escape(text)
    )
}

fn open_page(
    crumbs_leaf: &str,
    title: &str,
    blurb: &str,
    installed: bool,
    missing_msg: &str,
    notice: Option<&str>,
    error: Option<&str>,
) -> String {
    if !installed {
        return feature_shell(
            &[
                ("Dashboard", Some("/dashboard")),
                ("Server", Some("/server")),
                (crumbs_leaf, None),
            ],
            title,
            blurb,
            &not_configured_body(
                missing_msg,
                "Install OpenLiteSpeed or LiteSpeed Enterprise, then refresh this page.",
            ),
            notice,
            error,
        );
    }
    let url = webadmin_url();
    let reachable = webadmin_reachable();
    let version = detect_version_label();
    let unit = service_unit_hint();
    let reach_label = if reachable {
        "Listener responds on WebAdmin port"
    } else {
        "Port probe failed (service may be down, or TLS/firewall blocks the probe)"
    };
    let body = format!(
        r#"{notice}{error}
<ul class="kv-list">
  <li><span>Edition</span><strong>{title}</strong></li>
  <li><span>Version</span><strong>{version}</strong></li>
  <li><span>Service</span><strong>{unit}</strong></li>
  <li><span>WebAdmin</span><strong><code>{url}</code></strong></li>
  <li><span>Reachability</span><strong>{reach}</strong></li>
</ul>
<p style="display:flex;flex-wrap:wrap;gap:10px;margin:16px 0;">
  <a class="btn-primary" href="{url}" target="_blank" rel="noopener noreferrer">Open WebAdmin</a>
  <a class="btn-secondary" href="/server/litespeed">LiteSpeed plans &amp; versions</a>
  <a class="btn-secondary" href="/server/services">Services Status</a>
</p>
<p class="muted">WebAdmin uses HTTPS (often a self-signed certificate in labs). Accept the browser warning when opening <code>{url}</code>. Default lab URL is <code>https://127.0.0.1:7080</code>; override under LiteSpeed plans &amp; versions if needed.</p>"#,
        notice = flash("ok", notice),
        error = flash("error", error),
        title = html_escape(title),
        version = html_escape(&version),
        unit = html_escape(&unit),
        url = html_escape(&url),
        reach = html_escape(reach_label),
    );
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Server", Some("/server")),
            (crumbs_leaf, None),
        ],
        title,
        blurb,
        &body,
        None,
        None,
    )
}

pub fn open_ols_page(notice: Option<&str>, error: Option<&str>) -> String {
    open_page(
        "Open OLS",
        "OpenLiteSpeed WebAdmin",
        "Open the OpenLiteSpeed WebAdmin console for this host.",
        openlitespeed_installed(),
        "OpenLiteSpeed is not installed on this host. Install it during CPN setup (OpenLiteSpeed engine) or from Apps / package recipes, then return here.",
        notice,
        error,
    )
}

pub fn open_olse_page(notice: Option<&str>, error: Option<&str>) -> String {
    open_page(
        "Open OLSE",
        "LiteSpeed Enterprise WebAdmin",
        "Open the LiteSpeed Enterprise (OLSE) WebAdmin console for this host.",
        litespeed_enterprise_installed(),
        "LiteSpeed Enterprise is not installed on this host. Install a licensed LSWS build from LiteSpeed, then return here. OpenLiteSpeed-only hosts use Open OLS instead.",
        notice,
        error,
    )
}

fn plan_options(selected: &str) -> String {
    let mut out = String::from(r#"<option value="">Select owned LSWS tier…</option>"#);
    for plan in OWNED_PLANS {
        let sel = if plan.id == selected { " selected" } else { "" };
        out.push_str(&format!(
            r#"<option value="{id}"{sel}>{label}</option>"#,
            id = html_escape(plan.id),
            sel = sel,
            label = html_escape(plan.label),
        ));
    }
    out
}

pub fn litespeed_manage_page(
    can_manage: bool,
    notice: Option<&str>,
    error: Option<&str>,
) -> String {
    let kind = detect_kind();
    let edition = match kind {
        Some(LiteSpeedKind::OpenLiteSpeed) => "OpenLiteSpeed",
        Some(LiteSpeedKind::Enterprise) => "LiteSpeed Enterprise",
        None => "Not installed",
    };
    let version = detect_version_label();
    let unit = service_unit_hint();
    let url = webadmin_url();
    let cfg = load_config();
    let selected = cfg.selected_tier.clone().unwrap_or_default();
    let serial_mask = cfg
        .serial
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .map(mask_serial)
        .unwrap_or_else(|| "-".into());
    let open_links = match kind {
        Some(LiteSpeedKind::OpenLiteSpeed) => {
            r#"<a class="btn-primary" href="/server/openlitespeed">Open OLS</a>"#
        }
        Some(LiteSpeedKind::Enterprise) => {
            r#"<a class="btn-primary" href="/server/litespeed-enterprise">Open OLSE</a>"#
        }
        None => r#"<span class="muted">Install OLS or LSWS first.</span>"#,
    };

    let manage = if can_manage {
        format!(
            r#"<div class="stack-form" style="margin-top:18px;max-width:640px;">
  <h3 style="margin:0 0 8px;font-size:16px;">Plan / tier (owned LSWS)</h3>
  <p class="muted">Purchase or change the license on the LiteSpeed store, then apply the serial here. CPN does not scrape store credentials.</p>
  <p style="display:flex;flex-wrap:wrap;gap:10px;margin:10px 0;">
    <a class="btn-secondary" href="{store_owned}" target="_blank" rel="noopener noreferrer">Owned LSWS store</a>
    <a class="btn-secondary" href="{store_support}" target="_blank" rel="noopener noreferrer">Support services store</a>
  </p>
  <form method="post" action="/server/litespeed/tier" class="stack-form">
    <label for="tier">Selected tier
      <select id="tier" name="tier" style="display:block;width:100%;margin-top:6px;">{options}</select>
    </label>
    <button type="submit" class="btn-primary" style="margin-top:10px;">Save tier preference</button>
  </form>
  <form method="post" action="/server/litespeed/serial" class="stack-form" style="margin-top:18px;"
    onsubmit="return confirm('Apply this serial to /usr/local/lsws/conf/serial.no and restart LiteSpeed?');">
    <label for="serial">License serial
      <input id="serial" name="serial" type="password" autocomplete="off" spellcheck="false"
        placeholder="Paste serial from LiteSpeed store"
        style="display:block;width:100%;margin-top:6px;box-sizing:border-box;padding:8px 10px;" />
    </label>
    <p class="muted">Stored serial (masked): <strong>{serial_mask}</strong></p>
    <button type="submit" class="btn-primary">Apply serial &amp; restart</button>
  </form>
  <form method="post" action="/server/litespeed/webadmin-url" class="stack-form" style="margin-top:18px;">
    <label for="webadmin_url">WebAdmin URL override
      <input id="webadmin_url" name="webadmin_url" type="url" value="{url}"
        placeholder="https://127.0.0.1:7080"
        style="display:block;width:100%;margin-top:6px;box-sizing:border-box;padding:8px 10px;" />
    </label>
    <button type="submit" class="btn-secondary" style="margin-top:10px;">Save WebAdmin URL</button>
  </form>
  <h3 style="margin:22px 0 8px;font-size:16px;">Package upgrade / downgrade</h3>
  <p class="muted">OpenLiteSpeed uses the LiteSpeed RPM/APT repos already prepared by CPN. Enterprise binary switches follow your licensed store tier + serial; use the store links above for purchases.</p>
  <form method="post" action="/server/litespeed/upgrade" style="margin-top:10px;"
    onsubmit="return confirm('Upgrade OpenLiteSpeed packages on this host?');">
    <button type="submit" class="btn-primary">Upgrade OpenLiteSpeed packages</button>
  </form>
  <form method="post" action="/server/litespeed/downgrade" class="stack-form" style="margin-top:14px;"
    onsubmit="return confirm('Downgrade OpenLiteSpeed to the typed version? This can break sites if the package is unavailable.');">
    <label for="version">Downgrade to version
      <input id="version" name="version" type="text" autocomplete="off" spellcheck="false"
        placeholder="example: 1.8.1"
        style="display:block;width:100%;margin-top:6px;box-sizing:border-box;padding:8px 10px;" />
    </label>
    <button type="submit" class="btn-secondary" style="margin-top:10px;">Downgrade OpenLiteSpeed</button>
  </form>
</div>"#,
            store_owned = STORE_OWNED_LSWS,
            store_support = STORE_SUPPORT,
            options = plan_options(&selected),
            serial_mask = html_escape(&serial_mask),
            url = html_escape(&url),
        )
    } else {
        r#"<p class="muted" style="margin-top:16px;">Only the panel admin can change tiers, apply serials, or upgrade/downgrade packages.</p>"#
            .to_string()
    };

    let body = format!(
        r#"{notice}{error}
<ul class="kv-list">
  <li><span>Detected</span><strong>{edition}</strong></li>
  <li><span>Version</span><strong>{version}</strong></li>
  <li><span>Service</span><strong>{unit}</strong></li>
  <li><span>WebAdmin</span><strong><code>{url}</code></strong></li>
  <li><span>Preferred tier</span><strong>{tier}</strong></li>
</ul>
<p style="display:flex;flex-wrap:wrap;gap:10px;margin:16px 0;">{open_links}
  <a class="btn-secondary" href="{url}" target="_blank" rel="noopener noreferrer">Open WebAdmin</a>
</p>
{manage}"#,
        notice = flash("ok", notice),
        error = flash("error", error),
        edition = html_escape(edition),
        version = html_escape(&version),
        unit = html_escape(&unit),
        url = html_escape(&url),
        tier = html_escape(if selected.is_empty() {
            "-"
        } else {
            OWNED_PLANS
                .iter()
                .find(|p| p.id == selected)
                .map(|p| p.label)
                .unwrap_or(selected.as_str())
        }),
        open_links = open_links,
        manage = manage,
    );

    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Server", Some("/server")),
            ("LiteSpeed", None),
        ],
        "LiteSpeed plans & versions",
        "Open WebAdmin, choose owned LSWS tiers, apply license serials, and upgrade or downgrade OpenLiteSpeed packages.",
        &body,
        None,
        None,
    )
}

pub fn run_set_tier(tier: &str) -> Result<String, String> {
    set_selected_tier(tier)
}

pub fn run_apply_serial(serial: &str) -> Result<String, String> {
    apply_serial(serial)
}

pub fn run_set_webadmin_url(url: &str) -> Result<String, String> {
    set_webadmin_url(url)
}

pub fn run_upgrade() -> Result<String, String> {
    upgrade_openlitespeed_packages()
}

pub fn run_downgrade(version: &str) -> Result<String, String> {
    downgrade_openlitespeed_to(version)
}

#[cfg(test)]
mod tests {
    use super::plan_options;

    #[test]
    fn plan_options_include_elite() {
        let html = plan_options("web_host_elite");
        assert!(html.contains("web_host_elite"));
        assert!(html.contains("selected"));
        assert!(html.contains("Web Host Elite"));
    }
}
