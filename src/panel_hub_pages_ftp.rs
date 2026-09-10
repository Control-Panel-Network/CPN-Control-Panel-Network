//! Databases & FTP hub pages for jailed OpenSSH SFTP.

use crate::panel_hubs::{feature_shell, status_kv};
use crate::panel_ops_ftp::detect_ftp;

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn ftp_accounts_page(notice: Option<&str>, error: Option<&str>) -> String {
    let status = detect_ftp();
    let accounts = crate::resource_accounts::list_ftp_accounts();
    let sites = crate::sites::list_sites().unwrap_or_default();
    let kv = status_kv(&[
        ("Stack", &status.stack),
        ("Accounts", &accounts.len().to_string()),
        ("Sites", &sites.len().to_string()),
    ]);
    let list = if accounts.is_empty() {
        "<p class=\"empty-state\">No jailed SFTP accounts yet.</p>".to_string()
    } else {
        let mut rows = String::from(
            r#"<div class="table-wrap"><table class="data-table"><thead><tr><th>Username</th><th>Domain</th><th>Owner</th></tr></thead><tbody>"#,
        );
        for acct in &accounts {
            rows.push_str(&format!(
                "<tr><td><code>{}</code></td><td>{}</td><td>{}</td></tr>",
                html_escape(&acct.username),
                html_escape(&acct.domain),
                html_escape(&acct.owner),
            ));
        }
        rows.push_str("</tbody></table></div>");
        rows
    };
    let help = format!(
        "<p class=\"muted\">{} Each account is OpenSSH <code>internal-sftp</code> only, chrooted to the site home. Shell login is disabled.</p>
        <p><a class=\"btn-primary\" href=\"/ftp/create\">Create SFTP account</a>
        <a class=\"btn-secondary\" href=\"/ftp/reset\" style=\"margin-left:8px;\">Reset SFTP stack</a></p>",
        html_escape(&status.detail)
    );
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Databases & FTP", Some("/databases")),
            ("SFTP Accounts", None),
        ],
        "SFTP Accounts",
        "Jailed OpenSSH SFTP users per website and subdomain.",
        &format!("{kv}{list}{help}"),
        notice,
        error,
    )
}

pub fn ftp_create_page(notice: Option<&str>, error: Option<&str>) -> String {
    let sites = crate::sites::list_sites().unwrap_or_default();
    let options = if sites.is_empty() {
        "<option value=\"\">Create a website first</option>".to_string()
    } else {
        sites
            .iter()
            .map(|s| {
                format!(
                    "<option value=\"{d}\">{d}</option>",
                    d = html_escape(&s.domain)
                )
            })
            .collect::<Vec<_>>()
            .join("")
    };
    let form = format!(
        r#"<form method="post" action="/ftp/create" class="stack-form" style="max-width:460px;">
      <label for="domain">Website / subdomain</label>
      <select id="domain" name="domain" required>{options}</select>
      <label for="username">SFTP username</label>
      <input id="username" name="username" type="text" required pattern="[a-z][a-z0-9_]{{2,31}}" maxlength="32" placeholder="site_sftp" autocomplete="off">
      <label for="password">Password</label>
      <input id="password" name="password" type="password" required minlength="8" maxlength="128" autocomplete="new-password">
      <button type="submit" class="btn-primary" {disabled}>Create jailed SFTP account</button>
    </form>
    <p class="muted">Jail root is the site home under <code>/home/…</code>. Only <code>public_html</code> is writable. No shell login.</p>"#,
        options = options,
        disabled = if sites.is_empty() { "disabled" } else { "" },
    );
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Databases & FTP", Some("/databases")),
            ("Create SFTP Account", None),
        ],
        "Create SFTP Account",
        "Jailed per website or subdomain.",
        &form,
        notice,
        error,
    )
}

pub fn ftp_delete_page(notice: Option<&str>, error: Option<&str>) -> String {
    let accounts = crate::resource_accounts::list_ftp_accounts();
    let options = if accounts.is_empty() {
        "<option value=\"\">No accounts</option>".to_string()
    } else {
        accounts
            .iter()
            .map(|a| {
                format!(
                    "<option value=\"{u}\">{u} ({d})</option>",
                    u = html_escape(&a.username),
                    d = html_escape(&a.domain)
                )
            })
            .collect::<Vec<_>>()
            .join("")
    };
    let form = format!(
        r#"<form method="post" action="/ftp/delete" class="stack-form" style="max-width:460px;" onsubmit="return confirm('Delete this jailed SFTP user? Site files are kept.');">
      <label for="username">Account</label>
      <select id="username" name="username" required>{options}</select>
      <button type="submit" class="btn-primary" {disabled}>Delete SFTP account</button>
    </form>
    <p class="muted">Removes the system user and registry entry. Document roots are not deleted.</p>"#,
        options = options,
        disabled = if accounts.is_empty() { "disabled" } else { "" },
    );
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Databases & FTP", Some("/databases")),
            ("Delete SFTP Account", None),
        ],
        "Delete SFTP Account",
        "Remove a jailed SFTP user.",
        &form,
        notice,
        error,
    )
}

pub fn ftp_reset_page(notice: Option<&str>, error: Option<&str>) -> String {
    let status = detect_ftp();
    let body = format!(
        r#"<p>{}</p>
        <form method="post" action="/ftp/reset" class="stack-form" style="max-width:460px;">
          <button type="submit" class="btn-primary">Install / refresh SFTP jail config</button>
        </form>
        <p class="muted">Writes <code>/etc/ssh/sshd_config.d/99-cpn-sftp.conf</code>, ensures group <code>cpn-sftp</code>, and reloads sshd. Plain FTP daemons are not required.</p>
        <form method="post" action="/ftp/reset-password" class="stack-form" style="max-width:460px;margin-top:24px;">
          <h3>Reset account password</h3>
          <label for="username">Username</label>
          <input id="username" name="username" required pattern="[a-z][a-z0-9_]{{2,31}}" maxlength="32">
          <label for="password">New password</label>
          <input id="password" name="password" type="password" required minlength="8" maxlength="128" autocomplete="new-password">
          <button type="submit" class="btn-secondary">Update password</button>
        </form>"#,
        html_escape(&status.detail)
    );
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Databases & FTP", Some("/databases")),
            ("Reset SFTP", None),
        ],
        "Reset SFTP",
        "Install jail Match block and manage passwords.",
        &body,
        notice,
        error,
    )
}
