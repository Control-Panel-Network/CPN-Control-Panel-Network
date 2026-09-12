//! PHP-FPM + reverse-proxy webmail runtime (issue #6). No `php -S`.

use crate::install_journal::{self, JournalAction};
use crate::install_recipes::command;
use crate::install_webmail_proxy::{caddy_webmail_snippet, nginx_webmail_conf, ols_webmail_vhconf};
use crate::installer::{AppState, run_command};
use crate::model::ServerEngine;
use std::path::{Path, PathBuf};
use tokio::process::Command;

const STAGE: &str = "webmail_runtime";
const FPM_POOL: &str = "/etc/php-fpm.d/cpn-webmail.conf";
const NGINX_CONF: &str = "/etc/nginx/conf.d/cpn-webmail.conf";
const CADDY_SNIPPET: &str = "/etc/caddy/Caddyfile.d/cpn-webmail.caddy";
const WEBMAIL_URL: &str = "http://127.0.0.1:8080/";
/// SnappyMail application data outside the HTTP docroot.
pub const SNAPPYMAIL_DATA_DIR: &str = "/var/lib/cpn-webmail/snappymail/";

pub fn webmail_health_url() -> &'static str {
    WEBMAIL_URL
}

fn is_snappymail_docroot(docroot: &str) -> bool {
    docroot.contains("snappymail")
}

/// Ensure service user exists, then configure PHP-FPM + selected engine proxy.
pub async fn configure_webmail_runtime(
    state: &AppState,
    docroot: &str,
    engine: ServerEngine,
) -> Result<(), String> {
    install_journal::ensure_journal_dirs()?;
    ensure_webmail_user(state).await?;
    reset_current_link(Path::new(docroot))?;
    if is_snappymail_docroot(docroot) {
        configure_snappymail_external_data(docroot)?;
    }
    write_php_fpm_pool(docroot)?;
    harden_permissions(docroot).await?;
    // Stop legacy php -S unit only when the unit file exists (missing unit is not an error).
    let _ = Command::new("bash")
        .args([
            "-c",
            "if systemctl cat cpn-webmail >/dev/null 2>&1; then \
               systemctl disable --now cpn-webmail >/dev/null 2>&1 || true; \
               systemctl reset-failed cpn-webmail >/dev/null 2>&1 || true; \
             fi",
        ])
        .status()
        .await;
    if Path::new("/etc/systemd/system/cpn-webmail.service").exists() {
        let _ = std::fs::remove_file("/etc/systemd/system/cpn-webmail.service");
        install_journal::record(
            STAGE,
            JournalAction::Note,
            "/etc/systemd/system/cpn-webmail.service",
            None,
            Some("removed transitional php -S unit".into()),
        )?;
    }

    let _ = Command::new("bash")
        .args([
            "-c",
            "command -v semanage >/dev/null 2>&1 && \
             (semanage port -a -t http_port_t -p tcp 8080 || semanage port -m -t http_port_t -p tcp 8080 || true); \
             if [ -d /var/lib/cpn-webmail ]; then \
               (semanage fcontext -a -t httpd_sys_rw_content_t '/var/lib/cpn-webmail(/.*)?' || \
                semanage fcontext -m -t httpd_sys_rw_content_t '/var/lib/cpn-webmail(/.*)?' || true); \
               restorecon -Rv /var/lib/cpn-webmail >/dev/null 2>&1 || true; \
             fi",
        ])
        .status()
        .await;

    match engine {
        ServerEngine::Nginx => configure_nginx_proxy(docroot)?,
        ServerEngine::Caddy => configure_caddy_proxy(docroot)?,
        ServerEngine::Openlitespeed => configure_ols_proxy(docroot)?,
    }

    run_command(
        state,
        command(
            "systemctl",
            vec!["enable", "--now", "php-fpm"],
            "Enabling PHP-FPM for webmail",
            "installing",
            84,
        ),
    )
    .await?;
    run_command(
        state,
        command(
            "systemctl",
            vec!["restart", "php-fpm"],
            "Restarting PHP-FPM to load the webmail pool",
            "installing",
            85,
        ),
    )
    .await?;
    run_command(state, command("bash", vec!["-c", "for attempt in $(seq 1 15); do test -S /run/php-fpm/cpn-webmail.sock && exit 0; sleep 1; done; php-fpm -t 2>&1 || true; systemctl status php-fpm --no-pager 2>&1 || true; exit 1"], "Waiting for the webmail PHP-FPM socket", "testing", 86)).await.map_err(|_| "PHP-FPM did not create /run/php-fpm/cpn-webmail.sock; check installation.log for configuration diagnostics.".to_string())?;
    install_journal::record(STAGE, JournalAction::EnabledService, "php-fpm", None, None)?;

    match engine {
        ServerEngine::Nginx => {
            run_command(
                state,
                command(
                    "systemctl",
                    vec!["reload", "nginx"],
                    "Reloading Nginx (webmail)",
                    "installing",
                    86,
                ),
            )
            .await?;
        }
        ServerEngine::Caddy => {
            run_command(
                state,
                command(
                    "systemctl",
                    vec!["reload", "caddy"],
                    "Reloading Caddy (webmail)",
                    "installing",
                    86,
                ),
            )
            .await?;
        }
        ServerEngine::Openlitespeed => {
            let unit = if Path::new("/usr/lib/systemd/system/lshttpd.service").exists() {
                "lshttpd"
            } else {
                "lsws"
            };
            let _ = Command::new("systemctl")
                .args(["reload", unit])
                .status()
                .await;
            let _ = Command::new("/usr/local/lsws/bin/lswsctrl")
                .args(["restart"])
                .status()
                .await;
        }
    }

    verify_code_not_writable_by_service(docroot)?;
    Ok(())
}

fn configure_snappymail_external_data(docroot: &str) -> Result<(), String> {
    std::fs::create_dir_all(SNAPPYMAIL_DATA_DIR).map_err(|error| error.to_string())?;
    // Prefer include.php APP_DATA_FOLDER_PATH (official SnappyMail hardening).
    let include = format!(
        "<?php\n\
         // Managed by CPN: keep application data outside the HTTP docroot.\n\
         define('APP_DATA_FOLDER_PATH', '{SNAPPYMAIL_DATA_DIR}');\n"
    );
    let include_path = Path::new(docroot).join("include.php");
    install_journal::write_file_tracked(STAGE, &include_path, &include)?;
    // Remove any in-docroot data tree so it cannot be served.
    let web_data = Path::new(docroot).join("data");
    if web_data.exists() {
        let _ = std::fs::remove_dir_all(&web_data);
    }
    install_journal::record(
        STAGE,
        JournalAction::Note,
        SNAPPYMAIL_DATA_DIR,
        None,
        Some("SnappyMail APP_DATA_FOLDER_PATH outside docroot".into()),
    )?;
    Ok(())
}

async fn ensure_webmail_user(state: &AppState) -> Result<(), String> {
    let user_exists = Command::new("id")
        .args(["-u", "cpn-webmail"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .await
        .is_ok_and(|status| status.success());
    if user_exists {
        return Ok(());
    }
    run_command(
        state,
        command(
            "useradd",
            vec![
                "--system",
                "--home-dir",
                "/opt/cpn-webmail",
                "--shell",
                "/sbin/nologin",
                "cpn-webmail",
            ],
            "Creating the isolated webmail user",
            "installing",
            80,
        ),
    )
    .await
}

fn reset_current_link(target: &Path) -> Result<(), String> {
    let current = Path::new("/opt/cpn-webmail/current");
    if current.symlink_metadata().is_ok() {
        std::fs::remove_file(current).map_err(|error| error.to_string())?;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        symlink(target, current).map_err(|error| format!("Failed to activate webmail: {error}"))
    }
    #[cfg(not(unix))]
    {
        let _ = target;
        Err("Webmail symlink activation requires a Unix host".into())
    }
}

fn write_php_fpm_pool(docroot: &str) -> Result<(), String> {
    let open_basedir = if is_snappymail_docroot(docroot) {
        "/opt/cpn-webmail:/var/lib/cpn-webmail:/tmp"
    } else {
        "/opt/cpn-webmail:/tmp"
    };
    let pool = format!(
        "[cpn-webmail]\n\
         user = cpn-webmail\n\
         group = cpn-webmail\n\
         listen = /run/php-fpm/cpn-webmail.sock\n\
         listen.owner = root\n\
         listen.group = root\n\
         listen.mode = 0666\n\
         pm = ondemand\n\
         pm.max_children = 8\n\
         pm.process_idle_timeout = 10s\n\
         chdir = {docroot}\n\
         security.limit_extensions = .php\n\
         php_admin_value[open_basedir] = {open_basedir}\n\
         php_admin_flag[allow_url_fopen] = off\n"
    );
    install_journal::write_file_tracked(STAGE, Path::new(FPM_POOL), &pool)?;
    let _ = std::process::Command::new("restorecon")
        .args(["-v", FPM_POOL])
        .status();
    Ok(())
}

async fn harden_permissions(docroot: &str) -> Result<(), String> {
    let snappy = if is_snappymail_docroot(docroot) {
        format!(
            "mkdir -p {SNAPPYMAIL_DATA_DIR} && \
             chown -R cpn-webmail:cpn-webmail /var/lib/cpn-webmail && \
             find /var/lib/cpn-webmail -type d -exec chmod 750 {{}} + && \
             find /var/lib/cpn-webmail -type f -exec chmod 640 {{}} + && \
             rm -rf {docroot}/data {docroot}/temp {docroot}/logs 2>/dev/null || true"
        )
    } else {
        format!(
            "mkdir -p {docroot}/data {docroot}/temp {docroot}/logs \
               /opt/cpn-webmail/roundcube/temp /opt/cpn-webmail/roundcube/logs && \
             chown -R cpn-webmail:cpn-webmail {docroot}/data {docroot}/temp {docroot}/logs \
               /opt/cpn-webmail/roundcube/temp /opt/cpn-webmail/roundcube/logs 2>/dev/null || true"
        )
    };
    let script = format!(
        "chown -R root:root /opt/cpn-webmail && \
         find /opt/cpn-webmail -type d -exec chmod 755 {{}} + && \
         find /opt/cpn-webmail -type f -exec chmod 644 {{}} + && \
         {snappy} && \
         if [ -f /opt/cpn-webmail/roundcube/db.sqlite ]; then \
           chown cpn-webmail:cpn-webmail /opt/cpn-webmail/roundcube/db.sqlite; \
           chmod 0600 /opt/cpn-webmail/roundcube/db.sqlite; \
         fi && \
         if [ -d /opt/cpn-webmail/roundcube/config ]; then \
           chown -R root:cpn-webmail /opt/cpn-webmail/roundcube/config; \
           chmod 750 /opt/cpn-webmail/roundcube/config; \
           find /opt/cpn-webmail/roundcube/config -type f -name '*.php' -exec chmod 640 {{}} +; \
         fi"
    );
    let status = Command::new("bash")
        .args(["-c", &script])
        .kill_on_drop(true)
        .status()
        .await
        .map_err(|error| error.to_string())?;
    if !status.success() {
        return Err("Failed to harden webmail permissions".into());
    }
    install_journal::record(
        STAGE,
        JournalAction::Note,
        "/opt/cpn-webmail",
        None,
        Some("root-owned code; writable runtime data outside or denied under docroot".into()),
    )?;
    Ok(())
}

fn configure_nginx_proxy(docroot: &str) -> Result<(), String> {
    install_journal::write_file_tracked(
        STAGE,
        Path::new(NGINX_CONF),
        &nginx_webmail_conf(docroot),
    )?;
    Ok(())
}

/// Rewrite loopback Nginx/Caddy/OLS webmail config when the active client docroot drifted
/// (example: Roundcube root while SnappyMail is preferred) or PATH_INFO support is missing.
pub fn heal_webmail_loopback_config() -> Result<(), String> {
    let Some(docroot) = crate::panel_webmail::webmail_backend_docroot() else {
        return Ok(());
    };
    if Path::new(NGINX_CONF).is_file() {
        let raw = std::fs::read_to_string(NGINX_CONF).unwrap_or_default();
        let needs = !raw.contains(docroot)
            || !raw.contains("fastcgi_split_path_info")
            || !raw.contains("PATH_INFO");
        if needs {
            configure_nginx_proxy(docroot)?;
            let _ = std::process::Command::new("systemctl")
                .args(["reload", "nginx"])
                .status();
        }
    }
    if Path::new(CADDY_SNIPPET).is_file() {
        let raw = std::fs::read_to_string(CADDY_SNIPPET).unwrap_or_default();
        if !raw.contains(docroot) {
            configure_caddy_proxy(docroot)?;
            let _ = std::process::Command::new("systemctl")
                .args(["reload", "caddy"])
                .status();
        }
    }
    // SnappyMail data lives under /var/lib/cpn-webmail; stale pools that omit it show
    // "Permission denied!" instead of the login form.
    if is_snappymail_docroot(docroot) {
        if Path::new(FPM_POOL).is_file() {
            let raw = std::fs::read_to_string(FPM_POOL).unwrap_or_default();
            if !raw.contains("/var/lib/cpn-webmail") {
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
    }
    Ok(())
}

fn configure_caddy_proxy(docroot: &str) -> Result<(), String> {
    std::fs::create_dir_all("/etc/caddy/Caddyfile.d").map_err(|error| error.to_string())?;
    install_journal::write_file_tracked(
        STAGE,
        Path::new(CADDY_SNIPPET),
        &caddy_webmail_snippet(docroot),
    )?;

    let main = Path::new("/etc/caddy/Caddyfile");
    let import_line = "import /etc/caddy/Caddyfile.d/*.caddy\n";
    let mut body = if main.exists() {
        std::fs::read_to_string(main).unwrap_or_default()
    } else {
        String::new()
    };
    if !body.contains("Caddyfile.d/*.caddy") {
        if !body.is_empty() && !body.ends_with('\n') {
            body.push('\n');
        }
        body.push_str(import_line);
        install_journal::write_file_tracked(STAGE, main, &body)?;
    }
    Ok(())
}

fn configure_ols_proxy(docroot: &str) -> Result<(), String> {
    let vh_dir = "/usr/local/lsws/conf/vhosts/CPNWebmail";
    std::fs::create_dir_all(vh_dir).map_err(|error| error.to_string())?;
    let vhconf_path_buf = PathBuf::from(format!("{vh_dir}/vhconf.conf"));
    install_journal::write_file_tracked(STAGE, &vhconf_path_buf, &ols_webmail_vhconf(docroot))?;
    if !vhconf_path_buf.exists() {
        return Err(format!(
            "OpenLiteSpeed rejected {vh_dir}/vhconf.conf (file missing after write)"
        ));
    }

    let httpd = "/usr/local/lsws/conf/httpd_config.conf";
    let mut conf = std::fs::read_to_string(httpd).unwrap_or_default();
    if !conf.contains("virtualHost CPNWebmail") {
        conf.push_str(
            "\nvirtualHost CPNWebmail {\n  vhRoot                  /opt/cpn-webmail/\n  configFile              $SERVER_ROOT/conf/vhosts/CPNWebmail/vhconf.conf\n  allowSymbolLink         1\n  enableScript            1\n  restrained              1\n}\n\nlistener CPNWebmailHttp {\n  address                 127.0.0.1:8080\n  secure                  0\n  map                     CPNWebmail *\n}\n",
        );
        install_journal::write_file_tracked(STAGE, Path::new(httpd), &conf)?;
    }
    Ok(())
}

/// Fail if PHP/code paths under docroot are writable by cpn-webmail.
pub fn verify_code_not_writable_by_service(docroot: &str) -> Result<(), String> {
    let runtime_check = if is_snappymail_docroot(docroot) {
        format!(
            "su -s /bin/bash cpn-webmail -c \"test -w '{SNAPPYMAIL_DATA_DIR}'\" || {{ echo 'snappy data not writable'; exit 1; }}\n\
             if [ -e '{docroot}/data' ]; then echo 'docroot data must not exist for SnappyMail'; exit 1; fi\n"
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_url_is_loopback_8080() {
        assert_eq!(webmail_health_url(), "http://127.0.0.1:8080/");
    }

    #[test]
    fn pool_config_uses_unix_socket() {
        let sample = "listen = /run/php-fpm/cpn-webmail.sock";
        assert!(sample.contains("cpn-webmail.sock"));
        assert!(!sample.contains("php -S"));
    }
}
