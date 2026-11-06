//! `cpn network` subcommands (listen port, hostname, public URL, migration, HTTPS, phone-home).

use crate::panel_access_mode::{PanelAccessMode, load_panel_access_mode, set_panel_access_mode};
use crate::panel_host_aliases::{
    add_panel_host_alias, clear_panel_host_aliases, load_panel_host_aliases,
    remove_panel_host_alias,
};
use crate::panel_https_front::enable_panel_https_with;
use crate::panel_network::{
    OldPortPolicy, apply_network_change, clear_panel_hostname, clear_port_migration,
    load_panel_hostname, load_port_migration, network_public, preferred_listen_port_or_default,
    save_panel_hostname,
};
use crate::panel_phonehome::{
    phonehome_status, register_disabled_remote, register_now, set_phonehome_enabled,
};
use crate::panel_public_url::{
    clear_panel_public_url, load_panel_public_url, save_panel_public_url,
};
use clap::Subcommand;

#[derive(Subcommand, Debug)]
pub enum NetworkCommands {
    /// Show listen port, hostname, public URL, and migration (no secrets)
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
    /// Set panel hostname / subdomain (HTTPS without a port in the public URL)
    SetHostname {
        #[arg(long)]
        hostname: String,
        /// Skip automatic reverse-proxy + Let's Encrypt setup
        #[arg(long)]
        skip_https: bool,
        /// Keep HTTP IP:port backup bind (0.0.0.0) after HTTPS proxy (default: Domain-only)
        #[arg(long)]
        keep_ip_backup: bool,
    },
    /// Clear panel hostname (fall back to public URL or loopback)
    ClearHostname,
    /// Configure reverse proxy + certificate for the panel hostname (HTTPS on 443)
    EnableHttps {
        /// Hostname override (default: saved panel_hostname)
        #[arg(long)]
        hostname: Option<String>,
        /// Skip Let's Encrypt issue (proxy only)
        #[arg(long)]
        skip_cert: bool,
        /// Keep HTTP IP:port backup bind after proxy setup
        #[arg(long)]
        keep_ip_backup: bool,
    },
    /// Domain-only vs Domain + IP backup (HTTP on SERVER_IP:listen_port)
    SetIpBackup {
        /// Enable HTTP IP:port backup (binds 0.0.0.0)
        #[arg(long)]
        on: bool,
        /// Disable IP backup (Domain-only / loopback)
        #[arg(long)]
        off: bool,
    },
    /// Set external panel base URL for emails/browsers (example: http://127.0.0.1:2089)
    SetPublicUrl {
        #[arg(long)]
        url: String,
    },
    /// Clear external panel public URL
    ClearPublicUrl,
    /// Add an extra hostname alias for the same panel install (CSRF / Host allowlist)
    AddHostAlias {
        #[arg(long)]
        hostname: String,
    },
    /// Remove one hostname alias
    RemoveHostAlias {
        #[arg(long)]
        hostname: String,
    },
    /// List hostname aliases (same install; not separate panels)
    ListHostAliases,
    /// Clear all hostname aliases
    ClearHostAliases,
    /// Clear port migration record (stops future redirect helper starts)
    ClearMigration,
    /// Phone-home to api.newstargeted.com (register this panel for creator broadcasts)
    PhoneHome {
        /// Enable periodic registration (default for official builds)
        #[arg(long, conflicts_with_all = ["off", "now", "status"])]
        on: bool,
        /// Disable phone-home (opt out; local only until next successful register with enabled=false)
        #[arg(long, conflicts_with_all = ["on", "now", "status"])]
        off: bool,
        /// Register once now
        #[arg(long, conflicts_with_all = ["on", "off", "status"])]
        now: bool,
        /// Show phone-home status (no secrets)
        #[arg(long, conflicts_with_all = ["on", "off", "now"])]
        status: bool,
    },
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
            println!(
                "panel_public_url={}",
                summary.panel_public_url.as_deref().unwrap_or("")
            );
            let aliases = load_panel_host_aliases();
            if aliases.is_empty() {
                println!("panel_host_aliases=");
            } else {
                println!("panel_host_aliases={}", aliases.join(","));
            }
            println!("access_mode={}", load_panel_access_mode().as_str());
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
        NetworkCommands::SetHostname {
            hostname,
            skip_https,
            keep_ip_backup,
        } => {
            require_root()?;
            save_panel_hostname(&hostname)?;
            println!(
                "panel_hostname={}",
                load_panel_hostname().unwrap_or_default()
            );
            if skip_https {
                println!(
                    "note: skipped HTTPS setup; later run: cpn network enable-https (DNS A/AAAA must point here; never use https://IP:2087)"
                );
            } else {
                let report = enable_panel_https_with(Some(&hostname), None, true, keep_ip_backup)?;
                println!("{}", report.summary_line());
                for note in report.notes {
                    println!("note: {note}");
                }
            }
            Ok(())
        }
        NetworkCommands::ClearHostname => {
            require_root()?;
            clear_panel_hostname()?;
            println!("panel_hostname=");
            Ok(())
        }
        NetworkCommands::EnableHttps {
            hostname,
            skip_cert,
            keep_ip_backup,
        } => {
            require_root()?;
            let report =
                enable_panel_https_with(hostname.as_deref(), None, !skip_cert, keep_ip_backup)?;
            println!("{}", report.summary_line());
            for note in report.notes {
                println!("note: {note}");
            }
            Ok(())
        }
        NetworkCommands::SetIpBackup { on, off } => {
            require_root()?;
            if on == off {
                return Err(
                    "Pass exactly one of --on or --off (Domain + IP backup vs Domain-only)".into(),
                );
            }
            let mode = if on {
                PanelAccessMode::DomainPlusIpBackup
            } else {
                PanelAccessMode::DomainOnly
            };
            let notes = set_panel_access_mode(mode, true)?;
            println!("access_mode={}", mode.as_str());
            for note in notes {
                println!("note: {note}");
            }
            Ok(())
        }
        NetworkCommands::SetPublicUrl { url } => {
            require_root()?;
            save_panel_public_url(&url)?;
            println!(
                "panel_public_url={}",
                load_panel_public_url().unwrap_or_default()
            );
            println!(
                "note: password-reset and login emails use this URL first (before hostname or loopback)"
            );
            Ok(())
        }
        NetworkCommands::ClearPublicUrl => {
            require_root()?;
            clear_panel_public_url()?;
            println!("panel_public_url=");
            Ok(())
        }
        NetworkCommands::AddHostAlias { hostname } => {
            require_root()?;
            let host = add_panel_host_alias(&hostname)?;
            println!("panel_host_alias_added={host}");
            println!(
                "note: aliases share this install (one MFA store). Passkeys are bound to the hostname used at enroll; use one primary HTTPS hostname for passkeys, or enroll again on this alias. TOTP works on every alias."
            );
            println!(
                "note: point DNS and terminate TLS on 443 with a reverse proxy to the CPN listen port for this hostname"
            );
            Ok(())
        }
        NetworkCommands::RemoveHostAlias { hostname } => {
            require_root()?;
            let removed = remove_panel_host_alias(&hostname)?;
            println!(
                "panel_host_alias_removed={}",
                if removed { "1" } else { "0" }
            );
            Ok(())
        }
        NetworkCommands::ListHostAliases => {
            let aliases = load_panel_host_aliases();
            if aliases.is_empty() {
                println!("panel_host_aliases=");
            } else {
                for host in aliases {
                    println!("panel_host_alias={host}");
                }
            }
            Ok(())
        }
        NetworkCommands::ClearHostAliases => {
            require_root()?;
            clear_panel_host_aliases()?;
            println!("panel_host_aliases=");
            Ok(())
        }
        NetworkCommands::ClearMigration => {
            require_root()?;
            let _ = load_port_migration();
            clear_port_migration()?;
            println!("port_migration=");
            Ok(())
        }
        NetworkCommands::PhoneHome {
            on,
            off,
            now,
            status,
        } => {
            if status || (!on && !off && !now) {
                let s = phonehome_status();
                println!("phonehome_enabled={}", s.enabled);
                println!("install_id={}", s.install_id);
                println!("api_url={}", s.api_url);
                println!("panel_public_url={}", s.panel_public_url);
                println!("has_hmac={}", s.has_hmac);
                println!("allow_http={}", s.allow_http);
                println!("last_register_at={}", s.last_register_at);
                println!("last_register_ok={}", s.last_register_ok);
                println!("last_error={}", s.last_error);
                return Ok(());
            }
            require_root()?;
            if on {
                let s = set_phonehome_enabled(true)?;
                println!("phonehome_enabled={}", s.enabled);
                println!(
                    "note: run with --now to register immediately; background worker also runs daily"
                );
                return Ok(());
            }
            if off {
                let s = set_phonehome_enabled(false)?;
                println!("phonehome_enabled={}", s.enabled);
                match register_disabled_remote() {
                    Ok(_) => println!("note: API registry marked enabled=false"),
                    Err(err) => println!("note: local opt-out saved; remote update skipped: {err}"),
                }
                return Ok(());
            }
            if now {
                let s = register_now()?;
                println!("phonehome_enabled={}", s.enabled);
                println!("install_id={}", s.install_id);
                println!("last_register_ok={}", s.last_register_ok);
                println!("panel_public_url={}", s.panel_public_url);
                println!("note: remote-push receiver enabled when API issued push_hmac");
                return Ok(());
            }
            Ok(())
        }
    }
}
