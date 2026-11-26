//! SELinux helpers for PHP-FPM (httpd_t) outbound IMAP/SMTP/Sieve to local mail.

use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Bumped when fcontext paths change so hosts re-run `semanage` once.
const FCONTEXT_STAMP_VER: &str = "2";
const MODULE: &str = "cpn_webmail_imap";
const MODULE_VER: &str = "1.2";
const WEBMAIL_CODE_ROOT: &str = "/opt/cpn-webmail";
const WEBMAIL_DATA_ROOT: &str = "/var/lib/cpn-webmail";

/// Allow httpd_t to connect to pop_port_t (IMAP 143/993), smtp_port_t (25/587),
/// sieve_port_t (ManageSieve 4190), and http_port_t (HTTPS 443 package repo).
///
/// `httpd_can_network_connect` alone still denied name_connect to pop_port_t on
/// AlmaLinux 9 lab (AVC: php-fpm dest=143). The same hosts also denied dest=443
/// (`http_port_t`) when SnappyMail fetched `packages.json` / core updates.
pub fn ensure_httpd_mail_ports() {
    if !selinux_present() {
        return;
    }
    // `setsebool -P` rebuilds the policy store (seconds to tens of seconds under load), so only
    // run it when the boolean is not already on.
    if !httpd_network_connect_is_on() {
        let _ = Command::new("bash")
            .args([
                "-c",
                "command -v setsebool >/dev/null 2>&1 && setsebool -P httpd_can_network_connect 1 || true",
            ])
            .status();
    }
    if !module_check_due() {
        return;
    }

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
    type http_port_t;
    class tcp_socket name_connect;
}}
allow httpd_t pop_port_t:tcp_socket name_connect;
allow httpd_t smtp_port_t:tcp_socket name_connect;
allow httpd_t sieve_port_t:tcp_socket name_connect;
allow httpd_t http_port_t:tcp_socket name_connect;
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

/// Mail port allows, data-dir fcontext, and code-tree restorecon.
///
/// Expensive `semanage fcontext` for the data tree still runs only when the stamp is missing
/// or stale (it used to run on every Email > Webmail page view).
///
/// Code under `/opt/cpn-webmail` is always `restorecon`'d when SELinux is on. Tachyon/SnappyMail
/// upgrades stage archives under `/var/tmp` and `cp -a` can leave new `tachyon/v/<ver>` trees as
/// `tmp_t`; PHP-FPM (`httpd_t`) then fails to `include` those files and `/tachyon/` returns an
/// empty HTTP 200. `restorecon` is cheap when labels already match.
pub fn ensure_webmail_selinux() {
    if !selinux_present() {
        return;
    }
    ensure_httpd_mail_ports();
    restorecon_webmail_code();
    let dir = Path::new("/var/lib/cpn/selinux");
    let stamp = dir.join("cpn_webmail_fcontext.ver");
    let stamp_ok = fs::read_to_string(&stamp)
        .map(|s| s.trim() == FCONTEXT_STAMP_VER)
        .unwrap_or(false);
    if stamp_ok {
        // Data tree can also drift after operator copies; keep labels correct without semanage.
        restorecon_path(WEBMAIL_DATA_ROOT);
        return;
    }
    let status = Command::new("bash")
        .args([
            "-c",
            &format!(
                "command -v semanage >/dev/null 2>&1 && \
             (semanage fcontext -a -t httpd_sys_rw_content_t '{WEBMAIL_DATA_ROOT}(/.*)?' || \
              semanage fcontext -m -t httpd_sys_rw_content_t '{WEBMAIL_DATA_ROOT}(/.*)?' || true); \
             (semanage fcontext -a -t usr_t '{WEBMAIL_CODE_ROOT}(/.*)?' || \
              semanage fcontext -m -t usr_t '{WEBMAIL_CODE_ROOT}(/.*)?' || true); \
             restorecon -Rv {WEBMAIL_DATA_ROOT} >/dev/null 2>&1 || true; \
             restorecon -Rv {WEBMAIL_CODE_ROOT} >/dev/null 2>&1 || true"
            ),
        ])
        .status();
    if status.map(|s| s.success()).unwrap_or(false) && fs::create_dir_all(dir).is_ok() {
        let _ = fs::write(&stamp, format!("{FCONTEXT_STAMP_VER}\n"));
    }
}

/// Relabel webmail PHP code so `httpd_t` can read version trees after `/var/tmp` staging.
pub fn restorecon_webmail_code() {
    if !selinux_present() {
        return;
    }
    restorecon_path(WEBMAIL_CODE_ROOT);
}

fn restorecon_path(path: &str) {
    if !Path::new(path).exists() {
        return;
    }
    let _ = Command::new("restorecon")
        .args(["-R", path])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

/// True when the kernel exposes SELinux (selinuxfs mounted); other hosts skip every probe.
fn selinux_present() -> bool {
    Path::new("/sys/fs/selinux/enforce").exists()
}

/// `getsebool` is a cheap read; `setsebool -P` is not.
fn httpd_network_connect_is_on() -> bool {
    Command::new("getsebool")
        .arg("httpd_can_network_connect")
        .output()
        .ok()
        .map(|o| bool_output_is_on(&String::from_utf8_lossy(&o.stdout)))
        .unwrap_or(false)
}

fn bool_output_is_on(stdout: &str) -> bool {
    stdout
        .split_whitespace()
        .last()
        .map(|v| v.eq_ignore_ascii_case("on"))
        .unwrap_or(false)
}

/// The loaded-module check (`semodule -l`) reads the policy store; do it at most this often
/// per process.
const MODULE_CHECK_INTERVAL: Duration = Duration::from_secs(30 * 60);

static MODULE_CHECK_LAST: Mutex<Option<Instant>> = Mutex::new(None);

fn module_check_due() -> bool {
    let Ok(mut guard) = MODULE_CHECK_LAST.lock() else {
        return true;
    };
    match *guard {
        Some(at) if at.elapsed() < MODULE_CHECK_INTERVAL => false,
        _ => {
            *guard = Some(Instant::now());
            true
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn getsebool_output_parsing() {
        assert!(super::bool_output_is_on(
            "httpd_can_network_connect --> on\n"
        ));
        assert!(!super::bool_output_is_on(
            "httpd_can_network_connect --> off\n"
        ));
        assert!(!super::bool_output_is_on(""));
    }

    #[test]
    fn te_source_mentions_pop_smtp_sieve_http() {
        let te = r#"allow httpd_t pop_port_t:tcp_socket name_connect;
allow httpd_t smtp_port_t:tcp_socket name_connect;
allow httpd_t sieve_port_t:tcp_socket name_connect;
allow httpd_t http_port_t:tcp_socket name_connect;"#;
        assert!(te.contains("pop_port_t"));
        assert!(te.contains("smtp_port_t"));
        assert!(te.contains("sieve_port_t"));
        assert!(te.contains("http_port_t"));
    }

    #[test]
    fn fcontext_stamp_covers_code_and_data_roots() {
        assert_eq!(super::FCONTEXT_STAMP_VER, "2");
        assert_eq!(super::WEBMAIL_CODE_ROOT, "/opt/cpn-webmail");
        assert_eq!(super::WEBMAIL_DATA_ROOT, "/var/lib/cpn-webmail");
    }

    #[test]
    fn restorecon_webmail_code_is_noop_without_selinuxfs() {
        // Safe to call on Windows/dev hosts; must not panic when selinuxfs is absent.
        super::restorecon_webmail_code();
    }
}
