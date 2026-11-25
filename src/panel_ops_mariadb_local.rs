//! Local MariaDB CLI with credential fallbacks for classic migrated hosts.
//!
//! Greenfield installs often use unix-socket auth as root. After a classic panel
//! cutover, MariaDB `root` may require a password while Dovecot still reads
//! `e_users`. Mail and import helpers try socket auth first, then well-known
//! credential stores (CPN data dir, classic panel paths, Dovecot SQL connect).

use crate::panel_ops_db::mariadb_client_bin;
use crate::panel_ops_db_secret::redact_db_cli_stderr;
use crate::paths::join_data;
use std::fmt::Write as FmtWrite;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

const CLASSIC_MYSQL_PASSWORD_FILE: &str = "/etc/cyberpanel/mysqlPassword";
const DOVECOT_SQL_CONF: &str = "/etc/dovecot/dovecot-sql.conf.ext";
const ROOT_MY_CNF: &str = "/root/.my.cnf";
const DEBIAN_CNF: &str = "/etc/mysql/debian.cnf";

#[derive(Debug, Clone)]
struct MariaDbLogin {
    user: Option<String>,
    password: Option<String>,
    host: Option<String>,
    source: &'static str,
}

impl MariaDbLogin {
    fn socket_default() -> Self {
        Self {
            user: None,
            password: None,
            host: None,
            source: "socket",
        }
    }

    fn needs_defaults_file(&self) -> bool {
        self.password.is_some()
    }
}

struct TempCnf {
    path: PathBuf,
}

impl TempCnf {
    fn write(login: &MariaDbLogin) -> Result<Self, String> {
        let user = login
            .user
            .as_deref()
            .filter(|u| !u.is_empty())
            .unwrap_or("root");
        let password = login.password.as_deref().ok_or_else(|| {
            "Internal error: defaults file requested without password".to_string()
        })?;
        let mut dir = join_data("tmp");
        if fs::create_dir_all(&dir).is_err() {
            dir = std::env::temp_dir();
        }
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = dir.join(format!("mariadb-client-{}-{stamp}.cnf", std::process::id()));
        let quoted = quote_cnf_value(password);
        let mut body = String::from("[client]\n");
        let _ = writeln!(body, "user={user}");
        let _ = writeln!(body, "password={quoted}");
        if let Some(host) = login.host.as_deref().filter(|h| !h.is_empty()) {
            let _ = writeln!(body, "host={host}");
        }
        fs::write(&path, body)
            .map_err(|e| format!("Could not write MariaDB client defaults: {e}"))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
        }
        Ok(Self { path })
    }
}

impl Drop for TempCnf {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn quote_cnf_value(value: &str) -> String {
    if value
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
    {
        return value.to_string();
    }
    let escaped = value.replace('\\', "\\\\").replace('\"', "\\\"");
    format!("\"{escaped}\"")
}

/// Run the local MariaDB client, trying socket auth then known credential sources.
pub fn run_local_mariadb(extra_args: &[&str]) -> Result<Output, String> {
    run_local_mariadb_inner(extra_args, None)
}

/// Like [`run_local_mariadb`], but feeds `sql` on stdin so statements that carry
/// secrets (for example `IDENTIFIED BY`) never appear in the process argument list.
pub fn run_local_mariadb_stdin(extra_args: &[&str], sql: &str) -> Result<Output, String> {
    run_local_mariadb_inner(extra_args, Some(sql))
}

fn run_local_mariadb_inner(extra_args: &[&str], stdin_sql: Option<&str>) -> Result<Output, String> {
    let bin = mariadb_client_bin().ok_or_else(|| "MariaDB client is not installed".to_string())?;
    let attempts = local_login_chain();
    let mut last_err = String::new();
    for login in attempts {
        let output = match invoke_client(bin, &login, extra_args, stdin_sql) {
            Ok(o) => o,
            Err(e) => {
                last_err = e;
                continue;
            }
        };
        if output.status.success() {
            return Ok(output);
        }
        let stderr = redact_db_cli_stderr(&String::from_utf8_lossy(&output.stderr));
        last_err = stderr.clone();
        if !should_retry_auth(&stderr) {
            return Err(format!("MariaDB client failed: {}", stderr.trim()));
        }
    }
    if last_err.trim().is_empty() {
        Err("MariaDB client failed (no credentials worked)".into())
    } else {
        Err(format!("MariaDB client failed: {}", last_err.trim()))
    }
}

fn invoke_client(
    bin: &str,
    login: &MariaDbLogin,
    extra_args: &[&str],
    stdin_sql: Option<&str>,
) -> Result<Output, String> {
    let mut cmd = Command::new(bin);
    cmd.stdin(if stdin_sql.is_some() {
        Stdio::piped()
    } else {
        Stdio::null()
    });
    if login.needs_defaults_file() {
        let cnf = TempCnf::write(login)?;
        cmd.arg(format!("--defaults-extra-file={}", cnf.path.display()));
    } else if let Some(user) = login.user.as_deref().filter(|u| !u.is_empty()) {
        cmd.args(["-u", user]);
        if let Some(host) = login.host.as_deref().filter(|h| !h.is_empty()) {
            cmd.args(["-h", host]);
        }
    }
    cmd.args(extra_args);
    let Some(sql) = stdin_sql else {
        return cmd
            .output()
            .map_err(|e| format!("Could not run MariaDB client ({}): {e}", login.source));
    };
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Could not run MariaDB client ({}): {e}", login.source))?;
    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        // A broken pipe means the client exited early; its status/stderr explain why.
        let _ = stdin.write_all(sql.as_bytes());
    }
    child
        .wait_with_output()
        .map_err(|e| format!("MariaDB client did not finish ({}): {e}", login.source))
}

fn should_retry_auth(stderr: &str) -> bool {
    let lower = stderr.to_ascii_lowercase();
    lower.contains("access denied") || lower.contains("using password: no")
}

fn local_login_chain() -> Vec<MariaDbLogin> {
    let mut out = vec![MariaDbLogin::socket_default()];
    let mut seen = std::collections::BTreeSet::new();
    for login in discover_file_logins() {
        let key = (
            login.user.clone(),
            login.password.clone(),
            login.host.clone(),
        );
        if seen.insert(key) {
            out.push(login);
        }
    }
    out
}

fn discover_file_logins() -> Vec<MariaDbLogin> {
    let mut out = Vec::new();
    if let Some(password) = read_trimmed_file(Path::new(CLASSIC_MYSQL_PASSWORD_FILE)) {
        out.push(MariaDbLogin {
            user: Some("root".into()),
            password: Some(password),
            host: None,
            source: "classic-mysql-password",
        });
    }
    out.extend(read_cpn_mariadb_admin_secret());
    out.extend(parse_my_cnf(Path::new(ROOT_MY_CNF), "root-mycnf"));
    out.extend(parse_my_cnf(Path::new(DEBIAN_CNF), "debian-cnf"));
    if let Some(login) = parse_dovecot_sql_connect() {
        out.push(login);
    }
    out
}

fn read_cpn_mariadb_admin_secret() -> Vec<MariaDbLogin> {
    let path = join_data("mariadb-admin.secret");
    let Some(raw) = fs::read_to_string(&path).ok() else {
        return Vec::new();
    };
    let mut user = Some("root".to_string());
    let mut password = None;
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            let key = k.trim().to_ascii_lowercase();
            let value = v.trim();
            if value.is_empty() {
                continue;
            }
            match key.as_str() {
                "user" => user = Some(value.to_string()),
                "password" | "pass" => password = Some(value.to_string()),
                _ => {}
            }
        } else if password.is_none() {
            password = Some(line.to_string());
        }
    }
    let Some(pass) = password.filter(|p| !p.is_empty()) else {
        return Vec::new();
    };
    vec![MariaDbLogin {
        user,
        password: Some(pass),
        host: None,
        source: "cpn-mariadb-admin-secret",
    }]
}

fn read_trimmed_file(path: &Path) -> Option<String> {
    let raw = fs::read_to_string(path).ok()?;
    let value = raw.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn push_my_cnf_login(
    out: &mut Vec<MariaDbLogin>,
    source: &'static str,
    user: &mut Option<String>,
    password: &mut Option<String>,
    host: &mut Option<String>,
) {
    if let Some(pass) = password.take().filter(|p| !p.is_empty()) {
        out.push(MariaDbLogin {
            user: user.take().or_else(|| Some("root".into())),
            password: Some(pass),
            host: host.take(),
            source,
        });
    }
}

fn parse_my_cnf(path: &Path, source: &'static str) -> Vec<MariaDbLogin> {
    let Ok(raw) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut in_client = false;
    let mut user = None;
    let mut password = None;
    let mut host = None;
    let mut out = Vec::new();
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            push_my_cnf_login(&mut out, source, &mut user, &mut password, &mut host);
            let section = line[1..line.len() - 1].trim().to_ascii_lowercase();
            in_client = section == "client";
            user = None;
            password = None;
            host = None;
            continue;
        }
        if !in_client {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        let key = k.trim().to_ascii_lowercase();
        let value = v.trim().trim_matches('"');
        match key.as_str() {
            "user" => user = Some(value.to_string()),
            "password" | "pass" => password = Some(value.to_string()),
            "host" => host = Some(value.to_string()),
            _ => {}
        }
    }
    push_my_cnf_login(&mut out, source, &mut user, &mut password, &mut host);
    out
}

fn parse_dovecot_sql_connect() -> Option<MariaDbLogin> {
    let raw = fs::read_to_string(DOVECOT_SQL_CONF).ok()?;
    let mut connect = None;
    for line in raw.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("connect") else {
            continue;
        };
        let rest = rest.trim_start_matches('=').trim();
        let rest = rest.trim_matches('"');
        if !rest.is_empty() {
            connect = Some(rest.to_string());
        }
    }
    let connect = connect?;
    let mut user = None;
    let mut password = None;
    let mut host = None;
    for token in connect.split_whitespace() {
        let Some((k, v)) = token.split_once('=') else {
            continue;
        };
        match k.to_ascii_lowercase().as_str() {
            "user" => user = Some(v.to_string()),
            "password" | "pass" => password = Some(v.to_string()),
            "host" => host = Some(v.to_string()),
            _ => {}
        }
    }
    let pass = password.filter(|p| !p.is_empty())?;
    Some(MariaDbLogin {
        user: user.or_else(|| Some("cyberpanel".into())),
        password: Some(pass),
        host,
        source: "dovecot-sql-connect",
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_dovecot_connect_tokens() {
        let connect = "host=127.0.0.1 dbname=cyberpanel user=mailuser password=TestCredAlphaNum99";
        let mut user = None;
        let mut password = None;
        for token in connect.split_whitespace() {
            let Some((k, v)) = token.split_once('=') else {
                continue;
            };
            match k.to_ascii_lowercase().as_str() {
                "user" => user = Some(v.to_string()),
                "password" => password = Some(v.to_string()),
                _ => {}
            }
        }
        assert_eq!(user.as_deref(), Some("mailuser"));
        assert_eq!(password.as_deref(), Some("TestCredAlphaNum99"));
    }

    #[test]
    fn quote_cnf_escapes_specials() {
        let probe: String = ['a', 'b', '!', '@'].into_iter().collect();
        let quoted = quote_cnf_value(&probe);
        assert!(quoted.starts_with('"'));
        assert!(quoted.ends_with('"'));
    }

    #[test]
    fn retry_only_on_access_denied() {
        assert!(should_retry_auth(
            "ERROR 1045 (28000): Access denied for user"
        ));
        assert!(!should_retry_auth(
            "ERROR 1146 (42S02): Table does not exist"
        ));
    }
}
