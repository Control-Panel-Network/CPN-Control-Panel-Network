//! Create `cpn_panel` / `cpn_mail`, the dedicated accounts, and least-privilege grants.
//!
//! Privilege model (separate from every website database user):
//! - `cpn_panel`: SELECT/INSERT/UPDATE/DELETE on `cpn_panel.*` and `cpn_mail.*` only,
//!   no DDL, no GRANT OPTION, no global privileges.
//! - `cpn_mail_auth` (Dovecot): SELECT on the `cpn_mail.mailbox_auth` view only.
//!   Both accounts are `@localhost` (unix socket), never remote.

use super::client::{PanelSql, SqlExec};
use super::config::PanelDbConfig;
use super::migrate;
use super::sqlfmt::{ident, safe_name, safe_password};

const DML: &[&str] = &["SELECT", "INSERT", "UPDATE", "DELETE"];

fn account(user: &str) -> Result<String, String> {
    Ok(format!("'{}'@'localhost'", safe_name(user, 80)?))
}

/// Statements that create databases and accounts (before migrations).
pub fn base_sql(cfg: &PanelDbConfig) -> Result<String, String> {
    let panel_db = ident(&cfg.panel_db)?;
    let mail_db = ident(&cfg.mail_db)?;
    let panel = account(&cfg.panel_user)?;
    let auth = account(&cfg.mail_auth_user)?;
    let panel_pw = safe_password(&cfg.panel_password)?;
    let auth_pw = safe_password(&cfg.mail_auth_password)?;
    let dml = DML.join(", ");
    Ok(format!(
        "CREATE DATABASE IF NOT EXISTS {panel_db} CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;\n\
         CREATE DATABASE IF NOT EXISTS {mail_db} CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;\n\
         CREATE USER IF NOT EXISTS {panel} IDENTIFIED BY '{panel_pw}';\n\
         ALTER USER {panel} IDENTIFIED BY '{panel_pw}';\n\
         CREATE USER IF NOT EXISTS {auth} IDENTIFIED BY '{auth_pw}';\n\
         ALTER USER {auth} IDENTIFIED BY '{auth_pw}';\n\
         GRANT {dml} ON {panel_db}.* TO {panel};\n\
         GRANT {dml} ON {mail_db}.* TO {panel};\n\
         FLUSH PRIVILEGES;\n"
    ))
}

/// Grants that need the migrated `mailbox_auth` view to exist.
pub fn auth_grant_sql(cfg: &PanelDbConfig) -> Result<String, String> {
    let mail_db = ident(&cfg.mail_db)?;
    let auth = account(&cfg.mail_auth_user)?;
    Ok(format!(
        "GRANT SELECT ON {mail_db}.`mailbox_auth` TO {auth};\nFLUSH PRIVILEGES;\n"
    ))
}

/// Create (or heal) databases, accounts, schema, and grants. Idempotent.
pub fn provision(admin: &dyn SqlExec, cfg: &PanelDbConfig) -> Result<Vec<String>, String> {
    admin.run(None, &base_sql(cfg)?)?;
    let applied = migrate::migrate(admin, cfg)?;
    admin.run(None, &auth_grant_sql(cfg)?)?;
    Ok(applied)
}

/// Prove both accounts actually work with their own credentials.
pub fn self_test(cfg: &PanelDbConfig) -> Result<(), String> {
    let panel = PanelSql::new(cfg);
    panel
        .run(Some(&cfg.panel_db), "SELECT COUNT(*) FROM panel_documents")
        .map_err(|e| format!("Panel account self-test failed: {e}"))?;
    panel
        .run(Some(&cfg.mail_db), "SELECT COUNT(*) FROM mailboxes")
        .map_err(|e| format!("Panel account mail self-test failed: {e}"))?;
    PanelSql::mail_auth(cfg)
        .run(Some(&cfg.mail_db), "SELECT COUNT(*) FROM mailbox_auth")
        .map_err(|e| format!("Mail auth account self-test failed: {e}"))?;
    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
pub struct GrantRule {
    /// Backtick-stripped target, for example `cpn_panel.*`.
    pub target: String,
    pub allowed: Vec<&'static str>,
}

pub fn expected_rules(cfg: &PanelDbConfig) -> (Vec<GrantRule>, Vec<GrantRule>) {
    let panel = vec![
        GrantRule {
            target: format!("{}.*", cfg.panel_db),
            allowed: DML.to_vec(),
        },
        GrantRule {
            target: format!("{}.*", cfg.mail_db),
            allowed: DML.to_vec(),
        },
    ];
    let auth = vec![GrantRule {
        target: format!("{}.mailbox_auth", cfg.mail_db),
        allowed: vec!["SELECT"],
    }];
    (panel, auth)
}

/// Check `SHOW GRANTS` output lines against `rules`; returns violations.
/// Only privilege names and targets are inspected and reported, never hashes.
pub fn grant_violations(lines: &[String], rules: &[GrantRule]) -> Vec<String> {
    let mut out = Vec::new();
    for line in lines {
        let upper = line.to_ascii_uppercase();
        let Some(rest) = upper.strip_prefix("GRANT ") else {
            continue;
        };
        let (privs, after) = match rest.split_once(" ON ") {
            Some(parts) => parts,
            None => continue,
        };
        let target_raw = after.split(" TO ").next().unwrap_or("").replace('`', "");
        let target_lower = target_raw.trim().to_ascii_lowercase();
        if upper.contains("WITH GRANT OPTION") {
            out.push(format!("{target_lower}: GRANT OPTION is not allowed"));
        }
        let granted: Vec<&str> = privs.split(',').map(str::trim).collect();
        if target_lower == "*.*" {
            if granted.iter().any(|p| *p != "USAGE") {
                out.push("global privileges are not allowed".into());
            }
            continue;
        }
        let Some(rule) = rules
            .iter()
            .find(|r| r.target.eq_ignore_ascii_case(&target_lower))
        else {
            out.push(format!("unexpected grant on {target_lower}"));
            continue;
        };
        for p in granted {
            if p != "USAGE" && !rule.allowed.contains(&p) {
                out.push(format!("{target_lower}: extra privilege {p}"));
            }
        }
    }
    out
}

/// Audit the live grants of both accounts (read via admin access).
pub fn audit_privileges(admin: &dyn SqlExec, cfg: &PanelDbConfig) -> Result<Vec<String>, String> {
    let (panel_rules, auth_rules) = expected_rules(cfg);
    let mut problems = Vec::new();
    for (user, rules) in [
        (&cfg.panel_user, panel_rules),
        (&cfg.mail_auth_user, auth_rules),
    ] {
        let sql = format!("SHOW GRANTS FOR {}", account(user)?);
        let lines: Vec<String> = admin.run(None, &sql)?.lines().map(str::to_string).collect();
        for v in grant_violations(&lines, &rules) {
            problems.push(format!("{user}: {v}"));
        }
    }
    Ok(problems)
}

#[cfg(test)]
mod tests {
    use super::super::client::fake::FakeSql;
    use super::*;

    fn cfg() -> PanelDbConfig {
        PanelDbConfig::generate(1)
    }

    #[test]
    fn base_sql_is_least_privilege() {
        let sql = base_sql(&cfg()).unwrap();
        let upper = sql.to_ascii_uppercase();
        assert!(upper.contains("@'LOCALHOST'"));
        assert!(!upper.contains("'%'"));
        assert!(!upper.contains("GRANT OPTION"));
        assert!(!upper.contains("ALL PRIVILEGES"));
        assert!(!upper.contains("ON *.*"));
        for line in sql.lines().filter(|l| l.starts_with("GRANT")) {
            assert!(line.contains("SELECT, INSERT, UPDATE, DELETE"), "{line}");
            assert!(!line.contains("CREATE") && !line.contains("DROP") && !line.contains("ALTER"));
        }
        // Dovecot account never gets table-wide access in the base step.
        assert!(!sql.contains("GRANT SELECT ON `cpn_mail`.*"));
    }

    #[test]
    fn auth_account_only_reads_the_view() {
        let sql = auth_grant_sql(&cfg()).unwrap();
        assert!(sql.contains("`cpn_mail`.`mailbox_auth`"));
        assert!(!sql.contains(".*"));
        assert!(sql.contains("GRANT SELECT"));
    }

    #[test]
    fn hostile_names_are_refused() {
        let mut c = cfg();
        c.panel_db = "x`; DROP DATABASE mysql; --".into();
        assert!(base_sql(&c).is_err());
        let mut c = cfg();
        c.panel_password = "a'b".into();
        assert!(base_sql(&c).is_err());
    }

    #[test]
    fn provision_orders_base_migrate_grant() {
        let sql = FakeSql::default();
        provision(&sql, &cfg()).unwrap();
        let stmts = sql.statements();
        assert!(stmts.first().unwrap().contains("CREATE DATABASE"));
        assert!(stmts.last().unwrap().contains("mailbox_auth"));
        assert!(
            stmts
                .iter()
                .any(|s| s.contains("CREATE TABLE IF NOT EXISTS mailboxes"))
        );
    }

    #[test]
    fn grant_audit_accepts_expected_and_flags_extras() {
        let c = cfg();
        let (panel_rules, auth_rules) = expected_rules(&c);
        let ok = vec![
            "GRANT USAGE ON *.* TO `cpn_panel`@`localhost` IDENTIFIED BY PASSWORD '*ABC'".into(),
            "GRANT SELECT, INSERT, UPDATE, DELETE ON `cpn_panel`.* TO `cpn_panel`@`localhost`"
                .into(),
            "GRANT SELECT, INSERT, UPDATE, DELETE ON `cpn_mail`.* TO `cpn_panel`@`localhost`"
                .into(),
        ];
        assert!(grant_violations(&ok, &panel_rules).is_empty());
        let bad = vec![
            "GRANT ALL PRIVILEGES ON *.* TO `cpn_panel`@`localhost` WITH GRANT OPTION".into(),
            "GRANT SELECT, DROP ON `cpn_panel`.* TO `cpn_panel`@`localhost`".into(),
            "GRANT SELECT ON `news_cms`.* TO `cpn_panel`@`localhost`".into(),
        ];
        let v = grant_violations(&bad, &panel_rules);
        assert!(v.iter().any(|m| m.contains("GRANT OPTION")));
        assert!(v.iter().any(|m| m.contains("global")));
        assert!(v.iter().any(|m| m.contains("extra privilege DROP")));
        assert!(v.iter().any(|m| m.contains("unexpected grant on news_cms")));
        let auth_bad = vec!["GRANT SELECT ON `cpn_mail`.* TO `cpn_mail_auth`@`localhost`".into()];
        assert!(!grant_violations(&auth_bad, &auth_rules).is_empty());
        let auth_ok =
            vec!["GRANT SELECT ON `cpn_mail`.`mailbox_auth` TO `cpn_mail_auth`@`localhost`".into()];
        assert!(grant_violations(&auth_ok, &auth_rules).is_empty());
    }

    #[test]
    fn audit_reads_both_accounts_without_leaking_hashes() {
        let sql = FakeSql::default();
        sql.reply(
            "'cpn_panel'@'localhost'",
            Ok("GRANT USAGE ON *.* TO `cpn_panel`@`localhost` IDENTIFIED BY PASSWORD '*SECRETHASH'\n\
                GRANT SELECT, INSERT, UPDATE, DELETE ON `cpn_panel`.* TO `cpn_panel`@`localhost`\n"),
        );
        sql.reply(
            "'cpn_mail_auth'@'localhost'",
            Ok("GRANT SELECT ON `cpn_mail`.* TO `cpn_mail_auth`@`localhost`\n"),
        );
        let problems = audit_privileges(&sql, &cfg()).unwrap();
        assert_eq!(problems.len(), 1);
        assert!(problems[0].contains("cpn_mail_auth"));
        assert!(!problems.join(" ").contains("SECRETHASH"));
    }
}
