//! `cpn secrets`: audit and heal panel secrets without ever printing a value.

use crate::secret_hygiene_audit::{SecretReport, audit, heal, render_table};
use clap::Subcommand;

#[derive(Subcommand, Debug)]
pub enum SecretsCommands {
    /// Report presence, length, entropy estimate, file mode, and weak/default state (no values)
    Audit {
        /// Machine-readable output
        #[arg(long)]
        json: bool,
    },
    /// Rotate weak or default secrets and tighten file modes (root; strong secrets are kept)
    Heal,
}

fn print_audit(json: bool, reports: &[SecretReport]) -> Result<(), String> {
    if json {
        let body = serde_json::to_string_pretty(reports).map_err(|e| e.to_string())?;
        println!("{body}");
    } else {
        print!("{}", render_table(reports));
    }
    Ok(())
}

pub fn run(
    command: SecretsCommands,
    require_root: fn() -> Result<(), String>,
) -> Result<(), String> {
    match command {
        SecretsCommands::Audit { json } => print_audit(json, &audit()),
        SecretsCommands::Heal => {
            require_root()?;
            let notes = heal();
            if notes.is_empty() {
                println!("All secrets are strong; nothing to rotate.");
            } else {
                for line in &notes {
                    println!("{line}");
                }
            }
            let weak: Vec<SecretReport> =
                audit().into_iter().filter(|r| r.weak.is_some()).collect();
            if weak.is_empty() {
                Ok(())
            } else {
                print!("{}", render_table(&weak));
                Err(format!(
                    "{} secret(s) are still weak and cannot be rotated automatically",
                    weak.len()
                ))
            }
        }
    }
}
