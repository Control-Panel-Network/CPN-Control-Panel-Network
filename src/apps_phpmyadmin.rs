//! Host-level phpMyAdmin packaging and local HTTP exposure.

use crate::apps_pkg::{
    enable_now, install_packages_dnf_or_apt, package_manager, rpm_or_dpkg_installed,
};
use crate::model::ServerEngine;
use crate::service_detect::{port_open, systemd_unit_active};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const NGINX_CONF: &str = "/etc/nginx/conf.d/cpn-phpmyadmin.conf";
const FPM_POOL: &str = "/etc/php-fpm.d/cpn-phpmyadmin.conf";
const LISTEN_URL: &str = "http://127.0.0.1:8081/";
const SHARE_CANDIDATES: &[&str] = &["/usr/share/phpMyAdmin", "/usr/share/phpmyadmin"];

pub fn phpmyadmin_health_url() -> &'static str {
    LISTEN_URL
}

pub fn phpmyadmin_share_dir() -> Option<PathBuf> {
    SHARE_CANDIDATES
        .iter()
        .map(PathBuf::from)
        .find(|path| path.is_dir())
}

/// Install packages (EPEL on dnf hosts) and wire a loopback listener when safe.
pub fn install_and_expose() -> Result<String, String> {
    install_and_expose_for(None)
}

/// Same as [`install_and_expose`], but respects the selected web engine.
///
/// When OpenLiteSpeed or Caddy owns HTTP, do **not** start/reload/enable nginx
/// (default nginx.conf binds :80 and conflicts). Packages + php-fpm still install.
pub fn install_and_expose_for(web_server: Option<ServerEngine>) -> Result<String, String> {
    ensure_epel()?;
    install_packages_dnf_or_apt(
        &[
            "phpMyAdmin",
            "php-mysqlnd",
            "php-fpm",
            "php-json",
            "php-mbstring",
        ],
        &["phpmyadmin", "php-mysql", "php-fpm", "php-mbstring"],
    )?;
    let share = phpmyadmin_share_dir().ok_or_else(|| {
        String::from(
            "phpMyAdmin packages installed but share path /usr/share/phpMyAdmin was not found.",
        )
    })?;
    write_fpm_pool(&share)?;
    ensure_selinux_http_port(8081);
    reload_or_enable_php_fpm();

    if !should_wire_nginx_listener(web_server) {
        return Ok(format!(
            "Installed phpMyAdmin under {}. Nginx loopback listener skipped (OpenLiteSpeed/Caddy owns HTTP, or nginx is not the selected engine). Share path is ready; wire a vhost later from Apps if needed.",
            share.display()
        ));
    }

    write_nginx_vhost(&share)?;
    match activate_nginx_for_phpmyadmin() {
        Ok(()) => {}
        Err(note) => {
            return Ok(format!(
                "Installed phpMyAdmin under {}. {note} Local URL when nginx can start safely: {}.",
                share.display(),
                LISTEN_URL
            ));
        }
    }

    if !port_open("127.0.0.1:8081", 500) {
        return Ok(format!(
            "Installed phpMyAdmin under {}. Local listener {} is not accepting yet; reload nginx/php-fpm or open Apps.",
            share.display(),
            LISTEN_URL
        ));
    }
    Ok(format!(
        "Installed phpMyAdmin under {}. Reachable at {} (loopback).",
        share.display(),
        LISTEN_URL
    ))
}

fn should_wire_nginx_listener(web_server: Option<ServerEngine>) -> bool {
    match web_server {
        Some(ServerEngine::Nginx) => true,
        Some(ServerEngine::Openlitespeed) | Some(ServerEngine::Caddy) => false,
        None => {
            // Apps / standalone: only manage nginx when it is already the HTTP stack,
            // or when OLS/Caddy are not active (avoid reclaiming :80 from OLS).
            if ols_or_caddy_active() {
                return false;
            }
            nginx_binary_present()
        }
    }
}

fn ols_or_caddy_active() -> bool {
    systemd_unit_active("lshttpd")
        || systemd_unit_active("lsws")
        || systemd_unit_active("openlitespeed")
        || systemd_unit_active("caddy")
}

fn nginx_binary_present() -> bool {
    Path::new("/usr/sbin/nginx").exists() || Path::new("/usr/bin/nginx").exists()
}

fn reload_or_enable_php_fpm() {
    if !systemd_unit_active("php-fpm") {
        let _ = enable_now(&["php-fpm"]);
    } else {
        let _ = quiet_systemctl(&["reload", "php-fpm"]);
    }
}

/// Reload nginx when already active; otherwise start only if :80 is free or already nginx.
/// Never enable a unit that cannot start (avoids broken enable symlink spam).
fn activate_nginx_for_phpmyadmin() -> Result<(), String> {
    if !nginx_binary_present() {
        return Err(
            "Nginx binary not present; skipped loopback :8081 vhost (install nginx only when it is the selected web engine)."
                .into(),
        );
    }

    let _ = quiet_command("nginx", &["-t"]);

    if systemd_unit_active("nginx") {
        if quiet_systemctl(&["reload", "nginx"]) {
            return Ok(());
        }
        if quiet_systemctl(&["restart", "nginx"]) && systemd_unit_active("nginx") {
            return Ok(());
        }
        return Err(
            "Nginx reload/restart failed; left unit as-is (check /etc/nginx and :8081).".into(),
        );
    }

    if port_open("0.0.0.0:80", 200) || port_open("127.0.0.1:80", 200) {
        return Err(
            "Skipped starting nginx: TCP :80 is already in use (often OpenLiteSpeed). Default nginx.conf also binds :80, so starting it would fail. phpMyAdmin packages remain installed."
                .into(),
        );
    }

    if quiet_systemctl(&["start", "nginx"]) && systemd_unit_active("nginx") {
        let _ = quiet_systemctl(&["enable", "nginx"]);
        return Ok(());
    }

    Err(
        "Skipped enabling nginx: start failed (likely a port or config conflict). phpMyAdmin packages remain installed."
            .into(),
    )
}

fn quiet_systemctl(args: &[&str]) -> bool {
    Command::new("systemctl")
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn quiet_command(program: &str, args: &[&str]) -> bool {
    Command::new(program)
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn ensure_epel() -> Result<(), String> {
    if package_manager()? != "dnf" {
        return Ok(());
    }
    if rpm_or_dpkg_installed(&["epel-release"]) {
        return Ok(());
    }
    install_packages_dnf_or_apt(&["epel-release"], &[])?;
    Ok(())
}

fn write_fpm_pool(share: &Path) -> Result<(), String> {
    let body = format!(
        "[cpn-phpmyadmin]\n\
         user = nginx\n\
         group = nginx\n\
         listen = /run/php-fpm/cpn-phpmyadmin.sock\n\
         listen.owner = nginx\n\
         listen.group = nginx\n\
         listen.mode = 0660\n\
         pm = ondemand\n\
         pm.max_children = 5\n\
         php_admin_value[open_basedir] = {share}:/tmp\n\
         php_admin_flag[allow_url_fopen] = on\n",
        share = share.display()
    );
    // Fall back to apache user on apt hosts without nginx user.
    let body = if user_exists("nginx") {
        body
    } else if user_exists("www-data") {
        body.replace("user = nginx", "user = www-data")
            .replace("group = nginx", "group = www-data")
            .replace("listen.owner = nginx", "listen.owner = www-data")
            .replace("listen.group = nginx", "listen.group = www-data")
    } else {
        body
    };
    fs::write(FPM_POOL, body).map_err(|error| format!("Could not write {FPM_POOL}: {error}"))?;
    Ok(())
}

fn write_nginx_vhost(share: &Path) -> Result<(), String> {
    if !nginx_binary_present() {
        return Ok(());
    }
    let conf = format!(
        "# Managed by CPN (MariaDB / phpMyAdmin defaults)\n\
         server {{\n\
         listen 127.0.0.1:8081;\n\
         server_name localhost;\n\
         root {share};\n\
         index index.php index.html;\n\
         location / {{\n\
         try_files $uri $uri/ /index.php?$query_string;\n\
         }}\n\
         location ~ \\.php$ {{\n\
         include fastcgi_params;\n\
         fastcgi_param SCRIPT_FILENAME $document_root$fastcgi_script_name;\n\
         fastcgi_pass unix:/run/php-fpm/cpn-phpmyadmin.sock;\n\
         }}\n\
         location ~ /(libraries|setup|templates|locale|vendor) {{\n\
         deny all;\n\
         }}\n\
         }}\n",
        share = share.display()
    );
    fs::write(NGINX_CONF, conf)
        .map_err(|error| format!("Could not write {NGINX_CONF}: {error}"))?;
    Ok(())
}

fn user_exists(name: &str) -> bool {
    Command::new("id")
        .arg(name)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

/// Port 8081 is labeled `transproxy_port_t` on some EL hosts; nginx (httpd_t) cannot bind.
fn ensure_selinux_http_port(port: u16) {
    if !Path::new("/usr/sbin/semanage").exists() && !Path::new("/usr/bin/semanage").exists() {
        return;
    }
    let port_s = port.to_string();
    let add = Command::new("semanage")
        .args(["port", "-a", "-t", "http_port_t", "-p", "tcp", &port_s])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false);
    if !add {
        let _ = Command::new("semanage")
            .args(["port", "-m", "-t", "http_port_t", "-p", "tcp", &port_s])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
}

#[cfg(test)]
mod tests {
    use super::{phpmyadmin_health_url, should_wire_nginx_listener};
    use crate::model::ServerEngine;

    #[test]
    fn health_url_is_loopback_8081() {
        assert_eq!(phpmyadmin_health_url(), "http://127.0.0.1:8081/");
    }

    #[test]
    fn ols_and_caddy_skip_nginx_listener() {
        assert!(!should_wire_nginx_listener(Some(
            ServerEngine::Openlitespeed
        )));
        assert!(!should_wire_nginx_listener(Some(ServerEngine::Caddy)));
        assert!(should_wire_nginx_listener(Some(ServerEngine::Nginx)));
    }
}
