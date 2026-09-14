//! Webmail host packages: SnappyMail, Tachyon, Roundcube (LIVE), NextSnapMail / SOGo (honest gates).

use crate::apps::{AppId, AppStateKind, AppStatus};
use crate::host_packages_catalog::{HostInstallStatus, meta_for};
use crate::install_webmail::install_webmail;
use crate::installer::{AppState, InstallLogDetail};
use crate::model::{InstallerStatus, MailSystem, ServerEngine};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex, RwLock, atomic::AtomicBool};
use tokio::sync::broadcast;

fn path_installed(rel: &str) -> bool {
    let root = Path::new("/opt/cpn-webmail").join(rel);
    root.is_dir() && root.join("index.php").is_file()
}

fn roundcube_installed() -> bool {
    Path::new("/opt/cpn-webmail/roundcube/public_html/index.php").is_file()
        || Path::new("/opt/cpn-webmail/roundcube/index.php").is_file()
}

pub fn detect_webmail_app(id: AppId) -> AppStatus {
    let meta = meta_for(id);
    match id {
        AppId::Snappymail => {
            let installed = path_installed("snappymail");
            let (state, detail) = if installed {
                (
                    AppStateKind::Running,
                    "SnappyMail files present under /opt/cpn-webmail/snappymail.".into(),
                )
            } else {
                (
                    AppStateKind::NotInstalled,
                    "SnappyMail not detected under /opt/cpn-webmail.".into(),
                )
            };
            AppStatus {
                id,
                state,
                detail,
                warning: None,
            }
        }
        AppId::Tachyon => {
            let installed = path_installed("tachyon");
            let (state, detail) = if installed {
                (
                    AppStateKind::Running,
                    "Tachyon files present under /opt/cpn-webmail/tachyon.".into(),
                )
            } else {
                (
                    AppStateKind::NotInstalled,
                    "Tachyon not detected under /opt/cpn-webmail.".into(),
                )
            };
            AppStatus {
                id,
                state,
                detail,
                warning: None,
            }
        }
        AppId::Roundcube => {
            let installed = roundcube_installed();
            let (state, detail) = if installed {
                (
                    AppStateKind::Running,
                    "Roundcube files present under /opt/cpn-webmail/roundcube.".into(),
                )
            } else {
                (
                    AppStateKind::NotInstalled,
                    "Roundcube not detected under /opt/cpn-webmail.".into(),
                )
            };
            AppStatus {
                id,
                state,
                detail,
                warning: None,
            }
        }
        AppId::Nextsnapmail => AppStatus {
            id,
            state: AppStateKind::NotInstalled,
            detail: meta.description.into(),
            warning: Some(
                "NextSnapMail requires Nextcloud. Install Nextcloud first, then add NextSnapMail from the Nextcloud App Store (apps.nextcloud.com/apps/nextsnapmail)."
                    .into(),
            ),
        },
        AppId::Sogo => {
            let pkgs = crate::apps_pkg::rpm_or_dpkg_installed(&["sogo", "sogo-activesync"]);
            let running = crate::service_detect::systemd_unit_active("sogo");
            let (state, detail) = if running {
                (
                    AppStateKind::Running,
                    "SOGo unit active (operator-installed packages).".into(),
                )
            } else if pkgs {
                (
                    AppStateKind::Installed,
                    "SOGo packages present but unit not running.".into(),
                )
            } else {
                (
                    AppStateKind::NotInstalled,
                    "SOGo host install is SCAFFOLD: Inverse packages are not auto-provisioned yet."
                        .into(),
                )
            };
            AppStatus {
                id,
                state,
                detail,
                warning: Some(
                    "Full SOGo (groupware + CalDAV/CardDAV) packaging is not LIVE in CPN yet. Card is registry/UI only until Inverse recipes ship."
                        .into(),
                ),
            }
        }
        _ => AppStatus {
            id,
            state: AppStateKind::NotInstalled,
            detail: "Not a webmail host package.".into(),
            warning: None,
        },
    }
}

fn nextcloud_present() -> bool {
    Path::new("/var/www/nextcloud").is_dir()
        || Path::new("/usr/share/nextcloud").is_dir()
        || Path::new("/opt/nextcloud").is_dir()
        || std::env::var_os("CPN_NEXTCLOUD_ROOT").is_some()
}

fn detect_server_engine() -> Option<ServerEngine> {
    if Path::new("/usr/local/lsws").is_dir() || Path::new("/usr/local/lsws/bin/lswsctrl").is_file()
    {
        return Some(ServerEngine::Openlitespeed);
    }
    if Command::new("systemctl")
        .args(["is-active", "--quiet", "nginx"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
    {
        return Some(ServerEngine::Nginx);
    }
    if Command::new("systemctl")
        .args(["is-active", "--quiet", "caddy"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
    {
        return Some(ServerEngine::Caddy);
    }
    None
}

fn quiet_app_state(engine: ServerEngine) -> Arc<AppState> {
    let (tx, _) = broadcast::channel(32);
    let status = InstallerStatus {
        selected_server: Some(engine),
        ..Default::default()
    };
    Arc::new(AppState {
        status: RwLock::new(status),
        events: tx,
        token: String::new(),
        session_id: String::new(),
        bind_port: crate::listen_port::DEFAULT_PORT,
        allow_remote: false,
        allowed_hosts: Vec::new(),
        cancel_requested: AtomicBool::new(false),
        active_child_pids: Mutex::new(Vec::new()),
        install_log_detail: Mutex::new(InstallLogDetail::Minimal),
    })
}

fn mail_for_app(id: AppId) -> Result<MailSystem, String> {
    match id {
        AppId::Snappymail => Ok(MailSystem::Snappymail),
        AppId::Tachyon => Ok(MailSystem::Tachyon),
        AppId::Roundcube => Ok(MailSystem::Roundcube),
        AppId::Nextsnapmail => Ok(MailSystem::Nextsnapmail),
        AppId::Sogo => Ok(MailSystem::Sogo),
        _ => Err("Not a webmail host package.".into()),
    }
}

/// Sync install for Host packages / `cpn app install` (no installer progress UI).
pub fn install_webmail_app(id: AppId) -> Result<String, String> {
    match meta_for(id).install_status {
        HostInstallStatus::RequiresNextcloud => {
            if !nextcloud_present() {
                return Err(
                    "NextSnapMail needs Nextcloud on this host. CPN will not ship a broken standalone stub. Install Nextcloud, then install NextSnapMail from the Nextcloud App Store."
                        .into(),
                );
            }
            Err(
                "Nextcloud was detected, but CPN does not auto-install NextSnapMail into the Nextcloud apps tree yet. Install from https://apps.nextcloud.com/apps/nextsnapmail (or oe79/NextSnapMail)."
                    .into(),
            )
        }
        HostInstallStatus::Scaffold => Err(
            "SOGo install is SCAFFOLD (not LIVE). Use Inverse SOGo packages manually for now; CPN will wire a full recipe in a later release."
                .into(),
        ),
        HostInstallStatus::Live => {
            let mail = mail_for_app(id)?;
            let engine = detect_server_engine().ok_or_else(|| {
                "No supported web server detected (OpenLiteSpeed, Nginx, or Caddy). Install a web server before webmail."
                    .to_string()
            })?;
            let state = quiet_app_state(engine);
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| format!("Could not start install runtime: {e}"))?;
            rt.block_on(install_webmail(&state, mail, engine))?;
            Ok(format!(
                "Installed {} under /opt/cpn-webmail and configured the webmail runtime.",
                id.label()
            ))
        }
    }
}

pub fn uninstall_webmail_app(id: AppId) -> Result<String, String> {
    let dir = match id {
        AppId::Snappymail => "/opt/cpn-webmail/snappymail",
        AppId::Tachyon => "/opt/cpn-webmail/tachyon",
        AppId::Roundcube => "/opt/cpn-webmail/roundcube",
        AppId::Nextsnapmail | AppId::Sogo => {
            return Err(format!(
                "{} was not installed by CPN (gate/scaffold only).",
                id.label()
            ));
        }
        _ => return Err("Not a webmail host package.".into()),
    };
    if Path::new(dir).exists() {
        std::fs::remove_dir_all(dir).map_err(|e| format!("Could not remove {dir}: {e}"))?;
    }
    Ok(format!("Removed {} files from {dir}.", id.label()))
}

pub fn is_webmail_app(id: AppId) -> bool {
    matches!(
        id,
        AppId::Snappymail | AppId::Tachyon | AppId::Roundcube | AppId::Nextsnapmail | AppId::Sogo
    )
}
