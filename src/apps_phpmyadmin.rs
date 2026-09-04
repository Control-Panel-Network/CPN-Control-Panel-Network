//! Host-level phpMyAdmin packaging and local HTTP exposure.

use crate::apps_pkg::{
    enable_now, install_packages_dnf_or_apt, package_manager, rpm_or_dpkg_installed,
};
use crate::service_detect::{port_open, systemd_unit_active};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

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

/// Install packages (EPEL on dnf hosts) and wire a loopback nginx + php-fpm listener.
pub fn install_and_expose() -> Result<String, String> {
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
    write_nginx_vhost(&share)?;
    ensure_selinux_http_port(8081);
    if !systemd_unit_active("php-fpm") {
        let _ = enable_now(&["php-fpm"]);
    } else {
        let _ = Command::new("systemctl")
            .args(["reload", "php-fpm"])
            .status();
    }
    if Path::new("/usr/sbin/nginx").exists() || Path::new("/usr/bin/nginx").exists() {
        let _ = Command::new("nginx").args(["-t"]).status();
        let reload_ok = Command::new("systemctl")
            .args(["reload", "nginx"])
            .status()
            .map(|status| status.success())
            .unwrap_or(false);
        if !reload_ok {
            let _ = Command::new("systemctl")
                .args(["restart", "nginx"])
                .status();
        }
        if !systemd_unit_active("nginx") {
            enable_now(&["nginx"])?;
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
    if !(Path::new("/usr/sbin/nginx").exists() || Path::new("/usr/bin/nginx").exists()) {
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
        .status()
        .map(|status| status.success())
        .unwrap_or(false);
    if !add {
        let _ = Command::new("semanage")
            .args(["port", "-m", "-t", "http_port_t", "-p", "tcp", &port_s])
            .status();
    }
}

#[cfg(test)]
mod tests {
    use super::phpmyadmin_health_url;

    #[test]
    fn health_url_is_loopback_8081() {
        assert_eq!(phpmyadmin_health_url(), "http://127.0.0.1:8081/");
    }
}
