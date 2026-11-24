//! Versioned, checksummed, idempotent schema migrations for `cpn_panel` and `cpn_mail`.
//!
//! Each database keeps its own `cpn_schema_migrations` ledger. A migration that
//! was applied and later edited is refused (checksum mismatch) rather than
//! silently re-run, so a half-upgraded host fails loudly and stays recoverable.

use super::client::SqlExec;
use super::config::PanelDbConfig;
use super::crypto::sha256_hex;
use super::sqlfmt::{ident, text_lit};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Panel,
    Mail,
}

pub struct Migration {
    pub target: Target,
    pub id: &'static str,
    pub sql: &'static str,
}

pub const MIGRATIONS: &[Migration] = &[
    Migration {
        target: Target::Panel,
        id: "0001_panel_core",
        sql: include_str!("../../sql/mariadb/0001_panel_core.sql"),
    },
    Migration {
        target: Target::Mail,
        id: "0002_cpn_mail",
        sql: include_str!("../../sql/mariadb/0002_cpn_mail.sql"),
    },
];

const LEDGER_DDL: &str = "CREATE TABLE IF NOT EXISTS cpn_schema_migrations (\
id VARCHAR(128) CHARACTER SET ascii NOT NULL PRIMARY KEY, \
checksum CHAR(64) CHARACTER SET ascii NOT NULL, \
applied_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP) ENGINE=InnoDB;";

pub fn checksum(sql: &str) -> String {
    sha256_hex(sql.replace("\r\n", "\n").as_bytes())
}

pub fn db_name(cfg: &PanelDbConfig, target: Target) -> &str {
    match target {
        Target::Panel => &cfg.panel_db,
        Target::Mail => &cfg.mail_db,
    }
}

/// Migrations not yet applied for `target`; errors when an applied one changed.
pub fn pending_for(
    applied: &[(String, String)],
    target: Target,
) -> Result<Vec<&'static Migration>, String> {
    let mut out = Vec::new();
    for m in MIGRATIONS.iter().filter(|m| m.target == target) {
        match applied.iter().find(|(id, _)| id == m.id) {
            Some((_, sum)) if *sum == checksum(m.sql) => {}
            Some(_) => {
                return Err(format!(
                    "Migration {} was edited after it was applied (checksum mismatch); refusing to continue",
                    m.id
                ));
            }
            None => out.push(m),
        }
    }
    Ok(out)
}

/// Apply every pending migration in both databases; returns applied ids.
pub fn migrate(admin: &dyn SqlExec, cfg: &PanelDbConfig) -> Result<Vec<String>, String> {
    let mut done = Vec::new();
    for target in [Target::Panel, Target::Mail] {
        let db = db_name(cfg, target);
        ident(db)?;
        admin.run(Some(db), LEDGER_DDL)?;
        let rows = admin.query(
            Some(db),
            "SELECT id, checksum FROM cpn_schema_migrations ORDER BY id",
        )?;
        let applied: Vec<(String, String)> = rows
            .into_iter()
            .filter_map(|r| Some((r.first()?.clone(), r.get(1)?.clone())))
            .collect();
        for m in pending_for(&applied, target)? {
            let stmt = format!(
                "{}\nINSERT INTO cpn_schema_migrations (id, checksum) VALUES ({}, '{}');\n",
                m.sql,
                text_lit(m.id),
                checksum(m.sql)
            );
            admin
                .run(Some(db), &stmt)
                .map_err(|e| format!("Migration {} failed: {e}", m.id))?;
            done.push(m.id.to_string());
        }
    }
    Ok(done)
}

#[cfg(test)]
mod tests {
    use super::super::client::fake::FakeSql;
    use super::*;

    #[test]
    fn migration_ids_are_unique_and_ordered() {
        let mut ids: Vec<&str> = MIGRATIONS.iter().map(|m| m.id).collect();
        let sorted = {
            let mut s = ids.clone();
            s.sort_unstable();
            s
        };
        assert_eq!(ids, sorted);
        ids.dedup();
        assert_eq!(ids.len(), MIGRATIONS.len());
    }

    #[test]
    fn all_migrations_are_idempotent_ddl() {
        for m in MIGRATIONS {
            let upper = m.sql.to_ascii_uppercase();
            for line in upper.lines() {
                let l = line.trim_start();
                if l.starts_with("CREATE TABLE") {
                    assert!(l.contains("IF NOT EXISTS"), "{}: {l}", m.id);
                }
            }
            assert!(!upper.contains("DROP "), "{} must not drop", m.id);
        }
    }

    #[test]
    fn pending_skips_applied_and_detects_edits() {
        let first = &MIGRATIONS[0];
        let applied = vec![(first.id.to_string(), checksum(first.sql))];
        assert!(pending_for(&applied, Target::Panel).unwrap().is_empty());
        assert_eq!(pending_for(&applied, Target::Mail).unwrap().len(), 1);
        let edited = vec![(first.id.to_string(), "0".repeat(64))];
        assert!(pending_for(&edited, Target::Panel).is_err());
    }

    #[test]
    fn migrate_runs_only_what_is_pending() {
        let cfg = PanelDbConfig::generate(1);
        let sql = FakeSql::default();
        let applied = migrate(&sql, &cfg).unwrap();
        assert_eq!(applied, vec!["0001_panel_core", "0002_cpn_mail"]);
        // Second run: ledger now reports both as applied.
        let sql2 = FakeSql::default();
        let reply: String = MIGRATIONS
            .iter()
            .filter(|m| m.target == Target::Panel)
            .map(|m| format!("{}\t{}\n", m.id, checksum(m.sql)))
            .collect();
        sql2.reply("FROM cpn_schema_migrations", Ok(&reply));
        let again = migrate(&sql2, &cfg).unwrap();
        // Panel already applied; the mail ledger query returns the same canned rows,
        // so only the mail migration (not in the reply) stays pending.
        assert_eq!(again, vec!["0002_cpn_mail"]);
    }
}
