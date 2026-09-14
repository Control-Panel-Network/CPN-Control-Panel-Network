//! HTML for Security hub and feature tile pages.

use crate::panel_admin::is_panel_admin;
use crate::panel_hub_defs::security_hub_sections;
use crate::panel_hubs::{
    feature_shell, hub_tiles_grid, not_configured_body, section_heading, status_kv,
};
use crate::panel_ops_security::{apply_sshd_toggle, fail2ban_status, sshd_status};
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
    let feats = crate::panel_feature_gate::InstalledOptionalFeatures::detect();
    let mut body = section_heading(
        "Security",
        "Firewall, SSH hardening, fail2ban, WAF, malware scan, and SSL certificates for this CPN node.",
    );
    let policy = crate::account::default_password_policy();
    let hint = crate::account::password_policy_hint(&policy);
    body.push_str(&format!(
        r#"<p class="muted" style="margin:0 0 16px;">Panel password policy: {}</p>"#,
        html_escape(&hint)
    ));
    for (title, tiles) in security_hub_sections() {
        let filtered = crate::panel_feature_gate::filter_hub_tiles(tiles, feats);
        if filtered.is_empty() {
            continue;
        }
        body.push_str(&hub_tiles_grid(title, &filtered));
    }
    body
}

pub fn firewall_page(notice: Option<&str>, error: Option<&str>, is_admin: bool) -> String {
    // Legacy entry point kept for callers; manager UI lives in panel_hub_pages_firewall.
    crate::panel_hub_pages_firewall::firewall_manager_page(
        "admin", "rules", notice, error, is_admin, None, None,
    )
}

pub fn secure_ssh_page(
    username: &str,
    notice: Option<&str>,
    error: Option<&str>,
    is_admin: bool,
) -> String {
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
    let review_block = if is_admin {
        match crate::panel_user_prefs::ssh_security_review_snooze_until(username) {
            Some(until) => {
                let until_label = crate::panel_user_prefs::format_epoch_dd_mm_yyyy(until);
                format!(
                    r#"<div class="stack-form" style="max-width:560px;margin-top:24px;padding-top:16px;border-top:1px solid var(--hairline,#e5e5ea);">
                  <h3 style="margin:0 0 8px;font-size:16px;">Activity Board SSH review</h3>
                  <p class="muted" style="margin:0 0 12px;">The SSH security best practices banner is hidden until <strong>{until}</strong>. Hiding only snoozes the banner; the tips still apply.</p>
                  <form method="post" action="/security/ssh/show-security-review">
                    <button type="submit" class="btn-secondary">Show review again</button>
                  </form>
                </div>"#,
                    until = html_escape(&until_label),
                )
            }
            None => r#"<div class="stack-form" style="max-width:560px;margin-top:24px;padding-top:16px;border-top:1px solid var(--hairline,#e5e5ea);">
                  <h3 style="margin:0 0 8px;font-size:16px;">Activity Board SSH review</h3>
                  <p class="muted" style="margin:0;">The SSH security best practices banner is visible on Dashboard, Recent SSH Logs. You can hide it there for up to 1 month.</p>
                </div>"#
                .to_string(),
        }
    } else {
        String::new()
    };
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Security", Some("/security")),
            ("Secure SSH", None),
        ],
        "Secure SSH",
        "Harden sshd with allowlisted toggles.",
        &format!("{kv}{form}{review_block}"),
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
        let engine_note = if st.engine == "nt-api" {
            "<p class=\"muted\">Paid path uses News Targeted API (<code>api.newstargeted.com</code>). Token lives in <code>/var/lib/cpn/malware.json</code> (mode 600).</p>"
        } else {
            "<p class=\"muted\">Free path: ClamAV binaries on this host. Install via Plugins / packages when missing.</p>"
        };
        format!("{}{engine_note}", pre_block(&st.detail))
    } else {
        not_configured_body(
            &st.detail,
            "CPN Malware scan never claims third-party product brands as CPN itself. Free: install ClamAV. Paid: configure api.newstargeted.com token in /var/lib/cpn/malware.json.",
        )
    };
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Security", Some("/security")),
            ("Malware scan", None),
        ],
        "Malware scan",
        "CPN malware status (ClamAV free, or paid News Targeted API).",
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
        assert!(html.contains("Manage SSL"));
        assert!(html.contains("Secure SSH") || html.contains("SSH"));
        assert!(!html.contains("CyberPanel"));
        assert!(!html.contains("Imunify"));
        // Fail2ban / Malware / Firewall tiles are feature-gated when not installed.
        // Hub copy may still mention them; live tiles appear only when enabled.
    }
}
