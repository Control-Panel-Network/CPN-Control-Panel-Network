//! PHP-FPM + reverse-proxy webmail runtime (issue #6). No `php -S`.

use crate::install_journal::{self, JournalAction};
use crate::install_recipes::command;
use crate::install_webmail_engines::{
    configure_caddy_proxy, configure_nginx_proxy, configure_ols_proxy,
};
use crate::installer::{AppState, run_command};
use crate::model::ServerEngine;
use crate::webmail_loopback;
use std::path::Path;
use tokio::process::Command;

const STAGE: &str = "webmail_runtime";
pub(crate) const FPM_POOL: &str = "/etc/php-fpm.d/cpn-webmail.conf";
/// SnappyMail application data outside the HTTP docroot.
pub const SNAPPYMAIL_DATA_DIR: &str = "/var/lib/cpn-webmail/snappymail/";

pub use crate::install_webmail_heal::{
    heal_webmail_loopback_config, verify_code_not_writable_by_service,
};

pub fn webmail_health_url() -> String {
    webmail_loopback::loopback_base_url()
}

pub(crate) fn is_snappy_lineage_docroot(docroot: &str) -> bool {
    docroot.contains("snappymail") || docroot.contains("tachyon")
}

/// SnappyMail / Tachyon application data outside the HTTP docroot.
pub fn snappy_lineage_data_dir(docroot: &str) -> &'static str {
    if docroot.contains("tachyon") {
        "/var/lib/cpn-webmail/tachyon/"
    } else {
        SNAPPYMAIL_DATA_DIR
    }
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
    if is_snappy_lineage_docroot(docroot) {
        configure_snappymail_external_data(docroot)?;
        let _ = crate::install_snappymail_repo::ensure_snappymail_repo_fallback(docroot);
        let _ = crate::install_snappymail_i18n::ensure_notification_count_placeholders();
    } else if docroot.contains("roundcube") {
        let _ = crate::install_webmail_openpgp::ensure_webmail_openpgp_defaults();
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

    let port = webmail_loopback::ensure_loopback_port()?;
    let semanage = format!(
        "command -v semanage >/dev/null 2>&1 && \
         (semanage port -a -t http_port_t -p tcp {port} || semanage port -m -t http_port_t -p tcp {port} || true); \
         if [ -d /var/lib/cpn-webmail ]; then \
           (semanage fcontext -a -t httpd_sys_rw_content_t '/var/lib/cpn-webmail(/.*)?' || \
            semanage fcontext -m -t httpd_sys_rw_content_t '/var/lib/cpn-webmail(/.*)?' || true); \
           restorecon -Rv /var/lib/cpn-webmail >/dev/null 2>&1 || true; \
         fi"
    );
    let _ = Command::new("bash").args(["-c", &semanage]).status().await;

    match engine {
        ServerEngine::Nginx => configure_nginx_proxy(docroot, port)?,
        ServerEngine::Caddy => configure_caddy_proxy(docroot, port)?,
        ServerEngine::Openlitespeed => configure_ols_proxy(docroot, port)?,
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

pub(crate) fn configure_snappymail_external_data(docroot: &str) -> Result<(), String> {
    let data_dir = snappy_lineage_data_dir(docroot);
    std::fs::create_dir_all(data_dir).map_err(|error| error.to_string())?;
    // Prefer include.php APP_DATA_FOLDER_PATH (official SnappyMail / Tachyon hardening).
    let include = format!(
        "<?php\n\
         // Managed by CPN: keep application data outside the HTTP docroot.\n\
         define('APP_DATA_FOLDER_PATH', '{data_dir}');\n"
    );
    let include_path = Path::new(docroot).join("include.php");
    install_journal::write_file_tracked(STAGE, &include_path, &include)?;
    // Remove any in-docroot data tree so it cannot be served.
    let web_data = Path::new(docroot).join("data");
    if web_data.exists() {
        let _ = std::fs::remove_dir_all(&web_data);
    }
    let _ = ensure_snappy_lineage_local_imap_defaults(data_dir);
    if is_snappy_lineage_docroot(docroot) {
        let _ = crate::install_snappymail_prefs::ensure_snappymail_operator_defaults();
    }
    install_journal::record(
        STAGE,
        JournalAction::Note,
        data_dir,
        None,
        Some("Webmail APP_DATA_FOLDER_PATH outside docroot".into()),
    )?;
    Ok(())
}

/// Point default + local host domain at 127.0.0.1 IMAP.
/// `shortLogin` stays on for PAM/system mailboxes and off for classic virtual-mail SQL.
pub fn ensure_snappymail_local_imap_defaults() -> Result<(), String> {
    for data_dir in crate::install_snappymail_lineage::lineage_data_dirs() {
        let _ = ensure_snappy_lineage_local_imap_defaults(&data_dir);
    }
    // Always try the canonical SnappyMail path even if the dir was just created.
    ensure_snappy_lineage_local_imap_defaults(SNAPPYMAIL_DATA_DIR)
}

fn ensure_snappy_lineage_local_imap_defaults(data_dir: &str) -> Result<(), String> {
    let domains = Path::new(data_dir).join("_data_/_default_/domains");
    if !domains.is_dir() {
        return Ok(());
    }
    let default_path = domains.join("default.json");
    if !default_path.is_file() {
        return Ok(());
    }
    let raw = std::fs::read_to_string(&default_path).map_err(|e| e.to_string())?;
    let Ok(mut data) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return Ok(());
    };
    let short_login = crate::mail_legacy_sql::prefer_webmail_short_login();
    for key in ["IMAP", "SMTP"] {
        if let Some(obj) = data.get_mut(key).and_then(|v| v.as_object_mut()) {
            obj.insert("host".into(), serde_json::Value::String("127.0.0.1".into()));
            obj.insert("shortLogin".into(), serde_json::Value::Bool(short_login));
            obj.insert("sasl".into(), serde_json::json!(["PLAIN", "LOGIN"]));
        }
    }
    if let Some(imap) = data.get_mut("IMAP").and_then(|v| v.as_object_mut()) {
        imap.insert("port".into(), serde_json::json!(143));
    }
    if let Some(smtp) = data.get_mut("SMTP").and_then(|v| v.as_object_mut()) {
        smtp.insert("port".into(), serde_json::json!(25));
    }
    // Enable ManageSieve so Filters work in SnappyMail/Tachyon.
    {
        let sieve = data.as_object_mut().map(|o| {
            o.entry("Sieve".to_string())
                .or_insert_with(|| serde_json::Value::Object(Default::default()))
        });
        if let Some(serde_json::Value::Object(obj)) = sieve {
            obj.insert("enabled".into(), serde_json::Value::Bool(true));
            obj.insert("host".into(), serde_json::Value::String("127.0.0.1".into()));
            obj.insert("port".into(), serde_json::json!(4190));
            obj.insert("shortLogin".into(), serde_json::Value::Bool(short_login));
            obj.insert("lowerLogin".into(), serde_json::Value::Bool(true));
            obj.insert("sasl".into(), serde_json::json!(["PLAIN", "LOGIN"]));
        }
    }
    // Never leave Sieve.shortLogin false when IMAP shortLogin is true.
    let _ = crate::install_snappymail_prefs::align_domain_sieve_short_login(&mut data);
    let pretty = serde_json::to_string_pretty(&data).map_err(|e| e.to_string())?;
    std::fs::write(&default_path, format!("{pretty}\n")).map_err(|e| e.to_string())?;
    // Mirror onto hostname and any existing local domain files (skip public providers).
    let mut mirror_names: Vec<String> = Vec::new();
    if let Ok(hostname) = std::fs::read_to_string("/etc/hostname") {
        let host = hostname.trim().to_ascii_lowercase();
        if !host.is_empty() {
            mirror_names.push(format!("{host}.json"));
        }
    }
    if let Ok(entries) = std::fs::read_dir(&domains) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let lower = name.to_ascii_lowercase();
            if !lower.ends_with(".json") || lower == "default.json" {
                continue;
            }
            if lower == "gmail.com.json" || lower == "hotmail.com.json" || lower == "yahoo.com.json"
            {
                continue;
            }
            if !mirror_names.iter().any(|n| n.eq_ignore_ascii_case(&name)) {
                mirror_names.push(name);
            }
        }
    }
    // Always provision apex + mail.<apex> profiles (Filters/IMAP auth hostnames).
    let apex_domains = crate::panel_ops_mail_onboarding::mail_client_fallback_domains();
    crate::install_snappymail_prefs::append_mail_and_apex_domain_profiles(
        &mut mirror_names,
        &apex_domains,
    );
    // Always keep a newstargeted.com domain profile for lab/production mailboxes.
    crate::install_snappymail_prefs::append_mail_and_apex_domain_profiles(
        &mut mirror_names,
        &["newstargeted.com".into()],
    );
    for name in mirror_names {
        let host_path = domains.join(name);
        let _ = std::fs::write(&host_path, format!("{pretty}\n"));
    }
    // Soften SameSite for panel reverse-proxy labs (host:port) and scope cookies
    // to the panel mount so CyberPanel Path=/ smtoken leftovers cannot collide.
    let ini = Path::new(data_dir).join("_data_/_default_/configs/application.ini");
    if let Ok(ini_raw) = std::fs::read_to_string(&ini) {
        let mut updated =
            ini_raw.replace("cookie_samesite = \"Strict\"", "cookie_samesite = \"Lax\"");
        let mount = crate::panel_webmail::load_webmail_config()
            .public_path
            .trim_end_matches('/')
            .to_string();
        if !mount.is_empty() && mount != "/" {
            updated = crate::install_snappymail_lineage::replace_ini_quoted(
                &updated,
                "cookie_default_path",
                &mount,
            );
            updated = crate::install_snappymail_lineage::replace_ini_bool(
                &updated,
                "cookie_default_secure",
                false,
            );
        }
        if updated != ini_raw {
            let _ = std::fs::write(&ini, updated);
        }
    }
    let _ = std::process::Command::new("chown")
        .args([
            "-R",
            "cpn-webmail:cpn-webmail",
            &format!("{data_dir}_data_"),
        ])
        .status();
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

pub(crate) fn write_php_fpm_pool(docroot: &str) -> Result<(), String> {
    // Include /proc so sys_getloadavg()/Admin About system load can read loadavg
    // when PHP enforces open_basedir on /proc (varies by PHP build).
    let open_basedir = if is_snappy_lineage_docroot(docroot) {
        "/opt/cpn-webmail:/var/lib/cpn-webmail:/tmp:/proc"
    } else {
        "/opt/cpn-webmail:/tmp:/proc"
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
         php_admin_value[default_socket_timeout] = 8\n\
         php_admin_flag[allow_url_fopen] = off\n"
    );
    install_journal::write_file_tracked(STAGE, Path::new(FPM_POOL), &pool)?;
    let _ = std::process::Command::new("restorecon")
        .args(["-v", FPM_POOL])
        .status();
    Ok(())
}

async fn harden_permissions(docroot: &str) -> Result<(), String> {
    let snappy = if is_snappy_lineage_docroot(docroot) {
        let data_dir = snappy_lineage_data_dir(docroot);
        format!(
            "mkdir -p {data_dir} && \
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_url_is_loopback_default() {
        assert_eq!(
            webmail_health_url(),
            format!("http://127.0.0.1:{}/", webmail_loopback::DEFAULT_PORT)
        );
    }

    #[test]
    fn pool_config_uses_unix_socket() {
        let sample = "listen = /run/php-fpm/cpn-webmail.sock";
        assert!(sample.contains("cpn-webmail.sock"));
        assert!(!sample.contains("php -S"));
    }
}
