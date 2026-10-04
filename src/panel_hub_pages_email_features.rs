//! Email hub pages: mailing lists, autoresponders, filters (LIVE provisioning).

use crate::apps_email::mail_backend_ready_fast;
use crate::install_snappymail_lineage::WEBMAIL_ADMIN_CHOICE;
use crate::panel_hubs::feature_shell;
use crate::panel_ops_email_csrf::email_csrf_token;
use crate::panel_ops_email_password::mailbox_choices;
use crate::panel_ops_mail_autorespond::list_autoresponders;
use crate::panel_ops_mail_extra::{load_forwards, mail_stack_note};
use crate::panel_ops_mail_filters::list_filters;
use crate::panel_ops_mail_lists::list_mailing_lists;
use crate::postfix_fallback::postfix_is_ready;

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn mail_down_banner() -> String {
    if mail_backend_ready_fast() || postfix_is_ready() {
        return String::new();
    }
    r#"<p class="badge-warn" style="padding:10px 12px;">Postfix/Dovecot is not ready. Rules can be saved in the panel; maps and Sieve apply when the mail stack is up.</p>"#.into()
}

pub fn email_forwarding_page_v2(user: &str, notice: Option<&str>, error: Option<&str>) -> String {
    let csrf = html_escape(&email_csrf_token(user));
    let rows = load_forwards();
    let mut list = String::new();
    if rows.is_empty() {
        list.push_str(r#"<p class="empty-state">No forwarders yet.</p>"#);
    } else {
        list.push_str("<table class=\"data-table\"><thead><tr><th>From</th><th>To</th><th></th></tr></thead><tbody>");
        for row in &rows {
            list.push_str(&format!(
                r#"<tr><td><code>{from}</code></td><td><code>{to}</code></td><td>
                <form method="post" action="/email/forwarding/delete" style="display:inline">
                  <input type="hidden" name="csrf" value="{csrf}">
                  <input type="hidden" name="from" value="{from}">
                  <button type="submit" class="btn-secondary">Delete</button>
                </form></td></tr>"#,
                from = html_escape(&row.from),
                to = html_escape(&row.to),
                csrf = csrf,
            ));
        }
        list.push_str("</tbody></table>");
    }
    let body = format!(
        r#"{banner}<p class="muted">{note}</p>
        {list}
        <form method="post" action="/email/forwarding/save" class="stack-form" style="max-width:520px;margin-top:16px;">
          <input type="hidden" name="csrf" value="{csrf}">
          <label for="from">From</label>
          <input id="from" name="from" type="email" required>
          <label for="to">To</label>
          <input id="to" name="to" type="email" required>
          <button type="submit" class="btn-primary">Add forwarder</button>
        </form>
        <form method="post" action="/email/forwarding/apply" style="margin-top:12px;">
          <input type="hidden" name="csrf" value="{csrf}">
          <button type="submit" class="btn-secondary">Re-apply Postfix maps</button>
        </form>"#,
        banner = mail_down_banner(),
        note = html_escape(&mail_stack_note()),
        list = list,
        csrf = csrf,
    );
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Email", Some("/email")),
            ("Forwarding", None),
        ],
        "Forwarding",
        "Forward to other addresses (Postfix virtual aliases).",
        &body,
        notice,
        error,
    )
}

pub fn autoresponders_page(user: &str, notice: Option<&str>, error: Option<&str>) -> String {
    let csrf = html_escape(&email_csrf_token(user));
    let items = list_autoresponders();
    let choices = mailbox_choices();
    let mut opts = String::from(r#"<option value="">Select mailbox</option>"#);
    for (id, addr) in &choices {
        if id.eq_ignore_ascii_case(WEBMAIL_ADMIN_CHOICE) {
            continue;
        }
        opts.push_str(&format!(
            r#"<option value="{a}">{a}</option>"#,
            a = html_escape(addr)
        ));
    }
    let mut list = String::new();
    if items.is_empty() {
        list.push_str(r#"<p class="empty-state">No autoresponders yet.</p>"#);
    } else {
        list.push_str("<table class=\"data-table\"><thead><tr><th>Mailbox</th><th>Subject</th><th>On</th><th></th></tr></thead><tbody>");
        for item in &items {
            list.push_str(&format!(
                r#"<tr><td><code>{addr}</code></td><td>{subj}</td><td>{on}</td><td>
                <form method="post" action="/email/autoresponders/delete" style="display:inline">
                  <input type="hidden" name="csrf" value="{csrf}">
                  <input type="hidden" name="id" value="{id}">
                  <button type="submit" class="btn-secondary">Delete</button>
                </form></td></tr>"#,
                addr = html_escape(&item.address),
                subj = html_escape(&item.subject),
                on = if item.enabled { "Yes" } else { "No" },
                id = html_escape(&item.id),
                csrf = csrf,
            ));
        }
        list.push_str("</tbody></table>");
    }
    let body = format!(
        r#"{banner}<p class="muted">Vacation auto-replies use Dovecot Sieve on the mailbox home (<code>~/sieve/cpn-vacation.sieve</code>).</p>
        {list}
        <form method="post" action="/email/autoresponders/save" class="stack-form" style="max-width:560px;margin-top:16px;">
          <input type="hidden" name="csrf" value="{csrf}">
          <label for="address">Mailbox</label>
          <select id="address" name="address" required>{opts}</select>
          <label for="subject">Subject</label>
          <input id="subject" name="subject" required maxlength="200" placeholder="Out of office">
          <label for="body">Message</label>
          <textarea id="body" name="body" rows="5" required maxlength="4000"></textarea>
          <label style="display:flex;align-items:center;gap:8px;">
            <input type="checkbox" name="enabled" value="1" checked>
            Enabled
          </label>
          <button type="submit" class="btn-primary">Save autoresponder</button>
        </form>"#,
        banner = mail_down_banner(),
        list = list,
        opts = opts,
        csrf = csrf,
    );
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Email", Some("/email")),
            ("Autoresponders", None),
        ],
        "Autoresponders",
        "Vacation and auto-reply for mailboxes.",
        &body,
        notice,
        error,
    )
}

pub fn email_filters_page(user: &str, notice: Option<&str>, error: Option<&str>) -> String {
    let csrf = html_escape(&email_csrf_token(user));
    let rules = list_filters();
    let choices = mailbox_choices();
    let mut opts = String::from(r#"<option value="">Select mailbox</option>"#);
    for (id, addr) in &choices {
        if id.eq_ignore_ascii_case(WEBMAIL_ADMIN_CHOICE) {
            continue;
        }
        opts.push_str(&format!(
            r#"<option value="{a}">{a}</option>"#,
            a = html_escape(addr)
        ));
    }
    let mut list = String::new();
    if rules.is_empty() {
        list.push_str(r#"<p class="empty-state">No email filters yet.</p>"#);
    } else {
        list.push_str("<table class=\"data-table\"><thead><tr><th>Mailbox</th><th>Match</th><th>Action</th><th></th></tr></thead><tbody>");
        for rule in &rules {
            list.push_str(&format!(
                r#"<tr><td><code>{addr}</code></td><td>{field} contains <code>{val}</code></td><td>{action} {arg}</td><td>
                <form method="post" action="/email/filters/delete" style="display:inline">
                  <input type="hidden" name="csrf" value="{csrf}">
                  <input type="hidden" name="id" value="{id}">
                  <button type="submit" class="btn-secondary">Delete</button>
                </form></td></tr>"#,
                addr = html_escape(&rule.address),
                field = html_escape(&rule.match_field),
                val = html_escape(&rule.match_value),
                action = html_escape(&rule.action),
                arg = html_escape(&rule.action_arg),
                id = html_escape(&rule.id),
                csrf = csrf,
            ));
        }
        list.push_str("</tbody></table>");
    }
    let body = format!(
        r#"{banner}<p class="muted">Filters compile to Dovecot Sieve (<code>~/sieve/cpn-filters.sieve</code>). ManageSieve on port 4190 remains available for webmail clients.</p>
        {list}
        <form method="post" action="/email/filters/save" class="stack-form" style="max-width:560px;margin-top:16px;">
          <input type="hidden" name="csrf" value="{csrf}">
          <label for="address">Mailbox</label>
          <select id="address" name="address" required>{opts}</select>
          <label for="match_field">Match field</label>
          <select id="match_field" name="match_field">
            <option value="subject_contains">Subject contains</option>
            <option value="from_contains">From contains</option>
            <option value="to_contains">To contains</option>
          </select>
          <label for="match_value">Match value</label>
          <input id="match_value" name="match_value" required maxlength="200">
          <label for="action">Action</label>
          <select id="action" name="action">
            <option value="discard">Discard</option>
            <option value="fileinto">File into folder</option>
            <option value="redirect">Redirect</option>
          </select>
          <label for="action_arg">Action argument (folder or redirect address)</label>
          <input id="action_arg" name="action_arg" placeholder="Junk or other@example.com">
          <button type="submit" class="btn-primary">Add filter</button>
        </form>"#,
        banner = mail_down_banner(),
        list = list,
        opts = opts,
        csrf = csrf,
    );
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Email", Some("/email")),
            ("Email Filters", None),
        ],
        "Email Filters",
        "Sieve filter rules for mailboxes.",
        &body,
        notice,
        error,
    )
}

pub fn mailing_lists_page(user: &str, notice: Option<&str>, error: Option<&str>) -> String {
    let csrf = html_escape(&email_csrf_token(user));
    let lists = list_mailing_lists();
    let mut list_html = String::new();
    if lists.is_empty() {
        list_html.push_str(r#"<p class="empty-state">No mailing lists yet.</p>"#);
    } else {
        for ml in &lists {
            let mut members = String::new();
            if ml.members.is_empty() {
                members.push_str(r#"<p class="muted">No members yet.</p>"#);
            } else {
                members.push_str("<ul>");
                for m in &ml.members {
                    members.push_str(&format!(
                        r#"<li><code>{m}</code>
                        <form method="post" action="/email/lists/member/delete" style="display:inline">
                          <input type="hidden" name="csrf" value="{csrf}">
                          <input type="hidden" name="list_id" value="{id}">
                          <input type="hidden" name="member" value="{m}">
                          <button type="submit" class="btn-secondary">Remove</button>
                        </form></li>"#,
                        m = html_escape(m),
                        id = html_escape(&ml.id),
                        csrf = csrf,
                    ));
                }
                members.push_str("</ul>");
            }
            list_html.push_str(&format!(
                r#"<article class="panel-card" style="margin-top:14px;">
                  <h3 style="margin:0 0 6px;">{name} <span class="muted" style="font-weight:500;"><code>{addr}</code></span></h3>
                  <p class="muted" style="margin:0 0 8px;">{count} member(s)</p>
                  {members}
                  <form method="post" action="/email/lists/member/add" class="stack-form" style="max-width:420px;margin-top:10px;">
                    <input type="hidden" name="csrf" value="{csrf}">
                    <input type="hidden" name="list_id" value="{id}">
                    <label>Add member
                      <input name="member" type="email" required>
                    </label>
                    <button type="submit" class="btn-primary">Add member</button>
                  </form>
                  <form method="post" action="/email/lists/delete" style="margin-top:10px;" onsubmit="return confirm('Delete this mailing list?');">
                    <input type="hidden" name="csrf" value="{csrf}">
                    <input type="hidden" name="list_id" value="{id}">
                    <button type="submit" class="btn-secondary">Delete list</button>
                  </form>
                </article>"#,
                name = html_escape(&ml.name),
                addr = html_escape(&ml.address),
                count = ml.members.len(),
                members = members,
                id = html_escape(&ml.id),
                csrf = csrf,
            ));
        }
    }
    let body = format!(
        r#"{banner}<p class="muted">CPN mailing lists are distribution addresses: mail to <code>list@domain</code> expands to all members through Postfix virtual aliases (not a full Mailman stack).</p>
        {list_html}
        <form method="post" action="/email/lists/create" class="stack-form" style="max-width:520px;margin-top:18px;">
          <input type="hidden" name="csrf" value="{csrf}">
          <label for="name">List name</label>
          <input id="name" name="name" required maxlength="120" placeholder="Team news">
          <label for="address">List address</label>
          <input id="address" name="address" type="email" required placeholder="news@example.com">
          <button type="submit" class="btn-primary">Create mailing list</button>
        </form>
        <form method="post" action="/email/lists/apply" style="margin-top:12px;">
          <input type="hidden" name="csrf" value="{csrf}">
          <button type="submit" class="btn-secondary">Re-apply Postfix maps</button>
        </form>"#,
        banner = mail_down_banner(),
        list_html = list_html,
        csrf = csrf,
    );
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Email", Some("/email")),
            ("Mailing Lists", None),
        ],
        "Mailing Lists",
        "Distribution lists for hosted domains.",
        &body,
        notice,
        error,
    )
}
