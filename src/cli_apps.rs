//! CLI handlers for `cpn app …` (optional --domain / --subdomain site scope).

use clap::Subcommand;

use crate::apps::{AppId, install_app_on, list_apps, reinstall_app_on, uninstall_app_on};
use crate::apps_control::{start_app, stop_app};
use crate::site_acl::resolve_target_domain;

#[derive(Subcommand, Debug)]
pub enum AppCommands {
    /// List app status on this host
    List,
    /// Install an app by name
    Install {
        #[arg(long)]
        name: String,
        #[arg(long)]
        domain: Option<String>,
        #[arg(long)]
        subdomain: Option<String>,
    },
    /// Start a host app service
    Start {
        #[arg(long)]
        name: String,
    },
    /// Stop a host app service
    Stop {
        #[arg(long)]
        name: String,
    },
    /// Reinstall an app by name
    Reinstall {
        #[arg(long)]
        name: String,
        #[arg(long)]
        domain: Option<String>,
        #[arg(long)]
        subdomain: Option<String>,
        #[arg(long)]
        yes: bool,
    },
    /// Uninstall an app by name
    Uninstall {
        #[arg(long)]
        name: String,
        #[arg(long)]
        domain: Option<String>,
        #[arg(long)]
        subdomain: Option<String>,
        #[arg(long)]
        yes: bool,
    },
}

fn optional_site(
    domain: Option<String>,
    subdomain: Option<String>,
) -> Result<Option<String>, String> {
    if domain.as_ref().map(|v| v.trim().is_empty()).unwrap_or(true)
        && subdomain
            .as_ref()
            .map(|v| v.trim().is_empty())
            .unwrap_or(true)
    {
        return Ok(None);
    }
    Ok(Some(resolve_target_domain(
        domain.as_deref(),
        subdomain.as_deref(),
    )?))
}

pub fn run(
    command: AppCommands,
    require_root: impl FnOnce() -> Result<(), String>,
    confirm: impl FnOnce(&str, bool) -> Result<(), String>,
) -> Result<(), String> {
    match command {
        AppCommands::List => list(),
        AppCommands::Install {
            name,
            domain,
            subdomain,
        } => {
            require_root()?;
            let site = optional_site(domain, subdomain)?;
            install(&name, site.as_deref())
        }
        AppCommands::Start { name } => {
            require_root()?;
            start(&name)
        }
        AppCommands::Stop { name } => {
            require_root()?;
            stop(&name)
        }
        AppCommands::Reinstall {
            name,
            domain,
            subdomain,
            yes,
        } => {
            require_root()?;
            confirm(
                &format!("Reinstall app `{name}`? Packages may be removed and reinstalled."),
                yes,
            )?;
            let site = optional_site(domain, subdomain)?;
            reinstall(&name, site.as_deref())
        }
        AppCommands::Uninstall {
            name,
            domain,
            subdomain,
            yes,
        } => {
            require_root()?;
            confirm(
                &format!("Uninstall app `{name}`? This removes packages and stops services."),
                yes,
            )?;
            let site = optional_site(domain, subdomain)?;
            uninstall(&name, site.as_deref())
        }
    }
}

pub fn list() -> Result<(), String> {
    for status in list_apps() {
        let warn = status
            .warning
            .as_ref()
            .map(|w| format!("\twarning={w}"))
            .unwrap_or_default();
        println!(
            "{}\tlabel={}\tstate={}\tdetail={}{}",
            status.id.as_str(),
            status.id.label(),
            status.state.as_str(),
            status.detail,
            warn
        );
    }
    Ok(())
}

pub fn install(name: &str, domain: Option<&str>) -> Result<(), String> {
    let id = AppId::parse(name)?;
    let msg = install_app_on(id, domain)?;
    println!("{msg}");
    Ok(())
}

pub fn start(name: &str) -> Result<(), String> {
    let id = AppId::parse(name)?;
    let msg = start_app(id)?;
    println!("{msg}");
    Ok(())
}

pub fn stop(name: &str) -> Result<(), String> {
    let id = AppId::parse(name)?;
    let msg = stop_app(id)?;
    println!("{msg}");
    Ok(())
}

pub fn reinstall(name: &str, domain: Option<&str>) -> Result<(), String> {
    let id = AppId::parse(name)?;
    let msg = reinstall_app_on(id, domain)?;
    println!("{msg}");
    Ok(())
}

pub fn uninstall(name: &str, domain: Option<&str>) -> Result<(), String> {
    let id = AppId::parse(name)?;
    let msg = uninstall_app_on(id, domain)?;
    println!("{msg}");
    Ok(())
}
