//! Email and webmail content staging for selective CPN backups.
//!
//! Site scope stages mailbox trees under `/home/<domain>/e-mails/` (plus classic
//! `/home/vmail/<domain>/` when present), domain DKIM keys, and email-related site
//! plugins. Panel scope stages mail inventory, DKIM, webmail data, and email host
//! plugins/apps. Secrets stay inside the archive (mode 0600); nothing is logged.

use crate::mail_home_layout::{CLASSIC_VMAIL_ROOT, EMAILS_DIR};
use crate::plugin_activation::{host_plugin_installed, host_plugin_path, host_plugins_dir};
use crate::plugins_settings::is_webmail_plugin_id;
use crate::sites::{SiteRecord, site_home_from_record, site_plugins_dir};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const WEBMAIL_DATA_ROOT: &str = "/var/lib/cpn-webmail";

/// Host / site plugin ids that belong with email backups (webmail + mail apps).
pub fn is_email_backup_plugin_id(plugin_id: &str) -> bool {
    let id = plugin_id.trim();
    if id.is_empty() {
        return false;
    }
    if is_webmail_plugin_id(id) {
        return true;
    }
    let lower = id.to_ascii_lowercase();
    matches!(
        lower.as_str(),
        "protonmail"
            | "roundcube"
            | "roundcubewebmail"
            | "emailmarketing"
            | "plusaddressing"
            | "tachyonwebmail"
            | "tachyon"
            | "snappymail"
            | "snappymailwebmail"
            | "snappymailadmin"
            | "nextsnapmail"
    ) || lower.contains("webmail")
        || lower.contains("snappymail")
        || lower.contains("tachyon")
        || (lower.contains("mail") && !lower.contains("malware"))
}

/// Stage site/subdomain email content into `staging/`.
pub fn stage_site_email(staging: &Path, site: &SiteRecord) -> Result<(), String> {
    let home = site_home_from_record(site);
    let emails_src = home.join(EMAILS_DIR);
    if emails_src.is_dir() {
        stage_path_copy(&emails_src, &staging.join(EMAILS_DIR))?;
    }
    let classic = Path::new(CLASSIC_VMAIL_ROOT).join(&site.domain);
    if classic.is_dir() {
        stage_path_copy(&classic, &staging.join("vmail").join(&site.domain))?;
    }
    if let Ok(dkim_dir) = crate::panel_ops_dkim_keys::domain_dkim_dir(&site.domain)
        && dkim_dir.is_dir()
    {
        stage_path_copy(&dkim_dir, &staging.join("dkim").join(&site.domain))?;
    }
    stage_site_email_plugins(staging, site)?;
    Ok(())
}

fn stage_site_email_plugins(staging: &Path, site: &SiteRecord) -> Result<(), String> {
    let plugins_root = site_plugins_dir(site);
    if !plugins_root.is_dir() {
        return Ok(());
    }
    let dest_root = staging.join("email-plugins");
    let mut any = false;
    let entries = match fs::read_dir(&plugins_root) {
        Ok(e) => e,
        Err(_) => return Ok(()),
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !is_email_backup_plugin_id(name) {
            continue;
        }
        stage_path_copy(&path, &dest_root.join(name))?;
        any = true;
    }
    if !any {
        let _ = fs::remove_dir_all(&dest_root);
    }
    Ok(())
}

/// Stage panel-wide email inventory, webmail data, DKIM, and email host plugins.
pub fn stage_panel_email_stack(staging: &Path) -> Result<(), String> {
    let root = staging.join("email-stack");
    fs::create_dir_all(&root).map_err(|e| format!("email-stack mkdir: {e}"))?;

    let panel_mail = root.join("panel-mail");
    fs::create_dir_all(&panel_mail).map_err(|e| format!("panel-mail mkdir: {e}"))?;
    let data = crate::account::data_dir();
    for rel in [
        "mail-accounts.json",
        "mail-port-policy.json",
        "active-webmail.json",
        "webmail-panel.json",
        "webmail-loopback.json",
        "plugin-activations.json",
    ] {
        let src = data.join(rel);
        if src.is_file() {
            fs::copy(&src, panel_mail.join(rel))
                .map_err(|e| format!("stage copy {rel}: {e}"))?;
        }
    }

    let dkim = data.join("dkim");
    if dkim.is_dir() {
        stage_path_copy(&dkim, &root.join("dkim"))?;
    }

    let contacts = data.join("webmail-contacts");
    if contacts.is_dir() {
        stage_path_copy(&contacts, &root.join("webmail-contacts"))?;
    }

    let webmail = Path::new(WEBMAIL_DATA_ROOT);
    if webmail.is_dir() {
        let dest = root.join("webmail-data");
        stage_path_copy(webmail, &dest)?;
        prune_webmail_cache(&dest);
    }

    stage_email_host_plugins(&root.join("host-plugins"))?;
    stage_mail_database_dumps(&root.join("databases"))?;
    Ok(())
}

fn stage_email_host_plugins(dest_root: &Path) -> Result<(), String> {
    let src_root = host_plugins_dir();
    if !src_root.is_dir() {
        return Ok(());
    }
    let mut any = false;
    let entries = match fs::read_dir(&src_root) {
        Ok(e) => e,
        Err(_) => return Ok(()),
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !is_email_backup_plugin_id(name) || !host_plugin_installed(name) {
            continue;
        }
        let src = host_plugin_path(name);
        if !src.is_dir() {
            continue;
        }
        stage_path_copy(&src, &dest_root.join(name))?;
        any = true;
    }
    if !any {
        let _ = fs::remove_dir_all(dest_root);
    }
    Ok(())
}

fn stage_mail_database_dumps(dest: &Path) -> Result<(), String> {
    if !crate::panel_ops_db::local_mariadb_ready() {
        return Ok(());
    }
    let Some(dump_bin) = crate::panel_ops_db::mariadb_dump_bin() else {
        return Ok(());
    };
    fs::create_dir_all(dest).map_err(|e| format!("mail db dump mkdir: {e}"))?;
    let mut wrote = false;
    for db in ["cpn_mail", "cpn_tachyon_ab"] {
        if !database_exists(db) {
            continue;
        }
        let dump_path = dest.join(format!("{db}.sql"));
        let file = fs::File::create(&dump_path).map_err(|e| format!("dump file {db}: {e}"))?;
        let status = Command::new(&dump_bin)
            .args([
                "--single-transaction",
                "--routines",
                "--databases",
                db,
            ])
            .stdout(file)
            .status();
        match status {
            Ok(code) if code.success() => wrote = true,
            Ok(_) => {
                let _ = fs::remove_file(&dump_path);
            }
            Err(_) => {
                let _ = fs::remove_file(&dump_path);
            }
        }
    }
    if !wrote {
        let _ = fs::remove_dir_all(dest);
    }
    Ok(())
}

fn database_exists(name: &str) -> bool {
    let bin = if Command::new("mariadb").arg("--version").output().is_ok() {
        "mariadb"
    } else if Command::new("mysql").arg("--version").output().is_ok() {
        "mysql"
    } else {
        return false;
    };
    let output = Command::new(bin)
        .args([
            "-N",
            "-e",
            &format!(
                "SELECT SCHEMA_NAME FROM information_schema.SCHEMATA WHERE SCHEMA_NAME='{}' LIMIT 1",
                name.replace('\'', "")
            ),
        ])
        .output();
    match output {
        Ok(out) if out.status.success() => {
            let s = String::from_utf8_lossy(&out.stdout);
            s.trim() == name
        }
        _ => false,
    }
}

fn prune_webmail_cache(webmail_dest: &Path) {
    for rel in [
        "tachyon/_data_/_default_/cache",
        "snappymail/_data_/_default_/cache",
        "sso-debug.log",
    ] {
        let path = webmail_dest.join(rel);
        if path.is_dir() {
            let _ = fs::remove_dir_all(&path);
        } else if path.is_file() {
            let _ = fs::remove_file(&path);
        }
    }
}

fn stage_path_copy(src: &Path, dest: &Path) -> Result<(), String> {
    if !src.exists() {
        return Ok(());
    }
    if src.is_file() {
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("copy mkdir: {e}"))?;
        }
        fs::copy(src, dest).map_err(|e| format!("copy file: {e}"))?;
        return Ok(());
    }
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("copy mkdir: {e}"))?;
    }
    // `cp -a src dest` when dest does not exist creates dest as a copy of src.
    let _ = fs::remove_dir_all(dest);
    let status = Command::new("cp")
        .args(["-a", &src.to_string_lossy(), &dest.to_string_lossy()])
        .status()
        .map_err(|e| format!("cp failed: {e}"))?;
    if !status.success() {
        return Err(format!("cp -a {} failed", src.display()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn email_plugin_ids_cover_webmail_and_mail_apps() {
        assert!(is_email_backup_plugin_id("protonMail"));
        assert!(is_email_backup_plugin_id("roundcubeWebmail"));
        assert!(is_email_backup_plugin_id("snappymailWebmail"));
        assert!(is_email_backup_plugin_id("emailMarketing"));
        assert!(!is_email_backup_plugin_id("malwareScanner"));
        assert!(!is_email_backup_plugin_id("clamav"));
        assert!(!is_email_backup_plugin_id("backupDestinations"));
        assert!(!is_email_backup_plugin_id(""));
    }
}
