//! Reseller Center top stats cards (Total Users / Total Websites / Resellers).

use crate::account_mgmt::list_accounts;
use crate::packages::is_panel_admin;
use crate::panel_reseller::{account_is_reseller, child_usernames, list_resellers};
use crate::sites::list_sites;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResellerCenterStats {
    pub total_users: u64,
    pub total_websites: u64,
    pub resellers: u64,
}

/// Counts for the signed-in scope: owner/admin see global totals; resellers see self + children.
pub fn reseller_center_stats(viewer: &str) -> ResellerCenterStats {
    let viewer = viewer.trim();
    if is_panel_admin(viewer) {
        let total_users = list_accounts().unwrap_or_default().len() as u64;
        let total_websites = list_sites().unwrap_or_default().len() as u64;
        let resellers = list_resellers().unwrap_or_default().len() as u64;
        return ResellerCenterStats {
            total_users,
            total_websites,
            resellers,
        };
    }

    let mut users = vec![viewer.to_string()];
    if let Ok(children) = child_usernames(viewer) {
        for child in children {
            if !users.iter().any(|u| u.eq_ignore_ascii_case(&child)) {
                users.push(child);
            }
        }
    }
    let total_users = users.len() as u64;
    let total_websites = list_sites()
        .unwrap_or_default()
        .into_iter()
        .filter(|site| {
            users
                .iter()
                .any(|u| site.owner.trim().eq_ignore_ascii_case(u))
        })
        .count() as u64;
    let resellers = if account_is_reseller(viewer) { 1 } else { 0 };
    ResellerCenterStats {
        total_users,
        total_websites,
        resellers,
    }
}

fn svg_users() -> &'static str {
    r#"<svg viewBox="0 0 24 24" width="22" height="22" aria-hidden="true" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2"/><circle cx="9" cy="7" r="4"/><path d="M22 21v-2a4 4 0 0 0-3-3.87"/><path d="M16 3.13a4 4 0 0 1 0 7.75"/></svg>"#
}

fn svg_globe() -> &'static str {
    r#"<svg viewBox="0 0 24 24" width="22" height="22" aria-hidden="true" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><path d="M2 12h20"/><path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z"/></svg>"#
}

fn svg_person() -> &'static str {
    r#"<svg viewBox="0 0 24 24" width="22" height="22" aria-hidden="true" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2"/><circle cx="12" cy="7" r="4"/></svg>"#
}

/// Horizontal row of three dark rounded stats cards for Reseller Center.
pub fn reseller_stats_cards_html(viewer: &str) -> String {
    let stats = reseller_center_stats(viewer);
    format!(
        r#"<style>
.reseller-stats-row {{ display:grid; grid-template-columns:repeat(3,minmax(0,1fr)); gap:14px; margin:0 0 18px; }}
.reseller-stat-card {{ display:flex; align-items:center; gap:14px; padding:16px 18px; border-radius:14px; background:rgba(255,255,255,.04); border:1px solid rgba(255,255,255,.08); box-shadow:0 10px 28px rgba(0,0,0,.18); }}
[data-color-mode="light"] .reseller-stat-card {{ background:#fff; border-color:#e4e7ec; box-shadow:0 8px 22px rgba(16,24,40,.06); }}
.reseller-stat-icon {{ flex:0 0 auto; width:48px; height:48px; border-radius:12px; display:grid; place-items:center; color:#fff; }}
.reseller-stat-icon.users {{ background:#7c3aed; }}
.reseller-stat-icon.websites {{ background:#16a34a; }}
.reseller-stat-icon.resellers {{ background:#ea580c; }}
.reseller-stat-meta {{ min-width:0; }}
.reseller-stat-value {{ display:block; font-size:1.85rem; font-weight:750; line-height:1.1; letter-spacing:-.02em; color:inherit; }}
.reseller-stat-label {{ display:block; margin-top:4px; font-size:.92rem; color:var(--muted,#98a2b3); }}
@media (max-width:820px) {{ .reseller-stats-row {{ grid-template-columns:1fr; }} }}
</style>
<div class="reseller-stats-row" role="group" aria-label="Reseller Center summary">
  <article class="reseller-stat-card">
    <span class="reseller-stat-icon users">{users_svg}</span>
    <span class="reseller-stat-meta">
      <span class="reseller-stat-value">{users}</span>
      <span class="reseller-stat-label">Total Users</span>
    </span>
  </article>
  <article class="reseller-stat-card">
    <span class="reseller-stat-icon websites">{globe_svg}</span>
    <span class="reseller-stat-meta">
      <span class="reseller-stat-value">{websites}</span>
      <span class="reseller-stat-label">Total Websites</span>
    </span>
  </article>
  <article class="reseller-stat-card">
    <span class="reseller-stat-icon resellers">{person_svg}</span>
    <span class="reseller-stat-meta">
      <span class="reseller-stat-value">{resellers}</span>
      <span class="reseller-stat-label">Resellers</span>
    </span>
  </article>
</div>"#,
        users_svg = svg_users(),
        globe_svg = svg_globe(),
        person_svg = svg_person(),
        users = stats.total_users,
        websites = stats.total_websites,
        resellers = stats.resellers,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stats_cards_markup_has_three_labels() {
        let html = reseller_stats_cards_html("cpnowner");
        assert!(html.contains("Total Users"));
        assert!(html.contains("Total Websites"));
        assert!(html.contains("Resellers"));
        assert!(html.contains("reseller-stats-row"));
        assert!(!html.to_lowercase().contains("cyberpanel"));
    }
}
