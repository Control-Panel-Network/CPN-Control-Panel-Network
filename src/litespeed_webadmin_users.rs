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
    Err(format!("Could not update WebAdmin password file: {last_err}"))
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
    if try_bcrypt.is_err() {
        run_htpasswd(&[
            if create { "-cb" } else { "-b" },
            &path_s,
            &user,
            pass,
        ])?;
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
    run_htpasswd(&["-D", &path_s, &user])?;
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
    Ok(format!(
        "Removed WebAdmin user `{user}`. {restart}"
    ))
}

pub fn webadmin_users_present() -> bool {
    Path::new(&htpasswd_path()).is_file()
}

#[cfg(test)]
mod tests {
    use super::{valid_password, valid_username};

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
}
