//! CPN operator CLI (`cpn`): account and site management over SSH.

use std::process::ExitCode;

use clap::{Parser, Subcommand};
use cpn_installer::account::default_password_policy;
use cpn_installer::account_mgmt::{
    create_account, delete_account, list_accounts, reset_account_password,
};
use cpn_installer::cli_apps;
use cpn_installer::cli_common::{
    confirm_delete, print_generated, read_password, require_root_for_mutation,
};
use cpn_installer::cli_network::{NetworkCommands, run_network};
use cpn_installer::cli_packages::{self, PackageCommands};
use cpn_installer::cli_plugins;
use cpn_installer::packages::require_site_create_allowed;
use cpn_installer::paths;
use cpn_installer::sites::{SiteModify, create_site, delete_site, list_sites, modify_site};

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
    /// Panel / operator accounts
    Account {
        #[command(subcommand)]
        command: AccountCommands,
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
    /// Host applications (MariaDB, MySQL, phpMyAdmin, Email, RabbitMQ)
    App {
        #[command(subcommand)]
        command: cli_apps::AppCommands,
    },
    /// Hosting packages and account quota assignment
    Package {
        #[command(subcommand)]
        command: PackageCommands,
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
}

#[derive(Subcommand, Debug)]
enum PluginCommands {
    /// List installed plugins (optional --domain filter)
    List {
        #[arg(long)]
        domain: Option<String>,
    },
    /// Install a plugin id from the community catalog into a site
    Install {
        #[arg(long)]
        domain: String,
        #[arg(long)]
        id: String,
    },
    /// Remove an installed plugin from a site
    Remove {
        #[arg(long)]
        domain: String,
        #[arg(long)]
        id: String,
        #[arg(long)]
        yes: bool,
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
            println!("account  Manage panel / operator accounts");
            println!(
                "site     Manage website records under {}/sites",
                paths::platform_data_dir()
            );
            println!("network  Manage listen port, hostname, and port migration");
            println!("plugin   Manage per-site plugins under /home/<domain>/plugins");
            println!("app      Manage host apps (mariadb, mysql, phpmyadmin, email, rabbitmq)");
            println!("package  Manage hosting packages and account assignments");
            println!("version  Print CLI version");
            println!("list     List command groups (this output)");
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
                let (password, generate) = read_password(password_stdin, generate)?;
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
                let (password, generate) = read_password(password_stdin, generate)?;
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
        Commands::Site { command } => match command {
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
                        "{}\towner={}\tenabled={}\tvhost_wired={}\tdocroot={}{}",
                        site.domain,
                        site.owner,
                        site.enabled,
                        site.vhost_wired,
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
            } => {
                require_root_for_mutation()?;
                require_site_create_allowed(&owner, &domain)?;
                let site = create_site(
                    &domain,
                    &owner,
                    docroot.as_deref(),
                    engine.as_deref(),
                    notes.as_deref(),
                )?;
                println!(
                    "created site {} docroot={} (vhost_wired={}; registry {}/sites/{}.json)",
                    site.domain,
                    site.docroot,
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
                let site = modify_site(
                    &domain,
                    SiteModify {
                        owner,
                        docroot,
                        enabled,
                        engine,
                        notes,
                    },
                )?;
                println!(
                    "updated site {} (vhost_wired={})",
                    site.domain, site.vhost_wired
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
            PluginCommands::Install { domain, id } => {
                require_root_for_mutation()?;
                cli_plugins::install(&domain, &id)
            }
            PluginCommands::Remove { domain, id, yes } => {
                require_root_for_mutation()?;
                confirm_delete(
                    &format!("Remove plugin `{id}` from `{domain}`? This cannot be undone."),
                    yes,
                )?;
                cli_plugins::remove(&domain, &id)
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
