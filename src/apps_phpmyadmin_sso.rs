//! phpMyAdmin OLS loopback exposure and panel sign-on helper.

use crate::apps_phpmyadmin::{phpmyadmin_health_url, phpmyadmin_share_dir};
use crate::litespeed_stack::{openlitespeed_installed, restart_litespeed};
use crate::paths::join_data;
use crate::service_detect::{port_open, systemd_unit_active};
use rand::RngCore;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const OLS_VHOST: &str = "CPNPhpMyAdmin";
const LISTENER: &str = "CPNPhpMyAdminHttp";
const HTTPD_CONF: &str = "/usr/local/lsws/conf/httpd_config.conf";
const SIGNON_SESSION: &str = "CPNPmaSignon";
/// Panel route that mints a fresh one-time token while the CPN session is live.
/// phpMyAdmin SignonURL must point here so php-fpm restarts do not strand users
/// on `/phpmyadmin/cpn-signon.php` without a token.
pub const PANEL_PMA_OPEN_URL: &str = "/databases/phpmyadmin/open";

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn random_token() -> String {
    let mut bytes = [0u8; 24];
    rand::rng().fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn random_password() -> String {
    let mut bytes = [0u8; 18];
    rand::rng().fill_bytes(&mut bytes);
    data_encoding::BASE64URL_NOPAD.encode(&bytes)
}

fn mariadb_cli() -> Option<&'static str> {
    ["mariadb", "mysql"].into_iter().find(|&candidate| {
        Command::new(candidate)
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    })
}

fn ols_vhconf(docroot: &str, sock: &str) -> String {
    format!(
        "docRoot                   {docroot}/\n\
         enableGzip                1\n\
         index  {{\n\
           useServer               0\n\
           indexFiles              index.php, index.html\n\
         }}\n\
         extprocessor cpnphpmyadmin {{\n\
           type                    fcgi\n\
           address                 uds://{sock}\n\
           maxConns                8\n\
           initTimeout             60\n\
           retryTimeout            0\n\
           persistConn             1\n\
           respBuffer              0\n\
           autoStart               0\n\
         }}\n\
         scriptHandler  {{\n\
           add                     fcgi:cpnphpmyadmin php\n\
         }}\n\
         context / {{\n\
           type                    NULL\n\
           location                {docroot}/\n\
           allowBrowse             1\n\
           rewrite  {{\n\
             enable                1\n\
           }}\n\
           addDefaultCharset       off\n\
         }}\n\
         context /cpn-private/ {{\n\
           type                    NULL\n\
           location                {docroot}/cpn-private/\n\
           allowBrowse             0\n\
           accessControl  {{\n\
             deny                  *\n\
           }}\n\
         }}\n"
    )
}

fn resolve_fpm_sock() -> String {
    for sock in ["/run/php-fpm/cpn-phpmyadmin.sock", "/run/php-fpm/www.sock"] {
        if Path::new(sock).exists() {
            return sock.to_string();
        }
    }
    "/run/php-fpm/cpn-phpmyadmin.sock".into()
}

/// Ensure OpenLiteSpeed serves phpMyAdmin on loopback :8081 when OLS owns HTTP.
pub fn ensure_ols_phpmyadmin_listener() -> Result<String, String> {
    if !openlitespeed_installed() {
        return Err("OpenLiteSpeed is not installed; use nginx Apps wiring or install OLS.".into());
    }
    let share = phpmyadmin_share_dir().ok_or_else(|| {
        "phpMyAdmin share path not found under /usr/share/phpMyAdmin.".to_string()
    })?;
    let _ = crate::apps_phpmyadmin::ensure_fpm_socket_for_ols();
    let _ = crate::apps_phpmyadmin::ensure_phpmyadmin_runtime_dirs();
    let sock = resolve_fpm_sock();
    let vh_dir = PathBuf::from(format!("/usr/local/lsws/conf/vhosts/{OLS_VHOST}"));
    fs::create_dir_all(&vh_dir).map_err(|e| format!("Could not create OLS vhost dir: {e}"))?;
    let vhconf = vh_dir.join("vhconf.conf");
    fs::write(&vhconf, ols_vhconf(&share.display().to_string(), &sock))
        .map_err(|e| format!("Could not write phpMyAdmin vhconf: {e}"))?;

    let mut conf = fs::read_to_string(HTTPD_CONF)
        .map_err(|e| format!("Could not read httpd_config.conf: {e}"))?;
    let mut changed = false;
    if !conf.contains(&format!("virtualHost {OLS_VHOST}")) {
        conf.push_str(&format!(
            "\nvirtualHost {OLS_VHOST} {{\n  vhRoot                  {share}/\n  configFile              $SERVER_ROOT/conf/vhosts/{OLS_VHOST}/vhconf.conf\n  allowSymbolLink         1\n  enableScript            1\n  restrained              1\n}}\n",
            share = share.display()
        ));
        changed = true;
    }
    if !conf.contains(&format!("listener {LISTENER}")) {
        conf.push_str(&format!(
            "\nlistener {LISTENER} {{\n  address                 127.0.0.1:8081\n  secure                  0\n  map                     {OLS_VHOST} *\n}}\n"
        ));
        changed = true;
    }
    if changed {
        fs::write(HTTPD_CONF, conf)
            .map_err(|e| format!("Could not update httpd_config.conf: {e}"))?;
        let _ = restart_litespeed();
    }
    write_signon_bridge(&share)?;
    let _ = crate::apps_phpmyadmin_storage::ensure_phpmyadmin_configuration_storage();
    if !systemd_unit_active("php-fpm") {
        let _ = Command::new("systemctl")
            .args(["start", "php-fpm"])
            .status();
    }
    if port_open("127.0.0.1:8081", 500) {
        Ok(format!(
            "phpMyAdmin OLS listener ready at {}",
            phpmyadmin_health_url()
        ))
    } else {
        Ok(format!(
            "phpMyAdmin OLS vhost configured for {}; listener may still be starting.",
            phpmyadmin_health_url()
        ))
    }
}

/// Refresh cpn-signon.php and migrate SignonURL to the panel remint route.
/// Safe after host PHP default changes (php-fpm restart) and on Open.
pub fn refresh_phpmyadmin_signon() -> Result<String, String> {
    let share = phpmyadmin_share_dir().ok_or_else(|| {
        "phpMyAdmin share path not found under /usr/share/phpMyAdmin.".to_string()
    })?;
    write_signon_bridge(&share)?;
    if openlitespeed_installed() {
        let _ = ensure_ols_phpmyadmin_listener();
    }
    Ok("phpMyAdmin sign-on bridge refreshed.".into())
}

/// Distro packages (EL) load `/etc/phpMyAdmin/config.inc.php`, not only the share copy.
fn phpmyadmin_config_candidates(share: &Path) -> Vec<PathBuf> {
    let mut paths = vec![
        PathBuf::from("/etc/phpMyAdmin/config.inc.php"),
        PathBuf::from("/etc/phpmyadmin/config.inc.php"),
        share.join("config.inc.php"),
    ];
    paths.dedup();
    paths
}

fn write_signon_bridge(share: &Path) -> Result<(), String> {
    let conf_dir = join_data("phpmyadmin");
    fs::create_dir_all(&conf_dir)
        .map_err(|e| format!("Could not create phpmyadmin data dir: {e}"))?;
    let signon_php = conf_dir.join("signon.php");
    // Location stays root-relative on the loopback backend; the panel proxy
    // rewrites it to /phpmyadmin/index.php for host browsers.
    // Missing/expired tokens redirect to the panel open route so a live CPN
    // session remints SSO without requiring a full panel re-login. Location is
    // rewritten by the panel proxy only for PMA-relative paths.
    let body = format!(
        r#"<?php
declare(strict_types=1);
session_name('{session}');
session_start();
$token = isset($_GET['token']) ? (string)$_GET['token'] : '';
$token = preg_replace('/[^a-f0-9]/', '', $token) ?? '';
$file = __DIR__ . '/cpn-private/tokens/' . $token . '.json';
if ($token === '' || !is_readable($file)) {{
  header('Location: {open}');
  exit;
}}
$raw = file_get_contents($file);
@unlink($file);
$data = json_decode($raw ?: '', true);
if (!is_array($data) || empty($data['user']) || !isset($data['password']) || empty($data['exp']) || (int)$data['exp'] < time()) {{
  header('Location: {open}');
  exit;
}}
$_SESSION['PMA_single_signon_user'] = (string)$data['user'];
$_SESSION['PMA_single_signon_password'] = (string)$data['password'];
$_SESSION['PMA_single_signon_host'] = $data['host'] ?? 'localhost';
header('Location: /index.php');
exit;
"#,
        session = SIGNON_SESSION,
        open = PANEL_PMA_OPEN_URL,
    );
    fs::write(&signon_php, body).map_err(|e| format!("Could not write signon.php: {e}"))?;
    let web_signon = share.join("cpn-signon.php");
    fs::copy(&signon_php, &web_signon)
        .map_err(|e| format!("Could not publish cpn-signon.php: {e}"))?;
    for conf in phpmyadmin_config_candidates(share) {
        let _ = ensure_config_includes_signon(&conf, SIGNON_SESSION);
        ensure_phpmyadmin_config_readable(&conf);
    }
    Ok(())
}

fn ensure_config_includes_signon(conf_inc: &Path, session: &str) -> Result<(), String> {
    let parent = conf_inc.parent().unwrap_or_else(|| Path::new("/"));
    if !parent.exists() {
        return Ok(());
    }
    if !conf_inc.is_file() {
        let body = format!(
            "<?php\n\
             $i = 0;\n\
             $i++;\n\
             $cfg['Servers'][$i]['auth_type'] = 'signon';\n\
             $cfg['Servers'][$i]['SignonSession'] = '{session}';\n\
             $cfg['Servers'][$i]['SignonURL'] = '{open}';\n\
             $cfg['Servers'][$i]['host'] = 'localhost';\n\
             $cfg['PmaAbsoluteUri'] = '/phpmyadmin/';\n\
             $cfg['BlowfishSecret'] = '{secret}';\n\
             $cfg['blowfish_secret'] = '{secret}';\n",
            session = session,
            open = PANEL_PMA_OPEN_URL,
            secret = random_token()
        );
        fs::write(conf_inc, body)
            .map_err(|e| format!("Could not write {}: {e}", conf_inc.display()))?;
        ensure_phpmyadmin_config_readable(conf_inc);
        return Ok(());
    }
    let raw = fs::read_to_string(conf_inc)
        .map_err(|e| format!("Could not read {}: {e}", conf_inc.display()))?;
    if raw.contains("CPN-SIGNON-PANEL") {
        let updated = rewrite_signon_url_to_panel_open(&raw);
        if updated != raw {
            fs::write(conf_inc, updated)
                .map_err(|e| format!("Could not update {}: {e}", conf_inc.display()))?;
        }
        ensure_phpmyadmin_config_readable(conf_inc);
        return Ok(());
    }
    // Append after distro cookie defaults so panel mount + sign-on win.
    // SignonURL is the panel open route so a live CPN session remints tokens
    // after php-fpm restarts (host PHP default changes).
    let append = format!(
        "\n// CPN-SIGNON-PANEL\n\
         $cfg['PmaAbsoluteUri'] = '/phpmyadmin/';\n\
         if (isset($cfg['Servers'][1])) {{\n\
           $cfg['Servers'][1]['auth_type'] = 'signon';\n\
           $cfg['Servers'][1]['SignonSession'] = '{session}';\n\
           $cfg['Servers'][1]['SignonURL'] = '{open}';\n\
         }} elseif (isset($i) && isset($cfg['Servers'][$i])) {{\n\
           $cfg['Servers'][$i]['auth_type'] = 'signon';\n\
           $cfg['Servers'][$i]['SignonSession'] = '{session}';\n\
           $cfg['Servers'][$i]['SignonURL'] = '{open}';\n\
         }}\n",
        session = session,
        open = PANEL_PMA_OPEN_URL,
    );
    fs::write(conf_inc, format!("{raw}{append}"))
        .map_err(|e| format!("Could not update {}: {e}", conf_inc.display()))?;
    ensure_phpmyadmin_config_readable(conf_inc);
    Ok(())
}

/// Migrate prior SignonURL values that pointed at cpn-signon.php (no remint).
fn rewrite_signon_url_to_panel_open(raw: &str) -> String {
    raw.replace(
        "$cfg['Servers'][1]['SignonURL'] = '/phpmyadmin/cpn-signon.php';",
        &format!("$cfg['Servers'][1]['SignonURL'] = '{PANEL_PMA_OPEN_URL}';"),
    )
    .replace(
        "$cfg['Servers'][$i]['SignonURL'] = '/phpmyadmin/cpn-signon.php';",
        &format!("$cfg['Servers'][$i]['SignonURL'] = '{PANEL_PMA_OPEN_URL}';"),
    )
}

/// EL packages ship `/etc/phpMyAdmin` as root-only (700/640). php-fpm runs as
/// nobody and must be able to read `config.inc.php` or sign-on never applies.
fn ensure_phpmyadmin_config_readable(conf_inc: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Some(parent) = conf_inc.parent()
            && parent.exists()
        {
            let _ = fs::set_permissions(parent, fs::Permissions::from_mode(0o755));
        }
        if conf_inc.is_file() {
            let _ = fs::set_permissions(conf_inc, fs::Permissions::from_mode(0o644));
        }
    }
}

fn create_ephemeral_db_user() -> Result<(String, String), String> {
    let bin = mariadb_cli().ok_or_else(|| "MariaDB/MySQL client not found".to_string())?;
    let user = format!("cpn_pma_{}", &random_token()[..8]);
    let pass = random_password();
    let pass_sql = pass.replace('\'', "''");
    let sql = format!(
        "CREATE USER IF NOT EXISTS '{user}'@'localhost' IDENTIFIED BY '{pass_sql}'; \
         GRANT ALL PRIVILEGES ON *.* TO '{user}'@'localhost' WITH GRANT OPTION; FLUSH PRIVILEGES;"
    );
    let out = Command::new(bin)
        .args(["-e", &sql])
        .output()
        .map_err(|e| format!("Failed to create phpMyAdmin DB user: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "Could not create ephemeral DB user: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok((user, pass))
}

fn token_dir(share: &Path) -> PathBuf {
    share.join("cpn-private").join("tokens")
}

fn ensure_token_dir(share: &Path) -> Result<PathBuf, String> {
    let dir = token_dir(share);
    fs::create_dir_all(&dir).map_err(|e| format!("Could not create token dir: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let private = share.join("cpn-private");
        let _ = fs::set_permissions(&private, fs::Permissions::from_mode(0o750));
        let _ = fs::set_permissions(&dir, fs::Permissions::from_mode(0o750));
        let owner = if openlitespeed_installed() {
            "nobody:nobody"
        } else if Path::new("/etc/nginx").is_dir() {
            "nginx:nginx"
        } else {
            "nobody:nobody"
        };
        let _ = Command::new("chown")
            .args(["-R", owner, &private.display().to_string()])
            .status();
    }
    // Deny web listing of private token files when OLS serves the share.
    let deny = share.join("cpn-private").join(".htaccess");
    if !deny.is_file() {
        let _ = fs::write(&deny, "Require all denied\n");
    }
    Ok(dir)
}

/// Create a short-lived sign-on token and return the Open URL (loopback). Never returns the DB password.
pub fn open_phpmyadmin_autologin() -> Result<String, String> {
    let _ = refresh_phpmyadmin_signon();
    let _ = crate::apps_phpmyadmin::ensure_phpmyadmin_runtime_dirs();
    let _ = crate::apps_phpmyadmin_storage::ensure_phpmyadmin_configuration_storage();
    let share = phpmyadmin_share_dir().ok_or_else(|| {
        "phpMyAdmin share path not found under /usr/share/phpMyAdmin.".to_string()
    })?;
    let (user, pass) = create_ephemeral_db_user()?;
    let token = random_token();
    let dir = ensure_token_dir(&share)?;
    let exp = now_unix() + 120;
    let payload = serde_json::json!({
        "user": user,
        "password": pass,
        "host": "localhost",
        "exp": exp,
    });
    let path = dir.join(format!("{token}.json"));
    fs::write(&path, payload.to_string()).map_err(|e| format!("Could not write token: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o640));
        let owner = if openlitespeed_installed() {
            "nobody:nobody"
        } else {
            "nginx:nginx"
        };
        let _ = Command::new("chown")
            .args([owner, &path.display().to_string()])
            .status();
    }
    // Same-origin panel mount so Windows/NAT browsers never need guest :8081.
    Ok(format!(
        "{}/cpn-signon.php?token={token}",
        crate::panel_phpmyadmin_proxy::PMA_MOUNT
    ))
}

pub fn phpmyadmin_open_status() -> (bool, String) {
    let installed = phpmyadmin_share_dir().is_some();
    let listening = port_open("127.0.0.1:8081", 200);
    let mount = crate::panel_phpmyadmin_proxy::PMA_MOUNT;
    let detail = if !installed {
        "phpMyAdmin packages are not installed.".into()
    } else if listening {
        format!(
            "Ready at {mount}/ (panel reverse-proxy to loopback {}).",
            phpmyadmin_health_url()
        )
    } else if openlitespeed_installed() {
        format!(
            "phpMyAdmin share present; OLS :8081 listener not responding yet (use Open with auto-login to wire {mount}/)."
        )
    } else {
        format!(
            "Share present; open via {mount}/ (backend {}).",
            phpmyadmin_health_url()
        )
    };
    (installed, detail)
}

#[cfg(test)]
mod tests {
    use super::{PANEL_PMA_OPEN_URL, rewrite_signon_url_to_panel_open};

    #[test]
    fn migrates_legacy_signon_url() {
        let raw = "\
$cfg['Servers'][1]['SignonURL'] = '/phpmyadmin/cpn-signon.php';\n\
$cfg['Servers'][$i]['SignonURL'] = '/phpmyadmin/cpn-signon.php';\n";
        let updated = rewrite_signon_url_to_panel_open(raw);
        assert!(updated.contains(PANEL_PMA_OPEN_URL));
        assert!(!updated.contains("/phpmyadmin/cpn-signon.php';"));
    }
}
