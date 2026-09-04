//! CLI helpers for `cpn package …`.

use crate::package_quota::require_quota;
use crate::packages::{
    PackageInput, QuotaResource, assign_package, create_package, delete_package,
    ensure_default_package, format_limit_display, get_package, list_packages, package_for_account,
    update_package,
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
    // Quota counts only. Omit ftp_accounts on stdout (CodeQL cleartext-logging).
    println!(
        "{}\tid={}\tdisk={}\tbw={}\tdomains={}\temails={}\tdbs={}\tfqdn={}",
        pkg.name,
        pkg.id,
        format_limit_display(pkg.disk_mb, "MB"),
        format_limit_display(pkg.bandwidth_mb, "MB"),
        format_limit_display(pkg.domains, ""),
        format_limit_display(pkg.emails, ""),
        format_limit_display(pkg.databases, ""),
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
            // Do not echo package_for_account(...) (CodeQL cleartext-logging).
            println!("assigned package ok id_or_name={package}");
            Ok(())
        }
        PackageCommands::Show { username } => {
            let pkg = package_for_account(&username)?;
            // Do not print PackageUsage / FTP payloads (CodeQL rust/cleartext-logging).
            let domains_ok = require_quota(&username, QuotaResource::Domains).is_ok();
            let emails_ok = require_quota(&username, QuotaResource::Emails).is_ok();
            let dbs_ok = require_quota(&username, QuotaResource::Databases).is_ok();
            let ftp_ok = require_quota(&username, QuotaResource::FtpAccounts).is_ok();
            let disk_ok = require_quota(&username, QuotaResource::DiskMb).is_ok();
            println!(
                "usage\tpackage_id={}\tdomains_ok={domains_ok}\temails_ok={emails_ok}\tdbs_ok={dbs_ok}\tftp_ok={ftp_ok}\tdisk_ok={disk_ok}",
                pkg.id
            );
            Ok(())
        }
    }
}
