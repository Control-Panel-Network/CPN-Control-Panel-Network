//! Webmail host packages: SnappyMail, Tachyon, Roundcube (LIVE), NextSnapMail (Nextcloud chain), SOGo gate.

use crate::active_webmail::{
    client_files_present, default_webmail_client, docroot_for, load_active_pref, mail_to_id,
    public_path_for, save_active_pref,
};
use crate::apps::{AppId, AppStateKind, AppStatus};
use crate::apps_nextcloud::{
    detect_nextcloud_status, install_nextcloud_files, install_nextsnapmail_app,
    nextsnapmail_app_present, uninstall_nextsnapmail_app,
};
use crate::host_packages_catalog::{HostInstallStatus, meta_for};
use crate::install_webmail::install_webmail;
use crate::install_webmail_runtime::configure_webmail_runtime;
use crate::installer::{AppState, InstallLogDetail};
use crate::model::{InstallerStatus, MailSystem, ServerEngine};
use crate::panel_webmail::{load_webmail_config, save_webmail_config};
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

pub fn mail_for_app(id: AppId) -> Result<MailSystem, String> {
    match id {
        AppId::Snappymail => Ok(MailSystem::Snappymail),
        AppId::Tachyon => Ok(MailSystem::Tachyon),
        AppId::Roundcube => Ok(MailSystem::Roundcube),
        AppId::Nextsnapmail => Ok(MailSystem::Nextsnapmail),
        AppId::Sogo => Ok(MailSystem::Sogo),
        AppId::Nextcloud => Err("Nextcloud is not a panel-proxied webmail client.".into()),
        _ => Err("Not a webmail host package.".into()),
    }
}

pub fn is_active_webmail(id: AppId) -> bool {
    let Ok(mail) = mail_for_app(id) else {
        return false;
    };
    if !client_files_present(mail) {
        return false;
    }
    if let Some(pref) = load_active_pref() {
        return pref == mail;
    }
    crate::panel_webmail::detect_webmail_client() == Some(mail)
}

fn active_suffix(id: AppId) -> String {
    if is_active_webmail(id) {
        " Active panel webmail.".into()
    } else {
        String::new()
    }
}

pub fn detect_webmail_app(id: AppId) -> AppStatus {
    let meta = meta_for(id);
    match id {
        AppId::Snappymail => {
            let installed = path_installed("snappymail");
            let (state, detail) = if installed {
                (
                    AppStateKind::Running,
                    format!(
                        "SnappyMail files present under /opt/cpn-webmail/snappymail.{}",
                        active_suffix(id)
                    ),
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
                    format!(
                        "Tachyon files present under /opt/cpn-webmail/tachyon.{}",
                        active_suffix(id)
                    ),
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
                    format!(
                        "Roundcube files present under /opt/cpn-webmail/roundcube.{}",
                        active_suffix(id)
                    ),
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
        AppId::Nextsnapmail => {
            let (nc_ok, nc_detail) = detect_nextcloud_status();
            if nextsnapmail_app_present() {
                AppStatus {
                    id,
                    state: AppStateKind::Running,
                    detail: format!(
                        "NextSnapMail app present under Nextcloud apps/. {nc_detail}{}",
                        active_suffix(id)
                    ),
                    warning: None,
                }
            } else if nc_ok {
                AppStatus {
                    id,
                    state: AppStateKind::NotInstalled,
                    detail: format!(
                        "Nextcloud is present. Install NextSnapMail into apps/nextsnapmail. {nc_detail}"
                    ),
                    warning: None,
                }
            } else {
                AppStatus {
                    id,
                    state: AppStateKind::NotInstalled,
                    detail: meta.description.into(),
                    warning: Some(
                        "NextSnapMail requires Nextcloud. Use Install to provision Nextcloud files under /opt/nextcloud, then NextSnapMail into apps/."
                            .into(),
                    ),
                }
            }
        }
        AppId::Nextcloud => {
            let (ok, detail) = detect_nextcloud_status();
            let state = if ok {
                AppStateKind::Running
            } else {
                AppStateKind::NotInstalled
            };
            AppStatus {
                id,
                state,
                detail,
                warning: if ok {
                    Some(
                        "CPN installs Nextcloud files under /opt/nextcloud. Finish OCC/web setup (database + admin) for a production Nextcloud site."
                            .into(),
                    )
                } else {
                    None
                },
            }
        }
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

fn block_on_runtime<F, T>(fut: F) -> Result<T, String>
where
    F: std::future::Future<Output = Result<T, String>>,
{
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("Could not start install runtime: {e}"))?;
    rt.block_on(fut)
}

/// Sync install for Host packages / `cpn app install` (no installer progress UI).
pub fn install_webmail_app(id: AppId) -> Result<String, String> {
    match id {
        AppId::Nextcloud => install_nextcloud_files(),
        AppId::Nextsnapmail => {
            let msg = install_nextsnapmail_app()?;
            // Do not auto-steal active panel proxy; operator uses Set as active.
            Ok(msg)
        }
        AppId::Sogo => Err(
            "SOGo install is SCAFFOLD (not LIVE). Use Inverse SOGo packages manually for now; CPN will wire a full recipe in a later release."
                .into(),
        ),
        AppId::Snappymail | AppId::Tachyon | AppId::Roundcube => {
            let mail = mail_for_app(id)?;
            let engine = detect_server_engine().ok_or_else(|| {
                "No supported web server detected (OpenLiteSpeed, Nginx, or Caddy). Install a web server before webmail."
                    .to_string()
            })?;
            let state = quiet_app_state(engine);
            block_on_runtime(install_webmail(&state, mail, engine))?;
            // Install already configured runtime + current symlink; persist preference + public path.
            save_active_pref(mail)?;
            let mut cfg = load_webmail_config();
            cfg.public_path = public_path_for(mail).to_string();
            let _ = save_webmail_config(&cfg);
            Ok(format!(
                "Installed {} under /opt/cpn-webmail and set it as the active panel webmail ({}).",
                id.label(),
                cfg.public_path
            ))
        }
        _ => match meta_for(id).install_status {
            HostInstallStatus::RequiresNextcloud => install_nextsnapmail_app(),
            HostInstallStatus::Scaffold => Err(format!("{} install is SCAFFOLD.", id.label())),
            HostInstallStatus::Live => Err("Unexpected webmail install path.".into()),
        },
    }
}

/// Switch active panel webmail without reinstalling (mailboxes stay on Postfix/Dovecot).
pub fn activate_webmail_app(id: AppId) -> Result<String, String> {
    let mail = mail_for_app(id)?;
    if !client_files_present(mail) {
        return Err(format!(
            "{} is not installed. Install it first, then set it as active.",
            id.label()
        ));
    }
    save_active_pref(mail)?;
    let mut cfg = load_webmail_config();
    cfg.public_path = public_path_for(mail).to_string();
    save_webmail_config(&cfg)?;

    if let Some(docroot) = docroot_for(mail) {
        let engine = detect_server_engine().ok_or_else(|| {
            "No supported web server detected (OpenLiteSpeed, Nginx, or Caddy).".to_string()
        })?;
        let state = quiet_app_state(engine);
        block_on_runtime(configure_webmail_runtime(&state, docroot, engine))?;
        Ok(format!(
            "Active panel webmail is now {} (proxy {} -> {}). Mailboxes on Postfix/Dovecot are unchanged.",
            id.label(),
            cfg.public_path,
            docroot
        ))
    } else if mail == MailSystem::Nextsnapmail {
        // Keep existing panel proxy/docroot for Tachyon/SnappyMail/Roundcube; preference marks NextSnapMail active in Host packages.
        Ok(
            "Active webmail preference set to NextSnapMail (runs inside Nextcloud under apps/nextsnapmail). Open it from Nextcloud after OCC setup. Panel /tachyon, /snappymail, and /roundcube proxies stay on the last panel client so mailboxes keep working."
                .into(),
        )
    } else {
        Err(format!("{} cannot be activated as panel webmail.", id.label()))
    }
}

pub fn uninstall_webmail_app(id: AppId) -> Result<String, String> {
    let was_active = is_active_webmail(id);
    let msg = match id {
        AppId::Snappymail => {
            let dir = "/opt/cpn-webmail/snappymail";
            if Path::new(dir).exists() {
                std::fs::remove_dir_all(dir).map_err(|e| format!("Could not remove {dir}: {e}"))?;
            }
            format!("Removed SnappyMail files from {dir}.")
        }
        AppId::Tachyon => {
            let dir = "/opt/cpn-webmail/tachyon";
            if Path::new(dir).exists() {
                std::fs::remove_dir_all(dir).map_err(|e| format!("Could not remove {dir}: {e}"))?;
            }
            format!("Removed Tachyon files from {dir}.")
        }
        AppId::Roundcube => {
            let dir = "/opt/cpn-webmail/roundcube";
            if Path::new(dir).exists() {
                std::fs::remove_dir_all(dir).map_err(|e| format!("Could not remove {dir}: {e}"))?;
            }
            format!("Removed Roundcube files from {dir}.")
        }
        AppId::Nextsnapmail => uninstall_nextsnapmail_app()?,
        AppId::Nextcloud => {
            return Err(
                "Uninstall Nextcloud from Host packages is not automated yet (removes a full app stack). Remove /opt/nextcloud manually if needed."
                    .into(),
            );
        }
        AppId::Sogo => {
            return Err("SOGo was not installed by CPN (scaffold only).".into());
        }
        _ => return Err("Not a webmail host package.".into()),
    };
    if was_active {
        // Prefer remaining panel client; fall back to default preference when none left.
        let fallback = [
            AppId::Tachyon,
            AppId::Snappymail,
            AppId::Roundcube,
            AppId::Nextsnapmail,
        ]
        .into_iter()
        .find(|cand| *cand != id && mail_for_app(*cand).ok().is_some_and(client_files_present));
        if let Some(cand) = fallback {
            let _ = activate_webmail_app(cand);
        } else if load_active_pref().is_some_and(|m| mail_to_id(m) == id.as_str()) {
            let _ = save_active_pref(default_webmail_client());
        }
    }
    Ok(msg)
}

pub fn is_webmail_app(id: AppId) -> bool {
    matches!(
        id,
        AppId::Snappymail
            | AppId::Tachyon
            | AppId::Roundcube
            | AppId::Nextsnapmail
            | AppId::Sogo
            | AppId::Nextcloud
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mail_mapping() {
        assert_eq!(mail_for_app(AppId::Tachyon).unwrap(), MailSystem::Tachyon);
        assert_eq!(mail_for_app(AppId::Roundcube).unwrap(), MailSystem::Roundcube);
        assert!(mail_for_app(AppId::Nextcloud).is_err());
    }
}
