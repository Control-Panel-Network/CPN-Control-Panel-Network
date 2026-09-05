//! Web server install stage (Nginx / Caddy / OpenLiteSpeed).

use crate::install_journal::{self, JournalAction};
use crate::install_recipes::{
    command, prepare_caddy_apt_command, prepare_caddy_repository,
    prepare_openlitespeed_apt_command, prepare_openlitespeed_repository, server_recipes,
};
use crate::installer::{AppState, finish, run_command};
use crate::model::ServerEngine;
use crate::os_support::require_installable_guest;
use std::process::Stdio;
use tokio::process::Command;

async fn detect_lsws_unit() -> Result<&'static str, String> {
    for unit in ["lsws", "lshttpd"] {
        let lib = format!("/usr/lib/systemd/system/{unit}.service");
        let etc = format!("/etc/systemd/system/{unit}.service");
        if std::path::Path::new(&lib).exists() || std::path::Path::new(&etc).exists() {
            return Ok(unit);
        }
    }
    Err("No se encontró la unidad systemd vendor de OpenLiteSpeed (lsws/lshttpd)".into())
}

async fn configure_openlitespeed(state: &AppState) -> Result<&'static str, String> {
    let wrapper = std::path::Path::new("/etc/systemd/system/openlitespeed.service");
    if wrapper.exists() {
        let _ = std::fs::remove_file(wrapper);
    }
    std::fs::create_dir_all("/var/www/cpn/html")
        .map_err(|error| format!("No se pudo crear el document root CPN: {error}"))?;
    std::fs::write(
        "/var/www/cpn/html/index.html",
        "<!doctype html><html><head><title>CPN</title></head><body><h1>CPN OpenLiteSpeed</h1></body></html>\n",
    )
    .map_err(|error| error.to_string())?;
    let vh_dir = "/usr/local/lsws/conf/vhosts/CPN";
    std::fs::create_dir_all(vh_dir)
        .map_err(|error| format!("No se pudo crear vhost CPN: {error}"))?;
    std::fs::write(
        format!("{vh_dir}/vhconf.conf"),
        "docRoot                   $VH_ROOT/html/\nenableGzip                1\nindex  {\n  useServer               0\n  indexFiles              index.html, index.php\n}\n",
    )
    .map_err(|error| error.to_string())?;
    let httpd = "/usr/local/lsws/conf/httpd_config.conf";
    let mut conf = std::fs::read_to_string(httpd).unwrap_or_default();
    if !conf.contains("virtualHost CPN") {
        conf.push_str(
            "\nvirtualHost CPN {\n  vhRoot                  /var/www/cpn/\n  configFile              $SERVER_ROOT/conf/vhosts/CPN/vhconf.conf\n  allowSymbolLink         1\n  enableScript            1\n  restrained              1\n}\n\nlistener CPNHttp {\n  address                 *:80\n  secure                  0\n  map                     CPN *\n}\n",
        );
        std::fs::write(httpd, conf).map_err(|error| error.to_string())?;
    }
    let admin = "/usr/local/lsws/admin/conf/admin_config.conf";
    if std::path::Path::new(admin).exists() {
        let _ = Command::new("sed")
            .args([
                "-i",
                "s#address[[:space:]]\\+\\*:7080#address                 127.0.0.1:7080#",
                admin,
            ])
            .status()
            .await;
    }
    let unit = detect_lsws_unit().await?;
    run_command(
        state,
        command(
            "systemctl",
            vec!["daemon-reload"],
            "Recargando systemd para OpenLiteSpeed",
            "installing",
            80,
        ),
    )
    .await?;
    let enable = match unit {
        "lshttpd" => command(
            "systemctl",
            vec!["enable", "--now", "lshttpd"],
            "Activando el servicio vendor lshttpd",
            "installing",
            84,
        ),
        _ => command(
            "systemctl",
            vec!["enable", "--now", "lsws"],
            "Activando el servicio vendor lsws",
            "installing",
            84,
        ),
    };
    run_command(state, enable).await?;
    Ok(unit)
}

fn server_service(server: ServerEngine) -> &'static str {
    match server {
        ServerEngine::Nginx => "nginx",
        ServerEngine::Caddy => "caddy",
        ServerEngine::Openlitespeed => "lsws",
    }
}

fn server_url(server: ServerEngine) -> &'static str {
    match server {
        ServerEngine::Openlitespeed | ServerEngine::Nginx | ServerEngine::Caddy => {
            "http://127.0.0.1/"
        }
    }
}

async fn open_service_ports(environment: &crate::model::EnvironmentInfo) -> Result<bool, String> {
    match environment.firewall.as_deref() {
        Some("firewalld") => {
            let mut ok = true;
            let mut journal = String::new();
            for service in ["http", "https"] {
                let already = Command::new("firewall-cmd")
                    .args(["--query-service", service])
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .kill_on_drop(true)
                    .status()
                    .await
                    .map(|status| status.success())
                    .unwrap_or(false);
                if already {
                    journal.push_str(&format!(
                        "firewalld {service} already; created=false; owner=preexisting\n"
                    ));
                    continue;
                }
                let status = Command::new("firewall-cmd")
                    .args(["--add-service", service])
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .kill_on_drop(true)
                    .status()
                    .await
                    .map_err(|error| format!("firewall-cmd failed: {error}"))?;
                if status.success() {
                    journal.push_str(&format!(
                        "firewalld {service} ok; created=true; owner=cpn\n"
                    ));
                } else {
                    ok = false;
                    journal.push_str(&format!("firewalld {service} failed; created=false\n"));
                }
            }
            let data = crate::paths::default_data_dir();
            let _ = std::fs::create_dir_all(&data);
            let _ = std::fs::write(data.join("firewall-journal.txt"), journal);
            if !ok {
                return Err(
                    "firewalld did not open http/https; refusing to claim external access".into(),
                );
            }
            Ok(true)
        }
        Some("ufw") => {
            let mut ok = true;
            let mut journal = String::new();
            for port in ["80/tcp", "443/tcp"] {
                let status = Command::new("ufw")
                    .args(["allow", port])
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .kill_on_drop(true)
                    .status()
                    .await
                    .map_err(|error| format!("ufw failed: {error}"))?;
                if status.success() {
                    journal.push_str(&format!("ufw {port} ok; created=true; owner=cpn\n"));
                } else {
                    ok = false;
                    journal.push_str(&format!("ufw {port} failed; created=false\n"));
                }
            }
            let data = crate::paths::default_data_dir();
            let _ = std::fs::create_dir_all(&data);
            let _ = std::fs::write(data.join("firewall-journal.txt"), journal);
            if !ok {
                return Err("ufw did not allow 80/443; refusing to claim external access".into());
            }
            Ok(true)
        }
        _ => Ok(false),
    }
}

/// Remove only firewall rules CPN recorded as created (issue #21).
pub async fn cleanup_service_ports() -> Result<(), String> {
    let path = crate::paths::default_data_dir().join("firewall-journal.txt");
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return Ok(());
    };
    for line in raw.lines() {
        if !line.contains("created=true") || !line.contains("owner=cpn") {
            continue;
        }
        if line.starts_with("firewalld http") {
            let _ = Command::new("firewall-cmd")
                .args(["--remove-service", "http"])
                .kill_on_drop(true)
                .status()
                .await;
        } else if line.starts_with("firewalld https") {
            let _ = Command::new("firewall-cmd")
                .args(["--remove-service", "https"])
                .kill_on_drop(true)
                .status()
                .await;
        } else if line.starts_with("ufw 80/tcp") {
            let _ = Command::new("ufw")
                .args(["delete", "allow", "80/tcp"])
                .kill_on_drop(true)
                .status()
                .await;
        } else if line.starts_with("ufw 443/tcp") {
            let _ = Command::new("ufw")
                .args(["delete", "allow", "443/tcp"])
                .kill_on_drop(true)
                .status()
                .await;
        }
    }
    Ok(())
}

pub async fn install(state: std::sync::Arc<AppState>, server: ServerEngine) {
    install_with_database(state, server, crate::model::DatabaseEngine::Mariadb, true).await;
}

/// Web server install plus optional MariaDB/MySQL + phpMyAdmin defaults.
pub async fn install_with_database(
    state: std::sync::Arc<AppState>,
    server: ServerEngine,
    database: crate::model::DatabaseEngine,
    install_phpmyadmin: bool,
) {
    let result = async {
        let _run = install_journal::begin_install_run("server")?;
        // Keep the Actix/tokio worker free: preflight uses sync process/IO.
        let report = tokio::task::spawn_blocking(|| install_journal::run_preflight(512))
            .await
            .map_err(|error| format!("preflight join failed: {error}"))??;
        for note in report.notes {
            state.log(format!("preflight: {note}"), "info");
        }
        state.progress("configuring", 0, "Configurando el repositorio verificado").await;
        let guest = require_installable_guest()?;
        state.log(
            format!(
                "Sistema invitado detectado: {} ({})",
                guest.label, guest.pretty_name
            ),
            "info",
        );
        if guest.is_windows() {
            return Err(crate::os_support::windows_linux_recipe_blocked_message(
                "Web server install (Nginx / Caddy / OpenLiteSpeed)",
            ));
        }
        if matches!(server, ServerEngine::Caddy) {
            prepare_caddy_repository(&guest)?;
            install_journal::record(
                "server",
                JournalAction::WroteRepo,
                "/etc/yum.repos.d/caddy.repo",
                None,
                Some("caddy repo".into()),
            )?;
            if guest.uses_apt() {
                run_command(&state, prepare_caddy_apt_command()).await?;
            }
        }
        if matches!(server, ServerEngine::Openlitespeed) {
            if matches!(guest.id.as_str(), "almalinux" | "rocky" | "centos") {
                run_command(&state, command("dnf", vec!["install", "-y", "epel-release", "dnf-plugins-core"], "Configurando las dependencias de OpenLiteSpeed", "configuring", 0)).await?;
                run_command(&state, command("dnf", vec!["config-manager", "--set-enabled", "crb"], "Habilitando CRB para las dependencias de PHP", "configuring", 0)).await?;
                // lsphp83-gd requires libgd.so.103 from remi-safe (gd3php).
                let repository = match guest.major {
                    9 => Some("https://rpms.remirepo.net/enterprise/remi-release-9.rpm"),
                    10 => Some("https://rpms.remirepo.net/enterprise/remi-release-10.rpm"),
                    _ => None,
                };
                if let Some(repository) = repository {
                    run_command(&state, command("dnf", vec!["install", "-y", repository], "Configurando Remi para las dependencias de PHP", "configuring", 0)).await?;
                }
            }
            prepare_openlitespeed_repository(&guest)?;
            let repo_path = if guest.uses_apt() {
                "/etc/apt/sources.list.d/lst_debian_repo.list"
            } else {
                "/etc/yum.repos.d/litespeed.repo"
            };
            install_journal::record(
                "server",
                JournalAction::WroteRepo,
                repo_path,
                None,
                Some("litespeed repo".into()),
            )?;
            if guest.uses_apt() {
                run_command(&state, prepare_openlitespeed_apt_command()).await?;
            }
        }
        for item in server_recipes(&guest, server) {
            run_command(&state, item).await?;
        }
        install_journal::record(
            "server",
            JournalAction::InstalledPackage,
            server.label(),
            None,
            Some("web server packages".into()),
        )?;
        let mut ols_unit = None;
        if matches!(server, ServerEngine::Openlitespeed) {
            ols_unit = Some(configure_openlitespeed(&state).await?);
        }
        state
            .progress("testing", 90, "Comprobando que el servicio está activo")
            .await;
        let service = ols_unit.unwrap_or_else(|| server_service(server));
        let service_check = match service {
            "lshttpd" => command(
                "systemctl",
                vec!["is-active", "--quiet", "lshttpd"],
                "Verificando el servicio con systemd",
                "testing",
                92,
            ),
            "lsws" => command(
                "systemctl",
                vec!["is-active", "--quiet", "lsws"],
                "Verificando el servicio con systemd",
                "testing",
                92,
            ),
            _ => command(
                "systemctl",
                vec!["is-active", "--quiet", server_service(server)],
                "Verificando el servicio con systemd",
                "testing",
                92,
            ),
        };
        run_command(&state, service_check).await?;
        if matches!(server, ServerEngine::Openlitespeed) {
            let status = Command::new("bash")
                .args([
                    "-c",
                    "set -o pipefail; for attempt in {1..15}; do if curl --fail --silent --show-error --max-time 2 http://127.0.0.1/ | grep -qi 'CPN OpenLiteSpeed'; then exit 0; fi; sleep 1; done; exit 1",
                ])
                .kill_on_drop(true)
                .status()
                .await
                .map_err(|error| error.to_string())?;
            if !status.success() {
                return Err("OpenLiteSpeed no sirvió el vhost CPN en :80".into());
            }
        } else {
            run_command(
                &state,
                command(
                    "curl",
                    vec![
                        "--fail",
                        "--silent",
                        "--show-error",
                        "--max-time",
                        "10",
                        "--output",
                        "/dev/null",
                        server_url(server),
                    ],
                    "Comprobando la respuesta HTTP local",
                    "testing",
                    96,
                ),
            )
                .await?;
        }
        let environment = state
            .status
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .environment
            .clone();
        if let Some(environment) = environment {
            let opened = open_service_ports(&environment).await?;
            let mut status = state.status.write().unwrap_or_else(|e| e.into_inner());
            status.external_ports_configured = opened;
            if opened {
                status.access_note = Some(
                    "Host firewall opened http/https successfully (firewall-journal.txt)."
                        .into(),
                );
            } else {
                status.access_note = Some(
                    "No host firewall detected; service verified on loopback only.".into(),
                );
            }
        }

        state
            .progress(
                "installing",
                97,
                format!(
                    "Installing database defaults ({})",
                    database.label()
                ),
            )
            .await;
        match tokio::task::spawn_blocking(move || {
            crate::db_defaults::ensure_database_defaults(database, install_phpmyadmin)
        })
        .await
        {
            Ok(Ok(notes)) => {
                for note in notes {
                    state.log(note, "info");
                }
            }
            Ok(Err(error)) => {
                state.log(
                    format!("Database defaults warning (continuing): {error}"),
                    "error",
                );
                // Soft-fail: web server already verified; operator can fix DB from Apps.
            }
            Err(error) => {
                state.log(
                    format!("Database defaults join failed (continuing): {error}"),
                    "error",
                );
            }
        }

        Ok::<_, String>(())
    }
    .await;
    finish(&state, result, server.label(), true, false).await;
}
