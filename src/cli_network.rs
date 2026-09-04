//! `cpn network` subcommands (listen port, hostname, migration).

use crate::panel_network::{
    OldPortPolicy, apply_network_change, clear_panel_hostname, clear_port_migration,
    load_panel_hostname, load_port_migration, network_public, preferred_listen_port_or_default,
    save_panel_hostname,
};
use clap::Subcommand;

#[derive(Subcommand, Debug)]
pub enum NetworkCommands {
    /// Show listen port, hostname, and migration (no secrets)
    Show,
    /// Set preferred listen port and old-port policy when changing
    SetPort {
        #[arg(long)]
        port: u16,
        /// redirect_1m | redirect_3m | deny (required when changing from the current preferred port)
        #[arg(long)]
        old_port_policy: Option<String>,
        /// Treat this as the previous port when recording migration (default: saved preference)
        #[arg(long)]
        from_port: Option<u16>,
    },
    /// Set panel hostname / subdomain (HTTPS without port in the public URL)
    SetHostname {
        #[arg(long)]
        hostname: String,
    },
    /// Clear panel hostname (fall back to host:port URLs)
    ClearHostname,
    /// Clear port migration record (stops future redirect helper starts)
    ClearMigration,
}

pub fn run_network(
    command: NetworkCommands,
    require_root: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    match command {
        NetworkCommands::Show => {
            let preferred = preferred_listen_port_or_default();
            let summary = network_public(preferred, None);
            println!("listen_port={}", summary.listen_port);
            println!("preferred_listen_port={}", summary.preferred_listen_port);
            println!(
                "panel_hostname={}",
                summary.panel_hostname.as_deref().unwrap_or("")
            );
            println!("public_base_url={}", summary.public_base_url);
            if let Some(migration) = summary.port_migration {
                println!(
                    "port_migration=old={} new={} mode={} expires_at={} active={} redirect_active={}",
                    migration.old_port,
                    migration.new_port,
                    migration.mode,
                    migration.expires_at,
                    migration.active,
                    migration.redirect_active
                );
            } else {
                println!("port_migration=");
            }
            Ok(())
        }
        NetworkCommands::SetPort {
            port,
            old_port_policy,
            from_port,
        } => {
            require_root()?;
            let current = from_port.unwrap_or_else(preferred_listen_port_or_default);
            let policy = match old_port_policy.as_deref() {
                None => None,
                Some(raw) => Some(OldPortPolicy::parse(raw)?),
            };
            let (preferred, migration) = apply_network_change(port, current, policy, None)?;
            println!("preferred_listen_port={preferred}");
            if let Some(migration) = migration {
                println!(
                    "port_migration=old={} new={} mode={} expires_at={}",
                    migration.old_port,
                    migration.new_port,
                    migration.mode.as_str(),
                    migration.expires_at
                );
                println!(
                    "note: restart cpn-installer --port {preferred} to bind the new port; redirect helper starts when mode is redirect_*"
                );
            }
            Ok(())
        }
        NetworkCommands::SetHostname { hostname } => {
            require_root()?;
            save_panel_hostname(&hostname)?;
            println!(
                "panel_hostname={}",
                load_panel_hostname().unwrap_or_default()
            );
            println!(
                "note: point DNS A/AAAA for this hostname at the server and terminate TLS on 443 with a reverse proxy to the CPN listen port"
            );
            Ok(())
        }
        NetworkCommands::ClearHostname => {
            require_root()?;
            clear_panel_hostname()?;
            println!("panel_hostname=");
            Ok(())
        }
        NetworkCommands::ClearMigration => {
            require_root()?;
            let _ = load_port_migration();
            clear_port_migration()?;
            println!("port_migration=");
            Ok(())
        }
    }
}
