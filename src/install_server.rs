//! Web server install stage (Nginx / Caddy / OpenLiteSpeed).

use crate::install_journal::{self, JournalAction};
use crate::install_recipes::{
    command, prepare_caddy_apt_command, prepare_caddy_repository,
    prepare_openlitespeed_apt_command, prepare_openlitespeed_repository, server_recipes,
    web_server_present,
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

/// Stop and disable other HTTP stacks that commonly hold :80/:443 so the
/// selected engine can bind its public listeners on a clean AlmaLinux lab image
/// (nginx is often enabled by default).
async fn stop_conflicting_http_services(
    state: &AppState,
    selected: ServerEngine,
) -> Result<(), String> {
    let conflicts: &[&str] = match selected {
        ServerEngine::Openlitespeed => &["nginx", "httpd", "apache2", "caddy"],
        ServerEngine::Nginx => &["httpd", "apache2", "lshttpd", "lsws", "caddy"],
        ServerEngine::Caddy => &["nginx", "httpd", "apache2", "lshttpd", "lsws"],
    };
    for unit in conflicts {
        let unit_path_lib = format!("/usr/lib/systemd/system/{unit}.service");
        let unit_path_etc = format!("/etc/systemd/system/{unit}.service");
        if !std::path::Path::new(&unit_path_lib).exists()
            && !std::path::Path::new(&unit_path_etc).exists()
        {
            continue;
        }
        let active = Command::new("systemctl")
            .args(["is-active", "--quiet", unit])
            .status()
            .await
            .map(|status| status.success())
            .unwrap_or(false);
        if !active {
            continue;
        }
        state
            .progress(
                "configuring",
                2,
                format!("Liberando :80/:443 deteniendo el servicio conflictivo {unit}"),
            )
            .await;
        let stop_status = Command::new("systemctl")
            .args(["stop", unit])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .status()
            .await
            .map_err(|error| format!("No se pudo detener {unit}: {error}"))?;
        if !stop_status.success() {
            return Err(format!(
                "systemctl stop {unit} falló (código {:?}); libera :80/:443 manualmente e inténtalo de nuevo",
                stop_status.code()
            ));
        }
        // Best-effort disable so a reboot does not steal :80 again.
        let _ = Command::new("systemctl")
            .args(["disable", unit])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await;
        install_journal::record(
            "server",
            JournalAction::Note,
            unit,
            None,
            Some(format!(
                "stopped conflicting HTTP service before installing {}",
                selected.label()
            )),
        )?;
    }
    Ok(())
}

fn openlitespeed_config_is_valid(success: bool, output: &str) -> bool {
    let diagnostics = output
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect::<Vec<_>>();
    let fatal = diagnostics
        .iter()
        .any(|line| line.contains("[ERROR]") || line.contains("[FATAL]"));
    !fatal
        && (success
            || (!diagnostics.is_empty() && diagnostics.iter().all(|line| line.contains("[WARN]"))))
}

fn bind_ols_admin_to_loopback(contents: &str) -> (String, bool) {
    let mut changed = false;
    let mut output = String::with_capacity(contents.len());
    for line in contents.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("address") && trimmed.contains("*:7080") {
            let indent_len = line.len().saturating_sub(trimmed.len());
            output.push_str(&line[..indent_len]);
            output.push_str("address                 127.0.0.1:7080");
            changed = true;
        } else {
            output.push_str(line);
        }
        output.push('\n');
    }
    if !contents.ends_with('\n') && !output.is_empty() {
        output.pop();
    }
    (output, changed)
}

fn openlitespeed_restart_command() -> &'static str {
    // The vendor unit uses `KillMode=none`, so `systemctl restart` can start a
    // second daemon while the previous one still owns the admin listener. Stop
    // it first and wait for that listener to be released before starting it.
    "systemctl stop \"$1\"; for attempt in $(seq 1 30); do if ! ss -ltn | grep -qE '[:.]7080[[:space:]]'; then systemctl start \"$1\"; exit $?; fi; sleep 1; done; echo 'OpenLiteSpeed admin listener on :7080 was not released after stop.' >&2; exit 1"
}

async fn configure_openlitespeed(state: &AppState) -> Result<&'static str, String> {
    // Do not delete /etc/systemd/system/openlitespeed.service. A unit in /etc is
    // administrator-owned state; CPN only enables the vendor lsws/lshttpd unit it finds.
    install_journal::write_file_tracked(
        "server",
        std::path::Path::new("/var/www/cpn/html/index.html"),
        "<!doctype html><html><head><title>CPN</title></head><body><h1>CPN OpenLiteSpeed</h1></body></html>\n",
    )?;

    let vhconf = "/usr/local/lsws/conf/vhosts/CPN/vhconf.conf";
    install_journal::write_file_tracked(
        "server",
        std::path::Path::new(vhconf),
        "docRoot                   $VH_ROOT/html/\nenableGzip                1\nindex  {\n  useServer               0\n  indexFiles              index.html, index.php\n}\n",
    )?;

    run_command(
        state,
        command(
            "chown",
            vec!["-R", "nobody:nobody", "/var/www/cpn"],
            "Ajustando permisos para OpenLiteSpeed",
            "installing",
            81,
        ),
    )
    .await?;

    let httpd = "/usr/local/lsws/conf/httpd_config.conf";
    let mut conf = std::fs::read_to_string(httpd)
        .map_err(|error| format!("No se pudo leer {httpd}: {error}"))?;
    let mut changed = false;
    if !conf.contains("virtualHost CPN") {
        conf.push_str("\nvirtualHost CPN {\n  vhRoot                  /var/www/cpn/\n  configFile              $SERVER_ROOT/conf/vhosts/CPN/vhconf.conf\n  allowSymbolLink         1\n  enableScript            1\n  restrained              1\n}\n");
        changed = true;
    }
    if !conf.contains("listener CPNHttp") {
        conf.push_str("\nlistener CPNHttp {\n  address                 *:80\n  secure                  0\n  map                     CPN *\n}\n");
        changed = true;
    }
    if changed {
        install_journal::write_file_tracked("server", std::path::Path::new(httpd), &conf)?;
    } else {
        install_journal::record(
            "server",
            JournalAction::Note,
            httpd,
            None,
            Some("OpenLiteSpeed CPN vhost/listener already configured".into()),
        )?;
    }

    let admin = "/usr/local/lsws/admin/conf/admin_config.conf";
    if std::path::Path::new(admin).exists() {
        let original = std::fs::read_to_string(admin)
            .map_err(|error| format!("No se pudo leer {admin}: {error}"))?;
        let (updated, admin_changed) = bind_ols_admin_to_loopback(&original);
        if admin_changed {
            install_journal::write_file_tracked("server", std::path::Path::new(admin), &updated)?;
        }
    }

    let vendor_unit = "/usr/local/lsws/admin/misc/lshttpd.service";
    if !std::path::Path::new("/usr/lib/systemd/system/lsws.service").exists()
        && !std::path::Path::new("/usr/lib/systemd/system/lshttpd.service").exists()
        && std::path::Path::new(vendor_unit).exists()
    {
        let contents = std::fs::read_to_string(vendor_unit).map_err(|error| {
            format!("No se pudo leer la unidad vendor de OpenLiteSpeed: {error}")
        })?;
        install_journal::write_file_tracked(
            "server",
            std::path::Path::new("/usr/lib/systemd/system/lshttpd.service"),
            &contents,
        )?;
    }

    let unit = detect_lsws_unit().await?;
    let validation = Command::new("/usr/local/lsws/bin/openlitespeed")
        .arg("-t")
        .env("LC_ALL", "C")
        .output()
        .await
        .map_err(|error| format!("No se pudo validar OpenLiteSpeed: {error}"))?;
    let validation_output = format!(
        "{}{}",
        String::from_utf8_lossy(&validation.stdout),
        String::from_utf8_lossy(&validation.stderr)
    );
    if !openlitespeed_config_is_valid(validation.status.success(), &validation_output) {
        return Err(format!(
            "La configuración de OpenLiteSpeed no es válida:\n{}",
            validation_output.trim()
        ));
    }

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
    run_command(
        state,
        command(
            "systemctl",
            vec!["enable", unit],
            "Habilitando OpenLiteSpeed al arrancar",
            "installing",
            83,
        ),
    )
    .await?;
    // The package may start OpenLiteSpeed before CPN writes its listener.
    // Its vendor unit has KillMode=none, so a direct restart races with the
    // previous daemon still holding :7080. Stop, wait for the listener, then
    // start to guarantee that the CPN configuration can bind both listeners.
    run_command(
        state,
        command(
            "bash",
            vec![
                "-c",
                openlitespeed_restart_command(),
                "cpn-ols-restart",
                unit,
            ],
            "Reiniciando OpenLiteSpeed con el vhost CPN",
            "installing",
            84,
        ),
    )
    .await?;
    Ok(unit)
}

/// Wait for the CPN listener without flooding the operator transcript with one
/// curl error per retry.  On failure, emit the useful OLS/socket diagnostics to
/// `installation.log` through `run_command`.
async fn verify_openlitespeed_vhost(state: &AppState) -> Result<(), String> {
    run_command(
        state,
        command(
            "bash",
            vec![
                "-c",
                "for attempt in $(seq 1 30); do if curl --fail --silent --max-time 2 http://127.0.0.1/ | grep -qi 'CPN OpenLiteSpeed'; then exit 0; fi; sleep 1; done; echo 'OpenLiteSpeed did not return CPN content on 127.0.0.1:80.' >&2; echo 'Listening TCP sockets:' >&2; (ss -ltnp 2>&1 || netstat -ltn 2>&1 || true) >&2; echo 'OpenLiteSpeed error log tail:' >&2; tail -n 80 /usr/local/lsws/logs/error.log 2>&1 || true; exit 1",
            ],
            "Comprobando el vhost CPN de OpenLiteSpeed en :80",
            "testing",
            96,
        ),
    )
    .await
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

fn ufw_status_allows_port(status: &str, port: &str) -> bool {
    status.lines().any(|line| {
        let trimmed = line.trim();
        trimmed.starts_with(port)
            && trimmed
                .split_whitespace()
                .any(|part| part.eq_ignore_ascii_case("ALLOW"))
    })
}

async fn ufw_port_allowed(port: &str) -> bool {
    Command::new("ufw")
        .arg("status")
        .output()
        .await
        .is_ok_and(|result| ufw_status_allows_port(&String::from_utf8_lossy(&result.stdout), port))
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
                if ufw_port_allowed(port).await {
                    journal.push_str(&format!(
                        "ufw {port} already; created=false; owner=preexisting\n"
                    ));
                    continue;
                }
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

/// Remove only firewall rules CPN recorded as created.
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
    let _ = std::fs::remove_file(path);
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

        state
            .progress("configuring", 0, "Revisando el sistema y los repositorios")
            .await;
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

        let server_preexisting = web_server_present(server);
        if server_preexisting {
            state.log(
                format!(
                    "{} ya está instalado; CPN reutilizará la instalación existente y continuará con activación/configuración.",
                    server.label()
                ),
                "info",
            );
        }

        if matches!(server, ServerEngine::Caddy) && !server_preexisting {
            prepare_caddy_repository(&guest)?;
            if guest.uses_apt() {
                run_command(&state, prepare_caddy_apt_command()).await?;
            }
        }

        if matches!(server, ServerEngine::Openlitespeed) && !server_preexisting {
            if matches!(guest.id.as_str(), "almalinux" | "rocky" | "centos") {
                run_command(
                    &state,
                    command(
                        "dnf",
                        vec![
                            "--setopt=lock_timeout=60",
                            "install",
                            "-y",
                            "epel-release",
                            "dnf-plugins-core",
                        ],
                        "Configurando las dependencias de OpenLiteSpeed",
                        "configuring",
                        0,
                    ),
                )
                .await?;
                run_command(
                    &state,
                    command(
                        "dnf",
                        vec![
                            "--setopt=lock_timeout=60",
                            "config-manager",
                            "--set-enabled",
                            "crb",
                        ],
                        "Habilitando CRB para las dependencias de PHP",
                        "configuring",
                        0,
                    ),
                )
                .await?;
                // lsphp83-gd requires libgd.so.103 from remi-safe (gd3php).
                let repository = match guest.major {
                    9 => Some("https://rpms.remirepo.net/enterprise/remi-release-9.rpm"),
                    10 => Some("https://rpms.remirepo.net/enterprise/remi-release-10.rpm"),
                    _ => None,
                };
                if let Some(repository) = repository {
                    run_command(
                        &state,
                        command(
                            "dnf",
                            vec!["--setopt=lock_timeout=60", "install", "-y", repository],
                            "Configurando Remi para las dependencias de PHP",
                            "configuring",
                            0,
                        ),
                    )
                    .await?;
                    // `gd3php` is published by remi-safe for some EL releases,
                    // but not every supported major.  Never turn an absent
                    // optional provider into an installer failure: when it is
                    // present install it before OpenLiteSpeed; otherwise let
                    // DNF resolve the server's actual dependency set.
                    run_command(
                        &state,
                        command(
                            "bash",
                            vec![
                                "-c",
                                "if dnf -q --disablerepo='*' --enablerepo=remi-safe list available gd3php >/dev/null 2>&1; then dnf --setopt=lock_timeout=60 --enablerepo=remi-safe install -y gd3php; else echo 'gd3php is not available from remi-safe for this guest; continuing with OpenLiteSpeed dependency resolution.'; fi",
                            ],
                            "Comprobando libgd para OpenLiteSpeed",
                            "configuring",
                            0,
                        ),
                    )
                    .await?;
                }
            }
            prepare_openlitespeed_repository(&guest)?;
            if guest.uses_apt() {
                run_command(&state, prepare_openlitespeed_apt_command()).await?;
            }
        }

        for item in server_recipes(&guest, server) {
            run_command(&state, item).await?;
        }
        if server_preexisting {
            install_journal::record(
                "server",
                JournalAction::Note,
                server.label(),
                None,
                Some("adopted pre-existing web server; package install skipped".into()),
            )?;
        } else {
            install_journal::record(
                "server",
                JournalAction::InstalledPackage,
                server.label(),
                None,
                Some("web server package installed by this CPN run".into()),
            )?;
        }

        stop_conflicting_http_services(&state, server).await?;

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
            // OLS packages can report the systemd unit active before the forked
            // listener is ready. Give it a bounded first window, then use its
            // native controller once and make the second failure diagnosable.
            if verify_openlitespeed_vhost(&state).await.is_err() {
                state.log(
                    "OpenLiteSpeed no respondió tras systemctl; reintentando con lswsctrl.",
                    "info",
                );
                run_command(
                    &state,
                    command(
                        "bash",
                        vec![
                            "-c",
                            "/usr/local/lsws/bin/lswsctrl restart || /usr/local/lsws/bin/lswsctrl start",
                        ],
                        "Reintentando OpenLiteSpeed con su controlador nativo",
                        "testing",
                        94,
                    ),
                )
                .await?;
                verify_openlitespeed_vhost(&state)
                    .await
                    .map_err(|_| "OpenLiteSpeed no sirvió el vhost CPN en :80; revisa installation.log, comprueba que nginx/httpd no ocupen el puerto, y el error log de OpenLiteSpeed.".to_string())?;
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
            let configured = open_service_ports(&environment).await?;
            let mut status = state.status.write().unwrap_or_else(|e| e.into_inner());
            status.external_ports_configured = configured;
            if configured {
                status.access_note = Some(
                    "Host firewall allows http/https; only CPN-owned additions are recorded for cleanup."
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
                format!("Installing database defaults ({})", database.label()),
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

#[cfg(test)]
mod tests {
    use super::{
        bind_ols_admin_to_loopback, openlitespeed_config_is_valid, openlitespeed_restart_command,
        ufw_status_allows_port,
    };

    #[test]
    fn openlitespeed_validation_accepts_success_and_warning_only_exit() {
        assert!(openlitespeed_config_is_valid(
            true,
            "[OK] configuration valid"
        ));
        assert!(openlitespeed_config_is_valid(
            false,
            "[WARN] module unavailable"
        ));
    }

    #[test]
    fn openlitespeed_validation_rejects_errors_and_unknown_failures() {
        assert!(!openlitespeed_config_is_valid(
            false,
            "[ERROR] listener conflict"
        ));
        assert!(!openlitespeed_config_is_valid(
            false,
            "configuration failed"
        ));
        assert!(!openlitespeed_config_is_valid(false, ""));
    }

    #[test]
    fn openlitespeed_admin_bind_preserves_other_lines() {
        let input = "listener WebAdmin {\n  address                 *:7080\n  secure                  0\n}\n";
        let (output, changed) = bind_ols_admin_to_loopback(input);
        assert!(changed);
        assert!(output.contains("127.0.0.1:7080"));
        assert!(output.contains("secure                  0"));
    }

    #[test]
    fn openlitespeed_restart_waits_for_admin_listener_before_starting() {
        let command = openlitespeed_restart_command();
        assert!(command.starts_with("systemctl stop \"$1\";"));
        assert!(command.contains("ss -ltn"));
        assert!(command.contains("systemctl start \"$1\""));
        assert!(!command.contains("systemctl restart"));
    }

    #[test]
    fn ufw_preexisting_rule_detection_is_port_specific() {
        let status = "Status: active\n80/tcp ALLOW Anywhere\n443/tcp ALLOW Anywhere\n";
        assert!(ufw_status_allows_port(status, "80/tcp"));
        assert!(ufw_status_allows_port(status, "443/tcp"));
        assert!(!ufw_status_allows_port(status, "8080/tcp"));
    }
}
