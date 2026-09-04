//! CLI helpers for `cpn package …`.

use crate::packages::{
    PackageInput, assign_package, create_package, delete_package, ensure_default_package,
    format_limit_display, get_package, list_packages, package_for_account, update_package,
    usage_for_account,
};
use clap::Subcommand;

#[derive(Subcommand, Debug)]
pub enum PackageCommands {
    /// List hosting packages
    List,
    /// Create a package
    Create {
        #[arg(long)]
        name: String,
        #[arg(long, default_value_t = 1000)]
        disk_mb: i64,
        #[arg(long, default_value_t = 1000)]
        bandwidth_mb: i64,
        #[arg(long, default_value_t = 20)]
        domains: i64,
        #[arg(long, default_value_t = 1000)]
        emails: i64,
        #[arg(long, default_value_t = 1000)]
        databases: i64,
        #[arg(long, default_value_t = 1000)]
        ftp_accounts: i64,
        #[arg(long, default_value_t = true)]
        fqdn_enabled: bool,
        #[arg(long, default_value = "")]
        notes: String,
    },
    /// Update an existing package by id
    Update {
        #[arg(long)]
        id: String,
        #[arg(long)]
        name: String,
        #[arg(long)]
        disk_mb: i64,
        #[arg(long)]
        bandwidth_mb: i64,
        #[arg(long)]
        domains: i64,
        #[arg(long)]
        emails: i64,
        #[arg(long)]
        databases: i64,
        #[arg(long)]
        ftp_accounts: i64,
        #[arg(long)]
        fqdn_enabled: bool,
        #[arg(long, default_value = "")]
        notes: String,
    },
    /// Delete a package (fails when accounts are still assigned)
    Delete {
        #[arg(long)]
        id: String,
        #[arg(long)]
        yes: bool,
    },
    /// Assign a package to an account
    Assign {
        #[arg(long)]
        username: String,
        #[arg(long)]
        package: String,
    },
    /// Show package limits and usage for an account
    Show {
        #[arg(long)]
        username: String,
    },
}

fn print_package_line(pkg: &crate::packages::Package) {
    println!(
        "{}\tid={}\tdisk={}\tbw={}\tdomains={}\temails={}\tdbs={}\tftp={}\tfqdn={}",
        pkg.name,
        pkg.id,
        format_limit_display(pkg.disk_mb, "MB"),
        format_limit_display(pkg.bandwidth_mb, "MB"),
        format_limit_display(pkg.domains, ""),
        format_limit_display(pkg.emails, ""),
        format_limit_display(pkg.databases, ""),
        format_limit_display(pkg.ftp_accounts, ""),
        pkg.fqdn_enabled
    );
}

pub fn run(
    command: PackageCommands,
    require_root: impl Fn() -> Result<(), String>,
    confirm_delete: impl Fn(&str, bool) -> Result<(), String>,
) -> Result<(), String> {
    let _ = ensure_default_package()?;
    match command {
        PackageCommands::List => {
            let packages = list_packages()?;
            if packages.is_empty() {
                println!("(no packages)");
                return Ok(());
            }
            for pkg in packages {
                print_package_line(&pkg);
            }
            Ok(())
        }
        PackageCommands::Create {
            name,
            disk_mb,
            bandwidth_mb,
            domains,
            emails,
            databases,
            ftp_accounts,
            fqdn_enabled,
            notes,
        } => {
            require_root()?;
            let pkg = create_package(PackageInput {
                name,
                disk_mb,
                bandwidth_mb,
                domains,
                emails,
                databases,
                ftp_accounts,
                fqdn_enabled,
                notes,
            })?;
            println!("created package {} id={}", pkg.name, pkg.id);
            Ok(())
        }
        PackageCommands::Update {
            id,
            name,
            disk_mb,
            bandwidth_mb,
            domains,
            emails,
            databases,
            ftp_accounts,
            fqdn_enabled,
            notes,
        } => {
            require_root()?;
            let _ = get_package(&id)?;
            let pkg = update_package(
                &id,
                PackageInput {
                    name,
                    disk_mb,
                    bandwidth_mb,
                    domains,
                    emails,
                    databases,
                    ftp_accounts,
                    fqdn_enabled,
                    notes,
                },
            )?;
            println!("updated package {} id={}", pkg.name, pkg.id);
            Ok(())
        }
        PackageCommands::Delete { id, yes } => {
            require_root()?;
            confirm_delete(
                &format!("Delete package `{id}`? This cannot be undone."),
                yes,
            )?;
            delete_package(&id)?;
            println!("deleted package {id}");
            Ok(())
        }
        PackageCommands::Assign { username, package } => {
            require_root()?;
            assign_package(&username, &package)?;
            let pkg = package_for_account(&username)?;
            println!("assigned package {} to account ok", pkg.name);
            Ok(())
        }
        PackageCommands::Show { username } => {
            let pkg = package_for_account(&username)?;
            let usage = usage_for_account(&username)?;
            print_package_line(&pkg);
            println!(
                "usage\tdomains={}/{}\temails={}/{}\tdbs={}/{}\tftp={}/{}\tdisk_mb={}/{}",
                usage.domains_used,
                format_limit_display(usage.domains_limit, ""),
                usage.emails_used,
                format_limit_display(usage.emails_limit, ""),
                usage.databases_used,
                format_limit_display(usage.databases_limit, ""),
                usage.ftp_used,
                format_limit_display(usage.ftp_limit, ""),
                usage.disk_mb_used,
                format_limit_display(usage.disk_mb_limit, "MB"),
            );
            Ok(())
        }
    }
}
