//! CLI flags for version-check / upgrade / repair / downgrade (no web UI).

use crate::installer::AppState;
use crate::manifest::detect_existing_install;
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
    if args.iter().any(|arg| arg == "--upgrade") {
        return Some(CliMode::Upgrade {
            version: version_flag("--version-target").or_else(|| version_flag("--to")),
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
  cpn-installer                 Start the web installer UI
  cpn-installer --port <PORT>   Listen port (default: 2087; also CPN_LISTEN_PORT)
  cpn-installer --panel-hostname <HOST>  Persist subdomain for HTTPS login without a port
  cpn-installer --old-port-policy <MODE>  redirect_1m | redirect_3m | deny (with --port)
  cpn-installer --version
  cpn-installer --version-check
  cpn-installer --upgrade [--to X.Y.Z]
  cpn-installer --repair [--to X.Y.Z] [--reset-data]
  cpn-installer --downgrade --to X.Y.Z --yes [--reset-data]
  cpn-installer --allow-remote  Bind 0.0.0.0 (HTTP without TLS; operator opt-in)

Notes:
  Default listen port is 2087 (Cloudflare-friendly, WHM HTTPS family). Lab installs may use another free port (for example 8787).
  Ports 1-65535 are accepted; prefer >1024 unless running as root.
  Preferred port, optional panel hostname, and port migration live under the CPN data directory (mode 0600 on Unix). See to-do/PANEL-PORT-SUBDOMAIN.md.
  Operator CLI: cpn network show|set-port|set-hostname|clear-hostname|clear-migration
  Repair overwrites core packaged files listed in install-manifest.json under the CPN data directory.
  Site data under the CPN data directory (accounts, bootstrap, SMTP secrets) is preserved unless --reset-data.
  Coordinate with `cpn version-check` when the operator CLI ships (see to-do/UPGRADE-REPAIR.md).
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
    Arc::new(AppState {
        status: tokio::sync::RwLock::new(Default::default()),
        events,
        token: "cli".into(),
        session_id: "clisessionid0000000000000001".into(),
        bind_port: crate::listen_port::DEFAULT_PORT,
        allow_remote: false,
        cancel_requested: std::sync::atomic::AtomicBool::new(false),
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
            let existing = detect_existing_install(VERSION);
            let check = releases::version_check(VERSION, &existing.package_version).await;
            print_json(&serde_json::json!({
                "existing": existing,
                "check": check,
            }));
            if check.error.is_some() { 2 } else { 0 }
        }
        CliMode::Upgrade { version } => {
            let state = make_state().await;
            let request = MaintenanceRequest {
                action: MaintenanceAction::Upgrade,
                version,
                confirm_downgrade: false,
                reset_data: false,
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
            };
            match run_maintenance(state, request).await {
                Ok(()) => 0,
                Err(error) => {
                    eprintln!("error: {error}");
                    1
                }
            }
        }
    }
}
