//! Dashboard Statistics card: used / limit rows for the signed-in account.

use crate::backups::is_subdomain_site;
use crate::packages::{UNLIMITED, is_panel_admin, package_for_account, usage_for_account};
use crate::panel_storage_fmt::{format_used_count, format_used_limit_for_user, unlimited_html};
use crate::panel_website_resources::site_used_bytes;
use crate::sites::{SiteRecord, list_sites};

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn owned_sites(username: &str) -> Vec<SiteRecord> {
    list_sites()
        .unwrap_or_default()
        .into_iter()
        .filter(|s| s.owner.trim().eq_ignore_ascii_case(username.trim()))
        .collect()
}

fn aliases_used(sites: &[SiteRecord]) -> u64 {
    sites.iter().map(|s| s.aliases.len() as u64).sum()
}

fn subdomains_used(sites: &[SiteRecord]) -> u64 {
    sites
        .iter()
        .filter(|s| is_subdomain_site(&s.domain))
        .count() as u64
}

fn disk_bytes_used(sites: &[SiteRecord]) -> u64 {
    sites
        .iter()
        .map(|s| site_used_bytes(s, 8_000).unwrap_or(0))
        .fold(0u64, u64::saturating_add)
}

fn count_html(used: u64, limit: i64) -> String {
    let text = format_used_count(used, limit);
    if crate::packages::is_unlimited(limit) {
        format!(
            r#"Used {} of {}"#,
            html_escape(&used.to_string()),
            unlimited_html()
        )
    } else {
        html_escape(&text)
    }
}

fn bytes_html(username: &str, used_bytes: u64, limit_mb: i64) -> String {
    let text = format_used_limit_for_user(username, used_bytes, limit_mb);
    if crate::packages::is_unlimited(limit_mb) {
        let used = crate::panel_storage_fmt::format_bytes_for_user(username, used_bytes);
        format!("Used {} of {}", html_escape(&used), unlimited_html())
    } else {
        html_escape(&text)
    }
}

fn row(label: &str, value_html: &str) -> String {
    format!(
        r#"<li><span class="cpn-stats-label">{label}</span><span class="cpn-stats-value">{value}</span></li>"#,
        label = html_escape(label),
        value = value_html,
    )
}

/// Statistics widget for `/dashboard`.
pub fn dashboard_stats_html(username: &str) -> String {
    let sites = owned_sites(username);
    let usage = usage_for_account(username).ok();
    let pkg = package_for_account(username).ok();
    let domains_limit = usage
        .as_ref()
        .map(|u| u.domains_limit)
        .or_else(|| pkg.as_ref().map(|p| p.domains))
        .unwrap_or(UNLIMITED);
    let emails_limit = usage
        .as_ref()
        .map(|u| u.emails_limit)
        .or_else(|| pkg.as_ref().map(|p| p.emails))
        .unwrap_or(UNLIMITED);
    let db_limit = usage
        .as_ref()
        .map(|u| u.databases_limit)
        .or_else(|| pkg.as_ref().map(|p| p.databases))
        .unwrap_or(UNLIMITED);
    let ftp_limit = usage
        .as_ref()
        .map(|u| u.ftp_limit)
        .or_else(|| pkg.as_ref().map(|p| p.ftp_accounts))
        .unwrap_or(UNLIMITED);
    let disk_limit = usage
        .as_ref()
        .map(|u| u.disk_mb_limit)
        .or_else(|| pkg.as_ref().map(|p| p.disk_mb))
        .unwrap_or(UNLIMITED);
    let bw_limit = usage
        .as_ref()
        .map(|u| u.bandwidth_mb_limit)
        .or_else(|| pkg.as_ref().map(|p| p.bandwidth_mb))
        .unwrap_or(UNLIMITED);
    let lists_limit = usage
        .as_ref()
        .map(|u| u.mailing_lists_limit)
        .or_else(|| pkg.as_ref().map(|p| p.mailing_lists))
        .unwrap_or(UNLIMITED);
    let ar_limit = usage
        .as_ref()
        .map(|u| u.autoresponders_limit)
        .or_else(|| pkg.as_ref().map(|p| p.autoresponders))
        .unwrap_or(UNLIMITED);
    let fwd_limit = usage
        .as_ref()
        .map(|u| u.forwarders_limit)
        .or_else(|| pkg.as_ref().map(|p| p.forwarders))
        .unwrap_or(UNLIMITED);
    let filt_limit = usage
        .as_ref()
        .map(|u| u.email_filters_limit)
        .or_else(|| pkg.as_ref().map(|p| p.email_filters))
        .unwrap_or(UNLIMITED);
    let alias_limit = usage
        .as_ref()
        .map(|u| u.alias_domains_limit)
        .or_else(|| pkg.as_ref().map(|p| p.alias_domains))
        .unwrap_or(UNLIMITED);
    let sub_limit = usage
        .as_ref()
        .map(|u| u.subdomains_limit)
        .or_else(|| pkg.as_ref().map(|p| p.subdomains))
        .unwrap_or(UNLIMITED);
    let domains_used = usage
        .as_ref()
        .map(|u| u.domains_used)
        .unwrap_or_else(|| sites.len() as u64);
    let emails_used = usage.as_ref().map(|u| u.emails_used).unwrap_or(0);
    let db_used = usage.as_ref().map(|u| u.databases_used).unwrap_or(0);
    let ftp_used = usage.as_ref().map(|u| u.ftp_used).unwrap_or(0);
    let lists_used = usage.as_ref().map(|u| u.mailing_lists_used).unwrap_or(0);
    let ar_used = usage.as_ref().map(|u| u.autoresponders_used).unwrap_or(0);
    let fwd_used = usage.as_ref().map(|u| u.forwarders_used).unwrap_or(0);
    let filt_used = usage.as_ref().map(|u| u.email_filters_used).unwrap_or(0);
    let alias_used = usage
        .as_ref()
        .map(|u| u.alias_domains_used)
        .unwrap_or_else(|| aliases_used(&sites));
    let sub_used = usage
        .as_ref()
        .map(|u| u.subdomains_used)
        .unwrap_or_else(|| subdomains_used(&sites));
    let db_disk = usage.as_ref().map(|u| u.database_disk_bytes).unwrap_or(0);
    let bw_bytes = crate::package_bandwidth::account_month_bytes(username);
    let disk_used = disk_bytes_used(&sites);
    let blurb = if is_panel_admin(username) {
        "Totals for sites you own, against your assigned package. ∞ means unlimited (package limit -1). 0 means none allowed. Assign a package on Manage user to change these limits."
    } else {
        "Used versus your assigned package quotas. ∞ means unlimited (package limit -1). 0 means none allowed."
    };
    let pkg_name = usage
        .as_ref()
        .map(|u| u.package_name.as_str())
        .or_else(|| pkg.as_ref().map(|p| p.name.as_str()))
        .unwrap_or("Default");
    let pkg_id = usage
        .as_ref()
        .map(|u| u.package_id.as_str())
        .or_else(|| pkg.as_ref().map(|p| p.id.as_str()))
        .unwrap_or("pkg-default");
    let db_disk_html = format!(
        "Used {}",
        html_escape(&crate::panel_storage_fmt::format_bytes_for_user(
            username, db_disk
        ))
    );
    let rows = format!(
        "{}{}{}{}{}{}{}{}{}{}{}{}{}",
        row("Websites", &count_html(domains_used, domains_limit)),
        row("Mailboxes", &count_html(emails_used, emails_limit)),
        row("Databases", &count_html(db_used, db_limit)),
        row("Database disk", &db_disk_html),
        row("FTP accounts", &count_html(ftp_used, ftp_limit)),
        row("Storage", &bytes_html(username, disk_used, disk_limit)),
        row("Bandwidth", &bytes_html(username, bw_bytes, bw_limit)),
        row("Alias domains", &count_html(alias_used, alias_limit),),
        row("Sub-domains", &count_html(sub_used, sub_limit),),
        row("Mailing lists", &count_html(lists_used, lists_limit)),
        row("Autoresponders", &count_html(ar_used, ar_limit)),
        row("Forwarders", &count_html(fwd_used, fwd_limit)),
        row("Email filters", &count_html(filt_used, filt_limit)),
    );
    format!(
        r#"<article class="status-card cpn-stats-card">
  <div class="status-card-heading">
    <div>
      <p class="eyebrow">ACCOUNT</p>
      <h2>Statistics</h2>
      <p class="muted" style="margin:6px 0 0;">{blurb} Assigned package: {pkg} (<code>{pkg_id}</code>).</p>
    </div>
  </div>
  <ul class="cpn-stats-list">{rows}</ul>
  <p class="muted" style="margin-top:12px;">Databases is account count versus the package limit. Database disk is live MariaDB schema size for owned databases (informational; not a separate package count limit). Storage is website home disk usage.</p>
</article>"#,
        blurb = html_escape(blurb),
        pkg = html_escape(pkg_name),
        pkg_id = html_escape(pkg_id),
        rows = rows,
    )
}

pub fn dashboard_stats_styles() -> &'static str {
    r#"
.cpn-unlimited { font-weight:700; font-size:1.2em; line-height:1; }
.cpn-stats-card { min-width:0; }
.cpn-stats-list { list-style:none; margin:0; padding:0; }
.cpn-stats-list li {
  display:flex; justify-content:space-between; align-items:baseline; gap:16px;
  padding:12px 0; border-bottom:1px solid var(--hairline,#e0e0e0);
}
.cpn-stats-list li:last-child { border-bottom:0; }
.cpn-stats-label { color:var(--blue,#0066cc); font-weight:600; font-size:15px; }
.cpn-stats-value { color:var(--muted,#6e6e73); font-size:14px; text-align:right; white-space:nowrap; }
@media (max-width:519.98px) {
  .cpn-stats-list li { flex-direction:column; align-items:flex-start; gap:4px; }
  .cpn-stats-value { text-align:left; }
}
"#
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;
    use crate::packages::ensure_default_package;

    #[test]
    fn stats_card_uses_infinity_and_honest_zeros() {
        with_test_data_dir(|| {
            let _ = ensure_default_package();
            let html = dashboard_stats_html("nobody-yet");
            assert!(html.contains("Statistics"), "{html}");
            assert!(html.contains("Websites"), "{html}");
            assert!(html.contains("Mailboxes"), "{html}");
            assert!(html.contains("FTP accounts"), "{html}");
            assert!(html.contains("Storage"), "{html}");
            assert!(html.contains("Bandwidth"), "{html}");
            assert!(html.contains("Alias domains"), "{html}");
            assert!(html.contains("Sub-domains"), "{html}");
            assert!(html.contains("Mailing lists"), "{html}");
            assert!(html.contains("Autoresponders"), "{html}");
            assert!(html.contains("Forwarders"), "{html}");
            assert!(html.contains("Email filters"), "{html}");
            assert!(html.contains("Database disk"), "{html}");
            assert!(html.contains("Databases"), "{html}");
            assert!(html.contains("Assigned package"), "{html}");
            assert!(html.contains("pkg-default"), "{html}");
            assert!(html.contains("∞"), "{html}");
            assert!(html.contains("Used "), "{html}");
            assert!(!html.contains("not provisioned yet"), "{html}");
            assert!(!html.contains("> -1<"), "{html}");
            assert!(!html.contains("/ -1"), "{html}");
            assert!(!html.contains("of -1"), "{html}");
            assert!(!html.to_ascii_lowercase().contains("cpanel"));
            assert!(!html.to_ascii_lowercase().contains("cyberpanel"));
        });
    }
}
