//! Email deliverability pages: debugger, queue, antispam, marketing.

use crate::panel_hubs::{feature_shell, status_kv};
use crate::panel_ops_email_antispam::{
    FilterStatus, mailscanner_status, rspamd_status, spamassassin_status,
};
use crate::panel_ops_email_csrf::email_csrf_token;
use crate::panel_ops_email_debug::{DebugReport, run_debug};
use crate::panel_ops_email_marketing::{list_campaigns, list_marketing_lists};
use crate::panel_ops_email_queue::{list_queue, queue_available};

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn pre_block(title: &str, text: &str) -> String {
    format!(
        r#"<h3 style="margin-top:16px;">{title}</h3><pre class="log-block" style="white-space:pre-wrap;max-height:280px;overflow:auto;">{body}</pre>"#,
        title = html_escape(title),
        body = html_escape(text),
    )
}

pub fn email_debugger_page(
    user: &str,
    domain: Option<&str>,
    notice: Option<&str>,
    error: Option<&str>,
) -> String {
    let _ = user;
    let domain_val = html_escape(domain.unwrap_or(""));
    let mut report_html = String::new();
    if let Some(d) = domain.filter(|s| !s.trim().is_empty()) {
        match run_debug(d) {
            Ok(r) => report_html.push_str(&render_debug_report(&r)),
            Err(e) => report_html.push_str(&format!(r#"<p class="error">{}</p>"#, html_escape(&e))),
        }
    }
    let body = format!(
        r#"<p class="muted">Diagnose deliverability: DNS MX/SPF/DKIM/DMARC, local SMTP banner, and recent mail log lines for the domain.</p>
        <form method="get" action="/email/debugger" class="stack-form" style="max-width:520px;">
          <label for="domain">Domain</label>
          <input id="domain" name="domain" value="{domain_val}" required placeholder="example.com">
          <button type="submit" class="btn-primary">Run diagnosis</button>
        </form>
        {report}"#,
        domain_val = domain_val,
        report = report_html,
    );
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Email", Some("/email")),
            ("Email Debugger", None),
        ],
        "Email Debugger",
        "Diagnose mail issues.",
        &body,
        notice,
        error,
    )
}

fn render_debug_report(r: &DebugReport) -> String {
    let mut out = status_kv(&[("Domain", &r.domain), ("Local SMTP", &r.smtp_local)]);
    out.push_str(&pre_block("MX", &r.mx));
    out.push_str(&pre_block("SPF / TXT", &r.spf));
    out.push_str(&pre_block("DKIM (default._domainkey)", &r.dkim));
    out.push_str(&pre_block("DMARC", &r.dmarc));
    out.push_str(&pre_block("Recent logs", &r.logs));
    out
}

pub fn email_queue_page(user: &str, notice: Option<&str>, error: Option<&str>) -> String {
    let csrf = html_escape(&email_csrf_token(user));
    let available = queue_available();
    let queue_text = if available {
        list_queue().unwrap_or_else(|e| e)
    } else {
        "postqueue/postsuper not found on this host.".into()
    };
    let body = format!(
        r#"<p class="muted">Inspect and manage the Postfix queue with allowlisted <code>postqueue</code> / <code>postsuper</code> only.</p>
        {pre}
        <div style="display:flex;flex-wrap:wrap;gap:8px;margin-top:12px;">
          <form method="post" action="/email/queue/flush">
            <input type="hidden" name="csrf" value="{csrf}">
            <button type="submit" class="btn-primary" {dis}>Flush queue</button>
          </form>
          <form method="post" action="/email/queue/delete-all" onsubmit="return confirm('Delete ALL queued messages?');">
            <input type="hidden" name="csrf" value="{csrf}">
            <button type="submit" class="btn-secondary" {dis}>Delete all</button>
          </form>
        </div>
        <form method="post" action="/email/queue/delete" class="stack-form" style="max-width:420px;margin-top:16px;">
          <input type="hidden" name="csrf" value="{csrf}">
          <label for="qid">Delete by queue id</label>
          <input id="qid" name="qid" pattern="[A-Za-z0-9.\-]+" maxlength="32" {dis}>
          <button type="submit" class="btn-secondary" {dis}>Delete id</button>
        </form>"#,
        pre = pre_block("Queue", &queue_text),
        csrf = csrf,
        dis = if available { "" } else { "disabled" },
    );
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Email", Some("/email")),
            ("Mail Queue", None),
        ],
        "Mail Queue",
        "Inspect the queue.",
        &body,
        notice,
        error,
    )
}

fn filter_page(
    title: &str,
    subtitle: &str,
    user: &str,
    status: &FilterStatus,
    enable_action: &str,
    notice: Option<&str>,
    error: Option<&str>,
) -> String {
    let csrf = html_escape(&email_csrf_token(user));
    let state = if !status.installed {
        "Unavailable"
    } else if status.active {
        "Active"
    } else {
        "Installed (inactive)"
    };
    let mut kv = vec![
        ("Status", state),
        ("Detail", status.detail.as_str()),
        ("Package", status.package_hint),
    ];
    let web = status.web_ui.clone().unwrap_or_default();
    if !web.is_empty() {
        kv.push(("Web UI", web.as_str()));
    }
    let mut body = status_kv(&kv);
    if let Some(url) = &status.web_ui {
        body.push_str(&format!(
            r#"<p style="margin-top:12px;"><a href="{}" target="_blank" rel="noopener noreferrer">Open web UI</a> (localhost; tunnel or proxy if remote).</p>"#,
            html_escape(url)
        ));
    }
    body.push_str(&format!(
        r#"<form method="post" action="{action}" style="margin-top:16px;">
          <input type="hidden" name="csrf" value="{csrf}">
          <button type="submit" class="btn-primary">Install / Enable</button>
        </form>"#,
        action = html_escape(enable_action),
        csrf = csrf,
    ));
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Email", Some("/email")),
            (title, None),
        ],
        title,
        subtitle,
        &body,
        notice,
        error,
    )
}

pub fn spamassassin_page(user: &str, notice: Option<&str>, error: Option<&str>) -> String {
    let st = spamassassin_status();
    filter_page(
        "SpamAssassin",
        "Spam filtering.",
        user,
        &st,
        "/email/spamassassin/enable",
        notice,
        error,
    )
}

pub fn rspamd_page(user: &str, notice: Option<&str>, error: Option<&str>) -> String {
    let st = rspamd_status();
    filter_page(
        "Rspamd",
        "Spam filtering.",
        user,
        &st,
        "/email/rspamd/enable",
        notice,
        error,
    )
}

pub fn mailscanner_page(user: &str, notice: Option<&str>, error: Option<&str>) -> String {
    let st = mailscanner_status();
    filter_page(
        "MailScanner",
        "Mail scanning.",
        user,
        &st,
        "/email/mailscanner/enable",
        notice,
        error,
    )
}

pub fn email_marketing_page(user: &str, notice: Option<&str>, error: Option<&str>) -> String {
    let csrf = html_escape(&email_csrf_token(user));
    let lists = list_marketing_lists();
    let campaigns = list_campaigns();
    let mut list_html = String::from("<ul>");
    if lists.is_empty() {
        list_html = r#"<p class="empty-state">No lists yet.</p>"#.into();
    } else {
        for l in &lists {
            list_html.push_str(&format!(
                "<li><code>{}</code> {} ({} recipients)</li>",
                html_escape(&l.id),
                html_escape(&l.name),
                l.recipients.len()
            ));
        }
        list_html.push_str("</ul>");
    }
    let mut list_opts = String::from(r#"<option value="">Select list</option>"#);
    for l in &lists {
        list_opts.push_str(&format!(
            r#"<option value="{}">{} ({})</option>"#,
            html_escape(&l.id),
            html_escape(&l.name),
            l.recipients.len()
        ));
    }
    let mut camp = String::from("<ul>");
    if campaigns.is_empty() {
        camp = r#"<p class="muted">No campaigns sent yet.</p>"#.into();
    } else {
        for c in campaigns.iter().take(10) {
            camp.push_str(&format!(
                "<li><strong>{}</strong> sent {}: {}</li>",
                html_escape(&c.subject),
                c.sent_count,
                html_escape(c.last_error.as_deref().unwrap_or("ok"))
            ));
        }
        camp.push_str("</ul>");
    }
    let body = format!(
        r#"<p class="muted">MVP campaigns: create a list, add recipients, send via local MTA/SMTP with rate limiting.</p>
        <h3>Lists</h3>
        {list_html}
        <form method="post" action="/email/marketing/list" class="stack-form" style="max-width:480px;">
          <input type="hidden" name="csrf" value="{csrf}">
          <label for="name">New list name</label>
          <input id="name" name="name" required maxlength="120">
          <button type="submit" class="btn-primary">Create list</button>
        </form>
        <form method="post" action="/email/marketing/recipient" class="stack-form" style="max-width:480px;margin-top:16px;">
          <input type="hidden" name="csrf" value="{csrf}">
          <label for="list_id_r">List</label>
          <select id="list_id_r" name="list_id" required>{list_opts}</select>
          <label for="email">Recipient</label>
          <input id="email" name="email" type="email" required>
          <button type="submit" class="btn-secondary">Add recipient</button>
        </form>
        <h3 style="margin-top:24px;">Send campaign</h3>
        <form method="post" action="/email/marketing/send" class="stack-form" style="max-width:560px;">
          <input type="hidden" name="csrf" value="{csrf}">
          <label for="list_id_s">List</label>
          <select id="list_id_s" name="list_id" required>{list_opts}</select>
          <label for="from_address">From</label>
          <input id="from_address" name="from_address" type="email" required>
          <label for="subject">Subject</label>
          <input id="subject" name="subject" required maxlength="200">
          <label for="body">Body</label>
          <textarea id="body" name="body" rows="6" required></textarea>
          <button type="submit" class="btn-primary">Send</button>
        </form>
        <h3 style="margin-top:24px;">Recent campaigns</h3>
        {camp}"#,
        list_html = list_html,
        csrf = csrf,
        list_opts = list_opts,
        camp = camp,
    );
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Email", Some("/email")),
            ("Email Marketing", None),
        ],
        "Email Marketing",
        "Campaigns and lists.",
        &body,
        notice,
        error,
    )
}
