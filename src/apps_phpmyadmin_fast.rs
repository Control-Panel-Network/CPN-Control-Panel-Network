//! Fail-fast helpers for phpMyAdmin Open and `/phpmyadmin` proxy.
//!
//! Keep Docker/Podman, package-manager, and SQL import work off the request path
//! when the sign-on bridge, storage marker, FPM socket, and :8081 listener are healthy.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

const LIB_DIR: &str = "/var/lib/phpMyAdmin";
const RUNTIME_STAMP: &str = ".cpn-runtime-ok";
pub const PMA_FPM_SOCK: &str = "/run/php-fpm/cpn-phpmyadmin.sock";
pub const FPM_POOL: &str = "/etc/php-fpm.d/cpn-phpmyadmin.conf";
const STORAGE_MARKER: &str = "CPN-PMA-STORAGE";
const SIGNON_MARKER: &str = "CPN-SIGNON-PANEL";

/// True when TempDir/upload dirs still need a heal (missing tree or stamp).
pub fn runtime_dirs_need_heal() -> bool {
    let root = Path::new(LIB_DIR);
    !root.join("temp").is_dir() || !root.join(RUNTIME_STAMP).is_file()
}

pub fn mark_runtime_dirs_ok() {
    let root = Path::new(LIB_DIR);
    if root.is_dir() {
        let _ = fs::write(root.join(RUNTIME_STAMP), b"ok\n");
    }
}

/// Prefer a filesystem hit over `mariadb --version` on every Open click.
pub fn mariadb_cli() -> Option<&'static str> {
    static CACHED: OnceLock<Option<&'static str>> = OnceLock::new();
    *CACHED.get_or_init(|| {
        for candidate in [
            "/usr/bin/mariadb",
            "/usr/bin/mysql",
            "/usr/local/bin/mariadb",
            "/usr/local/bin/mysql",
        ] {
            if Path::new(candidate).is_file() {
                return Some(candidate);
            }
        }
        ["mariadb", "mysql"].into_iter().find(|&bin| {
            Command::new(bin)
                .arg("--version")
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
        })
    })
}

pub fn fpm_pool_body(share: &Path, owner: &str, group: &str, mode: &str) -> String {
    format!(
        "[cpn-phpmyadmin]\n\
         user = {owner}\n\
         group = {group}\n\
         listen = {PMA_FPM_SOCK}\n\
         listen.owner = {owner}\n\
         listen.group = {group}\n\
         listen.mode = {mode}\n\
         pm = dynamic\n\
         pm.max_children = 5\n\
         pm.start_servers = 1\n\
         pm.min_spare_servers = 1\n\
         pm.max_spare_servers = 2\n\
         php_admin_value[open_basedir] = {share}:/etc/phpMyAdmin:/etc/phpmyadmin:/var/lib/phpMyAdmin:/var/lib/cpn/phpmyadmin:/tmp\n\
         php_admin_flag[allow_url_fopen] = on\n",
        share = share.display()
    )
}

/// Write the pool only when the desired body changed. Returns `true` if written.
pub fn write_fpm_pool_file(
    share: &Path,
    owner: &str,
    group: &str,
    mode: &str,
) -> Result<bool, String> {
    let body = fpm_pool_body(share, owner, group, mode);
    let previous = fs::read_to_string(FPM_POOL).unwrap_or_default();
    if previous == body {
        return Ok(false);
    }
    fs::write(FPM_POOL, body).map_err(|error| format!("Could not write {FPM_POOL}: {error}"))?;
    Ok(true)
}

pub fn config_has_storage_marker(raw: &str) -> bool {
    raw.contains(STORAGE_MARKER)
}

fn phpmyadmin_config_candidates(share: &Path) -> [PathBuf; 3] {
    [
        PathBuf::from("/etc/phpMyAdmin/config.inc.php"),
        PathBuf::from("/etc/phpmyadmin/config.inc.php"),
        share.join("config.inc.php"),
    ]
}

/// True when control secret and at least one config already have storage wired.
pub fn configuration_storage_ready() -> bool {
    let Some(share) = crate::apps_phpmyadmin::phpmyadmin_share_dir() else {
        return false;
    };
    if !crate::paths::join_data("phpmyadmin")
        .join("control.secret")
        .is_file()
    {
        return false;
    }
    phpmyadmin_config_candidates(&share).iter().any(|conf| {
        conf.is_file()
            && fs::read_to_string(conf)
                .map(|raw| config_has_storage_marker(&raw))
                .unwrap_or(false)
    })
}

pub fn signon_bridge_ready(share: &Path, panel_open: &str) -> bool {
    let web = share.join("cpn-signon.php");
    let Ok(body) = fs::read_to_string(&web) else {
        return false;
    };
    if !body.contains(panel_open) {
        return false;
    }
    phpmyadmin_config_candidates(share).iter().any(|conf| {
        conf.is_file()
            && fs::read_to_string(conf)
                .map(|raw| raw.contains(SIGNON_MARKER) && raw.contains(panel_open))
                .unwrap_or(false)
    })
}

pub fn pma_sock_ready() -> bool {
    Path::new(PMA_FPM_SOCK).exists()
}

pub fn pma_listener_ready() -> bool {
    crate::service_detect::port_open("127.0.0.1:8081", 80)
}

#[cfg(test)]
mod tests {
    use super::{config_has_storage_marker, fpm_pool_body, signon_bridge_ready};
    use std::path::Path;

    #[test]
    fn fpm_pool_keeps_a_warm_worker() {
        let body = fpm_pool_body(
            Path::new("/usr/share/phpMyAdmin"),
            "nobody",
            "nobody",
            "0660",
        );
        assert!(body.contains("pm = dynamic"));
        assert!(body.contains("pm.min_spare_servers = 1"));
        assert!(!body.contains("pm = ondemand"));
    }

    #[test]
    fn storage_marker_detects_wired_config() {
        assert!(config_has_storage_marker("// CPN-PMA-STORAGE\n"));
        assert!(!config_has_storage_marker(
            "$cfg['Servers'][1]['host'] = 'localhost';"
        ));
    }

    #[test]
    fn signon_ready_false_without_files() {
        assert!(!signon_bridge_ready(
            Path::new("/tmp/cpn-pma-missing"),
            "/databases/phpmyadmin/open"
        ));
    }
}
