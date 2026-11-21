//! Heal loopback webmail listeners/docroot after client switches or port conflicts.

use crate::install_webmail_engines::{
    caddy_snippet_path, configure_caddy_proxy, configure_nginx_proxy, configure_ols_proxy,
    nginx_conf_path,
};
use crate::install_webmail_runtime::{
    FPM_POOL, configure_snappymail_external_data, ensure_snappymail_local_imap_defaults,
    is_snappy_lineage_docroot, snappy_lineage_data_dir, write_php_fpm_pool,
};
use crate::webmail_loopback;
use std::path::Path;

/// Rewrite loopback Nginx/Caddy/OLS webmail config when the active client docroot drifted
/// (example: Roundcube root while SnappyMail is preferred) or PATH_INFO support is missing.
/// Also relocates the loopback listen port when a foreign process owns the preferred port.
pub fn heal_webmail_loopback_config() -> Result<(), String> {
    let Some(docroot) = crate::panel_webmail::webmail_backend_docroot() else {
        return Ok(());
    };
    let previous = webmail_loopback::loopback_port();
    let port = webmail_loopback::ensure_loopback_port()?;
    let port_changed = port != previous;
    let nginx_conf = Path::new(nginx_conf_path());
    if nginx_conf.is_file() {
        let raw = std::fs::read_to_string(nginx_conf).unwrap_or_default();
        let needs = port_changed
            || !raw.contains(docroot)
            || !raw.contains(&format!("127.0.0.1:{port}"))
            || !raw.contains("fastcgi_split_path_info")
            || !raw.contains("PATH_INFO");
        if needs {
            configure_nginx_proxy(docroot, port)?;
            let _ = std::process::Command::new("systemctl")
                .args(["reload", "nginx"])
                .status();
        }
    }
    let caddy_snippet = Path::new(caddy_snippet_path());
    if caddy_snippet.is_file() {
        let raw = std::fs::read_to_string(caddy_snippet).unwrap_or_default();
        if port_changed || !raw.contains(docroot) || !raw.contains(&format!("127.0.0.1:{port}")) {
            configure_caddy_proxy(docroot, port)?;
            let _ = std::process::Command::new("systemctl")
                .args(["reload", "caddy"])
                .status();
        }
    }
    let httpd = Path::new("/usr/local/lsws/conf/httpd_config.conf");
    if httpd.is_file() {
        let raw = std::fs::read_to_string(httpd).unwrap_or_default();
        let needs_ols = port_changed
            || (raw.contains("listener CPNWebmailHttp")
                && !raw.contains(&format!("127.0.0.1:{port}")));
        if needs_ols {
            configure_ols_proxy(docroot, port)?;
            let _ = std::process::Command::new("/usr/local/lsws/bin/lswsctrl")
                .arg("restart")
                .status();
        }
    }
    // SnappyMail data lives under /var/lib/cpn-webmail; stale pools that omit it show
    // "Permission denied!" instead of the login form.
    if is_snappy_lineage_docroot(docroot) {
        if Path::new(FPM_POOL).is_file() {
            let raw = std::fs::read_to_string(FPM_POOL).unwrap_or_default();
            if !raw.contains("/var/lib/cpn-webmail")
                || !raw.contains("default_socket_timeout")
                || !raw.contains(":/proc")
            {
                write_php_fpm_pool(docroot)?;
                let _ = std::process::Command::new("systemctl")
                    .args(["restart", "php-fpm"])
                    .status();
            }
        }
        let include_path = Path::new(docroot).join("include.php");
        let include_ok = include_path.is_file()
            && std::fs::read_to_string(&include_path)
                .unwrap_or_default()
                .contains("APP_DATA_FOLDER_PATH");
        if !include_ok {
            let _ = configure_snappymail_external_data(docroot);
        }
        let _ = ensure_snappymail_local_imap_defaults();
        let _ = crate::install_mail_sieve::ensure_dovecot_sieve_sync();
        let _ = crate::install_snappymail_prefs::ensure_snappymail_operator_defaults();
        let _ = crate::install_snappymail_repo::ensure_snappymail_repo_fallback(docroot);
        crate::install_selinux_mail::ensure_webmail_selinux();
    } else if docroot.contains("roundcube") {
        let _ = crate::install_webmail_openpgp::ensure_webmail_openpgp_defaults();
    }
    Ok(())
}

/// Fail if PHP/code paths under docroot are writable by cpn-webmail.
pub fn verify_code_not_writable_by_service(docroot: &str) -> Result<(), String> {
    let runtime_check = if is_snappy_lineage_docroot(docroot) {
        let data_dir = snappy_lineage_data_dir(docroot);
        format!(
            "su -s /bin/bash cpn-webmail -c \"test -w '{data_dir}'\" || {{ echo 'webmail data not writable'; exit 1; }}\n\
             if [ -e '{docroot}/data' ]; then echo 'docroot data must not exist for SnappyMail/Tachyon'; exit 1; fi\n"
        )
    } else {
        format!(
            "for d in '{docroot}/data' '{docroot}/temp' '{docroot}/logs'; do\n\
               if [ -d \"$d\" ]; then\n\
                 su -s /bin/bash cpn-webmail -c \"test -w '$d'\" || {{ echo \"not writable: $d\"; exit 1; }}\n\
               fi\n\
             done\n"
        )
    };
    let script = format!(
        "set -euo pipefail\n\
         bad=$(find '{docroot}' \\( -path '{docroot}/data' -o -path '{docroot}/temp' -o -path '{docroot}/logs' \\) -prune -o -type f -name '*.php' ! -user root -print -quit 2>/dev/null || true)\n\
         if [ -n \"$bad\" ]; then echo \"php not root-owned: $bad\"; exit 1; fi\n\
         owner=$(stat -c '%U' /opt/cpn-webmail)\n\
         if [ \"$owner\" = \"cpn-webmail\" ]; then echo '/opt/cpn-webmail owned by service user'; exit 1; fi\n\
         {runtime_check}\
         if su -s /bin/bash cpn-webmail -c \"touch '{docroot}/.__cpn_perm_probe.php'\" 2>/dev/null; then\n\
           rm -f '{docroot}/.__cpn_perm_probe.php'\n\
           echo 'service user can write PHP into docroot'; exit 1\n\
         fi\n\
         exit 0\n"
    );
    let status = std::process::Command::new("bash")
        .args(["-c", &script])
        .status()
        .map_err(|error| error.to_string())?;
    if !status.success() {
        return Err(
            "Webmail permission check failed: code must be root-owned and non-writable by cpn-webmail"
                .into(),
        );
    }
    Ok(())
}
