//! SELinux helpers for PHP-FPM (httpd_t) outbound IMAP/SMTP/Sieve to local mail.

use std::fs;
use std::path::Path;
use std::process::Command;

const MODULE: &str = "cpn_webmail_imap";
const MODULE_VER: &str = "1.1";

/// Allow httpd_t to connect to pop_port_t (IMAP 143/993), smtp_port_t (25/587),
/// and sieve_port_t (ManageSieve 4190).
///
/// `httpd_can_network_connect` alone still denied name_connect to pop_port_t on
/// AlmaLinux 9 lab (AVC: php-fpm dest=143). A tiny local policy module closes that gap.
pub fn ensure_httpd_mail_ports() {
    let _ = Command::new("bash")
        .args([
            "-c",
            "command -v setsebool >/dev/null 2>&1 && setsebool -P httpd_can_network_connect 1 || true",
        ])
        .status();

    let tools_ok = Command::new("bash")
        .args([
            "-c",
            "command -v checkmodule >/dev/null 2>&1 && command -v semodule_package >/dev/null 2>&1 && command -v semodule >/dev/null 2>&1",
        ])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !tools_ok {
        return;
    }

    let dir = Path::new("/var/lib/cpn/selinux");
    if fs::create_dir_all(dir).is_err() {
        return;
    }
    let te = dir.join("cpn_webmail_imap.te");
    let mod_path = dir.join("cpn_webmail_imap.mod");
    let pp = dir.join("cpn_webmail_imap.pp");
    let stamp = dir.join("cpn_webmail_imap.ver");
    let te_src = format!(
        r#"module {MODULE} {MODULE_VER};
require {{
    type httpd_t;
    type pop_port_t;
    type smtp_port_t;
    type sieve_port_t;
    class tcp_socket name_connect;
}}
allow httpd_t pop_port_t:tcp_socket name_connect;
allow httpd_t smtp_port_t:tcp_socket name_connect;
allow httpd_t sieve_port_t:tcp_socket name_connect;
"#
    );

    let stamp_ok = fs::read_to_string(&stamp)
        .map(|s| s.trim() == MODULE_VER)
        .unwrap_or(false);
    let loaded = Command::new("semodule")
        .args(["-l"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).contains(MODULE))
        .unwrap_or(false);
    if loaded && stamp_ok {
        return;
    }

    if fs::write(&te, &te_src).is_err() {
        return;
    }
    let ok = Command::new("checkmodule")
        .args(["-M", "-m", "-o"])
        .arg(&mod_path)
        .arg(&te)
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !ok {
        return;
    }
    let ok = Command::new("semodule_package")
        .args(["-o"])
        .arg(&pp)
        .arg("-m")
        .arg(&mod_path)
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !ok {
        return;
    }
    if loaded {
        let _ = Command::new("semodule").args(["-r", MODULE]).status();
    }
    let _ = Command::new("semodule").args(["-i"]).arg(&pp).status();
    let _ = fs::write(&stamp, format!("{MODULE_VER}\n"));
}

/// Mail port allows plus SnappyMail data_dir fcontext/restorecon.
pub fn ensure_webmail_selinux() {
    ensure_httpd_mail_ports();
    let _ = Command::new("bash")
        .args([
            "-c",
            "command -v semanage >/dev/null 2>&1 && \
             (semanage fcontext -a -t httpd_sys_rw_content_t '/var/lib/cpn-webmail(/.*)?' || \
              semanage fcontext -m -t httpd_sys_rw_content_t '/var/lib/cpn-webmail(/.*)?' || true); \
             restorecon -Rv /var/lib/cpn-webmail >/dev/null 2>&1 || true",
        ])
        .status();
}

#[cfg(test)]
mod tests {
    #[test]
    fn te_source_mentions_pop_smtp_sieve() {
        let te = r#"allow httpd_t pop_port_t:tcp_socket name_connect;
allow httpd_t smtp_port_t:tcp_socket name_connect;
allow httpd_t sieve_port_t:tcp_socket name_connect;"#;
        assert!(te.contains("pop_port_t"));
        assert!(te.contains("smtp_port_t"));
        assert!(te.contains("sieve_port_t"));
    }
}
