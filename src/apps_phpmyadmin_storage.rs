//! phpMyAdmin configuration storage (pmadb + controluser).

use crate::apps_phpmyadmin::phpmyadmin_share_dir;
use crate::paths::join_data;
use rand::RngCore;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const CONTROL_USER: &str = "cpn_pma";
const CONTROL_DB: &str = "phpmyadmin";
const MARKER: &str = "CPN-PMA-STORAGE";
const CREATE_TABLES: &str = "/usr/share/phpMyAdmin/sql/create_tables.sql";

fn config_candidates(share: &Path) -> Vec<PathBuf> {
    let mut paths = vec![
        PathBuf::from("/etc/phpMyAdmin/config.inc.php"),
        PathBuf::from("/etc/phpmyadmin/config.inc.php"),
        share.join("config.inc.php"),
    ];
    paths.dedup();
    paths
}

fn random_password() -> String {
    let mut bytes = [0u8; 24];
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

fn secret_path() -> PathBuf {
    join_data("phpmyadmin").join("control.secret")
}

fn load_or_create_control_password() -> Result<String, String> {
    let dir = join_data("phpmyadmin");
    fs::create_dir_all(&dir).map_err(|e| format!("Could not create phpmyadmin data dir: {e}"))?;
    let path = secret_path();
    if path.is_file() {
        let raw = fs::read_to_string(&path)
            .map_err(|e| format!("Could not read control secret: {e}"))?;
        let pass = raw.trim().to_string();
        if !pass.is_empty() {
            return Ok(pass);
        }
    }
    let pass = random_password();
    fs::write(&path, format!("{pass}\n"))
        .map_err(|e| format!("Could not write control secret: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    }
    Ok(pass)
}

fn run_sql(sql: &str) -> Result<(), String> {
    let bin = mariadb_cli().ok_or_else(|| "MariaDB/MySQL client not found".to_string())?;
    let out = Command::new(bin)
        .args(["-e", sql])
        .output()
        .map_err(|e| format!("Failed to run MariaDB SQL: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "MariaDB SQL failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(())
}

fn import_create_tables() -> Result<(), String> {
    if !Path::new(CREATE_TABLES).is_file() {
        return Err(format!("Missing {CREATE_TABLES}"));
    }
    let bin = mariadb_cli().ok_or_else(|| "MariaDB/MySQL client not found".to_string())?;
    let file = fs::File::open(CREATE_TABLES)
        .map_err(|e| format!("Could not open create_tables.sql: {e}"))?;
    let out = Command::new(bin)
        .arg(CONTROL_DB)
        .stdin(std::process::Stdio::from(file))
        .output()
        .map_err(|e| format!("Failed to import create_tables.sql: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "create_tables.sql import failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(())
}

fn ensure_control_db_and_user(pass: &str) -> Result<(), String> {
    let pass_sql = pass.replace('\'', "''");
    run_sql(&format!(
        "CREATE DATABASE IF NOT EXISTS `{CONTROL_DB}` DEFAULT CHARACTER SET utf8 COLLATE utf8_bin; \
         CREATE USER IF NOT EXISTS '{CONTROL_USER}'@'localhost' IDENTIFIED BY '{pass_sql}'; \
         ALTER USER '{CONTROL_USER}'@'localhost' IDENTIFIED BY '{pass_sql}'; \
         GRANT SELECT, INSERT, DELETE, UPDATE, ALTER ON `{CONTROL_DB}`.* TO '{CONTROL_USER}'@'localhost'; \
         FLUSH PRIVILEGES;"
    ))?;
    import_create_tables()?;
    Ok(())
}

fn storage_config_append(pass: &str) -> String {
    let pass_php = pass.replace('\\', "\\\\").replace('\'', "\\'");
    format!(
        "\n// {MARKER}\n\
         if (isset($cfg['Servers'][1])) {{\n\
           $cfg['Servers'][1]['controlhost'] = 'localhost';\n\
           $cfg['Servers'][1]['controluser'] = '{CONTROL_USER}';\n\
           $cfg['Servers'][1]['controlpass'] = '{pass_php}';\n\
           $cfg['Servers'][1]['pmadb'] = '{CONTROL_DB}';\n\
           $cfg['Servers'][1]['bookmarktable'] = 'pma__bookmark';\n\
           $cfg['Servers'][1]['relation'] = 'pma__relation';\n\
           $cfg['Servers'][1]['table_info'] = 'pma__table_info';\n\
           $cfg['Servers'][1]['table_coords'] = 'pma__table_coords';\n\
           $cfg['Servers'][1]['pdf_pages'] = 'pma__pdf_pages';\n\
           $cfg['Servers'][1]['column_info'] = 'pma__column_info';\n\
           $cfg['Servers'][1]['history'] = 'pma__history';\n\
           $cfg['Servers'][1]['table_uiprefs'] = 'pma__table_uiprefs';\n\
           $cfg['Servers'][1]['tracking'] = 'pma__tracking';\n\
           $cfg['Servers'][1]['userconfig'] = 'pma__userconfig';\n\
           $cfg['Servers'][1]['recent'] = 'pma__recent';\n\
           $cfg['Servers'][1]['favorite'] = 'pma__favorite';\n\
           $cfg['Servers'][1]['users'] = 'pma__users';\n\
           $cfg['Servers'][1]['usergroups'] = 'pma__usergroups';\n\
           $cfg['Servers'][1]['navigationhiding'] = 'pma__navigationhiding';\n\
           $cfg['Servers'][1]['savedsearches'] = 'pma__savedsearches';\n\
           $cfg['Servers'][1]['central_columns'] = 'pma__central_columns';\n\
           $cfg['Servers'][1]['designer_settings'] = 'pma__designer_settings';\n\
           $cfg['Servers'][1]['export_templates'] = 'pma__export_templates';\n\
         }}\n"
    )
}

fn ensure_config_has_storage(conf_inc: &Path, pass: &str) -> Result<(), String> {
    if !conf_inc.is_file() {
        return Ok(());
    }
    let raw = fs::read_to_string(conf_inc)
        .map_err(|e| format!("Could not read {}: {e}", conf_inc.display()))?;
    if raw.contains(MARKER) {
        return Ok(());
    }
    fs::write(conf_inc, format!("{}{}", raw, storage_config_append(pass)))
        .map_err(|e| format!("Could not update {}: {e}", conf_inc.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Some(parent) = conf_inc.parent() {
            let _ = fs::set_permissions(parent, fs::Permissions::from_mode(0o755));
        }
        let _ = fs::set_permissions(conf_inc, fs::Permissions::from_mode(0o644));
    }
    Ok(())
}

/// Create pmadb tables + controluser and wire them into phpMyAdmin config.
/// Control password lives only under `/var/lib/cpn/phpmyadmin/control.secret` (mode 600).
pub fn ensure_phpmyadmin_configuration_storage() -> Result<String, String> {
    let share = phpmyadmin_share_dir().ok_or_else(|| {
        "phpMyAdmin share path not found under /usr/share/phpMyAdmin.".to_string()
    })?;
    let pass = load_or_create_control_password()?;
    ensure_control_db_and_user(&pass)?;
    for conf in config_candidates(&share) {
        let _ = ensure_config_has_storage(&conf, &pass);
    }
    Ok(format!(
        "phpMyAdmin configuration storage ready (database `{CONTROL_DB}`, control user `{CONTROL_USER}`)."
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn storage_append_contains_required_keys() {
        let body = storage_config_append("secret'value");
        assert!(body.contains(MARKER));
        assert!(body.contains("controluser"));
        assert!(body.contains("pmadb"));
        assert!(body.contains("pma__relation"));
        assert!(body.contains("secret\\'value"));
    }
}
