//! `cpn panel-db`: status, migrate, verify, export (rollback), privilege audit,
//! and the `cpn_mail` Dovecot cutover. Output never contains secrets.

use crate::panel_audit_cli::record_cli;
use crate::panel_db::backup::BACKUP_ROOT;
use crate::panel_db::client::{AdminSql, PanelSql};
use crate::panel_db::config::PanelDbConfig;
use crate::panel_db::crypto::PanelKey;
use crate::panel_db::docs_store::MariaDocStore;
use crate::panel_db::import::{export, verify};
use crate::panel_db::mail::{self, CutoverPaths, DOVECOT_SQL_CONF, RealDovecot};
use crate::panel_db::{provision, state};
use crate::paths::default_data_dir;
use std::path::PathBuf;

#[derive(Debug, clap::Subcommand)]
pub enum PanelDbCommands {
    /// Show configuration, row counts, and the last migration result
    Status,
    /// Back up, provision, import, verify (idempotent; also runs on upgrade)
    Migrate {
        /// Run the full migration even when this version already completed one
        #[arg(long)]
        force: bool,
    },
    /// Classify what would be imported without writing anything
    Plan,
    /// Compare every file under the data dir with its database row
    Verify,
    /// Write every stored document back as files (rollback), decrypted, mode 0600
    Export {
        /// Destination directory (a new directory is safest)
        #[arg(long)]
        dest: PathBuf,
        /// Overwrite files that already exist
        #[arg(long)]
        force: bool,
    },
    /// Audit that both database accounts hold only their intended privileges
    Audit,
    /// Switch Dovecot from the classic mailbox table to cpn_mail (auto rollback on failure)
    #[command(name = "mail-cutover")]
    MailCutover,
    /// Restore the previous Dovecot passdb and sync hashes back
    #[command(name = "mail-rollback")]
    MailRollback,
}

fn load_cfg() -> Result<PanelDbConfig, String> {
    PanelDbConfig::load()?
        .ok_or_else(|| "Panel database is not provisioned; run `cpn panel-db migrate`".into())
}

fn cutover_paths() -> CutoverPaths {
    CutoverPaths {
        conf: PathBuf::from(DOVECOT_SQL_CONF),
        backup_dir: PathBuf::from(BACKUP_ROOT).join("dovecot"),
    }
}

fn legacy_db(admin: &AdminSql, cfg: &PanelDbConfig) -> Result<String, String> {
    cfg.legacy_mail_db
        .clone()
        .or_else(|| mail::detect_legacy_db(admin))
        .ok_or_else(|| "No classic mailbox table was found".into())
}

pub fn run(
    command: PanelDbCommands,
    require_root: fn() -> Result<(), String>,
) -> Result<(), String> {
    match command {
        PanelDbCommands::Status => {
            for line in state::status_lines() {
                println!("{line}");
            }
            Ok(())
        }
        PanelDbCommands::Migrate { force } => {
            require_root()?;
            let outcome = state::ensure("cli", force);
            let ok = outcome.is_ok();
            record_cli(
                "cli.panel-db.migrate",
                "panel-db",
                ok,
                if ok { "ok" } else { "failed" },
            );
            println!("{}", outcome?.message());
            Ok(())
        }
        PanelDbCommands::Plan => {
            let entries = crate::panel_db::manifest::scan(&default_data_dir())?;
            let mut counts = [0usize; 4];
            for e in &entries {
                use crate::panel_db::manifest::Class;
                match e.class {
                    Class::Bootstrap => counts[0] += 1,
                    Class::Skip(_) => counts[1] += 1,
                    Class::Deferred { .. } => counts[2] += 1,
                    Class::Document { .. } => counts[3] += 1,
                }
            }
            println!(
                "{} files: {} stay on disk (bootstrap), {} skipped (cache or generated), {} deferred (journals), {} importable documents",
                entries.len(),
                counts[0],
                counts[1],
                counts[2],
                counts[3]
            );
            Ok(())
        }
        PanelDbCommands::Verify => {
            let cfg = load_cfg()?;
            let panel = PanelSql::new(&cfg);
            let store = MariaDocStore::new(&panel, &cfg.panel_db, PanelKey::load(false)?);
            let report = verify(&default_data_dir(), &store)?;
            println!("{}", report.summary());
            for rel in report.missing_in_db.iter().chain(&report.mismatched) {
                println!("  needs sync: {rel}");
            }
            if report.ok() {
                Ok(())
            } else {
                Err("Verification found differences; run `cpn panel-db migrate`".into())
            }
        }
        PanelDbCommands::Export { dest, force } => {
            require_root()?;
            let cfg = load_cfg()?;
            let panel = PanelSql::new(&cfg);
            let store = MariaDocStore::new(&panel, &cfg.panel_db, PanelKey::load(false)?);
            let report = export(&store, &dest, force)?;
            record_cli("cli.panel-db.export", "panel-db", true, "exported");
            println!(
                "Wrote {} file(s); {} already existed",
                report.written, report.skipped_existing
            );
            Ok(())
        }
        PanelDbCommands::Audit => {
            let cfg = load_cfg()?;
            let problems = provision::audit_privileges(&AdminSql, &cfg)?;
            if problems.is_empty() {
                println!("Database accounts hold only their intended privileges");
                return Ok(());
            }
            for p in &problems {
                println!("  {p}");
            }
            Err("Privilege audit found problems".into())
        }
        PanelDbCommands::MailCutover => {
            require_root()?;
            let cfg = load_cfg()?;
            let legacy = legacy_db(&AdminSql, &cfg)?;
            let now = crate::account::now_unix();
            let msg = mail::cutover(
                &AdminSql,
                &cfg,
                &legacy,
                &cutover_paths(),
                &RealDovecot,
                now,
            )?;
            record_cli("cli.panel-db.mail-cutover", "cpn_mail", true, "cutover");
            println!("{msg}");
            Ok(())
        }
        PanelDbCommands::MailRollback => {
            require_root()?;
            let cfg = load_cfg()?;
            let legacy = legacy_db(&AdminSql, &cfg)?;
            let msg = mail::rollback(&AdminSql, &cfg, &legacy, &cutover_paths(), &RealDovecot)?;
            record_cli("cli.panel-db.mail-rollback", "cpn_mail", true, "rollback");
            println!("{msg}");
            Ok(())
        }
    }
}
