//! CLI flags for version-check / upgrade / repair / downgrade (no web UI).

use crate::installer::AppState;
use crate::manifest::{detect_existing_install, reconcile_stale_package_identity};
use crate::model::{MaintenanceAction, MaintenanceRequest};
use crate::releases;
use crate::upgrade::{build_plan, run_maintenance};
use std::sync::Arc;
use tokio::sync::broadcast;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone)]
pub enum CliMode {
    Version,
    VersionCheck,
    Upgrade {
        version: Option<String>,
        /// Refresh CPN-managed Docker stacks only (`--bypass` / CPN_UPGRADE_BYPASS=1).
        bypass_docker: bool,
    },
    Repair {
        version: Option<String>,
        reset_data: bool,
    },
    Downgrade {
        version: String,
        yes: bool,
        reset_data: bool,
    },
    EnsureDatabaseDefaults {
        database: crate::model::DatabaseEngine,
        install_phpmyadmin: bool,
    },
    Help,
}

pub fn parse_cli(args: &[String]) -> Option<CliMode> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        return Some(CliMode::Help);
    }
    if args.iter().any(|arg| arg == "--version" || arg == "-V") {
        return Some(CliMode::Version);
    }
    if args
        .iter()
        .any(|arg| arg == "--version-check" || arg == "version-check")
    {
        return Some(CliMode::VersionCheck);
    }
    let version_flag = |flag: &str| -> Option<String> {
        args.windows(2).find_map(|window| {
            if window[0] == flag {
                Some(window[1].clone())
            } else {
                None
            }
        })
    };
    let yes = args.iter().any(|arg| arg == "--yes" || arg == "-y");
    let reset_data = args.iter().any(|arg| arg == "--reset-data");
    if args.iter().any(|arg| arg == "--ensure-database-defaults") {
        let database = match version_flag("--database") {
            None => crate::model::DatabaseEngine::Mariadb,
            Some(raw) => match crate::model::DatabaseEngine::parse_cli(&raw) {
                Ok(engine) => engine,
                Err(error) => {
                    eprintln!("error: {error}");
                    return Some(CliMode::Help);
                }
            },
        };
        let install_phpmyadmin = !args.iter().any(|arg| arg == "--skip-phpmyadmin");
        return Some(CliMode::EnsureDatabaseDefaults {
            database,
            install_phpmyadmin,
        });
    }
    if args.iter().any(|arg| arg == "--upgrade") {
        let bypass_docker = args.iter().any(|arg| arg == "--bypass")
            || std::env::var("CPN_UPGRADE_BYPASS").ok().as_deref() == Some("1");
        return Some(CliMode::Upgrade {
            version: version_flag("--version-target").or_else(|| version_flag("--to")),
            bypass_docker,
        });
    }
    if args.iter().any(|arg| arg == "--repair") {
        return Some(CliMode::Repair {
            version: version_flag("--version-target").or_else(|| version_flag("--to")),
            reset_data,
        });
    }
    if args.iter().any(|arg| arg == "--downgrade") {
        let version = version_flag("--version-target")
            .or_else(|| version_flag("--to"))
            .unwrap_or_default();
        return Some(CliMode::Downgrade {
            version,
            yes,
            reset_data,
        });
    }
    None
}

pub fn print_help() {
    println!(
        "cpn-installer {VERSION}

Usage:
  cpn-installer                 Interactive: choose Web UI or SSH/CLI (TTY). Non-TTY defaults to Web UI.
  cpn-installer --web           Start the web installer UI (auto-detected language)
  cpn-installer --ui            Alias for --web
  cpn-installer --cli           Interactive SSH/CLI installer (questions in the terminal)
  cpn-installer --ssh           Alias for --cli
  cpn-installer --port <PORT>   Web UI listen port (default: 2087; also CPN_LISTEN_PORT)
  cpn-installer --panel-hostname <HOST>  Persist subdomain for HTTPS login without a port
  cpn-installer --old-port-policy <MODE>  redirect_1m | redirect_3m | deny (with --port)
  cpn-installer --version
  cpn-installer --version-check
  cpn-installer --upgrade [--to X.Y.Z] [--bypass]
  cpn-installer --repair [--to X.Y.Z] [--reset-data]
  cpn-installer --downgrade --to X.Y.Z --yes [--reset-data]
  cpn-installer --allow-remote  Bind 0.0.0.0 for the web UI (HTTP without TLS; operator opt-in)
  cpn-installer --bypass        With --upgrade: refresh CPN-managed Docker compose/stacks only (preserves volumes; never touches unlabeled user containers)
  cpn-installer --ensure-database-defaults [--database mariadb|mysql|none] [--skip-phpmyadmin]
                                 Install MariaDB (default) + phpMyAdmin on Linux without the UI

Notes:
  Installer language follows the saved browser choice, browser locale, or system locale; unsupported locales use English.
  Fresh web-server installs also install MariaDB + phpMyAdmin by default (override with API/UI/CLI or --database / --skip-phpmyadmin).
  Default listen port is 2087 (Cloudflare-supported alternate HTTPS port; WHM HTTPS family). Lab installs may use another free port (for example 8787).
  Ports 1-65535 are accepted; prefer >1024 unless running as root.
  Preferred port, optional panel hostname, and port migration live under the CPN data directory (mode 0600 on Unix).
  Operator network CLI: cpn network show|set-port|set-hostname|clear-hostname|set-public-url|clear-public-url|clear-migration
  Repair overwrites only core packaged files listed in install-manifest.json under the CPN data directory.
  Accounts, bootstrap state, SMTP secrets, and other CPN data are preserved unless --reset-data is explicitly requested.
  Use --version-check before upgrade/downgrade when you need to inspect the latest published release.
  Upgrade cleans only stale CPN packaging/staging (never websites, apps, user docker, or configs).
  Upgrade may refresh already-installed CPN-managed packages (MariaDB, OpenLiteSpeed, PHP) via dnf/apt; databases and docroots are never dropped.
  Without --bypass, Docker stacks are left running as-is; with --bypass, only CPN-managed compose under /var/lib/cpn/docker and containers labeled com.cpn.managed=1 are refreshed.
  Leftover package identity 1.0.0/1.0.1 (retired GitHub retags) may be replaced by official --upgrade / upgrade.sh onto current 0.2.x-alpha (rpm --oldpackage; not a hostile downgrade).
  Version Management and --version-check reconcile a stale 1.0.x install-manifest when the live RPM is already on 0.2.x.
  GitHub Releases lists are cached under the CPN data dir (github-releases-cache.json; TTL 30m). Manual Check for updates is rate-limited (60s). On HTTP 403/429, last good cache is shown when available.
  systemd / non-interactive starts default to the web UI (use --web explicitly in unit files).
"
    );
}

fn print_json(value: &impl serde::Serialize) {
    println!(
        "{}",
        serde_json::to_string_pretty(value).unwrap_or_else(|_| "{}".into())
    );
}

async fn make_state() -> Arc<AppState> {
    let (events, _) = broadcast::channel(64);
    let bind_port = crate::listen_port::DEFAULT_PORT;
    Arc::new(AppState {
        status: std::sync::RwLock::new(Default::default()),
        events,
        token: "cli".into(),
        session_id: "clisessionid0000000000000001".into(),
        bind_port,
        allow_remote: false,
        allowed_hosts: crate::http_helpers::build_allowed_hosts(bind_port, &[]),
        cancel_requested: std::sync::atomic::AtomicBool::new(false),
        active_child_pids: std::sync::Mutex::new(Vec::new()),
        install_log_detail: std::sync::Mutex::new(crate::installer::InstallLogDetail::Full),
    })
}

pub async fn run_cli(mode: CliMode) -> i32 {
    match mode {
        CliMode::Help => {
            print_help();
            0
        }
        CliMode::Version => {
            println!("cpn-installer {VERSION}");
            0
        }
        CliMode::VersionCheck => {
            let reconcile_note = reconcile_stale_package_identity(VERSION);
            let existing = detect_existing_install(VERSION);
            let check = releases::version_check(VERSION, &existing.package_version).await;
            print_json(&serde_json::json!({
                "existing": existing,
                "check": check,
                "reconcile": reconcile_note,
            }));
            if check.error.is_some() { 2 } else { 0 }
        }
        CliMode::Upgrade {
            version,
            bypass_docker,
        } => {
            let state = make_state().await;
            let request = MaintenanceRequest {
                action: MaintenanceAction::Upgrade,
                version,
                confirm_downgrade: false,
                reset_data: false,
                confirm_execute: true,
                bypass_docker,
            };
            match run_maintenance(state, request).await {
                Ok(()) => 0,
                Err(error) => {
                    eprintln!("error: {error}");
                    1
                }
            }
        }
        CliMode::Repair {
            version,
            reset_data,
        } => {
            let state = make_state().await;
            let existing = detect_existing_install(VERSION);
            let target = version.unwrap_or(existing.package_version.clone());
            let plan = build_plan(
                MaintenanceAction::Repair,
                Some(&target),
                &existing.package_version,
                reset_data,
            );
            eprintln!("{}", plan.summary);
            let request = MaintenanceRequest {
                action: MaintenanceAction::Repair,
                version: Some(target),
                confirm_downgrade: true,
                reset_data,
                confirm_execute: true,
                bypass_docker: false,
            };
            match run_maintenance(state, request).await {
                Ok(()) => 0,
                Err(error) => {
                    eprintln!("error: {error}");
                    1
                }
            }
        }
        CliMode::Downgrade {
            version,
            yes,
            reset_data,
        } => {
            if version.trim().is_empty() {
                eprintln!("error: --downgrade requires --to X.Y.Z");
                return 1;
            }
            if !yes {
                eprintln!("error: downgrade requires --yes");
                return 1;
            }
            let state = make_state().await;
            let request = MaintenanceRequest {
                action: MaintenanceAction::Downgrade,
                version: Some(version),
                confirm_downgrade: true,
                reset_data,
                confirm_execute: true,
                bypass_docker: false,
            };
            match run_maintenance(state, request).await {
                Ok(()) => 0,
                Err(error) => {
                    eprintln!("error: {error}");
                    1
                }
            }
        }
        CliMode::EnsureDatabaseDefaults {
            database,
            install_phpmyadmin,
        } => match crate::db_defaults::ensure_database_defaults(database, install_phpmyadmin, None)
        {
            Ok(notes) => {
                for note in notes {
                    println!("{note}");
                }
                0
            }
            Err(error) => {
                eprintln!("error: {error}");
                1
            }
        },
    }
}
