//! Simple mailbox create UI: local-part + password + website/sub-domain picker.
//! SMTP/IMAP client details stay read-only (mail client / accounts), not create fields.

use crate::backups::is_subdomain_site;
use crate::mail_postfix_maps::can_manage_mail_domain;
use crate::panel_hub_pages_mail_client::mail_client_config_html;
use crate::sites::list_sites;

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn section_heading(title: &str, blurb: &str) -> String {
    format!(
        r#"
      <div class="dashboard-heading">
        <div>
          <p class="eyebrow">CPN PANEL</p>
          <h1>{title}</h1>
          <p>{blurb}</p>
        </div>
      </div>"#,
        title = html_escape(title),
        blurb = html_escape(blurb),
    )
}

fn notice_block(kind: &str, message: Option<&str>) -> String {
    let Some(message) = message.filter(|value| !value.is_empty()) else {
        return String::new();
    };
    let class = if kind == "error" {
        "panel-notice error"
    } else {
        "panel-notice ok"
    };
    format!(
        r#"<p class="{class}" role="status">{msg}</p>"#,
        msg = html_escape(message)
    )
}

/// Website (apex) and sub-domain FQDNs the user may create mail for.
pub fn mailbox_create_domain_choices(username: &str) -> (Vec<String>, Vec<String>) {
    let mut websites = Vec::new();
    let mut subdomains = Vec::new();
    let Ok(sites) = list_sites() else {
        return (websites, subdomains);
    };
    for site in sites {
        if !site.enabled {
            continue;
        }
        if !can_manage_mail_domain(username, &site.domain) {
            continue;
        }
        if is_subdomain_site(&site.domain) {
            subdomains.push(site.domain);
        } else {
            websites.push(site.domain);
        }
    }
    websites.sort();
    subdomains.sort();
    (websites, subdomains)
}

fn options_html(domains: &[String], empty_label: &str) -> String {
    let mut out = format!(
        r#"<option value="">{label}</option>"#,
        label = html_escape(empty_label)
    );
    for domain in domains {
        out.push_str(&format!(
            r#"<option value="{v}">{v}</option>"#,
            v = html_escape(domain)
        ));
    }
    out
}

/// Shared create-mailbox form (`return_path` is post-success redirect base).
pub fn email_mailbox_create_form_html(username: &str, return_path: &str) -> String {
    let return_path = if return_path.trim().is_empty() {
        "/email/accounts"
    } else {
        return_path.trim()
    };
    let (websites, subdomains) = mailbox_create_domain_choices(username);
    let has_websites = !websites.is_empty();
    let has_subs = !subdomains.is_empty();
    let website_disabled = if has_websites { "" } else { " disabled" };
    let sub_disabled = if has_subs { "" } else { " disabled" };
    let default_scope = if has_websites {
        "website"
    } else if has_subs {
        "subdomain"
    } else {
        "website"
    };
    let website_checked = if default_scope == "website" {
        " checked"
    } else {
        ""
    };
    let sub_checked = if default_scope == "subdomain" {
        " checked"
    } else {
        ""
    };
    let empty_msg = if !has_websites && !has_subs {
        r#"<p class="panel-notice error" role="alert">No hosted websites or sub-domains yet. Create a site under <a href="/websites/create">Websites</a> or <a href="/subdomains/create">Sub-domains</a> first.</p>"#
    } else {
        ""
    };
    let website_hint = if has_websites {
        ""
    } else {
        r#"<p class="muted">No main websites available for your account.</p>"#
    };
    let sub_hint = if has_subs {
        ""
    } else {
        r#"<p class="muted">No sub-domains available. Add one under <a href="/subdomains/create">Sub-domains</a> when needed.</p>"#
    };
    let policy = crate::account::default_password_policy();
    let policy_hint = crate::account::password_policy_hint_html(&policy);
    let min_len = policy.min_length;

    format!(
        r#"
      <form method="post" action="/email/accounts/create" class="stack-form" style="max-width:560px;margin-top:16px;" id="mailbox-create-form">
        <input type="hidden" name="return_to" value="{return_path}">
        <input type="hidden" name="smtp_mode" value="postfix_local">
        <input type="hidden" name="enabled" value="1">
        {empty}
        <fieldset style="border:0;padding:0;margin:0 0 12px;">
          <legend style="font-weight:600;margin-bottom:8px;">Create mailbox for</legend>
          <label style="display:flex;align-items:center;gap:8px;margin-bottom:6px;">
            <input type="radio" name="domain_scope" value="website" id="scope_website"{website_checked}{website_disabled}>
            Website (main domain)
          </label>
          <label style="display:flex;align-items:center;gap:8px;">
            <input type="radio" name="domain_scope" value="subdomain" id="scope_subdomain"{sub_checked}{sub_disabled}>
            Sub-domain
          </label>
        </fieldset>
        <label for="domain_website">Website domain</label>
        <select id="domain_website" name="domain_website"{website_disabled}>
          {website_opts}
        </select>
        {website_hint}
        <label for="domain_subdomain">Sub-domain</label>
        <select id="domain_subdomain" name="domain_subdomain"{sub_disabled}>
          {sub_opts}
        </select>
        {sub_hint}
        <input type="hidden" name="domain" id="domain" value="">
        <label for="local_part">Email username (before @)</label>
        <input id="local_part" name="local_part" type="text" required autocomplete="off" placeholder="info" pattern="[A-Za-z0-9._+-]+" maxlength="64">
        <p class="muted" id="mailbox-preview">Mailbox: <code>—</code></p>
        <label for="mailbox_password">Password</label>
        <input id="mailbox_password" name="mailbox_password" type="password" autocomplete="new-password" required minlength="{min_len}" maxlength="256" placeholder="Panel password policy">
        <label for="mailbox_password_confirm">Confirm password</label>
        <input id="mailbox_password_confirm" name="mailbox_password_confirm" type="password" autocomplete="new-password" required minlength="{min_len}" maxlength="256">
        <p class="muted">{policy_hint}</p>
        <button type="submit" class="btn-primary"{submit_disabled}>Create mailbox</button>
      </form>
      <script>
      (function () {{
        var form = document.getElementById("mailbox-create-form");
        if (!form) return;
        var scopeWebsite = document.getElementById("scope_website");
        var scopeSub = document.getElementById("scope_subdomain");
        var selWebsite = document.getElementById("domain_website");
        var selSub = document.getElementById("domain_subdomain");
        var domainHidden = document.getElementById("domain");
        var localPart = document.getElementById("local_part");
        var preview = document.getElementById("mailbox-preview");
        function activeSelect() {{
          if (scopeSub && scopeSub.checked) return selSub;
          return selWebsite;
        }}
        function sync() {{
          var useSub = !!(scopeSub && scopeSub.checked);
          if (selWebsite) {{
            selWebsite.disabled = useSub || selWebsite.options.length <= 1;
            selWebsite.style.display = useSub ? "none" : "";
            var labW = form.querySelector('label[for="domain_website"]');
            if (labW) labW.style.display = useSub ? "none" : "";
          }}
          if (selSub) {{
            selSub.disabled = !useSub || selSub.options.length <= 1;
            selSub.style.display = useSub ? "" : "none";
            var labS = form.querySelector('label[for="domain_subdomain"]');
            if (labS) labS.style.display = useSub ? "" : "none";
          }}
          var sel = activeSelect();
          var domain = sel && sel.value ? sel.value : "";
          if (domainHidden) domainHidden.value = domain;
          var local = (localPart && localPart.value ? localPart.value.trim() : "");
          if (preview) {{
            preview.innerHTML = domain && local
              ? ("Mailbox: <code>" + local.toLowerCase() + "@" + domain + "</code>")
              : "Mailbox: <code>—</code>";
          }}
        }}
        ["change", "input"].forEach(function (ev) {{
          form.addEventListener(ev, sync);
        }});
        sync();
      }})();
      </script>"#,
        return_path = html_escape(return_path),
        empty = empty_msg,
        website_checked = website_checked,
        website_disabled = website_disabled,
        sub_checked = sub_checked,
        sub_disabled = sub_disabled,
        website_opts = options_html(&websites, "Select a website…"),
        sub_opts = options_html(&subdomains, "Select a sub-domain…"),
        website_hint = website_hint,
        sub_hint = sub_hint,
        min_len = min_len,
        policy_hint = policy_hint,
        submit_disabled = if !has_websites && !has_subs {
            " disabled"
        } else {
            ""
        },
    )
}

fn created_address_from_notice(notice: Option<&str>) -> Option<String> {
    let notice = notice?;
    let prefix = "Created mailbox ";
    if let Some(rest) = notice.strip_prefix(prefix) {
        let addr = rest.trim();
        if addr.contains('@') {
            return Some(addr.to_ascii_lowercase());
        }
    }
    None
}

/// Create Email page body (simple local-part + password + domain scope).
pub fn email_create_main(username: &str, notice: Option<&str>, error: Option<&str>) -> String {
    let client_help = if let Some(addr) = created_address_from_notice(notice) {
        let domain = addr
            .rsplit_once('@')
            .map(|(_, d)| d)
            .unwrap_or("")
            .to_string();
        if domain.is_empty() {
            String::new()
        } else {
            format!(
                r#"<p class="muted">Use these read-only settings in your mail app (Thunderbird, Outlook, Apple Mail). Host is <code>mail.{domain}</code>.</p>
        {card}"#,
                domain = html_escape(&domain),
                card = mail_client_config_html(&domain),
            )
        }
    } else {
        r#"<p class="muted">After create, open <a href="/email/accounts">Email Accounts</a> for read-only IMAP/SMTP/Sieve host and port guidance (<code>mail.&lt;domain&gt;</code>). Ports follow panel Settings (TLS-first defaults).</p>"#.into()
    };

    format!(
        r#"{heading}
      {ok}
      {err}
      {stack}
      <article class="section-card">
        <h2>Create mailbox</h2>
        <p>Enter a username and password, then pick a website or sub-domain. Local Postfix and Dovecot use panel Settings for ports and TLS.</p>
        {create}
      </article>
      {client_help}
      <p class="muted"><a href="/email/accounts">View all mailboxes</a></p>"#,
        heading = section_heading(
            "Create Email",
            "Provision a mailbox with an email address and password.",
        ),
        ok = notice_block("ok", notice),
        err = notice_block("error", error),
        stack = crate::panel_sections::email_mail_stack_notice_html(),
        create = email_mailbox_create_form_html(username, "/email/create"),
        client_help = client_help,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_form_omits_smtp_pickers() {
        let html = email_mailbox_create_form_html("admin", "/email/create");
        assert!(html.contains("local_part"));
        assert!(html.contains("mailbox_password_confirm"));
        assert!(html.contains("domain_scope"));
        assert!(html.contains("postfix_local"));
        assert!(!html.contains("name=\"smtp_host\""));
        assert!(!html.contains("name=\"smtp_port\""));
        assert!(!html.contains("name=\"smtp_tls\""));
        assert!(!html.contains("name=\"smtp_username\""));
        assert!(!html.contains("name=\"smtp_password\""));
        assert!(!html.contains(">SMTP mode<"));
        assert!(!html.contains("External SMTP"));
    }

    #[test]
    fn created_address_parses_notice() {
        assert_eq!(
            created_address_from_notice(Some("Created mailbox Info@Example.COM")),
            Some("info@example.com".into())
        );
        assert!(created_address_from_notice(Some("something else")).is_none());
    }
}
