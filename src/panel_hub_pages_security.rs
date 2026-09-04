//! HTML for Security hub and feature tile pages.

use crate::panel_admin::is_panel_admin;
use crate::panel_hub_defs::security_hub_sections;
use crate::panel_hubs::{
    feature_shell, hub_tiles_grid, not_configured_body, section_heading, status_kv,
};
use crate::panel_ops_security::{apply_sshd_toggle, fail2ban_status, firewall_status, sshd_status};
use crate::panel_ops_security_ssl::{
    hostname_ssl_status, list_modsec_rule_files, mail_ssl_status, malware_scan_status,
    modsec_status, site_ssl_rows,
};

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn pre_block(text: &str) -> String {
    format!(
        r#"<pre class="manage-log-pre" style="white-space:pre-wrap;max-height:360px;overflow:auto;">{}</pre>"#,
        html_escape(text)
    )
}

pub fn security_hub_main() -> String {
    let mut body = section_heading(
        "Security",
        "Firewall, SSH hardening, fail2ban, WAF, malware scan, and SSL certificates for this CPN node.",
    );
    for (title, tiles) in security_hub_sections() {
        body.push_str(&hub_tiles_grid(title, &tiles));
    }
    body
}

pub fn firewall_page() -> String {
    let st = firewall_status();
    let services = if st.services.is_empty() {
        "n/a".into()
    } else {
        st.services.join(", ")
    };
    let kv = status_kv(&[
        ("Backend", &st.backend),
        ("Active", if st.active { "yes" } else { "no" }),
        ("Services", &services),
    ]);
    let journal = if st.journal_excerpt.is_empty() {
        "<p class=\"muted\">No CPN firewall journal yet (issue #21 journal is written on install).</p>"
            .to_string()
    } else {
        format!(
            "<h3>CPN firewall journal</h3>{}",
            pre_block(&st.journal_excerpt)
        )
    };
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Security", Some("/security")),
            ("Firewall", None),
        ],
        "Firewall",
        "Live firewalld / ufw / iptables status.",
        &format!(
            "{kv}<h3>Status</h3>{}{journal}<p class=\"muted\">Rule edits that open arbitrary ports stay admin-gated for a later release. CPN only manages journaled http/https rules from install.</p>",
            pre_block(&st.detail)
        ),
        None,
        None,
    )
}

pub fn secure_ssh_page(notice: Option<&str>, error: Option<&str>, is_admin: bool) -> String {
    let st = sshd_status();
    let kv = status_kv(&[
        ("Config", &st.config_path),
        ("Present", if st.present { "yes" } else { "no" }),
        ("sshd unit", &st.unit_active),
        ("PermitRootLogin", &st.permit_root_login),
        ("PasswordAuthentication", &st.password_authentication),
    ]);
    let form = if is_admin && st.present {
        r#"<form method="post" action="/security/ssh/toggle" class="stack-form" style="max-width:480px;margin-top:16px;">
          <label for="key">Directive</label>
          <select id="key" name="key">
            <option value="PermitRootLogin">PermitRootLogin</option>
            <option value="PasswordAuthentication">PasswordAuthentication</option>
          </select>
          <label for="value">Value</label>
          <select id="value" name="value">
            <option value="no">no</option>
            <option value="yes">yes</option>
            <option value="prohibit-password">prohibit-password (root)</option>
          </select>
          <button type="submit" class="btn-primary">Apply with backup</button>
        </form>
        <p class="muted">Writes a timestamped backup under the CPN data dir, runs <code>sshd -t</code> when available, then reloads sshd.</p>"#.to_string()
    } else if !is_admin {
        "<p class=\"muted\">Only the panel admin can change sshd settings.</p>".into()
    } else {
        "<p class=\"muted\">sshd_config was not found; read-only probes only.</p>".into()
    };
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Security", Some("/security")),
            ("Secure SSH", None),
        ],
        "Secure SSH",
        "Harden sshd with allowlisted toggles.",
        &format!("{kv}{form}"),
        notice,
        error,
    )
}

pub fn run_sshd_toggle(user: &str, key: &str, value: &str) -> Result<String, String> {
    if !is_panel_admin(user) {
        return Err("Only the panel admin can change sshd settings".into());
    }
    apply_sshd_toggle(key, value)
}

pub fn fail2ban_page() -> String {
    let st = fail2ban_status();
    if !st.installed {
        return feature_shell(
            &[
                ("Dashboard", Some("/dashboard")),
                ("Security", Some("/security")),
                ("Fail2ban", None),
            ],
            "Fail2ban",
            "Brute-force protection.",
            &not_configured_body(
                &st.detail,
                "Install fail2ban on this host, then reopen this tile for live jail status.",
            ),
            None,
            None,
        );
    }
    let jails = if st.jails.is_empty() {
        "<p class=\"muted\">No jails listed (service may be idle).</p>".to_string()
    } else {
        let mut ul = String::from("<ul>");
        for j in &st.jails {
            ul.push_str(&format!("<li><code>{}</code></li>", html_escape(j)));
        }
        ul.push_str("</ul>");
        ul
    };
    let kv = status_kv(&[
        ("Installed", "yes"),
        ("Active", &st.active),
        ("Enabled", &st.enabled),
    ]);
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Security", Some("/security")),
            ("Fail2ban", None),
        ],
        "Fail2ban",
        "Live jail status.",
        &format!("{kv}<h3>Jails</h3>{jails}{}", pre_block(&st.detail)),
        None,
        None,
    )
}

pub fn modsec_page() -> String {
    let st = modsec_status();
    let kv = status_kv(&[
        ("Detected", if st.detected { "yes" } else { "no" }),
        ("Engine", &st.engine),
    ]);
    let packs = if st.rule_paths.is_empty() {
        "<p class=\"muted\">No OWASP CRS / rule pack directories found.</p>".to_string()
    } else {
        let mut ul = String::from("<ul>");
        for p in &st.rule_paths {
            ul.push_str(&format!("<li><code>{}</code></li>", html_escape(p)));
        }
        ul.push_str("</ul>");
        format!("<h3>Rule pack directories</h3>{ul}")
    };
    let body = if st.detected {
        format!("{kv}{}{packs}", pre_block(&st.detail))
    } else {
        format!(
            "{kv}{}",
            not_configured_body(
                &st.detail,
                "This tile stays honest: no WAF success is claimed without detection."
            )
        )
    };
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Security", Some("/security")),
            ("ModSecurity", None),
        ],
        "ModSecurity / WAF",
        "Detect httpd or nginx ModSecurity.",
        &body,
        None,
        None,
    )
}

pub fn modsec_rules_page() -> String {
    let files = list_modsec_rule_files(40);
    let body = if files.is_empty() {
        not_configured_body(
            "No ModSecurity .conf / .rules files were found under common paths.",
            "Install OWASP CRS or vendor rule packs, then return here for a live file list.",
        )
    } else {
        let mut ul = String::from("<ul>");
        for f in &files {
            ul.push_str(&format!("<li><code>{}</code></li>", html_escape(f)));
        }
        ul.push_str("</ul>");
        format!("<p class=\"muted\">Showing up to 40 rule files.</p>{ul}")
    };
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Security", Some("/security")),
            ("ModSec Rules", None),
        ],
        "ModSec Rules",
        "List detected ModSecurity rule files.",
        &body,
        None,
        None,
    )
}

pub fn rule_packs_page() -> String {
    let st = modsec_status();
    let body = if st.rule_paths.is_empty() {
        not_configured_body(
            "No OWASP CRS or Comodo-style rule pack directories detected.",
            "CPN will list pack roots when they appear under /etc/modsecurity or vendor paths.",
        )
    } else {
        let mut ul = String::from("<ul>");
        for p in &st.rule_paths {
            ul.push_str(&format!("<li><code>{}</code></li>", html_escape(p)));
        }
        ul.push_str("</ul>");
        format!("<p>Detected rule pack roots:</p>{ul}")
    };
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Security", Some("/security")),
            ("Rule Packs", None),
        ],
        "Rule Packs",
        "OWASP CRS and related pack roots.",
        &body,
        None,
        None,
    )
}

pub fn malware_scan_page() -> String {
    let st = malware_scan_status();
    let kv = status_kv(&[
        ("Engine", &st.engine),
        ("Installed", if st.installed { "yes" } else { "no" }),
    ]);
    let extra = if st.installed {
        format!(
            "{}<p class=\"muted\">On-demand scan UI is next; status above is live from ClamAV binaries.</p>",
            pre_block(&st.detail)
        )
    } else {
        not_configured_body(
            &st.detail,
            "CPN Malware scan never claims third-party products. Install ClamAV for live status.",
        )
    };
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Security", Some("/security")),
            ("Malware scan", None),
        ],
        "Malware scan",
        "CPN malware status (ClamAV when present).",
        &format!("{kv}{extra}"),
        None,
        None,
    )
}

pub fn manage_ssl_page() -> String {
    let rows = site_ssl_rows();
    let body = if rows.is_empty() {
        "<p class=\"empty-state\">No websites registered yet. Create a site, then manage SSL from Websites.</p>
         <p><a class=\"hub-tile\" href=\"/websites\"><strong>Websites</strong><span>Create or manage sites</span></a></p>"
            .to_string()
    } else {
        let mut t = String::from(
            r#"<div class="table-wrap"><table class="data-table"><thead><tr><th>Domain</th><th>Certificate</th><th></th></tr></thead><tbody>"#,
        );
        for r in &rows {
            let cert = if r.has_cert { "Present" } else { "Missing" };
            t.push_str(&format!(
                r#"<tr><td><code>{domain}</code></td><td>{cert}</td><td><a href="/websites/manage?domain={q}&amp;tab=ssl">Manage SSL</a></td></tr>"#,
                domain = html_escape(&r.domain),
                cert = cert,
                q = urlencoding_lite(&r.domain),
            ));
        }
        t.push_str("</tbody></table></div>");
        t
    };
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Security", Some("/security")),
            ("Manage SSL", None),
        ],
        "Manage SSL",
        "Site certificates linked to Manage hubs.",
        &body,
        None,
        None,
    )
}

fn urlencoding_lite(value: &str) -> String {
    let mut out = String::new();
    for b in value.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(b as char);
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

pub fn hostname_ssl_page() -> String {
    let st = hostname_ssl_status();
    let host = st.hostname.as_deref().unwrap_or("(not set)");
    let kv = status_kv(&[
        ("Panel hostname", host),
        (
            "Certificate",
            if st.has_cert { "Present" } else { "Missing" },
        ),
        (
            "certbot",
            if st.certbot { "Available" } else { "Not found" },
        ),
    ]);
    let hint = if st.certbot && st.hostname.is_some() && !st.has_cert {
        format!(
            r#"<p class="muted">Example (run as root):</p><pre class="manage-log-pre">certbot certonly --standalone -d {}</pre>"#,
            html_escape(st.hostname.as_deref().unwrap_or(""))
        )
    } else {
        String::new()
    };
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Security", Some("/security")),
            ("Hostname SSL", None),
        ],
        "Hostname SSL",
        "Panel hostname certificate status.",
        &format!("{kv}<p>{}</p>{hint}", html_escape(&st.detail)),
        None,
        None,
    )
}

pub fn mail_ssl_page() -> String {
    let st = mail_ssl_status();
    let kv = status_kv(&[(
        "Certificate",
        if st.has_cert { "Present" } else { "Missing" },
    )]);
    let checked = {
        let mut ul = String::from("<ul>");
        for p in &st.paths_checked {
            ul.push_str(&format!("<li><code>{}</code></li>", html_escape(p)));
        }
        ul.push_str("</ul>");
        ul
    };
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Security", Some("/security")),
            ("Mail Server SSL", None),
        ],
        "Mail Server SSL",
        "Mail stack certificate detection.",
        &format!(
            "{kv}{}<h3>Paths checked</h3>{checked}",
            pre_block(&st.detail)
        ),
        None,
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hub_contains_sections() {
        let html = security_hub_main();
        assert!(html.contains("Firewall"));
        assert!(html.contains("Malware scan"));
        assert!(html.contains("Manage SSL"));
        assert!(!html.contains("CyberPanel"));
        assert!(!html.contains("Imunify"));
    }
}
