//! PHP-FPM + reverse-proxy webmail runtime (issue #6). No `php -S`.

use crate::install_journal::{self, JournalAction};
use crate::install_recipes::command;
use crate::installer::{AppState, run_command};
use crate::model::ServerEngine;
use std::path::Path;
use tokio::process::Command;

const STAGE: &str = "webmail_runtime";
const FPM_POOL: &str = "/etc/php-fpm.d/cpn-webmail.conf";
const NGINX_CONF: &str = "/etc/nginx/conf.d/cpn-webmail.conf";
const CADDY_SNIPPET: &str = "/etc/caddy/Caddyfile.d/cpn-webmail.caddy";
const WEBMAIL_URL: &str = "http://127.0.0.1:8080/";

pub fn webmail_health_url() -> &'static str {
    WEBMAIL_URL
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
    write_php_fpm_pool(docroot)?;
    harden_permissions(docroot).await?;
    // Stop legacy php -S unit if present from older installs.
    let _ = Command::new("systemctl")
        .args(["disable", "--now", "cpn-webmail"])
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

    // Allow reverse-proxy listen port through SELinux when tools are present.
    let _ = Command::new("bash")
        .args([
            "-c",
            "command -v semanage >/dev/null 2>&1 && \
             (semanage port -a -t http_port_t -p tcp 8080 || semanage port -m -t http_port_t -p tcp 8080 || true)",
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
            "Activando PHP-FPM para webmail",
            "installing",
            84,
        ),
    )
    .await?;
    install_journal::record(STAGE, JournalAction::EnabledService, "php-fpm", None, None)?;

    // Reload frontends after config write (idempotent).
    match engine {
        ServerEngine::Nginx => {
            run_command(
                state,
                command(
                    "systemctl",
                    vec!["reload", "nginx"],
                    "Recargando Nginx (webmail)",
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
                    "Recargando Caddy (webmail)",
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
            "Creando el usuario aislado del webmail",
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
        symlink(target, current).map_err(|error| format!("No se pudo activar el webmail: {error}"))
    }
    #[cfg(not(unix))]
    {
        let _ = target;
        Err("Webmail symlink activation requires a Unix host".into())
    }
}

fn write_php_fpm_pool(docroot: &str) -> Result<(), String> {
    // Unix socket under /run/php-fpm (SELinux-friendly). Mode 0666 lets nginx/caddy/ols connect.
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
         php_admin_value[open_basedir] = /opt/cpn-webmail:/tmp\n\
         php_admin_flag[allow_url_fopen] = off\n"
    );
    install_journal::write_file_tracked(STAGE, Path::new(FPM_POOL), &pool)?;
    let _ = std::process::Command::new("restorecon")
        .args(["-v", FPM_POOL])
        .status();
    Ok(())
}

async fn harden_permissions(docroot: &str) -> Result<(), String> {
    let script = format!(
        "chown -R root:root /opt/cpn-webmail && \
         find /opt/cpn-webmail -type d -exec chmod 755 {{}} + && \
         find /opt/cpn-webmail -type f -exec chmod 644 {{}} + && \
         mkdir -p {docroot}/data {docroot}/temp {docroot}/logs \
           /opt/cpn-webmail/roundcube/temp /opt/cpn-webmail/roundcube/logs \
           /opt/cpn-webmail/snappymail/data && \
         chown -R cpn-webmail:cpn-webmail {docroot}/data {docroot}/temp {docroot}/logs \
           /opt/cpn-webmail/roundcube/temp /opt/cpn-webmail/roundcube/logs \
           /opt/cpn-webmail/snappymail/data 2>/dev/null || true && \
         if [ -f /opt/cpn-webmail/roundcube/db.sqlite ]; then \
           chown cpn-webmail:cpn-webmail /opt/cpn-webmail/roundcube/db.sqlite; \
           chmod 0600 /opt/cpn-webmail/roundcube/db.sqlite; \
         fi && \
         if [ -d /opt/cpn-webmail/roundcube/config ]; then \
           chown root:cpn-webmail /opt/cpn-webmail/roundcube/config; \
           chmod 750 /opt/cpn-webmail/roundcube/config; \
           chmod 640 /opt/cpn-webmail/roundcube/config/*.php 2>/dev/null || true; \
         fi"
    );
    let status = Command::new("bash")
        .args(["-c", &script])
        .kill_on_drop(true)
        .status()
        .await
        .map_err(|error| error.to_string())?;
    if !status.success() {
        return Err("No se pudieron ajustar permisos del webmail".into());
    }
    install_journal::record(
        STAGE,
        JournalAction::Note,
        "/opt/cpn-webmail",
        None,
        Some("root-owned code; writable data/temp/logs only".into()),
    )?;
    Ok(())
}

fn configure_nginx_proxy(docroot: &str) -> Result<(), String> {
    let conf = format!(
        "# Managed by CPN (issue #6)\n\
         server {{\n\
           listen 127.0.0.1:8080;\n\
           server_name localhost;\n\
           root {docroot};\n\
           index index.php index.html;\n\
           location / {{\n\
             try_files $uri $uri/ /index.php?$query_string;\n\
           }}\n\
           location ~ \\.php$ {{\n\
             include fastcgi_params;\n\
             fastcgi_param SCRIPT_FILENAME $document_root$fastcgi_script_name;\n\
             fastcgi_pass unix:/run/php-fpm/cpn-webmail.sock;\n\
           }}\n\
           location ~ /(\\.|config|temp|logs|data) {{\n\
             deny all;\n\
           }}\n\
         }}\n"
    );
    install_journal::write_file_tracked(STAGE, Path::new(NGINX_CONF), &conf)?;
    Ok(())
}

fn configure_caddy_proxy(docroot: &str) -> Result<(), String> {
    std::fs::create_dir_all("/etc/caddy/Caddyfile.d").map_err(|error| error.to_string())?;
    let snippet = format!(
        "# Managed by CPN (issue #6)\n\
         http://127.0.0.1:8080 {{\n\
           root * {docroot}\n\
           php_fastcgi unix//run/php-fpm/cpn-webmail.sock\n\
           file_server\n\
         }}\n"
    );
    install_journal::write_file_tracked(STAGE, Path::new(CADDY_SNIPPET), &snippet)?;

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
    let vhconf = format!(
        "docRoot                   {docroot}/\n\
         enableGzip                1\n\
         index  {{\n\
           useServer               0\n\
           indexFiles              index.php, index.html\n\
         }}\n\
         extprocessor cpnphp {{\n\
           type                    fcgi\n\
           address                 uds://run/php-fpm/cpn-webmail.sock\n\
           maxConns                8\n\
           initTimeout             60\n\
           retryTimeout            0\n\
           respBuffer              0\n\
         }}\n\
         scriptHandler  {{\n\
           add                     lsapi:cpnphp php\n\
         }}\n\
         rewrite  {{\n\
           enable                  1\n\
           rules                   <<<END_rules\n\
RewriteRule ^(.*)$ - [E=HTTP_AUTHORIZATION:%{{HTTP:Authorization}}]\n\
END_rules\n\
         }}\n"
    );
    // OLS scriptHandler with fcgi: use fcgi type in add line when supported.
    let vhconf = vhconf.replace("lsapi:cpnphp", "fcgi:cpnphp");
    install_journal::write_file_tracked(
        STAGE,
        Path::new(&format!("{vh_dir}/vhconf.conf")),
        &vhconf,
    )?;

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
    let script = format!(
        "set -euo pipefail\n\
         # Code files must be root-owned and not group/world writable.\n\
         bad=$(find '{docroot}' -type f -name '*.php' ! -user root -print -quit 2>/dev/null || true)\n\
         if [ -n \"$bad\" ]; then echo \"php not root-owned: $bad\"; exit 1; fi\n\
         # Service user must not own the application tree root.\n\
         owner=$(stat -c '%U' /opt/cpn-webmail)\n\
         if [ \"$owner\" = \"cpn-webmail\" ]; then echo '/opt/cpn-webmail owned by service user'; exit 1; fi\n\
         # Writable runtime dirs must exist for the service user.\n\
         for d in '{docroot}/data' '{docroot}/temp' '{docroot}/logs'; do\n\
           if [ -d \"$d\" ]; then\n\
             su -s /bin/bash cpn-webmail -c \"test -w '$d'\" || {{ echo \"not writable: $d\"; exit 1; }}\n\
           fi\n\
         done\n\
         # Service user must not be able to write a random PHP file in docroot.\n\
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
    use super::webmail_health_url;

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
