//! MariaDB access for the panel-state store.
//!
//! Everything goes through the local `mariadb` client with SQL on stdin, the
//! same transport the rest of CPN uses. Credentials reach the client through a
//! short-lived 0600 defaults file, never argv. [`SqlExec`] is a trait so the
//! import, mail, and migration logic is unit-tested without a server.

use super::config::PanelDbConfig;
use super::sqlfmt::{parse_rows, sanitize_sql_error};
use crate::panel_ops_db::mariadb_client_bin;
use crate::panel_ops_mariadb_local::run_local_mariadb_stdin;
use crate::paths::join_data;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

const BASE_ARGS: [&str; 4] = [
    "--batch",
    "--skip-column-names",
    "--default-character-set=utf8mb4",
    "--connect-timeout=5",
];

pub trait SqlExec {
    /// Run `sql` (one or more statements) and return raw tab-separated stdout.
    fn run(&self, db: Option<&str>, sql: &str) -> Result<String, String>;

    fn query(&self, db: Option<&str>, sql: &str) -> Result<Vec<Vec<String>>, String> {
        self.run(db, sql).map(|out| parse_rows(&out))
    }
}

fn finish(output: Output) -> Result<String, String> {
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
    }
    Err(sanitize_sql_error(&String::from_utf8_lossy(&output.stderr)))
}

fn args_for(db: Option<&str>) -> Vec<String> {
    let mut args: Vec<String> = BASE_ARGS.iter().map(|a| a.to_string()).collect();
    if let Some(db) = db {
        args.push("-D".into());
        args.push(db.into());
    }
    args
}

/// Administrative access (socket auth, then known credential stores).
pub struct AdminSql;

impl SqlExec for AdminSql {
    fn run(&self, db: Option<&str>, sql: &str) -> Result<String, String> {
        let owned = args_for(db);
        let args: Vec<&str> = owned.iter().map(String::as_str).collect();
        run_local_mariadb_stdin(&args, sql)
            .map_err(|e| sanitize_sql_error(&e))
            .and_then(finish)
    }
}

/// Least-privilege access as the dedicated panel account.
pub struct PanelSql {
    user: String,
    password: String,
}

impl PanelSql {
    pub fn new(cfg: &PanelDbConfig) -> Self {
        Self {
            user: cfg.panel_user.clone(),
            password: cfg.panel_password.clone(),
        }
    }

    /// Account used by Dovecot; exposed so provisioning can self-test it.
    pub fn mail_auth(cfg: &PanelDbConfig) -> Self {
        Self {
            user: cfg.mail_auth_user.clone(),
            password: cfg.mail_auth_password.clone(),
        }
    }
}

struct TempCnf(PathBuf);

impl TempCnf {
    fn write(user: &str, password: &str) -> Result<Self, String> {
        let mut dir = join_data("tmp");
        if fs::create_dir_all(&dir).is_err() {
            dir = std::env::temp_dir();
        }
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = dir.join(format!("panel-db-{}-{stamp}.cnf", std::process::id()));
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options
            .open(&path)
            .map_err(|e| format!("Could not create MariaDB client defaults: {e}"))?;
        // Values are validated alphanumeric/underscore, so no cnf quoting is needed.
        file.write_all(format!("[client]\nuser={user}\npassword={password}\n").as_bytes())
            .map_err(|e| format!("Could not write MariaDB client defaults: {e}"))?;
        Ok(Self(path))
    }
}

impl Drop for TempCnf {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

impl SqlExec for PanelSql {
    fn run(&self, db: Option<&str>, sql: &str) -> Result<String, String> {
        super::sqlfmt::safe_name(&self.user, 80)?;
        super::sqlfmt::safe_password(&self.password)?;
        let bin = mariadb_client_bin().ok_or("MariaDB client is not installed")?;
        let cnf = TempCnf::write(&self.user, &self.password)?;
        let mut cmd = Command::new(bin);
        cmd.arg(format!("--defaults-extra-file={}", cnf.0.display()))
            .args(args_for(db))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = cmd
            .spawn()
            .map_err(|e| format!("Could not run MariaDB client: {e}"))?;
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(sql.as_bytes());
        }
        let output = child
            .wait_with_output()
            .map_err(|e| format!("MariaDB client did not finish: {e}"))?;
        finish(output)
    }
}

#[cfg(test)]
pub(crate) mod fake {
    //! In-memory `SqlExec` double: records statements and replays canned output.
    use super::SqlExec;
    use std::cell::RefCell;

    #[derive(Default)]
    pub struct FakeSql {
        pub log: RefCell<Vec<(Option<String>, String)>>,
        pub replies: RefCell<Vec<(String, Result<String, String>)>>,
    }

    impl FakeSql {
        /// First matching `needle` in the SQL text decides the reply.
        pub fn reply(&self, needle: &str, out: Result<&str, &str>) {
            self.replies.borrow_mut().push((
                needle.to_string(),
                out.map(str::to_string).map_err(str::to_string),
            ));
        }

        pub fn statements(&self) -> Vec<String> {
            self.log.borrow().iter().map(|(_, s)| s.clone()).collect()
        }
    }

    impl SqlExec for FakeSql {
        fn run(&self, db: Option<&str>, sql: &str) -> Result<String, String> {
            self.log
                .borrow_mut()
                .push((db.map(str::to_string), sql.to_string()));
            for (needle, reply) in self.replies.borrow().iter() {
                if sql.contains(needle.as_str()) {
                    return reply.clone();
                }
            }
            Ok(String::new())
        }
    }
}
