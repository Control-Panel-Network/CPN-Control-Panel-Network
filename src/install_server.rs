//! Web server install stage (Nginx / Caddy / OpenLiteSpeed).

use crate::install_journal::{self, JournalAction};
use crate::install_recipes::{
    command, prepare_caddy_apt_command, prepare_caddy_repository, prepare_openlitespeed_repository,
    server_recipes,
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

async fn open_service_ports(environment: &crate::model::EnvironmentInfo) -> Result<(), String> {
    match environment.firewall.as_deref() {
        Some("firewalld") => {
            for service in ["http", "https"] {
                let _ = Command::new("firewall-cmd")
                    .args(["--add-service", service])
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status()
                    .await;
            }
            let _ = std::fs::create_dir_all("/var/lib/cpn");
            let _ = std::fs::write(
                "/var/lib/cpn/firewall-journal.txt",
                "firewalld http\nfirewalld https\n",
            );
        }
        Some("ufw") => {
            for port in ["80/tcp", "443/tcp"] {
                let _ = Command::new("ufw")
                    .args(["allow", port])
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status()
                    .await;
            }
            let _ = std::fs::create_dir_all("/var/lib/cpn");
            let _ = std::fs::write(
                "/var/lib/cpn/firewall-journal.txt",
                "ufw 80/tcp\nufw 443/tcp\n",
            );
        }
        _ => {}
    }
    Ok(())
}

pub async fn install(state: std::sync::Arc<AppState>, server: ServerEngine) {
    let result = async {
        let report = install_journal::run_preflight(512)?;
        for note in report.notes {
            state.log(format!("preflight: {note}"), "info");
        }
        let guest = require_installable_guest()?;
        state.log(
            format!(
                "Sistema invitado detectado: {} ({})",
                guest.label, guest.pretty_name
            ),
            "info",
        );
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
            prepare_openlitespeed_repository(&guest)?;
            install_journal::record(
                "server",
                JournalAction::WroteRepo,
                "/etc/yum.repos.d/litespeed.repo",
                None,
                Some("litespeed repo".into()),
            )?;
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
                    "curl --fail --silent --show-error --max-time 10 http://127.0.0.1/ | grep -qi 'CPN OpenLiteSpeed'",
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
        if let Some(environment) = state.status.read().await.environment.clone() {
            open_service_ports(&environment).await?;
        }
        Ok::<_, String>(())
    }
    .await;
    finish(&state, result, server.label(), true, false).await;
}
