//! `cpn_mail`: import the classic virtual-mailbox passdb, verify it, and move
//! Dovecot from it to the new database (with automatic rollback on failure).
//!
//! The import runs entirely inside MariaDB (`INSERT ... SELECT`), so password
//! hashes never pass through this process, its logs, or its memory. The old
//! table is never modified by import or cutover, so every step is reversible.

use super::client::SqlExec;
use super::config::PanelDbConfig;
use super::sqlfmt::{ident, safe_name};
use std::fs;
use std::path::{Path, PathBuf};

pub const DOVECOT_SQL_CONF: &str = "/etc/dovecot/dovecot-sql.conf.ext";
const LEGACY_TABLE: &str = "e_users";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PassdbBackend {
    Legacy,
    CpnMail,
    Other,
}

/// Which passdb the given `dovecot-sql.conf.ext` text points at.
pub fn backend_of(conf: &str) -> PassdbBackend {
    let lower = conf.to_ascii_lowercase();
    if lower.contains("dbname=cpn_mail") {
        PassdbBackend::CpnMail
    } else if lower.contains(LEGACY_TABLE) {
        PassdbBackend::Legacy
    } else {
        PassdbBackend::Other
    }
}

/// Find the database that still holds the legacy mailbox table, if any.
pub fn detect_legacy_db(admin: &dyn SqlExec) -> Option<String> {
    let rows = admin
        .query(
            None,
            "SELECT table_schema FROM information_schema.tables WHERE table_name = 'e_users' \
             AND table_schema NOT IN ('mysql','information_schema','performance_schema','sys') LIMIT 1",
        )
        .ok()?;
    let name = rows.first()?.first()?.clone();
    safe_name(&name, 64).ok()?;
    Some(name)
}

/// Server-side import statements; `refresh` overwrites hashes (pre-cutover only).
pub fn import_sql(legacy_db: &str, mail_db: &str, refresh: bool) -> Result<String, String> {
    let l = ident(legacy_db)?;
    let m = ident(mail_db)?;
    let on_dup = if refresh {
        "password_hash = VALUES(password_hash), mail_location = VALUES(mail_location), home = VALUES(home), deleted_at = NULL"
    } else {
        "updated_at = updated_at"
    };
    Ok(format!(
        "INSERT IGNORE INTO {m}.mail_domains (domain) \
           SELECT DISTINCT LOWER(SUBSTRING_INDEX(email, '@', -1)) FROM {l}.{LEGACY_TABLE} WHERE email LIKE '%@%';\n\
         INSERT INTO {m}.mailboxes (email, local_part, domain, password_hash, mail_location, home, uid, gid, enabled, source) \
           SELECT LOWER(e.email), LOWER(SUBSTRING_INDEX(e.email, '@', 1)), LOWER(SUBSTRING_INDEX(e.email, '@', -1)), \
                  e.password, e.mail, \
                  CONCAT('/home/vmail/', SUBSTRING_INDEX(e.email, '@', -1), '/', SUBSTRING_INDEX(e.email, '@', 1)), \
                  5000, 5000, 1, 'legacy' \
           FROM {l}.{LEGACY_TABLE} e \
           WHERE e.email LIKE '%@%' AND e.password IS NOT NULL AND e.password <> '' \
           ON DUPLICATE KEY UPDATE {on_dup};\n\
         INSERT INTO {m}.mail_meta (k, v) VALUES ('last_import', NOW()) ON DUPLICATE KEY UPDATE v = VALUES(v);\n"
    ))
}

pub fn cutover_done(admin: &dyn SqlExec, cfg: &PanelDbConfig) -> Result<bool, String> {
    let rows = admin.query(
        Some(&cfg.mail_db),
        "SELECT v FROM mail_meta WHERE k = 'passdb_cutover'",
    )?;
    Ok(rows
        .first()
        .and_then(|r| r.first())
        .is_some_and(|v| v == "1"))
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MailCounts {
    pub legacy: u64,
    pub cpn_mail: u64,
    pub missing: u64,
    pub hash_mismatch: u64,
}

impl MailCounts {
    /// Safe to switch Dovecot: nothing missing and no stale hash.
    pub fn ready_for_cutover(&self) -> bool {
        self.cpn_mail > 0 && self.missing == 0 && self.hash_mismatch == 0
    }

    pub fn summary(&self) -> String {
        format!(
            "legacy mailboxes {}, cpn_mail mailboxes {}, missing {}, stale hashes {}",
            self.legacy, self.cpn_mail, self.missing, self.hash_mismatch
        )
    }
}

pub fn count_sql(legacy_db: &str, mail_db: &str) -> Result<String, String> {
    let l = ident(legacy_db)?;
    let m = ident(mail_db)?;
    Ok(format!(
        "SELECT \
         (SELECT COUNT(*) FROM {l}.{LEGACY_TABLE} WHERE email LIKE '%@%' AND password IS NOT NULL AND password <> ''), \
         (SELECT COUNT(*) FROM {m}.mailboxes WHERE deleted_at IS NULL), \
         (SELECT COUNT(*) FROM {l}.{LEGACY_TABLE} e LEFT JOIN {m}.mailboxes mb ON mb.email = LOWER(e.email) \
            WHERE e.email LIKE '%@%' AND e.password IS NOT NULL AND e.password <> '' AND mb.email IS NULL), \
         (SELECT COUNT(*) FROM {l}.{LEGACY_TABLE} e JOIN {m}.mailboxes mb ON mb.email = LOWER(e.email) \
            WHERE BINARY e.password <> BINARY mb.password_hash);"
    ))
}

pub fn counts(
    admin: &dyn SqlExec,
    legacy_db: &str,
    cfg: &PanelDbConfig,
) -> Result<MailCounts, String> {
    let rows = admin.query(None, &count_sql(legacy_db, &cfg.mail_db)?)?;
    let row = rows.first().ok_or("Empty mailbox count result")?;
    let n = |i: usize| row.get(i).and_then(|v| v.parse().ok()).unwrap_or(0u64);
    Ok(MailCounts {
        legacy: n(0),
        cpn_mail: n(1),
        missing: n(2),
        hash_mismatch: n(3),
    })
}

/// Import (insert-missing, or refresh) and return the post-import counts.
pub fn import_legacy(
    admin: &dyn SqlExec,
    cfg: &PanelDbConfig,
    legacy_db: &str,
    refresh: bool,
) -> Result<MailCounts, String> {
    if refresh && cutover_done(admin, cfg)? {
        return Err("Refusing to overwrite hashes after the passdb cutover".into());
    }
    admin.run(None, &import_sql(legacy_db, &cfg.mail_db, refresh)?)?;
    counts(admin, legacy_db, cfg)
}

/// Dovecot SQL driver config for `cpn_mail` (SELECT-only account, view only).
pub fn render_dovecot_conf(cfg: &PanelDbConfig) -> Result<String, String> {
    safe_name(&cfg.mail_db, 64)?;
    safe_name(&cfg.mail_auth_user, 80)?;
    super::sqlfmt::safe_password(&cfg.mail_auth_password)?;
    Ok(format!(
        "# Managed by CPN (cpn_mail). `cpn panel-db mail rollback` restores the previous file.\n\
         driver = mysql\n\
         connect = host=localhost dbname={} user={} password={} port=3306\n\
         default_pass_scheme = BLF-CRYPT\n\
         password_query = SELECT email AS user, password_hash AS password FROM mailbox_auth WHERE email='%u';\n\
         user_query = SELECT uid, gid, mail_location AS mail, home FROM mailbox_auth WHERE email='%u';\n\
         iterate_query = SELECT email AS user FROM mailbox_auth;\n",
        cfg.mail_db, cfg.mail_auth_user, cfg.mail_auth_password
    ))
}

/// Dovecot control surface, abstracted so cutover logic is testable.
pub trait DovecotCtl {
    fn config_ok(&self) -> Result<(), String>;
    fn reload(&self) -> Result<(), String>;
    /// userdb lookup for one mailbox (proves `user_query` works end to end).
    fn lookup_ok(&self, email: &str) -> Result<(), String>;
}

pub struct RealDovecot;

impl DovecotCtl for RealDovecot {
    fn config_ok(&self) -> Result<(), String> {
        run_ok("doveconf", &["-n"])
    }
    fn reload(&self) -> Result<(), String> {
        run_ok("doveadm", &["reload"])
    }
    fn lookup_ok(&self, email: &str) -> Result<(), String> {
        run_ok("doveadm", &["user", "-u", email])
    }
}

fn run_ok(bin: &str, args: &[&str]) -> Result<(), String> {
    let out = std::process::Command::new(bin)
        .args(args)
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|e| format!("{bin} could not run: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(format!(
            "{bin} {} failed",
            args.first().copied().unwrap_or("")
        ))
    }
}

#[derive(Debug)]
pub struct CutoverPaths {
    pub conf: PathBuf,
    pub backup_dir: PathBuf,
}

fn write_conf_like(path: &Path, body: &str, like: &Path) -> Result<(), String> {
    super::config::write_private_atomic(path, body.as_bytes())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        if let Ok(meta) = fs::metadata(like) {
            let _ = fs::set_permissions(path, fs::Permissions::from_mode(meta.mode() & 0o777));
            let _ = std::os::unix::fs::chown(path, Some(meta.uid()), Some(meta.gid()));
        }
    }
    #[cfg(not(unix))]
    let _ = like;
    Ok(())
}

fn latest_backup(dir: &Path) -> Option<PathBuf> {
    let mut files: Vec<PathBuf> = fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("dovecot-sql.conf.ext.") && n.ends_with(".bak"))
        })
        .collect();
    files.sort();
    files.pop()
}

/// Switch Dovecot to `cpn_mail` after a fresh refresh + clean verify.
/// Any failure restores the previous config and reloads.
pub fn cutover(
    admin: &dyn SqlExec,
    cfg: &PanelDbConfig,
    legacy_db: &str,
    paths: &CutoverPaths,
    ctl: &dyn DovecotCtl,
    now_unix: u64,
) -> Result<String, String> {
    let current = fs::read_to_string(&paths.conf)
        .map_err(|e| format!("Could not read the Dovecot SQL config: {}", e.kind()))?;
    if backend_of(&current) == PassdbBackend::CpnMail {
        return Ok("Dovecot already uses cpn_mail; nothing to do".into());
    }
    let c = import_legacy(admin, cfg, legacy_db, true)?;
    if !c.ready_for_cutover() {
        return Err(format!(
            "cpn_mail is not ready for cutover ({})",
            c.summary()
        ));
    }
    fs::create_dir_all(&paths.backup_dir)
        .map_err(|e| format!("Could not create backup dir: {}", e.kind()))?;
    let backup = paths
        .backup_dir
        .join(format!("dovecot-sql.conf.ext.{now_unix}.bak"));
    fs::copy(&paths.conf, &backup)
        .map_err(|e| format!("Could not back up config: {}", e.kind()))?;
    let sample = admin
        .query(
            Some(&cfg.mail_db),
            "SELECT email FROM mailboxes WHERE enabled = 1 AND deleted_at IS NULL ORDER BY email LIMIT 1",
        )?
        .first()
        .and_then(|r| r.first().cloned())
        .ok_or("No mailbox available to test the userdb lookup")?;
    write_conf_like(&paths.conf, &render_dovecot_conf(cfg)?, &backup)?;
    let verdict = ctl
        .config_ok()
        .and_then(|()| ctl.reload())
        .and_then(|()| ctl.lookup_ok(&sample));
    if let Err(e) = verdict {
        let _ = write_conf_like(&paths.conf, &current, &backup);
        let _ = ctl.reload();
        return Err(format!("Cutover rolled back automatically: {e}"));
    }
    admin.run(
        Some(&cfg.mail_db),
        "INSERT INTO mail_meta (k, v) VALUES ('passdb_cutover', '1') ON DUPLICATE KEY UPDATE v = '1';",
    )?;
    Ok(format!(
        "Dovecot now authenticates against cpn_mail ({} mailboxes). Previous config kept in the cutover backup dir.",
        c.cpn_mail
    ))
}

/// Copy hashes back to the legacy table and restore the previous Dovecot config.
pub fn rollback(
    admin: &dyn SqlExec,
    cfg: &PanelDbConfig,
    legacy_db: &str,
    paths: &CutoverPaths,
    ctl: &dyn DovecotCtl,
) -> Result<String, String> {
    let backup = latest_backup(&paths.backup_dir).ok_or("No Dovecot config backup found")?;
    let l = ident(legacy_db)?;
    let m = ident(&cfg.mail_db)?;
    admin.run(
        None,
        &format!(
            "UPDATE {l}.{LEGACY_TABLE} e JOIN {m}.mailboxes mb ON mb.email = LOWER(e.email) \
             SET e.password = mb.password_hash WHERE BINARY e.password <> BINARY mb.password_hash;"
        ),
    )?;
    let old =
        fs::read_to_string(&backup).map_err(|e| format!("Could not read backup: {}", e.kind()))?;
    write_conf_like(&paths.conf, &old, &backup)?;
    ctl.config_ok()
        .and_then(|()| ctl.reload())
        .map_err(|e| format!("Restored config but Dovecot reload failed: {e}"))?;
    admin.run(
        Some(&cfg.mail_db),
        "INSERT INTO mail_meta (k, v) VALUES ('passdb_cutover', '0') ON DUPLICATE KEY UPDATE v = '0';",
    )?;
    Ok("Dovecot restored to the previous passdb; password hashes synced back.".into())
}

#[cfg(test)]
mod tests {
    use super::super::client::fake::FakeSql;
    use super::*;
    use std::cell::RefCell;

    struct Ctl {
        fail_lookup: bool,
        calls: RefCell<Vec<&'static str>>,
    }

    impl DovecotCtl for Ctl {
        fn config_ok(&self) -> Result<(), String> {
            self.calls.borrow_mut().push("config");
            Ok(())
        }
        fn reload(&self) -> Result<(), String> {
            self.calls.borrow_mut().push("reload");
            Ok(())
        }
        fn lookup_ok(&self, _: &str) -> Result<(), String> {
            self.calls.borrow_mut().push("lookup");
            if self.fail_lookup {
                Err("userdb lookup failed".into())
            } else {
                Ok(())
            }
        }
    }

    fn cfg() -> PanelDbConfig {
        PanelDbConfig::generate(1)
    }

    fn tmp(tag: &str) -> CutoverPaths {
        let dir = std::env::temp_dir().join(format!("cpn-pdb-mail-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let conf = dir.join("dovecot-sql.conf.ext");
        fs::write(&conf, "connect = host=localhost dbname=legacy user=u password=x\npassword_query = SELECT email as user, password FROM e_users WHERE email='%u';\n").unwrap();
        CutoverPaths {
            conf,
            backup_dir: dir.join("bak"),
        }
    }

    fn ready_sql() -> FakeSql {
        let sql = FakeSql::default();
        sql.reply("SELECT v FROM mail_meta", Ok(""));
        sql.reply("(SELECT COUNT(*)", Ok("3\t3\t0\t0\n"));
        sql.reply("SELECT email FROM mailboxes", Ok("info@example.com\n"));
        sql
    }

    #[test]
    fn backend_detection() {
        assert_eq!(backend_of("dbname=cpn_mail user=x"), PassdbBackend::CpnMail);
        assert_eq!(backend_of("FROM e_users WHERE"), PassdbBackend::Legacy);
        assert_eq!(backend_of("passwd-file"), PassdbBackend::Other);
    }

    #[test]
    fn import_runs_server_side_and_never_selects_hashes_into_the_client() {
        let sql = import_sql("legacydb", "cpn_mail", false).unwrap();
        assert!(sql.contains("INSERT INTO `cpn_mail`.mailboxes"));
        assert!(sql.contains("SELECT LOWER(e.email)"));
        assert!(sql.contains("updated_at = updated_at"));
        let refresh = import_sql("legacydb", "cpn_mail", true).unwrap();
        assert!(refresh.contains("password_hash = VALUES(password_hash)"));
        assert!(import_sql("x`y", "cpn_mail", false).is_err());
    }

    #[test]
    fn rendered_conf_uses_view_and_select_only_account() {
        let c = cfg();
        let text = render_dovecot_conf(&c).unwrap();
        assert!(text.contains("FROM mailbox_auth"));
        assert!(!text.contains("FROM mailboxes"));
        assert!(text.contains(&format!("user={}", c.mail_auth_user)));
        assert!(!text.contains(&c.panel_password));
        assert_eq!(backend_of(&text), PassdbBackend::CpnMail);
    }

    #[test]
    fn cutover_switches_and_records_marker() {
        let paths = tmp("ok");
        let sql = ready_sql();
        let ctl = Ctl {
            fail_lookup: false,
            calls: RefCell::default(),
        };
        let msg = cutover(&sql, &cfg(), "legacydb", &paths, &ctl, 100).unwrap();
        assert!(msg.contains("cpn_mail"));
        let now = fs::read_to_string(&paths.conf).unwrap();
        assert_eq!(backend_of(&now), PassdbBackend::CpnMail);
        assert!(
            paths
                .backup_dir
                .join("dovecot-sql.conf.ext.100.bak")
                .exists()
        );
        assert!(
            sql.statements()
                .iter()
                .any(|s| s.contains("passdb_cutover', '1'"))
        );
        assert_eq!(*ctl.calls.borrow(), vec!["config", "reload", "lookup"]);
        let _ = fs::remove_dir_all(paths.conf.parent().unwrap());
    }

    #[test]
    fn failed_lookup_rolls_back_automatically() {
        let paths = tmp("fail");
        let before = fs::read_to_string(&paths.conf).unwrap();
        let ctl = Ctl {
            fail_lookup: true,
            calls: RefCell::default(),
        };
        let err = cutover(&ready_sql(), &cfg(), "legacydb", &paths, &ctl, 200).unwrap_err();
        assert!(err.contains("rolled back"));
        assert_eq!(fs::read_to_string(&paths.conf).unwrap(), before);
        let _ = fs::remove_dir_all(paths.conf.parent().unwrap());
    }

    #[test]
    fn cutover_refuses_when_hashes_are_stale() {
        let paths = tmp("stale");
        let sql = FakeSql::default();
        sql.reply("SELECT v FROM mail_meta", Ok(""));
        sql.reply("(SELECT COUNT(*)", Ok("3\t2\t1\t0\n"));
        let ctl = Ctl {
            fail_lookup: false,
            calls: RefCell::default(),
        };
        assert!(cutover(&sql, &cfg(), "legacydb", &paths, &ctl, 1).is_err());
        assert!(ctl.calls.borrow().is_empty());
        assert_eq!(
            backend_of(&fs::read_to_string(&paths.conf).unwrap()),
            PassdbBackend::Legacy
        );
        let _ = fs::remove_dir_all(paths.conf.parent().unwrap());
    }

    #[test]
    fn rollback_restores_config_and_syncs_hashes_back() {
        let paths = tmp("rb");
        let sql = ready_sql();
        let ctl = Ctl {
            fail_lookup: false,
            calls: RefCell::default(),
        };
        cutover(&sql, &cfg(), "legacydb", &paths, &ctl, 300).unwrap();
        let msg = rollback(&sql, &cfg(), "legacydb", &paths, &ctl).unwrap();
        assert!(msg.contains("restored"));
        assert_eq!(
            backend_of(&fs::read_to_string(&paths.conf).unwrap()),
            PassdbBackend::Legacy
        );
        assert!(
            sql.statements()
                .iter()
                .any(|s| s.starts_with("UPDATE `legacydb`.e_users"))
        );
        let _ = fs::remove_dir_all(paths.conf.parent().unwrap());
    }

    #[test]
    fn refresh_is_refused_after_cutover() {
        let sql = FakeSql::default();
        sql.reply("SELECT v FROM mail_meta", Ok("1\n"));
        assert!(import_legacy(&sql, &cfg(), "legacydb", true).is_err());
    }
}
