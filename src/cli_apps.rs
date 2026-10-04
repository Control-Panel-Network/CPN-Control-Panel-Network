//! CLI handlers for `cpn app …` (optional --domain / --subdomain site scope).

use clap::Subcommand;

use crate::apps::{AppId, install_app_on, list_apps, reinstall_app_on, uninstall_app_on};
use crate::apps_control::{start_app, stop_app};
use crate::site_acl::resolve_target_domain;

#[derive(Subcommand, Debug)]
pub enum AppCommands {
    /// List app status on this host
    List {
        #[arg(long)]
        domain: Option<String>,
        #[arg(long)]
        subdomain: Option<String>,
    },
    /// Install an app by name
    Install {
        #[arg(long)]
        name: String,
        #[arg(long)]
        domain: Option<String>,
        #[arg(long)]
        subdomain: Option<String>,
        #[arg(long)]
        version: Option<String>,
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
    /// Set the active panel webmail client (SnappyMail / Tachyon / Roundcube / NextSnapMail)
    Activate {
        #[arg(long)]
        name: String,
    },
    /// Update an app to the latest or selected source version (backup first)
    #[command(visible_alias = "upgrade")]
    Update {
        #[arg(long)]
        name: String,
        #[arg(long)]
        domain: String,
        #[arg(long)]
        version: Option<String>,
    },
    /// Install an older available version (backup first)
    Downgrade {
        #[arg(long)]
        name: String,
        #[arg(long)]
        domain: String,
        #[arg(long)]
        version: String,
    },
    /// Restore a prior app backup (`latest` when omitted)
    Restore {
        #[arg(long)]
        name: String,
        #[arg(long)]
        domain: String,
        #[arg(long)]
        backup: Option<String>,
    },
    /// List backup restore points for one site app
    Backups {
        #[command(subcommand)]
        command: AppBackupCommands,
    },
}

#[derive(Subcommand, Debug)]
pub enum AppBackupCommands {
    /// List available restore points
    List {
        #[arg(long)]
        name: String,
        #[arg(long)]
        domain: String,
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
        AppCommands::List { domain, subdomain } => {
            let site = optional_site(domain, subdomain)?;
            list(site.as_deref())
        }
        AppCommands::Install {
            name,
            domain,
            subdomain,
            version,
        } => {
            require_root()?;
            let site = optional_site(domain, subdomain)?;
            install(&name, site.as_deref(), version.as_deref())
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
            let id = AppId::parse(&name)?;
            let impacts = crate::uninstall_confirm::host_uninstall_impacts(id);
            let mut msg = format!(
                "Uninstall app `{}` ({})?\nIf you continue, these services/features stop or become unavailable:\n",
                name,
                id.label()
            );
            for line in &impacts {
                msg.push_str(&format!("  - {line}\n"));
            }
            confirm(&msg, yes)?;
            let site = optional_site(domain, subdomain)?;
            uninstall(&name, site.as_deref())
        }
        AppCommands::Activate { name } => {
            require_root()?;
            activate(&name)
        }
        AppCommands::Update {
            name,
            domain,
            version,
        } => {
            require_root()?;
            lifecycle_update(&name, &domain, version.as_deref())
        }
        AppCommands::Downgrade {
            name,
            domain,
            version,
        } => {
            require_root()?;
            lifecycle_downgrade(&name, &domain, &version)
        }
        AppCommands::Restore {
            name,
            domain,
            backup,
        } => {
            require_root()?;
            lifecycle_restore(&name, &domain, backup.as_deref())
        }
        AppCommands::Backups { command } => match command {
            AppBackupCommands::List { name, domain } => lifecycle_backups(&name, &domain),
        },
    }
}

pub fn list(domain: Option<&str>) -> Result<(), String> {
    if let Some(domain) = domain {
        let site = crate::sites::load_site(domain)?;
        for app in [
            crate::site_app_lifecycle::SiteAppId::Cmsms,
            crate::site_app_lifecycle::SiteAppId::Redis,
            crate::site_app_lifecycle::SiteAppId::Node,
            crate::site_app_lifecycle::SiteAppId::Python,
        ] {
            let status = crate::site_app_lifecycle::status(&site, app);
            println!(
                "{}\tinstalled={}\tlatest={}\tupdate={}\tbackups={}\tsource={}",
                app.as_str(),
                if status.installed_version.is_empty() {
                    "none"
                } else {
                    &status.installed_version
                },
                if status.latest_version.is_empty() {
                    "unavailable"
                } else {
                    &status.latest_version
                },
                crate::site_app_lifecycle::has_update(&status),
                status.backups.len(),
                status.source,
            );
        }
        return Ok(());
    }
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

fn lifecycle_update(name: &str, domain: &str, version: Option<&str>) -> Result<(), String> {
    let app = crate::site_app_lifecycle::SiteAppId::parse(name)?;
    let site = crate::sites::load_site(domain)?;
    println!(
        "{}",
        crate::site_app_lifecycle::update(&site, app, version)?
    );
    Ok(())
}

fn lifecycle_restore(name: &str, domain: &str, backup: Option<&str>) -> Result<(), String> {
    let app = crate::site_app_lifecycle::SiteAppId::parse(name)?;
    let site = crate::sites::load_site(domain)?;
    println!(
        "{}",
        crate::site_app_lifecycle::restore(&site, app, backup)?
    );
    Ok(())
}

fn lifecycle_downgrade(name: &str, domain: &str, version: &str) -> Result<(), String> {
    let app = crate::site_app_lifecycle::SiteAppId::parse(name)?;
    let site = crate::sites::load_site(domain)?;
    println!(
        "{}",
        crate::site_app_lifecycle::downgrade(&site, app, version)?
    );
    Ok(())
}

fn lifecycle_backups(name: &str, domain: &str) -> Result<(), String> {
    let app = crate::site_app_lifecycle::SiteAppId::parse(name)?;
    let site = crate::sites::load_site(domain)?;
    let status = crate::site_app_lifecycle::status(&site, app);
    if status.backups.is_empty() {
        println!("(no backups)");
    } else {
        for backup in status.backups {
            println!(
                "{}\tbytes={}\tpath={}",
                backup.id,
                backup.bytes,
                backup.path.display()
            );
        }
    }
    Ok(())
}

pub fn install(name: &str, domain: Option<&str>, version: Option<&str>) -> Result<(), String> {
    if let Ok(site_app) = crate::site_app_lifecycle::SiteAppId::parse(name)
        && (domain.is_some() || !matches!(site_app, crate::site_app_lifecycle::SiteAppId::Redis))
    {
        let domain = domain
            .ok_or_else(|| format!("--domain is required when installing site app `{name}`"))?;
        let site = crate::sites::load_site(domain)?;
        let mut message = crate::site_app_lifecycle::install(&site, site_app, version)?;
        if matches!(site_app, crate::site_app_lifecycle::SiteAppId::Redis) {
            message.push(' ');
            message.push_str(&crate::apps_site::apply_site_scope(AppId::Redis, domain)?);
        }
        println!("{message}");
        return Ok(());
    }
    if version.is_some() {
        return Err(
            "--version is supported for cmsms, redis, node, and python site app installs".into(),
        );
    }
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

pub fn activate(name: &str) -> Result<(), String> {
    let id = AppId::parse(name)?;
    let msg = crate::apps_webmail::activate_webmail_app(id)?;
    println!("{msg}");
    Ok(())
}
