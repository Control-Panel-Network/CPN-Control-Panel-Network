//! `cpn panel-migrate` : detect, plan, copy, and remove other hosting panels.

use clap::Subcommand;

use crate::foreign_panel::apply::{ApplyOptions, apply_panel, inventory_note};
use crate::foreign_panel::catalog::{CATALOG, spec_by_id};
use crate::foreign_panel::{detect_at, host_root, wipe_panel};

#[derive(Subcommand, Debug)]
pub enum PanelMigrateCommands {
    /// List panels installed on this host and their website counts
    Detect,
    /// Show the copy plan for one panel (no writes except nothing)
    Plan {
        #[arg(long)]
        id: String,
    },
    /// Copy websites, databases, mail, and DNS into CPN
    Apply {
        #[arg(long)]
        id: String,
        /// Required. Confirms the copy.
        #[arg(long)]
        yes: bool,
        /// Continue when the other panel uses CPN's listen port
        #[arg(long)]
        port_ok: bool,
    },
    /// Remove that panel's programs and duplicate files after a verified apply
    Wipe {
        #[arg(long)]
        id: String,
        /// Required. Confirms deletion of the paths stored in the verify report.
        #[arg(long)]
        yes: bool,
    },
}

pub fn run(command: PanelMigrateCommands) -> Result<(), String> {
    match command {
        PanelMigrateCommands::Detect => detect(),
        PanelMigrateCommands::Plan { id } => plan(&id),
        PanelMigrateCommands::Apply { id, yes, port_ok } => apply(&id, yes, port_ok),
        PanelMigrateCommands::Wipe { id, yes } => wipe(&id, yes),
    }
}

fn detect() -> Result<(), String> {
    let root = host_root();
    let found = detect_at(&root);
    if found.is_empty() {
        println!("No other hosting panels found under {}.", root.display());
        println!("Catalog ({}):", CATALOG.len());
        for spec in CATALOG {
            println!("  {:<16} {}", spec.id, spec.name);
        }
        return Ok(());
    }
    for row in found {
        let detail = if row.inventory.mapped {
            inventory_note(&row.inventory)
        } else {
            row.inventory.note.clone()
        };
        println!("{:<16} {:<28} {}  {}", row.id, row.name, row.tier, detail);
        for site in &row.inventory.sites {
            println!(
                "  {}  {}  dbs={} mail={} dns={}",
                site.domain,
                site.docroot,
                site.databases.len(),
                site.mail_dirs.len(),
                site.zone_files.len()
            );
        }
    }
    Ok(())
}

fn plan(id: &str) -> Result<(), String> {
    let spec = spec_by_id(id).ok_or_else(|| format!("Unknown panel `{id}`."))?;
    let root = host_root();
    let found = detect_at(&root);
    let row = found
        .iter()
        .find(|row| row.id == spec.id)
        .ok_or_else(|| format!("{} is not installed under {}.", spec.name, root.display()))?;
    println!("Plan for {} ({})", spec.name, spec.id);
    if !row.inventory.mapped {
        return Err(row.inventory.note.clone());
    }
    if row.inventory.sites.is_empty() {
        return Err("No websites found.".into());
    }
    for site in &row.inventory.sites {
        println!(
            "site {} owner={} source={}",
            site.domain,
            if site.owner.is_empty() {
                "cpnowner"
            } else {
                site.owner.as_str()
            },
            site.docroot
        );
        for db in &site.databases {
            println!("  database {db}");
        }
        for mail in &site.mail_dirs {
            println!("  mail {mail}");
        }
        for zone in &site.zone_files {
            println!("  dns {zone}");
        }
    }
    if spec.listen_port == 2087 {
        println!(
            "Port note: this panel defaults to 2087. apply requires --port-ok unless CPN uses another port."
        );
    }
    println!("Wipe prefixes (after a verified apply):");
    for rel_path in spec.wipe_rel {
        println!("  {rel_path}");
    }
    Ok(())
}

fn apply(id: &str, yes: bool, port_ok: bool) -> Result<(), String> {
    if !yes {
        return Err("Refusing to copy without --yes.".into());
    }
    let report = apply_panel(
        &host_root(),
        id,
        &ApplyOptions {
            port_ok,
            register_sites: true,
        },
    )?;
    println!(
        "Apply {} verified={} sites={} databases={} mail={} dns={} users={}",
        report.panel_name,
        report.verified,
        report.sites.len(),
        report.databases.len(),
        report.mail_copied.len(),
        report.zones_copied.len(),
        report.users.len()
    );
    for warning in &report.warnings {
        println!("warning: {warning}");
    }
    if report.verified {
        Ok(())
    } else {
        Err(format!(
            "Apply for {} did not verify. Wipe stays blocked.",
            report.panel_name
        ))
    }
}

fn wipe(id: &str, yes: bool) -> Result<(), String> {
    if !yes {
        return Err("Refusing to delete without --yes.".into());
    }
    let paths = wipe_panel(&host_root(), id)?;
    if paths.is_empty() {
        println!("Nothing left to remove for `{id}`.");
    } else {
        println!("Removed {} path(s):", paths.len());
        for path in paths {
            println!("  {path}");
        }
    }
    Ok(())
}
