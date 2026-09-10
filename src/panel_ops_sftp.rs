//! OpenSSH internal-sftp jails for CPN websites and subdomains.
//!
//! Each account is a system user in group `cpn-sftp`, shell `nologin`, home set to
//! the site home (`/home/<domain>/` or nested subdomain home). `sshd` Match Group
//! applies `ChrootDirectory %h` + `ForceCommand internal-sftp`. Jail roots stay
//! root-owned; only `public_html` (docroot) is writable by the SFTP user.

use crate::resource_accounts::{
    FtpAccountRecord, create_ftp_account, delete_ftp_account, list_ftp_accounts,
};
use crate::sites::{load_site, site_home_from_record};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub const SFTP_GROUP: &str = "cpn-sftp";
pub const SSHD_DROPIN: &str = "/etc/ssh/sshd_config.d/99-cpn-sftp.conf";

const RESERVED_USERS: &[&str] = &[
    "root", "admin", "cpn", "nobody", "nginx", "apache", "mysql", "mariadb", "postfix",
];

#[derive(Debug, Clone)]
pub struct SftpStackStatus {
    pub stack: String,
    pub detail: String,
    pub ready: bool,
}

pub fn detect_sftp_stack() -> SftpStackStatus {
    #[cfg(not(unix))]
    {
        return SftpStackStatus {
            stack: "Unavailable".into(),
            detail: "Jailed SFTP is supported on Linux panel hosts only.".into(),
            ready: false,
        };
    }
    #[cfg(unix)]
    {
        let dropin = Path::new(SSHD_DROPIN).is_file();
        let group_ok = Command::new("getent")
            .args(["group", SFTP_GROUP])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        let sshd_active = Command::new("systemctl")
            .args(["is-active", "sshd"])
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "active")
            .unwrap_or(false)
            || Command::new("systemctl")
                .args(["is-active", "ssh"])
                .output()
                .ok()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "active")
                .unwrap_or(false);
        if dropin && group_ok && sshd_active {
            return SftpStackStatus {
                stack: "OpenSSH SFTP (jailed)".into(),
                detail: format!(
                    "Group `{SFTP_GROUP}` and `{SSHD_DROPIN}` are configured. Accounts are chrooted to each site home."
                ),
                ready: true,
            };
        }
        if sshd_active {
            return SftpStackStatus {
                stack: "OpenSSH".into(),
                detail:
                    "sshd is running. Use Reset SFTP to install the CPN jail Match block and group."
                        .into(),
                ready: false,
            };
        }
        SftpStackStatus {
            stack: "Not detected".into(),
            detail:
                "OpenSSH sshd is not active. Install and start openssh-server, then Reset SFTP."
                    .into(),
            ready: false,
        }
    }
}

fn validate_username(raw: &str) -> Result<String, String> {
    let username = raw.trim().to_ascii_lowercase();
    if username.len() < 3 || username.len() > 32 {
        return Err("SFTP username must be 3 to 32 characters".into());
    }
    if !username
        .chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_')
    {
        return Err("SFTP username may only use lowercase letters, digits, and underscore".into());
    }
    if username.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        return Err("SFTP username cannot start with a digit".into());
    }
    if RESERVED_USERS.contains(&username.as_str()) {
        return Err(format!("Username `{username}` is reserved"));
    }
    Ok(username)
}

fn validate_password(raw: &str) -> Result<(), String> {
    let password = raw.trim_end_matches(['\r', '\n']);
    if password.chars().count() < 8 {
        return Err("Password must be at least 8 characters".into());
    }
    if password.chars().count() > 128 {
        return Err("Password is too long".into());
    }
    if password.chars().any(|ch| ch.is_control()) {
        return Err("Password cannot include control characters".into());
    }
    Ok(())
}

fn run_checked(cmd: &str, args: &[&str], context: &str) -> Result<(), String> {
    let output = Command::new(cmd)
        .args(args)
        .output()
        .map_err(|e| format!("{context}: could not run `{cmd}`: {e}"))?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let detail = if !stderr.trim().is_empty() {
        stderr.trim().to_string()
    } else {
        stdout.trim().to_string()
    };
    Err(format!(
        "{context}: `{cmd} {}` failed: {detail}",
        args.join(" ")
    ))
}

fn user_exists(username: &str) -> bool {
    Command::new("id")
        .arg(username)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn set_owner(path: &Path, username: &str) -> Result<(), String> {
    run_checked(
        "chown",
        &[
            "-R",
            &format!("{username}:{SFTP_GROUP}"),
            &path.display().to_string(),
        ],
        "chown writable tree",
    )
}

fn set_root_owned(path: &Path, mode: u32) -> Result<(), String> {
    run_checked(
        "chown",
        &["root:root", &path.display().to_string()],
        "chown jail root",
    )?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(mode))
            .map_err(|e| format!("chmod {}: {e}", path.display()))?;
    }
    let _ = mode;
    Ok(())
}

fn prepare_jail(site_home: &Path, docroot: &Path, username: &str) -> Result<(), String> {
    fs::create_dir_all(site_home)
        .map_err(|e| format!("Could not create jail {}: {e}", site_home.display()))?;
    fs::create_dir_all(docroot)
        .map_err(|e| format!("Could not create docroot {}: {e}", docroot.display()))?;

    // OpenSSH requires every chroot path component to be root-owned and not group-writable.
    // Only touch paths under the hosting home root (never `/` or system roots).
    let hosting_root = crate::sites::hosting_home_root();
    let mut chain = Vec::new();
    let mut cur = site_home.to_path_buf();
    loop {
        chain.push(cur.clone());
        if cur == hosting_root {
            break;
        }
        match cur.parent() {
            Some(parent) if parent != cur => {
                if !parent.starts_with(&hosting_root) && parent != hosting_root {
                    break;
                }
                cur = parent.to_path_buf();
            }
            _ => break,
        }
    }
    for path in chain.into_iter().rev() {
        if path.exists() {
            set_root_owned(&path, 0o755)?;
        }
    }
    set_owner(docroot, username)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(docroot, fs::Permissions::from_mode(0o755));
    }
    Ok(())
}

fn sshd_dropin_contents() -> String {
    format!(
        r#"# Managed by CPN. Jailed SFTP for group {SFTP_GROUP}.
# Do not grant shell access to members of this group.
Match Group {SFTP_GROUP}
    ChrootDirectory %h
    ForceCommand internal-sftp
    AllowTcpForwarding no
    X11Forwarding no
    PermitTunnel no
    AllowAgentForwarding no
"#
    )
}

fn reload_sshd() -> Result<(), String> {
    if Command::new("systemctl")
        .args(["reload", "sshd"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
    {
        return Ok(());
    }
    if Command::new("systemctl")
        .args(["reload", "ssh"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
    {
        return Ok(());
    }
    Err("Could not reload sshd/ssh via systemctl".into())
}

/// Install group + sshd Match drop-in and reload sshd.
pub fn ensure_sftp_stack() -> Result<String, String> {
    #[cfg(not(unix))]
    {
        return Err("Jailed SFTP requires a Linux host".into());
    }
    #[cfg(unix)]
    {
        if !Command::new("getent")
            .args(["group", SFTP_GROUP])
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
        {
            run_checked("groupadd", &["-f", SFTP_GROUP], "create sftp group")?;
        }
        let dropin = PathBuf::from(SSHD_DROPIN);
        if let Some(parent) = dropin.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Could not create {}: {e}", parent.display()))?;
        }
        fs::write(&dropin, sshd_dropin_contents())
            .map_err(|e| format!("Could not write {}: {e}", dropin.display()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&dropin, fs::Permissions::from_mode(0o644));
        }
        // Validate config before reload when sshd -t exists.
        let _ = Command::new("sshd").args(["-t"]).status();
        reload_sshd()?;
        Ok(format!(
            "Configured `{SFTP_GROUP}` and `{SSHD_DROPIN}`; sshd reloaded."
        ))
    }
}

fn set_password(username: &str, password: &str) -> Result<(), String> {
    validate_password(password)?;
    let mut child = Command::new("chpasswd")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Could not run chpasswd: {e}"))?;
    {
        use std::io::Write;
        let stdin = child
            .stdin
            .as_mut()
            .ok_or_else(|| "chpasswd stdin unavailable".to_string())?;
        writeln!(stdin, "{username}:{password}")
            .map_err(|e| format!("Could not write password: {e}"))?;
    }
    let output = child
        .wait_with_output()
        .map_err(|e| format!("chpasswd failed: {e}"))?;
    if !output.status.success() {
        return Err("Could not set SFTP password".into());
    }
    Ok(())
}

/// Create a jailed SFTP account rooted at the site home for `domain`.
pub fn create_jailed_sftp_account(
    owner: &str,
    username_raw: &str,
    domain_raw: &str,
    password: &str,
) -> Result<FtpAccountRecord, String> {
    #[cfg(not(unix))]
    {
        let _ = (owner, username_raw, domain_raw, password);
        return Err("Jailed SFTP requires a Linux host".into());
    }
    #[cfg(unix)]
    {
        let username = validate_username(username_raw)?;
        validate_password(password)?;
        let site = load_site(domain_raw)?;
        let jail = site_home_from_record(&site);
        let docroot = PathBuf::from(&site.docroot);
        if !docroot.starts_with(&jail) {
            return Err("Site docroot is outside the site home; refusing jail".into());
        }
        ensure_sftp_stack()?;
        if user_exists(&username) {
            return Err(format!("System user `{username}` already exists"));
        }
        // Registry first fails fast on duplicates without leaving orphan users.
        let record = create_ftp_account(owner, &username, &site.domain)?;
        let home = jail.display().to_string();
        let create = Command::new("useradd")
            .args([
                "-g",
                SFTP_GROUP,
                "-d",
                &home,
                "-s",
                "/usr/sbin/nologin",
                "-M",
                &username,
            ])
            .output();
        match create {
            Ok(out) if out.status.success() => {}
            Ok(out) => {
                let _ = delete_ftp_account(&username);
                let err = String::from_utf8_lossy(&out.stderr);
                return Err(format!(
                    "useradd failed: {}",
                    err.trim().chars().take(200).collect::<String>()
                ));
            }
            Err(e) => {
                let _ = delete_ftp_account(&username);
                return Err(format!("useradd failed: {e}"));
            }
        }
        if let Err(err) = prepare_jail(&jail, &docroot, &username) {
            let _ = Command::new("userdel").args(["-f", &username]).status();
            let _ = delete_ftp_account(&username);
            return Err(err);
        }
        if let Err(err) = set_password(&username, password) {
            let _ = Command::new("userdel").args(["-f", &username]).status();
            let _ = delete_ftp_account(&username);
            return Err(err);
        }
        let _ = record;
        // Re-read so callers get the persisted record.
        list_ftp_accounts()
            .into_iter()
            .find(|a| a.username == username)
            .ok_or_else(|| "SFTP account created but registry reload failed".to_string())
    }
}

pub fn delete_jailed_sftp_account(username_raw: &str) -> Result<String, String> {
    #[cfg(not(unix))]
    {
        let _ = username_raw;
        return Err("Jailed SFTP requires a Linux host".into());
    }
    #[cfg(unix)]
    {
        let username = validate_username(username_raw)?;
        if !list_ftp_accounts().iter().any(|a| a.username == username) {
            return Err(format!(
                "SFTP account `{username}` is not in the CPN registry"
            ));
        }
        if user_exists(&username) {
            // Do not remove site files; only the system user.
            let _ = Command::new("userdel").args(["-f", &username]).status();
        }
        delete_ftp_account(&username)?;
        Ok(format!("Removed jailed SFTP account `{username}`"))
    }
}

pub fn reset_sftp_password(username_raw: &str, password: &str) -> Result<String, String> {
    #[cfg(not(unix))]
    {
        let _ = (username_raw, password);
        return Err("Jailed SFTP requires a Linux host".into());
    }
    #[cfg(unix)]
    {
        let username = validate_username(username_raw)?;
        if !list_ftp_accounts().iter().any(|a| a.username == username) {
            return Err(format!(
                "SFTP account `{username}` is not in the CPN registry"
            ));
        }
        if !user_exists(&username) {
            return Err(format!(
                "System user `{username}` is missing; recreate the account"
            ));
        }
        set_password(&username, password)?;
        Ok(format!("Password updated for `{username}`"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn username_rules() {
        assert!(validate_username("site_ftp").is_ok());
        assert!(validate_username("ab").is_err());
        assert!(validate_username("Root").is_err());
        assert!(validate_username("1abc").is_err());
        assert!(validate_username("bad-name").is_err());
    }

    #[test]
    fn password_rules() {
        assert!(validate_password("short").is_err());
        assert!(validate_password("longenough1").is_ok());
    }

    #[test]
    fn dropin_mentions_internal_sftp() {
        let cfg = sshd_dropin_contents();
        assert!(cfg.contains("ForceCommand internal-sftp"));
        assert!(cfg.contains("ChrootDirectory %h"));
        assert!(cfg.contains(SFTP_GROUP));
    }
}
