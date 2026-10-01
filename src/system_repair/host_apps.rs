//! PHP, phpMyAdmin, WordPress, Cloudflare, and plugins checks for System Repair.

use super::{CheckStatus, HealResult, push};
use crate::apps_phpmyadmin::{
    ensure_fpm_socket_for_ols, ensure_phpmyadmin_runtime_dirs, phpmyadmin_health_url,
    phpmyadmin_share_dir,
};
use crate::apps_phpmyadmin_storage::ensure_phpmyadmin_configuration_storage;
use crate::panel_ops_cloudflare::load_cloudflare;
use crate::panel_ops_php::detect_php;
use crate::paths;
use crate::php_defaults;
use crate::service_detect::{port_open, systemd_unit_active};
use crate::wordpress::list_wordpress_sites;
use std::fs;
use std::path::Path;

fn phpmyadmin_installed() -> bool {
    phpmyadmin_share_dir().is_some()
}

pub fn collect(checks: &mut Vec<super::RepairCheck>) {
    let php = detect_php();
    let php_default = php_defaults::load_php_default();
    let php_detail = match (&php.version, &php_default) {
        (Some(v), Some(rec)) => {
            let req = if rec.requested.trim().is_empty() {
                "(default)"
            } else {
                rec.requested.as_str()
            };
            format!(
                "CLI {v}; host default stream={} requested={req}",
                rec.stream
            )
        }
        (Some(v), None) => format!("CLI {v}; host default file not set yet"),
        (None, _) => php.detail.clone(),
    };
    push(
        checks,
        "host.php",
        "php",
        "PHP runtime",
        if php.binary.is_some() {
            CheckStatus::Pass
        } else {
            CheckStatus::Warn
        },
        php_detail,
        false,
        None,
    );

    let fpm_units = ["php-fpm", "php84-php-fpm", "php85-php-fpm", "php83-php-fpm"];
    let fpm_active = fpm_units.iter().any(|u| systemd_unit_active(u));
    let sock_ok = Path::new("/run/php-fpm/cpn-phpmyadmin.sock").exists()
        || Path::new("/run/php-fpm/www.sock").exists()
        || Path::new("/run/php-fpm/cpn-webmail.sock").exists();
    push(
        checks,
        "host.php_fpm",
        "php",
        "PHP-FPM",
        if fpm_active || sock_ok {
            CheckStatus::Pass
        } else if php.binary.is_some() {
            CheckStatus::Warn
        } else {
            CheckStatus::Pass
        },
        if fpm_active || sock_ok {
            "PHP-FPM unit or CPN pool socket present"
        } else if php.binary.is_some() {
            "PHP CLI present but no active FPM unit/socket detected"
        } else {
            "PHP-FPM not required until PHP Host packages are installed"
        },
        false,
        None,
    );

    if phpmyadmin_installed() {
        let listening = port_open("127.0.0.1:8081", 250);
        let storage_marker = Path::new("/etc/phpMyAdmin/config.inc.php").is_file()
            || Path::new("/etc/phpmyadmin/config.inc.php").is_file();
        push(
            checks,
            "host.phpmyadmin",
            "database",
            "phpMyAdmin",
            if listening {
                CheckStatus::Pass
            } else {
                CheckStatus::Warn
            },
            format!(
                "share present; health URL {}; loopback :8081 {}; config {}",
                phpmyadmin_health_url(),
                if listening { "open" } else { "closed" },
                if storage_marker {
                    "config.inc.php present"
                } else {
                    "config.inc.php missing"
                }
            ),
            false,
            Some("phpmyadmin"),
        );
    } else {
        push(
            checks,
            "host.phpmyadmin",
            "database",
            "phpMyAdmin",
            CheckStatus::Pass,
            "phpMyAdmin not installed (optional)",
            false,
            None,
        );
    }

    let wp_sites = list_wordpress_sites();
    if wp_sites.is_empty() {
        push(
            checks,
            "host.wordpress",
            "wordpress",
            "WordPress sites",
            CheckStatus::Pass,
            "no WordPress sites registered",
            false,
            None,
        );
    } else {
        let mut missing = Vec::new();
        let mut ok_n = 0usize;
        for site in &wp_sites {
            let root = if site.docroot.trim().is_empty() {
                Path::new("/home")
                    .join(&site.domain)
                    .join("public_html")
            } else {
                Path::new(site.docroot.trim()).to_path_buf()
            };
            if root.join("wp-config.php").is_file() {
                ok_n += 1;
            } else {
                missing.push(site.domain.clone());
            }
        }
        push(
            checks,
            "host.wordpress",
            "wordpress",
            "WordPress sites",
            if missing.is_empty() {
                CheckStatus::Pass
            } else {
                CheckStatus::Warn
            },
            if missing.is_empty() {
                format!("{ok_n} registered site(s) have wp-config.php")
            } else {
                format!(
                    "{ok_n}/{} ok; missing wp-config.php: {}",
                    wp_sites.len(),
                    missing.join(", ")
                )
            },
            false,
            None,
        );
    }

    let cf = load_cloudflare();
    if cf.api_token.trim().is_empty() {
        push(
            checks,
            "host.cloudflare",
            "dns",
            "Cloudflare",
            CheckStatus::Pass,
            "Cloudflare not configured (optional)",
            false,
            None,
        );
    } else {
        match crate::panel_ops_cloudflare_verify::verify_cloudflare_connection() {
            Ok(res) if res.ok => push(
                checks,
                "host.cloudflare",
                "dns",
                "Cloudflare",
                CheckStatus::Pass,
                format!(
                    "connected; {} zone(s); {}",
                    res.zone_count, res.token_status
                ),
                false,
                None,
            ),
            Ok(res) => push(
                checks,
                "host.cloudflare",
                "dns",
                "Cloudflare",
                CheckStatus::Warn,
                res.message,
                false,
                None,
            ),
            Err(err) => push(
                checks,
                "host.cloudflare",
                "dns",
                "Cloudflare",
                CheckStatus::Warn,
                err,
                false,
                None,
            ),
        }
    }

    let sites_dir = paths::default_data_dir().join("sites");
    let mut plugin_roots = 0usize;
    if let Ok(entries) = fs::read_dir(&sites_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if !name.ends_with(".json") {
                continue;
            }
            let domain = name.trim_end_matches(".json");
            if Path::new("/home").join(domain).join("plugins").is_dir() {
                plugin_roots += 1;
            }
        }
    }
    let host_plugins = Path::new("/var/lib/cpn/plugins").is_dir();
    push(
        checks,
        "plugins.summary",
        "plugins",
        "Plugins summary",
        CheckStatus::Pass,
        format!(
            "site plugin trees={plugin_roots}; host plugins dir={}",
            if host_plugins { "present" } else { "absent" }
        ),
        false,
        None,
    );
}

pub fn heal_phpmyadmin() -> HealResult {
    #[cfg(unix)]
    {
        if unsafe { libc::geteuid() } != 0 {
            return HealResult {
                heal_id: "phpmyadmin".into(),
                ok: false,
                message: "requires root".into(),
            };
        }
    }
    if !phpmyadmin_installed() {
        return HealResult {
            heal_id: "phpmyadmin".into(),
            ok: true,
            message: "phpMyAdmin not installed; skipped".into(),
        };
    }
    let mut notes = Vec::new();
    match ensure_phpmyadmin_runtime_dirs() {
        Ok(m) => notes.push(m),
        Err(e) => notes.push(format!("runtime dirs: {e}")),
    }
    match ensure_phpmyadmin_configuration_storage() {
        Ok(m) => notes.push(m),
        Err(e) => notes.push(format!("config storage: {e}")),
    }
    match ensure_fpm_socket_for_ols() {
        Ok(()) => notes.push("FPM socket ensured for OLS".into()),
        Err(e) => notes.push(format!("FPM socket: {e}")),
    }
    let ok = port_open("127.0.0.1:8081", 500)
        || Path::new("/run/php-fpm/cpn-phpmyadmin.sock").exists();
    HealResult {
        heal_id: "phpmyadmin".into(),
        ok,
        message: notes.join("; "),
    }
}
