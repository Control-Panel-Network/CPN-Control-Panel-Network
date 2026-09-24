//! CPN operator CLI (`cpn`): account and site management over SSH.

use std::process::ExitCode;

use clap::{Parser, Subcommand};
use cpn_installer::account::{default_password_policy, load_bootstrap};
use cpn_installer::account_mgmt::{
    create_account, delete_account, list_accounts, reset_account_password,
};
use cpn_installer::cli_apps;
use cpn_installer::cli_common::{
    confirm_delete, print_generated, read_password_confirmed, require_root_for_mutation,
};
use cpn_installer::cli_doctor;
use cpn_installer::cli_mfa::{MfaCommands, run as run_mfa};
use cpn_installer::cli_network::{NetworkCommands, run_network};
use cpn_installer::cli_packages::{self, PackageCommands};
use cpn_installer::cli_panel::{PanelCommands, run_panel};
use cpn_installer::cli_passkeys::{PasskeyCommands, run as run_passkeys};
use cpn_installer::cli_plugins;
use cpn_installer::cli_totp::{TotpCommands, run as run_totp};
use cpn_installer::packages::require_site_create_allowed;
use cpn_installer::panel_ops_ssl_provider::SslProvider;
use cpn_installer::paths;
use cpn_installer::sites::{
    SiteModify, create_site_with_ssl, delete_site, list_sites, modify_site,
};

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Parser, Debug)]
#[command(
    name = "cpn",
    version = VERSION,
    about = "CPN Control Panel Network operator CLI"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Print package version
    Version,
    /// List top-level command groups
    List,
    /// Live panel login URL, status, and MOTD helpers (no secrets)
    Panel {
        #[command(subcommand)]
        command: PanelCommands,
    },
    /// Alias for `cpn panel status` (version, service, live login URL)
    Info {
        #[arg(long)]
        raw: bool,
    },
    /// Reset a panel password from the server terminal
    Password {
        /// Account username; defaults to the primary administrator account
        #[arg(long)]
        username: Option<String>,
        /// Read the new password from stdin (never from argv)
        #[arg(long)]
        password_stdin: bool,
        /// Generate an ASCII password and write it to a mode-600 temporary file
        #[arg(long)]
        generate: bool,
    },
    /// Panel / operator accounts
    Account {
        #[command(subcommand)]
        command: AccountCommands,
    },
    /// WebAuthn passkeys for panel accounts (list / remove / clear)
    Passkey {
        #[command(subcommand)]
        command: PasskeyCommands,
    },
    /// TOTP authenticator for panel accounts (status / disable / clear)
    Totp {
        #[command(subcommand)]
        command: TotpCommands,
    },
    /// Combined MFA clear (TOTP + passkeys + ceremonies)
    Mfa {
        #[command(subcommand)]
        command: MfaCommands,
    },
    /// Website records (JSON under $CPN_DATA_DIR/sites)
    Site {
        #[command(subcommand)]
        command: SiteCommands,
    },
    /// Listen port, panel hostname, and port migration
    Network {
        #[command(subcommand)]
        command: NetworkCommands,
    },
    /// Installed plugins and catalog installs
    Plugin {
        #[command(subcommand)]
        command: PluginCommands,
    },
    /// Host applications (MariaDB, PostgreSQL, phpMyAdmin, Email, RabbitMQ, webmail)
    App {
        #[command(subcommand)]
        command: cli_apps::AppCommands,
    },
    /// Hosting packages and account quota assignment
    Package {
        #[command(subcommand)]
        command: PackageCommands,
    },
    /// Health checks: CLI paths, panel unit, /login, core manifest, host summary
    Doctor {
        /// Remove stale `/usr/local/bin/cpn*` hot-deploy overrides (requires root)
        #[arg(long)]
        heal: bool,
    },
    /// Uninstall CPN from this host (same as `cpn-installer --uninstall`)
    Uninstall {
        /// Skip confirmation prompts
        #[arg(long, short = 'y')]
        yes: bool,
        /// Leave /var/lib/cpn and /etc/cpn
        #[arg(long)]
        keep_data: bool,
        /// Also remove CPN Docker volumes and webmail trees
        #[arg(long)]
        purge_all: bool,
        /// Delete registered website document roots (second confirmation)
        #[arg(long)]
        purge_sites: bool,
        /// Remove common MariaDB / OpenLiteSpeed packages when present
        #[arg(long)]
        purge_stack: bool,
        /// Print steps without changing the host
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Subcommand, Debug)]
enum AccountCommands {
    /// Create an account (first write goes to panel-bootstrap.json)
    Create {
        #[arg(long)]
        username: String,
        #[arg(long)]
        email: String,
        #[arg(long, default_value = "en")]
        language: String,
        /// Read password from stdin (no echo in argv; never logged)
        #[arg(long)]
        password_stdin: bool,
        /// Generate a password that meets the default policy
        #[arg(long)]
        generate: bool,
    },
    /// Delete an account (confirmation required unless --yes)
    Delete {
        #[arg(long)]
        username: String,
        #[arg(long)]
        yes: bool,
    },
    /// Reset / recover password
    Passwd {
        #[arg(long)]
        username: String,
        #[arg(long)]
        password_stdin: bool,
        #[arg(long)]
        generate: bool,
    },
    /// List configured accounts (no secrets)
    List,
}

#[derive(Subcommand, Debug)]
enum SiteCommands {
    /// Create a website under /home/<domain>/public_html (vhost wiring may be stubbed)
    Create {
        #[arg(long)]
        domain: String,
        #[arg(long, default_value = "admin")]
        owner: String,
        #[arg(long)]
        docroot: Option<String>,
        #[arg(long)]
        engine: Option<String>,
        #[arg(long)]
        notes: Option<String>,
        /// Per-domain SSL provider stored on this site only:
        /// letsencrypt|zerossl|cloudflare_ca|custom|none
        #[arg(long = "ssl-provider")]
        ssl_provider: Option<String>,
    },
    /// Modify an existing website record
    Modify {
        #[arg(long)]
        domain: String,
        #[arg(long)]
        owner: Option<String>,
        #[arg(long)]
        docroot: Option<String>,
        #[arg(long)]
        engine: Option<String>,
        #[arg(long)]
        notes: Option<String>,
        #[arg(long, action = clap::ArgAction::SetTrue)]
        enable: bool,
        #[arg(long, action = clap::ArgAction::SetTrue)]
        disable: bool,
        /// Change this domain's SSL provider only (siblings unchanged)
        #[arg(long = "ssl-provider")]
        ssl_provider: Option<String>,
    },
    /// Delete a website record (confirmation required unless --yes)
    Delete {
        #[arg(long)]
        domain: String,
        #[arg(long)]
        yes: bool,
    },
    /// List website records
    List,
    /// Re-run DKIM + SPF/DKIM/DMARC DNS + auto SSL for an existing site
    Ready {
        #[arg(long)]
        domain: String,
    },
}

#[derive(Subcommand, Debug)]
enum PluginCommands {
    /// List installed plugins (optional --domain filter)
    List {
        #[arg(long)]
        domain: Option<String>,
    },
    /// Install a plugin id from the community catalog (site) or host-plugins/ (--host)
    Install {
        #[arg(long)]
        domain: Option<String>,
        #[arg(long)]
        id: String,
        /// Install under $CPN_DATA_DIR/host-plugins/ (also auto for host-scoped Security ids)
        #[arg(long)]
        host: bool,
    },
    /// Remove an installed plugin from a site (or --host for host-scoped)
    Remove {
        #[arg(long)]
        domain: Option<String>,
        #[arg(long)]
        id: String,
        #[arg(long)]
        yes: bool,
        #[arg(long)]
        host: bool,
    },
    /// Enable an installed plugin on a site
    Enable {
        #[arg(long)]
        domain: String,
        #[arg(long)]
        id: String,
    },
    /// Disable an installed plugin on a site
    Disable {
        #[arg(long)]
        domain: String,
        #[arg(long)]
        id: String,
    },
    /// Move legacy $CPN_DATA_DIR/plugins into /home/<domain>/plugins
    Migrate {
        #[arg(long)]
        domain: String,
    },
}

fn run() -> Result<(), String> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Version => {
            println!("cpn {VERSION}");
            Ok(())
        }
        Commands::List => {
            println!("panel    Live login URL, panel status, and MOTD install helper");
            println!("info     Alias for: cpn panel status");
            println!("account  Manage panel / operator accounts");
            println!("passkey  List, remove, or clear WebAuthn passkeys for an account");
            println!("totp     Show or clear TOTP authenticator for an account");
            println!("mfa      Clear all MFA (TOTP + passkeys) for an account");
            println!("password Reset a panel password from this terminal");
            println!(
                "site     Manage website records under {}/sites",
                paths::platform_data_dir()
            );
            println!("network  Manage listen port, hostname, public URL, and port migration");
            println!("plugin   Manage host-scoped and per-site plugins (/home/<domain>/plugins)");
            println!(
                "app      Manage host apps (mariadb, postgresql, phpmyadmin, email, rabbitmq, snappymail, ...)"
            );
            println!("package  Manage hosting packages and account assignments");
            println!("doctor   Health checks (CLI paths, panel unit, /login, core files)");
            println!("uninstall Remove CPN from this host (see cpn-installer --uninstall)");
            println!("version  Print CLI version");
            println!("list     List command groups (this output)");
            Ok(())
        }
        Commands::Doctor { heal } => cli_doctor::run(heal),
        Commands::Uninstall {
            yes,
            keep_data,
            purge_all,
            purge_sites,
            purge_stack,
            dry_run,
        } => {
            let code =
                cpn_installer::cli_uninstall::run(cpn_installer::cli_uninstall::UninstallOptions {
                    yes,
                    keep_data,
                    purge_all,
                    purge_sites,
                    purge_stack,
                    dry_run,
                });
            if code == 0 {
                Ok(())
            } else {
                Err("uninstall failed".into())
            }
        }
        Commands::Panel { command } => run_panel(command, require_root_for_mutation),
        Commands::Info { raw } => {
            run_panel(PanelCommands::Status { raw }, require_root_for_mutation)
        }
        Commands::Password {
            username,
            password_stdin,
            generate,
        } => {
            require_root_for_mutation()?;
            let username = username
                .filter(|value| !value.trim().is_empty())
                .or_else(|| load_bootstrap().map(|account| account.username))
                .ok_or_else(|| {
                    "No primary account exists yet; finish installation before resetting a password"
                        .to_string()
                })?;
            let (password, generate) = read_password_confirmed(password_stdin, generate)?;
            let result = reset_account_password(&username, password.as_deref(), generate)?;
            let _ = result.public;
            println!("password updated ok");
            print_generated(result.generated_password)?;
            Ok(())
        }
        Commands::Account { command } => match command {
            AccountCommands::List => {
                let accounts = list_accounts()?;
                if accounts.is_empty() {
                    println!("(no accounts)");
                    return Ok(());
                }
                for account in accounts {
                    // Avoid cleartext usernames/emails on stdout (CodeQL cleartext-logging).
                    println!(
                        "account\tconfigured={}\trecovery_set={}",
                        account.configured,
                        !account.recovery_email.trim().is_empty()
                    );
                }
                Ok(())
            }
            AccountCommands::Create {
                username,
                email,
                language,
                password_stdin,
                generate,
            } => {
                require_root_for_mutation()?;
                let (password, generate) = read_password_confirmed(password_stdin, generate)?;
                let result = create_account(
                    &username,
                    password.as_deref(),
                    generate,
                    &email,
                    default_password_policy(),
                    &language,
                )?;
                let _ = result.public;
                println!("created account ok");
                print_generated(result.generated_password)?;
                Ok(())
            }
            AccountCommands::Passwd {
                username,
                password_stdin,
                generate,
            } => {
                require_root_for_mutation()?;
                let (password, generate) = read_password_confirmed(password_stdin, generate)?;
                let result = reset_account_password(&username, password.as_deref(), generate)?;
                let _ = result.public;
                println!("password updated ok");
                print_generated(result.generated_password)?;
                Ok(())
            }
            AccountCommands::Delete { username, yes } => {
                require_root_for_mutation()?;
                confirm_delete("Delete the selected account? This cannot be undone.", yes)?;
                delete_account(&username)?;
                println!("deleted account ok");
                Ok(())
            }
        },
        Commands::Passkey { command } => {
            run_passkeys(command, require_root_for_mutation, confirm_delete)
        }
        Commands::Totp { command } => run_totp(command, require_root_for_mutation, confirm_delete),
        Commands::Mfa { command } => run_mfa(command, require_root_for_mutation, confirm_delete),
        Commands::Site { command } => match command {
            SiteCommands::Ready { domain } => {
                require_root_for_mutation()?;
                let report = cpn_installer::panel_ops_domain_ready::after_site_created(&domain);
                println!("{}", report.summary());
                for w in &report.warnings {
                    eprintln!("warning: {w}");
                }
                Ok(())
            }
            SiteCommands::List => {
                let sites = list_sites()?;
                if sites.is_empty() {
                    println!("(no sites)");
                    return Ok(());
                }
                for site in sites {
                    let legacy = if cpn_installer::sites::is_legacy_docroot(&site.docroot) {
                        "\tlegacy_path=true"
                    } else {
                        ""
                    };
                    println!(
                        "{}\towner={}\tenabled={}\tvhost_wired={}\tssl={}\tdocroot={}{}",
                        site.domain,
                        site.owner,
                        site.enabled,
                        site.vhost_wired,
                        site.ssl.provider.as_str(),
                        site.docroot,
                        legacy
                    );
                }
                Ok(())
            }
            SiteCommands::Create {
                domain,
                owner,
                docroot,
                engine,
                notes,
                ssl_provider,
            } => {
                require_root_for_mutation()?;
                require_site_create_allowed(&owner, &domain)?;
                let provider = match ssl_provider.as_deref() {
                    Some(raw) => Some(SslProvider::parse(raw)?),
                    None => None,
                };
                let site = create_site_with_ssl(
                    &domain,
                    &owner,
                    docroot.as_deref(),
                    engine.as_deref(),
                    notes.as_deref(),
                    provider,
                )?;
                println!(
                    "created site {} docroot={} ssl_provider={} (vhost_wired={}; registry {}/sites/{}.json)",
                    site.domain,
                    site.docroot,
                    site.ssl.provider.as_str(),
                    site.vhost_wired,
                    paths::platform_data_dir(),
                    site.domain
                );
                if !site.vhost_wired {
                    eprintln!(
                        "note: web server vhost files are not written yet; files are under the docroot above"
                    );
                }
                Ok(())
            }
            SiteCommands::Modify {
                domain,
                owner,
                docroot,
                engine,
                notes,
                enable,
                disable,
                ssl_provider,
            } => {
                require_root_for_mutation()?;
                if enable && disable {
                    return Err("Use either --enable or --disable, not both".into());
                }
                let enabled = if enable {
                    Some(true)
                } else if disable {
                    Some(false)
                } else {
                    None
                };
                let ssl_provider = match ssl_provider.as_deref() {
                    Some(raw) => Some(SslProvider::parse(raw)?),
                    None => None,
                };
                let site = modify_site(
                    &domain,
                    SiteModify {
                        owner,
                        docroot,
                        enabled,
                        engine,
                        notes,
                        ssl_provider,
                        ..SiteModify::default()
                    },
                )?;
                println!(
                    "updated site {} ssl_provider={} (vhost_wired={})",
                    site.domain,
                    site.ssl.provider.as_str(),
                    site.vhost_wired
                );
                if !site.vhost_wired {
                    eprintln!(
                        "note: web server vhost files are not rewritten yet; only the site record was updated"
                    );
                }
                Ok(())
            }
            SiteCommands::Delete { domain, yes } => {
                require_root_for_mutation()?;
                confirm_delete(
                    &format!("Delete site `{domain}` record? This cannot be undone."),
                    yes,
                )?;
                delete_site(&domain)?;
                println!("deleted site {domain}");
                Ok(())
            }
        },
        Commands::Network { command } => run_network(command, require_root_for_mutation),
        Commands::Plugin { command } => match command {
            PluginCommands::List { domain } => cli_plugins::list_plugins(domain.as_deref()),
            PluginCommands::Install { domain, id, host } => {
                require_root_for_mutation()?;
                cli_plugins::install(domain.as_deref(), &id, host)
            }
            PluginCommands::Remove {
                domain,
                id,
                yes,
                host,
            } => {
                require_root_for_mutation()?;
                let target = if host {
                    format!("host plugin `{id}`")
                } else {
                    format!(
                        "plugin `{id}` from `{}`",
                        domain.as_deref().unwrap_or("(required)")
                    )
                };
                confirm_delete(&format!("Remove {target}? This cannot be undone."), yes)?;
                cli_plugins::remove(domain.as_deref(), &id, host)
            }
            PluginCommands::Enable { domain, id } => {
                require_root_for_mutation()?;
                cli_plugins::enable(&domain, &id)
            }
            PluginCommands::Disable { domain, id } => {
                require_root_for_mutation()?;
                cli_plugins::disable(&domain, &id)
            }
            PluginCommands::Migrate { domain } => {
                require_root_for_mutation()?;
                cli_plugins::migrate(&domain)
            }
        },
        Commands::App { command } => {
            cli_apps::run(command, require_root_for_mutation, confirm_delete)
        }
        Commands::Package { command } => {
            cli_packages::run(command, require_root_for_mutation, confirm_delete)
        }
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("error: {message}");
            ExitCode::FAILURE
        }
    }
}
