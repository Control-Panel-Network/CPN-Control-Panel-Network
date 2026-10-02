//! Owner-only System Repair hub UI (stacked cards, pass/warn/fail + Heal).

use crate::panel_hubs::feature_shell;
use crate::system_repair::{CheckStatus, PRODUCT_NAME, RepairCheck, RepairReport, run_suite};

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn status_badge(status: CheckStatus) -> String {
    let (class, label) = match status {
        CheckStatus::Pass => ("sr-badge sr-badge-pass", "Pass"),
        CheckStatus::Warn => ("sr-badge sr-badge-warn", "Warn"),
        CheckStatus::Fail => ("sr-badge sr-badge-fail", "Fail"),
    };
    format!(r#"<span class="{class}">{label}</span>"#)
}

fn styles() -> &'static str {
    r#"<style>
.sr-summary{display:flex;flex-wrap:wrap;gap:10px;margin:0 0 16px;}
.sr-pill{display:inline-flex;align-items:center;gap:6px;min-height:36px;padding:0 12px;border-radius:10px;font-weight:700;font-size:13px;background:#e2e8f0;color:#0f172a;}
.sr-pill-pass{background:#dcfce7;color:#14532d;}
.sr-pill-warn{background:#ffedd5;color:#9a3412;}
.sr-pill-fail{background:#fee2e2;color:#991b1b;}
[data-color-mode="dark"] .sr-pill{background:#334155;color:#f8fafc;}
[data-color-mode="dark"] .sr-pill-pass{background:rgba(22,163,74,.28);color:#bbf7d0;}
[data-color-mode="dark"] .sr-pill-warn{background:rgba(234,88,12,.28);color:#fed7aa;}
[data-color-mode="dark"] .sr-pill-fail{background:rgba(220,38,38,.28);color:#fecaca;}
.sr-actions{display:flex;flex-wrap:wrap;gap:8px;margin:0 0 18px;}
.sr-actions form{display:inline;}
.sr-grid{display:flex;flex-direction:column;gap:12px;}
.sr-card{border:1px solid #e2e8f0;border-radius:12px;padding:14px 16px;background:#fff;}
[data-color-mode="dark"] .sr-card{border-color:#334155;background:#1e293b;}
.sr-card-head{display:flex;flex-wrap:wrap;gap:8px;align-items:center;justify-content:space-between;margin:0 0 8px;}
.sr-card-title{margin:0;font-size:16px;font-weight:700;}
.sr-meta{margin:0;font-size:12px;color:#64748b;}
[data-color-mode="dark"] .sr-meta{color:#94a3b8;}
.sr-detail{margin:6px 0 0;font-size:14px;line-height:1.45;word-break:break-word;}
.sr-badge{display:inline-flex;align-items:center;min-height:24px;padding:0 8px;border-radius:999px;font-size:11px;font-weight:700;letter-spacing:.04em;text-transform:uppercase;}
.sr-badge-pass{background:#dcfce7;color:#14532d;}
.sr-badge-warn{background:#ffedd5;color:#9a3412;}
.sr-badge-fail{background:#fee2e2;color:#991b1b;}
[data-color-mode="dark"] .sr-badge-pass{background:rgba(22,163,74,.28);color:#bbf7d0;}
[data-color-mode="dark"] .sr-badge-warn{background:rgba(234,88,12,.28);color:#fed7aa;}
[data-color-mode="dark"] .sr-badge-fail{background:rgba(220,38,38,.28);color:#fecaca;}
.sr-heal{margin-top:10px;}
@media (max-width:720px){
  .sr-card-head{align-items:flex-start;}
  .sr-actions .btn-primary,.sr-actions .btn-secondary,.sr-heal .btn-secondary{width:100%;justify-content:center;}
}
</style>"#
}

fn card(check: &RepairCheck) -> String {
    let heal_btn = match &check.heal_id {
        Some(hid) if check.status != CheckStatus::Pass => format!(
            r#"<form method="post" action="/server/system-repair/heal" class="sr-heal">
  <input type="hidden" name="id" value="{hid}">
  <button type="submit" class="btn-secondary">Heal</button>
</form>"#,
            hid = html_escape(hid),
        ),
        _ => String::new(),
    };
    format!(
        r#"<article class="sr-card" data-status="{status}">
  <div class="sr-card-head">
    <h3 class="sr-card-title">{title}</h3>
    {badge}
  </div>
  <p class="sr-meta"><code>{id}</code> · {category}</p>
  <p class="sr-detail">{detail}</p>
  {heal}
</article>"#,
        status = check.status.as_str(),
        title = html_escape(&check.title),
        badge = status_badge(check.status),
        id = html_escape(&check.id),
        category = html_escape(&check.category),
        detail = html_escape(&check.detail),
        heal = heal_btn,
    )
}

fn heals_block(report: &RepairReport) -> String {
    if report.heals.is_empty() {
        return String::new();
    }
    let mut out = String::from(r#"<div class="sr-grid" style="margin-bottom:16px;">"#);
    for h in &report.heals {
        let badge = if h.ok {
            status_badge(CheckStatus::Pass)
        } else {
            status_badge(CheckStatus::Fail)
        };
        out.push_str(&format!(
            r#"<article class="sr-card"><div class="sr-card-head"><h3 class="sr-card-title">Heal: {id}</h3>{badge}</div><p class="sr-detail">{msg}</p></article>"#,
            id = html_escape(&h.heal_id),
            badge = badge,
            msg = html_escape(&h.message),
        ));
    }
    out.push_str("</div>");
    out
}

/// Full System Repair page body (owner-only gate is in the route).
pub fn system_repair_page(notice: Option<&str>, error: Option<&str>) -> String {
    let report = run_suite(false, None, None);
    let mut body = String::new();
    body.push_str(styles());
    body.push_str(&format!(
        r#"<div class="sr-summary">
  <span class="sr-pill sr-pill-pass">Pass {pass}</span>
  <span class="sr-pill sr-pill-warn">Warn {warn}</span>
  <span class="sr-pill sr-pill-fail">Fail {fail}</span>
</div>
<p class="muted">Owner diagnostics for install, upgrade, update, and downgrade recovery. Safe heals never wipe MFA. CLI: <code>cpn doctor</code>, <code>cpn troubleshoot</code>, or <code>cpn repair</code>.</p>
<div class="sr-actions">
  <a class="btn-secondary" href="/server/system-repair">Refresh checks</a>
  <form method="post" action="/server/system-repair/heal">
    <button type="submit" class="btn-primary">Heal all safe issues</button>
  </form>
  <a class="btn-secondary" href="/server/system-repair/api">JSON report</a>
</div>"#,
        pass = report.pass,
        warn = report.warn,
        fail = report.fail,
    ));
    body.push_str(&heals_block(&report));
    body.push_str(r#"<div class="sr-grid">"#);
    for check in &report.checks {
        body.push_str(&card(check));
    }
    body.push_str("</div>");
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Server", Some("/server")),
            (PRODUCT_NAME, None),
        ],
        PRODUCT_NAME,
        "Diagnose and safely heal panel, email, PHP, phpMyAdmin, and host services.",
        &body,
        notice,
        error,
    )
}
