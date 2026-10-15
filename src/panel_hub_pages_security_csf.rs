//! Optional CSF host package status and install UI.

use crate::panel_hubs::{feature_shell, not_configured_body, status_kv};
use crate::panel_ops_csf::{csf_status, enable_csf, install_csf};

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn pre_block(text: &str) -> String {
    format!(
        r#"<pre class="manage-log-pre" style="white-space:pre-wrap;max-height:280px;overflow:auto;">{}</pre>"#,
        html_escape(text)
    )
}

pub fn csf_page(notice: Option<&str>, error: Option<&str>) -> String {
    let st = csf_status();
    let kv = status_kv(&[
        ("Installed", if st.installed { "yes" } else { "no" }),
        ("Enabled", if st.enabled { "yes" } else { "no" }),
        (
            "Version",
            if st.version.is_empty() {
                "n/a"
            } else {
                &st.version
            },
        ),
    ]);
    let mut flash = String::new();
    if let Some(n) = notice.filter(|s| !s.is_empty()) {
        flash.push_str(&format!(
            r#"<p class="notice notice-ok">{}</p>"#,
            html_escape(n)
        ));
    }
    if let Some(e) = error.filter(|s| !s.is_empty()) {
        flash.push_str(&format!(
            r#"<p class="notice notice-err">{}</p>"#,
            html_escape(e)
        ));
    }
    let actions = if st.installed {
        r#"<form method="post" action="/security/csf/enable" style="margin:12px 0;">
          <button type="submit" class="btn-primary">Enable CSF and LFD</button>
        </form>
        <p class="muted">Server &gt; Services shows CSF as Active when the unit is running. Whitelist panel and admin IPs before leaving TESTING mode.</p>"#
            .to_string()
    } else {
        r#"<form method="post" action="/security/csf/install" style="margin:12px 0;">
          <button type="submit" class="btn-primary">Install CSF (Host package)</button>
        </form>
        <p class="muted">Optional Host package. Until installed, Services lists CSF as Not installed with a Store link.</p>"#
            .to_string()
    };
    let body = if st.installed {
        format!(
            "{flash}{kv}{detail}{actions}",
            flash = flash,
            kv = kv,
            detail = pre_block(&st.detail),
            actions = actions
        )
    } else {
        format!(
            "{flash}{kv}{body}{actions}",
            flash = flash,
            kv = kv,
            body = not_configured_body(
                &st.detail,
                "CSF is an optional Host firewall package. Install only when you need ConfigServer Security & Firewall alongside the built-in Firewall and Fail2ban tiles.",
            ),
            actions = actions
        )
    };
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Security", Some("/security")),
            ("CSF", None),
        ],
        "CSF",
        "Optional ConfigServer Security & Firewall host package.",
        &body,
        None,
        None,
    )
}

pub fn run_csf_install() -> Result<String, String> {
    install_csf()
}

pub fn run_csf_enable() -> Result<String, String> {
    enable_csf()
}
