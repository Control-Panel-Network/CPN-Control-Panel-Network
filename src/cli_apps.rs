//! CLI handlers for `cpn app …`.

use clap::Subcommand;

use crate::apps::{AppId, install_app, list_apps, reinstall_app, uninstall_app};

#[derive(Subcommand, Debug)]
pub enum AppCommands {
    /// List app status on this host
    List,
    /// Install an app by name
    Install {
        #[arg(long)]
        name: String,
    },
    /// Reinstall an app by name
    Reinstall {
        #[arg(long)]
        name: String,
        #[arg(long)]
        yes: bool,
    },
    /// Uninstall an app by name
    Uninstall {
        #[arg(long)]
        name: String,
        #[arg(long)]
        yes: bool,
    },
}

pub fn run(
    command: AppCommands,
    require_root: impl FnOnce() -> Result<(), String>,
    confirm: impl FnOnce(&str, bool) -> Result<(), String>,
) -> Result<(), String> {
    match command {
        AppCommands::List => list(),
        AppCommands::Install { name } => {
            require_root()?;
            install(&name)
        }
        AppCommands::Reinstall { name, yes } => {
            require_root()?;
            confirm(
                &format!("Reinstall app `{name}`? Packages may be removed and reinstalled."),
                yes,
            )?;
            reinstall(&name)
        }
        AppCommands::Uninstall { name, yes } => {
            require_root()?;
            confirm(
                &format!("Uninstall app `{name}`? This removes packages and stops services."),
                yes,
            )?;
            uninstall(&name)
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

pub fn install(name: &str) -> Result<(), String> {
    let id = AppId::parse(name)?;
    let msg = install_app(id)?;
    println!("{msg}");
    Ok(())
}

pub fn reinstall(name: &str) -> Result<(), String> {
    let id = AppId::parse(name)?;
    let msg = reinstall_app(id)?;
    println!("{msg}");
    Ok(())
}

pub fn uninstall(name: &str) -> Result<(), String> {
    let id = AppId::parse(name)?;
    let msg = uninstall_app(id)?;
    println!("{msg}");
    Ok(())
}
