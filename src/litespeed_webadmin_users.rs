//! OpenLiteSpeed / LiteSpeed Enterprise WebAdmin htpasswd and guest users.
//!
//! Paths: `/usr/local/lsws/admin/conf/htpasswd` and `admin_config.conf` (`user` / `guest` blocks).
//! Passwords are never returned after set; only usernames are listed.

use crate::litespeed_stack::restart_litespeed;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn admin_conf_dir() -> PathBuf {
    PathBuf::from("/usr/local/lsws/admin/conf")
}

pub fn htpasswd_path() -> PathBuf {
    admin_conf_dir().join("htpasswd")
}

pub fn admin_config_path() -> PathBuf {
    admin_conf_dir().join("admin_config.conf")
}

fn valid_username(raw: &str) -> Result<String, String> {
    let name = raw.trim();
    if name.is_empty() {
        return Err("Username is required.".into());
    }
    if name.len() > 64 {
        return Err("Username is too long.".into());
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
    {
        return Err(
            "Username may only contain letters, digits, underscore, hyphen, and dot.".into(),
        );
    }
    Ok(name.to_string())
}

fn valid_password(raw: &str) -> Result<&str, String> {
    let pass = raw.trim_matches(|c: char| c == '\n' || c == '\r');
    if pass.len() < 8 {
        return Err("Password must be at least 8 characters.".into());
    }
    if pass.len() > 128 {
        return Err("Password is too long.".into());
    }
    if pass.chars().any(|c| c.is_control()) {
        return Err("Password cannot include control characters.".into());
    }
    Ok(pass)
}

/// List usernames present in the WebAdmin htpasswd file (no hashes returned).
pub fn list_webadmin_usernames() -> Vec<String> {
    let path = htpasswd_path();
    let Ok(raw) = fs::read_to_string(&path) else {
        return Vec::new();
    };
    raw.lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                return None;
            }
            line.split_once(':').map(|(u, _)| u.trim().to_string())
        })
        .filter(|u| !u.is_empty())
        .collect()
}

fn run_htpasswd(args: &[&str]) -> Result<(), String> {
    let candidates = ["htpasswd", "/usr/bin/htpasswd", "/bin/htpasswd"];
    let mut last_err = String::from("htpasswd binary not found");
    for bin in candidates {
        let output = Command::new(bin)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output();
        match output {
            Ok(o) if o.status.success() => return Ok(()),
            Ok(o) => {
                last_err = String::from_utf8_lossy(&o.stderr).trim().to_string();
                if last_err.is_empty() {
                    last_err = format!("{bin} failed");
                }
            }
            Err(e) => last_err = format!("{bin}: {e}"),
        }
    }
    Err(format!(
        "Could not update WebAdmin password file: {last_err}"
    ))
}

/// Apache MD5 (`$apr1$`) via `openssl passwd` when `htpasswd` is not installed.
fn openssl_apr1_hash(password: &str) -> Result<String, String> {
    let candidates = ["openssl", "/usr/bin/openssl"];
    let mut last_err = String::from("openssl not found");
    for bin in candidates {
        let output = Command::new(bin)
            .args(["passwd", "-apr1", "-stdin"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .and_then(|mut child| {
                use std::io::Write;
                if let Some(mut stdin) = child.stdin.take() {
                    stdin.write_all(password.as_bytes())?;
                }
                child.wait_with_output()
            });
        match output {
            Ok(o) if o.status.success() => {
                let hash = String::from_utf8_lossy(&o.stdout).trim().to_string();
                if hash.starts_with("$apr1$") {
                    return Ok(hash);
                }
                last_err = format!("{bin} passwd returned unexpected hash");
            }
            Ok(o) => {
                last_err = String::from_utf8_lossy(&o.stderr).trim().to_string();
                if last_err.is_empty() {
                    last_err = format!("{bin} passwd failed");
                }
            }
            Err(e) => last_err = format!("{bin}: {e}"),
        }
    }
    Err(last_err)
}

fn upsert_htpasswd_line(username: &str, hash: &str) -> Result<(), String> {
    ensure_htpasswd_parent()?;
    let path = htpasswd_path();
    let mut lines: Vec<String> = if path.is_file() {
        fs::read_to_string(&path)
            .map_err(|e| format!("Could not read WebAdmin htpasswd: {e}"))?
            .lines()
            .map(|l| l.to_string())
            .collect()
    } else {
        Vec::new()
    };
    let mut replaced = false;
    for line in lines.iter_mut() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        match trimmed.split_once(':') {
            Some((u, _)) if u.trim() == username => {
                *line = format!("{username}:{hash}");
                replaced = true;
                break;
            }
            _ => {}
        }
    }
    if !replaced {
        lines.push(format!("{username}:{hash}"));
    }
    let body = if lines.is_empty() {
        format!("{username}:{hash}\n")
    } else {
        format!("{}\n", lines.join("\n"))
    };
    fs::write(&path, body).map_err(|e| format!("Could not write WebAdmin htpasswd: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

fn remove_htpasswd_line(username: &str) -> Result<(), String> {
    let path = htpasswd_path();
    if !path.is_file() {
        return Err("WebAdmin htpasswd file is missing.".into());
    }
    let raw =
        fs::read_to_string(&path).map_err(|e| format!("Could not read WebAdmin htpasswd: {e}"))?;
    let filtered: Vec<&str> = raw
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                return true;
            }
            match trimmed.split_once(':') {
                Some((u, _)) => u.trim() != username,
                None => true,
            }
        })
        .collect();
    fs::write(&path, format!("{}\n", filtered.join("\n")))
        .map_err(|e| format!("Could not write WebAdmin htpasswd: {e}"))?;
    Ok(())
}

fn ensure_htpasswd_parent() -> Result<(), String> {
    let dir = admin_conf_dir();
    if !dir.is_dir() {
        return Err(format!(
            "LiteSpeed admin conf directory missing: {}",
            dir.display()
        ));
    }
    Ok(())
}

/// Set or create a WebAdmin user password (admin or guest). Never logs the password.
pub fn set_webadmin_password(username: &str, password: &str) -> Result<String, String> {
    let user = valid_username(username)?;
    let pass = valid_password(password)?;
    ensure_htpasswd_parent()?;
    let path = htpasswd_path();
    let create = !path.is_file();
    let mut args: Vec<&str> = Vec::new();
    if create {
        args.push("-cb");
    } else {
        args.push("-b");
    }
    // Prefer bcrypt when available (-B); fall back without if unsupported.
    let path_s = path.to_string_lossy().into_owned();
    let try_bcrypt = run_htpasswd(&{
        let mut a = args.clone();
        a.push("-B");
        a.push(&path_s);
        a.push(&user);
        a.push(pass);
        a
    });
    if let Err(_bcrypt_err) = try_bcrypt {
        match run_htpasswd(&[if create { "-cb" } else { "-b" }, &path_s, &user, pass]) {
            Ok(()) => {}
            Err(ht_err) => {
                // AlmaLinux labs often lack httpd-tools; openssl is usually present.
                let hash = openssl_apr1_hash(pass)
                    .map_err(|e| format!("{ht_err}; openssl apr1 fallback also failed: {e}"))?;
                upsert_htpasswd_line(&user, &hash)?;
            }
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    }
    let restart = restart_litespeed();
    Ok(format!(
        "WebAdmin password updated for user `{user}` (password not shown). {restart}"
    ))
}

fn ensure_user_realm_line(conf: &str, username: &str, realm: &str) -> String {
    // OLS stores optional user lists; keep simple `user` / guest notes in a CPN block comment
    // and ensure username exists in htpasswd only. Real realm ACLs vary by edition.
    let marker = format!("# CPN-webadmin-user:{username}:{realm}");
    if conf.lines().any(|l| l.trim() == marker) {
        return conf.to_string();
    }
    let mut out = conf.trim_end().to_string();
    out.push('\n');
    out.push_str(&marker);
    out.push('\n');
    out
}

/// Add a guest WebAdmin user (htpasswd entry + annotated admin_config note).
pub fn add_webadmin_guest(username: &str, password: &str) -> Result<String, String> {
    let user = valid_username(username)?;
    if user.eq_ignore_ascii_case("admin") {
        return Err("Use the admin password form for the primary `admin` user.".into());
    }
    let _ = valid_password(password)?;
    let msg = set_webadmin_password(&user, password)?;
    let conf_path = admin_config_path();
    if conf_path.is_file() {
        let raw = fs::read_to_string(&conf_path)
            .map_err(|e| format!("Could not read admin_config.conf: {e}"))?;
        let next = ensure_user_realm_line(&raw, &user, "guest");
        if next != raw {
            fs::write(&conf_path, next)
                .map_err(|e| format!("Could not update admin_config.conf: {e}"))?;
        }
    }
    Ok(format!(
        "Guest WebAdmin user `{user}` ready. {msg} Guests use the same WebAdmin login page; grant roles inside WebAdmin as needed."
    ))
}

/// Remove a non-admin WebAdmin user from htpasswd.
pub fn remove_webadmin_user(username: &str) -> Result<String, String> {
    let user = valid_username(username)?;
    if user.eq_ignore_ascii_case("admin") {
        return Err("Refusing to remove the primary `admin` WebAdmin user.".into());
    }
    let path = htpasswd_path();
    if !path.is_file() {
        return Err("WebAdmin htpasswd file is missing.".into());
    }
    let path_s = path.to_string_lossy().into_owned();
    if run_htpasswd(&["-D", &path_s, &user]).is_err() {
        remove_htpasswd_line(&user)?;
    }
    if let Ok(raw) = fs::read_to_string(admin_config_path()) {
        let marker = format!("# CPN-webadmin-user:{user}:");
        let filtered: String = raw
            .lines()
            .filter(|l| !l.trim().starts_with(&marker))
            .collect::<Vec<_>>()
            .join("\n");
        let _ = fs::write(admin_config_path(), format!("{filtered}\n"));
    }
    let restart = restart_litespeed();
    Ok(format!("Removed WebAdmin user `{user}`. {restart}"))
}

pub fn webadmin_users_present() -> bool {
    Path::new(&htpasswd_path()).is_file()
}

/// CPN bootstrap admin username, or `admin` when no account exists yet.
pub fn cpn_admin_username_for_webadmin() -> String {
    crate::account::load_bootstrap()
        .map(|boot| boot.username)
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| "admin".into())
}

fn webadmin_username_from_cpn(raw: &str) -> (String, bool) {
    match valid_username(raw) {
        Ok(name) => (name, false),
        Err(_) => ("admin".into(), true),
    }
}

fn try_remove_htpasswd_user(username: &str) {
    let path = htpasswd_path();
    if !path.is_file() {
        return;
    }
    let path_s = path.to_string_lossy().into_owned();
    if run_htpasswd(&["-D", &path_s, username]).is_err() {
        let _ = remove_htpasswd_line(username);
    }
}

/// Align OLS WebAdmin htpasswd with a CPN admin username and plaintext password.
///
/// OLS stores apr1/bcrypt in htpasswd; CPN stores PBKDF2 (or legacy) hashes. The formats
/// are not interchangeable, so the password is re-hashed for WebAdmin. Never logs the password.
pub fn align_webadmin_with_credentials(username: &str, password: &str) -> Result<String, String> {
    if !crate::litespeed_stack::openlitespeed_installed() {
        return Err("OpenLiteSpeed is not installed.".into());
    }
    ensure_htpasswd_parent()?;
    let (user, fell_back) = webadmin_username_from_cpn(username);
    let msg = set_webadmin_password(&user, password)?;
    // Package default is often a random `admin` password operators never see.
    if user != "admin" {
        try_remove_htpasswd_user("admin");
        let _ = restart_litespeed();
    }
    let fallback_note = if fell_back {
        " CPN username is not valid for OLS htpasswd; WebAdmin user is `admin` with the same password."
    } else {
        ""
    };
    Ok(format!(
        "WebAdmin aligned to CPN admin user `{user}` (password not shown). OLS htpasswd uses apr1/bcrypt; CPN panel hashes are not copied. {msg}{fallback_note}"
    ))
}

/// After first-account setup: if OLS is present, align WebAdmin. Soft-fail (never blocks setup).
pub fn maybe_align_webadmin_after_account_setup(username: &str, password: &str) -> Option<String> {
    if !crate::litespeed_stack::openlitespeed_installed() {
        return None;
    }
    if !admin_conf_dir().is_dir() {
        return None;
    }
    match align_webadmin_with_credentials(username, password) {
        Ok(msg) => Some(msg),
        Err(err) => Some(format!(
            "WebAdmin was not aligned automatically ({err}). Use /server/openlitespeed to set it (uses your CPN admin account by default)."
        )),
    }
}

/// Reset WebAdmin to the CPN admin username after confirming the CPN admin password.
/// Required when OLS was installed later or credentials drifted (hashes are not reversible).
pub fn reset_webadmin_to_cpn_admin(confirmed_password: &str) -> Result<String, String> {
    let boot = crate::account::load_bootstrap()
        .ok_or_else(|| "CPN admin account is not configured yet.".to_string())?;
    if !crate::account::verify_password(
        confirmed_password,
        &boot.password_salt,
        &boot.password_hash,
    ) {
        return Err("Password does not match the CPN admin account.".into());
    }
    align_webadmin_with_credentials(&boot.username, confirmed_password)
}

#[cfg(test)]
mod tests {
    use super::{valid_password, valid_username, webadmin_username_from_cpn};

    #[test]
    fn username_rules() {
        assert!(valid_username("admin").is_ok());
        assert!(valid_username("guest.ops-1").is_ok());
        assert!(valid_username("bad user").is_err());
        assert!(valid_username("").is_err());
    }

    #[test]
    fn password_rules() {
        assert!(valid_password("short").is_err());
        assert!(valid_password("longenough1").is_ok());
    }

    #[test]
    fn webadmin_username_falls_back_for_invalid() {
        let (name, fell_back) = webadmin_username_from_cpn("ops");
        assert_eq!(name, "ops");
        assert!(!fell_back);
        let (name, fell_back) = webadmin_username_from_cpn("bad user");
        assert_eq!(name, "admin");
        assert!(fell_back);
    }
}
