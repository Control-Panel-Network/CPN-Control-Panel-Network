//! Email hub pages: pattern forwarding, limits, password, plus-addressing.

use crate::panel_hubs::feature_shell;
use crate::panel_ops_email_csrf::email_csrf_token;
use crate::panel_ops_email_limits::load_send_limits;
use crate::panel_ops_email_password::mailbox_choices;
use crate::panel_ops_email_pattern::load_pattern_rules;
use crate::panel_ops_email_plus::{current_postfix_delimiter, load_plus_settings};
use crate::postfix_fallback::postfix_is_ready;

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn pattern_forwarding_page(user: &str, notice: Option<&str>, error: Option<&str>) -> String {
    let csrf = html_escape(&email_csrf_token(user));
    let rules = load_pattern_rules();
    let mut list = String::new();
    if rules.is_empty() {
        list.push_str(r#"<p class="empty-state">No pattern rules yet.</p>"#);
    } else {
        list.push_str("<table class=\"data-table\"><thead><tr><th>Kind</th><th>Pattern</th><th>Destination</th><th></th></tr></thead><tbody>");
        for r in &rules {
            list.push_str(&format!(
                r#"<tr><td>{kind}</td><td><code>{pat}</code></td><td><code>{dest}</code></td><td>
                <form method="post" action="/email/pattern-forwarding/delete" style="display:inline">
                  <input type="hidden" name="csrf" value="{csrf}">
                  <input type="hidden" name="id" value="{id}">
                  <button type="submit" class="btn-secondary">Delete</button>
                </form></td></tr>"#,
                kind = html_escape(&r.kind),
                pat = html_escape(&r.pattern),
                dest = html_escape(&r.destination),
                id = html_escape(&r.id),
                csrf = csrf,
            ));
        }
        list.push_str("</tbody></table>");
    }
    let mta = if postfix_is_ready() {
        "Postfix ready: glob rules apply to virtual maps."
    } else {
        "Postfix not ready: rules persist in the panel; maps apply when MTA is up."
    };
    let body = format!(
        r#"<p class="muted">{mta} Regex rules stay panel-side (not written as raw Postfix LHS).</p>
        {list}
        <form method="post" action="/email/pattern-forwarding/save" class="stack-form" style="max-width:560px;margin-top:16px;">
          <input type="hidden" name="csrf" value="{csrf}">
          <label for="kind">Kind</label>
          <select id="kind" name="kind">
            <option value="glob">Glob</option>
            <option value="regex">Regex</option>
          </select>
          <label for="pattern">Pattern</label>
          <input id="pattern" name="pattern" required placeholder="sales-*@example.com">
          <label for="destination">Destination</label>
          <input id="destination" name="destination" type="email" required>
          <button type="submit" class="btn-primary">Add rule</button>
        </form>
        <form method="post" action="/email/pattern-forwarding/apply" style="margin-top:12px;">
          <input type="hidden" name="csrf" value="{csrf}">
          <button type="submit" class="btn-secondary">Re-apply Postfix maps</button>
        </form>"#,
        mta = html_escape(mta),
        list = list,
        csrf = csrf,
    );
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Email", Some("/email")),
            ("Pattern Forwarding", None),
        ],
        "Pattern Forwarding",
        "Rule-based forwarding.",
        &body,
        notice,
        error,
    )
}

pub fn email_limits_page(user: &str, notice: Option<&str>, error: Option<&str>) -> String {
    let csrf = html_escape(&email_csrf_token(user));
    let limits = load_send_limits();
    let mut list = String::new();
    if limits.is_empty() {
        list.push_str(r#"<p class="empty-state">No send limits configured.</p>"#);
    } else {
        list.push_str("<table class=\"data-table\"><thead><tr><th>Scope</th><th>Max</th><th>Window (min)</th><th>Note</th><th></th></tr></thead><tbody>");
        for lim in &limits {
            list.push_str(&format!(
                r#"<tr><td><code>{scope}</code></td><td>{max}</td><td>{win}</td><td>{note}</td><td>
                <form method="post" action="/email/limits/delete" style="display:inline">
                  <input type="hidden" name="csrf" value="{csrf}">
                  <input type="hidden" name="id" value="{id}">
                  <button type="submit" class="btn-secondary">Delete</button>
                </form></td></tr>"#,
                scope = html_escape(&lim.scope),
                max = lim.max_messages,
                win = lim.window_minutes,
                note = html_escape(&lim.note),
                id = html_escape(&lim.id),
                csrf = csrf,
            ));
        }
        list.push_str("</tbody></table>");
    }
    let body = format!(
        r#"<p class="muted">Limits are stored as CPN rate metadata and written to a Postfix-friendly policy file under the data dir. Marketing sends respect the tightest configured rate.</p>
        {list}
        <form method="post" action="/email/limits/save" class="stack-form" style="max-width:560px;margin-top:16px;">
          <input type="hidden" name="csrf" value="{csrf}">
          <label for="scope">Scope (mailbox@domain or @domain)</label>
          <input id="scope" name="scope" required placeholder="user@example.com">
          <label for="max_messages">Max messages</label>
          <input id="max_messages" name="max_messages" type="number" min="1" max="100000" value="100" required>
          <label for="window_minutes">Window (minutes)</label>
          <input id="window_minutes" name="window_minutes" type="number" min="1" max="10080" value="60" required>
          <label for="note">Note</label>
          <input id="note" name="note" maxlength="200">
          <button type="submit" class="btn-primary">Add limit</button>
        </form>"#,
        list = list,
        csrf = csrf,
    );
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Email", Some("/email")),
            ("Email Limits", None),
        ],
        "Email Limits",
        "Sending limits.",
        &body,
        notice,
        error,
    )
}

pub fn email_password_page(user: &str, notice: Option<&str>, error: Option<&str>) -> String {
    // Live-heal lineage prefs and refresh admin_login from application.ini for the dropdown.
    let _ = crate::install_snappymail_prefs::ensure_snappymail_operator_defaults();
    let csrf = html_escape(&email_csrf_token(user));
    let choices = mailbox_choices();
    let mut opts = String::from(r#"<option value="">Select mailbox</option>"#);
    for (id, addr) in &choices {
        opts.push_str(&format!(
            r#"<option value="{}">{}</option>"#,
            html_escape(id),
            html_escape(addr)
        ));
    }
    let admin_paths = crate::install_snappymail_lineage::webmail_admin_paths_summary();
    let body = format!(
        r#"<p class="muted">Resets the mailbox login password in the panel registry and hashes it into the local system mailbox store when Postfix/Dovecot provisioning is available. Choose <strong>Webmail admin</strong> to set the admin password for every installed SnappyMail-family client ({admin_paths}). Mailbox rows only change that mailbox. Roundcube is not included.</p>
        <form method="post" action="/email/password/save" class="stack-form" style="max-width:520px;">
          <input type="hidden" name="csrf" value="{csrf}">
          <label for="mailbox">Mailbox</label>
          <select id="mailbox" name="mailbox" required>{opts}</select>
          <label for="password">New password</label>
          <input id="password" name="password" type="password" autocomplete="new-password" minlength="8" required>
          <label for="password2">Confirm password</label>
          <input id="password2" name="password2" type="password" autocomplete="new-password" minlength="8" required>
          <button type="submit" class="btn-primary">Change password</button>
        </form>"#,
        csrf = csrf,
        opts = opts,
        admin_paths = admin_paths,
    );
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Email", Some("/email")),
            ("Change Password", None),
        ],
        "Change Password",
        "Reset mailbox or webmail admin password.",
        &body,
        notice,
        error,
    )
}

pub fn plus_addressing_page(user: &str, notice: Option<&str>, error: Option<&str>) -> String {
    let csrf = html_escape(&email_csrf_token(user));
    let settings = load_plus_settings();
    let live = current_postfix_delimiter();
    let checked = if settings.enabled { " checked" } else { "" };
    let body = format!(
        r#"<p class="muted">Enables <code>user+tag@domain</code> delivery by setting Postfix <code>recipient_delimiter</code>.</p>
        <ul class="kv-list">
          <li><span>Panel preference</span><strong>{pref}</strong></li>
          <li><span>Live postconf</span><strong><code>{live}</code></strong></li>
        </ul>
        <form method="post" action="/email/plus-addressing/save" class="stack-form" style="max-width:480px;margin-top:16px;">
          <input type="hidden" name="csrf" value="{csrf}">
          <label><input type="checkbox" name="enabled" value="1"{checked}> Enable plus-addressing</label>
          <label for="delimiter">Delimiter</label>
          <select id="delimiter" name="delimiter">
            <option value="+" {plus}>+</option>
            <option value="-" {minus}>-</option>
            <option value="=" {eq}>=</option>
          </select>
          <button type="submit" class="btn-primary">Save</button>
        </form>"#,
        pref = if settings.enabled {
            format!("enabled ({})", html_escape(&settings.delimiter))
        } else {
            "disabled".into()
        },
        live = html_escape(&live),
        csrf = csrf,
        checked = checked,
        plus = if settings.delimiter == "+" {
            "selected"
        } else {
            ""
        },
        minus = if settings.delimiter == "-" {
            "selected"
        } else {
            ""
        },
        eq = if settings.delimiter == "=" {
            "selected"
        } else {
            ""
        },
    );
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Email", Some("/email")),
            ("Plus-Addressing", None),
        ],
        "Plus-Addressing",
        "user+tag addressing.",
        &body,
        notice,
        error,
    )
}
