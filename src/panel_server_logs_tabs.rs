//! Tab strip shared by the Server > Logs overview and every log viewer.
//!
//! A compact primary strip (Overview and the everyday logs) plus a **More** menu that holds the
//! remaining detailed viewers, so the pills never wrap onto a second line.

use crate::panel_server_logs_kind::HostLogKind;

/// Viewers shown directly in the strip (after Overview).
pub const PRIMARY: [HostLogKind; 3] = [HostLogKind::Panel, HostLogKind::Access, HostLogKind::Error];
/// Viewers folded into the More menu.
pub const MORE: [HostLogKind; 3] = [HostLogKind::Email, HostLogKind::Ftp, HostLogKind::ModSec];

pub fn tabs_styles() -> &'static str {
    r#"<style>
.log-tabs{display:flex;flex-wrap:wrap;align-items:center;gap:8px;margin:0 0 14px;max-width:100%;}
.log-tabs>a,.log-more>summary{display:inline-flex;align-items:center;gap:6px;min-height:34px;padding:0 14px;
  border-radius:999px;border:1px solid var(--hairline);font-weight:700;font-size:13px;text-decoration:none;
  color:var(--ink);background:var(--canvas);cursor:pointer;white-space:nowrap;list-style:none;}
.log-more>summary::-webkit-details-marker{display:none;}
.log-tabs>a.active,.log-more.has-active>summary{background:#1d4ed8;border-color:#1d4ed8;color:#fff;}
.log-more{position:relative;}
.log-more>summary svg{transition:transform .15s ease;}
.log-more[open]>summary svg{transform:rotate(180deg);}
.log-more-menu{position:absolute;z-index:30;top:calc(100% + 6px);right:0;min-width:190px;max-width:calc(100vw - 32px);
  display:flex;flex-direction:column;gap:2px;padding:6px;border:1px solid var(--hairline);border-radius:12px;
  background:var(--canvas);box-shadow:0 10px 28px rgba(15,23,42,.18);}
.log-more-menu a{display:block;padding:8px 12px;border-radius:8px;font-size:13px;font-weight:600;
  text-decoration:none;color:var(--ink);}
.log-more-menu a:hover,.log-more-menu a:focus-visible{background:rgba(29,78,216,.1);outline:none;}
.log-more-menu a.active{background:#1d4ed8;color:#fff;}
@media (max-width:480px){
  .log-tabs{gap:6px;}
  .log-tabs>a,.log-more>summary{padding:0 10px;font-size:12px;min-height:32px;}
}
[data-color-mode="dark"] .log-tabs>a,[data-color-mode="dark"] .log-more>summary{background:#1a1d26;}
[data-color-mode="dark"] .log-tabs>a.active,[data-color-mode="dark"] .log-more.has-active>summary{background:#1d4ed8;}
[data-color-mode="dark"] .log-more-menu{background:#1a1d26;box-shadow:0 10px 28px rgba(0,0,0,.55);}
[data-color-mode="dark"] .log-more-menu a:hover,[data-color-mode="dark"] .log-more-menu a:focus-visible{background:rgba(96,165,250,.18);}
</style>"#
}

fn tabs_script() -> &'static str {
    r#"<script>(function(){var m=document.querySelector('details.log-more');if(!m)return;
document.addEventListener('click',function(e){if(m.open&&!m.contains(e.target)){m.open=false;}});
document.addEventListener('keydown',function(e){if(e.key==='Escape'&&m.open){m.open=false;var s=m.querySelector('summary');if(s)s.focus();}});})();</script>"#
}

fn chevron() -> &'static str {
    r#"<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" aria-hidden="true"><path d="m6 9 6 6 6-6"/></svg>"#
}

fn link(href: &str, title: &str, active: bool) -> String {
    let class = if active {
        r#" class="active" aria-current="page""#
    } else {
        ""
    };
    format!(r#"<a{class} href="{href}">{title}</a>"#)
}

/// `active` is `None` on the Overview page.
pub fn log_tabs(active: Option<HostLogKind>) -> String {
    let mut out = String::from(tabs_styles());
    out.push_str(r#"<nav class="log-tabs" aria-label="Log sources">"#);
    out.push_str(&link("/server/logs", "Overview", active.is_none()));
    for kind in PRIMARY {
        out.push_str(&link(kind.href(), kind.title(), active == Some(kind)));
    }
    let more_active = active.is_some_and(|k| MORE.contains(&k));
    let more_class = if more_active {
        "log-more has-active"
    } else {
        "log-more"
    };
    let items: String = MORE
        .iter()
        .map(|k| link(k.href(), k.title(), active == Some(*k)))
        .collect();
    out.push_str(&format!(
        r#"<details class="{more_class}"><summary aria-haspopup="true">More{chev}</summary><div class="log-more-menu">{items}</div></details>"#,
        chev = chevron(),
    ));
    out.push_str("</nav>");
    out.push_str(tabs_script());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_log_is_reachable_exactly_once() {
        let html = log_tabs(Some(HostLogKind::Ftp));
        for kind in HostLogKind::ALL {
            assert_eq!(
                html.matches(&format!(r#"href="/server/logs/{}""#, kind.slug()))
                    .count(),
                1,
                "{}",
                kind.slug()
            );
        }
        assert!(html.contains(r#"href="/server/logs""#));
        assert_eq!(PRIMARY.len() + MORE.len(), HostLogKind::ALL.len());
    }

    #[test]
    fn primary_strip_is_compact_and_detail_tabs_live_in_more() {
        let html = log_tabs(None);
        let nav = html.split("log-more-menu").next().unwrap();
        assert!(nav.contains("Main Log"));
        assert!(nav.contains("Access Logs"));
        assert!(nav.contains("Error Logs"));
        assert!(!nav.contains("ModSec Audit"));
        let menu = html.split("log-more-menu").nth(1).unwrap();
        assert!(menu.contains("Email Log"));
        assert!(menu.contains("FTP Logs"));
        assert!(menu.contains("ModSec Audit"));
    }

    #[test]
    fn active_tab_is_marked_once_and_more_highlights_for_hidden_tabs() {
        let overview = log_tabs(None);
        assert_eq!(overview.matches("aria-current=\"page\"").count(), 1);
        assert!(!overview.contains("log-more has-active"));
        let modsec = log_tabs(Some(HostLogKind::ModSec));
        assert_eq!(modsec.matches("aria-current=\"page\"").count(), 1);
        assert!(modsec.contains("log-more has-active"));
        let access = log_tabs(Some(HostLogKind::Access));
        assert!(!access.contains("log-more has-active"));
    }
}
