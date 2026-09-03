use crate::install_recipes::{
    CommandSpec, DnfProgress, apt_update_command, command, php_install_command,
    php_module_enable_command, pkg_install, prepare_caddy_apt_command, prepare_caddy_repository,
    prepare_openlitespeed_repository, server_recipes,
};
use crate::install_webmail::install_webmail;
use crate::manifest::{self, ManifestSource};
use crate::model::{InstallerEvent, InstallerStatus, MailSystem, ServerEngine};
use crate::os_support::require_installable_guest;
use std::process::Stdio;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Command,
    sync::{RwLock, broadcast},
};

pub struct AppState {
    pub status: RwLock<InstallerStatus>,
    pub events: broadcast::Sender<InstallerEvent>,
    pub token: String,
}

impl AppState {
    pub async fn progress(&self, phase: &'static str, progress: u8, message: impl Into<String>) {
        let mut status = self.status.write().await;
        status.phase = phase;
        status.progress = progress;
        status.message = message.into();
        status.error = None;
        let _ = self.events.send(InstallerEvent::Progress {
            status: status.clone(),
        });
    }

    pub fn log(&self, line: impl Into<String>, level: &'static str) {
        let _ = self.events.send(InstallerEvent::Log {
            line: line.into(),
            level,
        });
    }
}

fn fraction(line: &str) -> Option<(u32, u32)> {
    line.split_whitespace().find_map(|token| {
        let clean =
            token.trim_matches(|character: char| !character.is_ascii_digit() && character != '/');
        let (current, total) = clean.split_once('/')?;
        let current = current.parse().ok()?;
        let total = total.parse().ok()?;
        (total > 0 && current <= total).then_some((current, total))
    })
}

async fn process_dnf_line(
    state: &AppState,
    tracking: DnfProgress,
    transaction: &mut bool,
    line: &str,
) {
    if line.contains("Running transaction") {
        *transaction = true;
        state
            .progress(
                "installing",
                tracking.install_start,
                format!("Instalando {}", tracking.label),
            )
            .await;
        return;
    }
    if let Some((current, total)) = fraction(line) {
        let ratio = current as f32 / total as f32;
        let (phase, start, end, action) = if *transaction {
            (
                "installing",
                tracking.install_start,
                tracking.install_end,
                "Instalando",
            )
        } else {
            (
                "downloading",
                tracking.download_start,
                tracking.download_end,
                "Descargando",
            )
        };
        let progress = start + ((end - start) as f32 * ratio).round() as u8;
        state
            .progress(phase, progress, format!("{action} {}", tracking.label))
            .await;
    }
}

pub(crate) async fn run_command(state: &AppState, spec: CommandSpec) -> Result<(), String> {
    if let Some(tracking) = spec.dnf {
        state
            .progress(
                "downloading",
                tracking.download_start,
                format!("Descargando {}", tracking.label),
            )
            .await;
    } else {
        state
            .progress(spec.phase, spec.progress, spec.description)
            .await;
    }
    state.log(format!("â€º {}", spec.description), "info");
    let mut child = Command::new(spec.program)
        .args(&spec.args)
        .env("LC_ALL", "C")
        .kill_on_drop(true)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("No se pudo ejecutar {}: {error}", spec.program))?;
    let stdout = child
        .stdout
        .take()
        .ok_or("No se pudo leer la salida del proceso")?;
    let stderr = child
        .stderr
        .take()
        .ok_or("No se pudo leer el error del proceso")?;
    let mut out_lines = BufReader::new(stdout).lines();
    let mut err_lines = BufReader::new(stderr).lines();
    let (mut out_done, mut err_done, mut transaction) = (false, false, false);
    let pump = async {
        while !out_done || !err_done {
            tokio::select! {
                line = out_lines.next_line(), if !out_done => match line {
                    Ok(Some(line)) => {
                        if !line.trim().is_empty() { state.log(&line, "info"); }
                        if let Some(tracking) = spec.dnf { process_dnf_line(state, tracking, &mut transaction, &line).await; }
                    }
                    Ok(None) | Err(_) => out_done = true,
                },
                line = err_lines.next_line(), if !err_done => match line {
                    Ok(Some(line)) => {
                        if !line.trim().is_empty() { state.log(&line, "info"); }
                        if let Some(tracking) = spec.dnf { process_dnf_line(state, tracking, &mut transaction, &line).await; }
                    }
                    Ok(None) | Err(_) => err_done = true,
                },
            }
        }
        let exit = child.wait().await.map_err(|error| error.to_string())?;
        if !exit.success() {
            return Err(format!(
                "{} terminó con código {}",
                spec.description,
                exit.code().unwrap_or(-1)
            ));
        }
        Ok::<(), String>(())
    };
    match tokio::time::timeout(std::time::Duration::from_secs(1800), pump).await {
        Ok(result) => result?,
        Err(_) => {
            let _ = child.kill().await;
            let _ = child.wait().await;
            return Err(format!("Tiempo de espera agotado en: {}", spec.description));
        }
    }
    if let Some(tracking) = spec.dnf {
        state
            .progress(
                "installing",
                tracking.install_end,
                format!("{} instalado", tracking.label),
            )
            .await;
    }
    Ok(())
}

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
    // Prefer vendor unit; remove any leftover CPN wrapper (issue #14).
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
        // Never use Example:8088 as success criteria (issue #15).
        ServerEngine::Openlitespeed | ServerEngine::Nginx | ServerEngine::Caddy => {
            "http://127.0.0.1/"
        }
    }
}

pub async fn install(state: std::sync::Arc<AppState>, server: ServerEngine) {
    let result = async {
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
            if guest.uses_apt() {
                run_command(&state, prepare_caddy_apt_command()).await?;
            }
        }
        if matches!(server, ServerEngine::Openlitespeed) {
            prepare_openlitespeed_repository(&guest)?;
        }
        for item in server_recipes(&guest, server) {
            run_command(&state, item).await?;
        }
        let mut ols_unit = None;
        if matches!(server, ServerEngine::Openlitespeed) {
            ols_unit = Some(configure_openlitespeed(&state).await?);
        }
        state
            .progress("testing", 90, "Comprobando que el servicio estÃ¡ activo")
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
        // Open http/https when a host firewall is active (issue #21).
        if let Some(environment) = state.status.read().await.environment.clone() {
            open_service_ports(&environment).await?;
        }
        Ok::<_, String>(())
    }
    .await;
    finish(&state, result, server.label(), true, false).await;
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

pub(crate) async fn install_php_runtime(
    state: &AppState,
    label: &'static str,
) -> Result<(), String> {
    let guest = require_installable_guest()?;
    if let Some(enable) = php_module_enable_command(&guest) {
        run_command(state, enable).await?;
    } else {
        let message = if guest.uses_apt() {
            format!("Usando paquetes PHP de apt en {}", guest.label)
        } else {
            format!("Usando PHP de AppStream en {}", guest.label)
        };
        state.progress("downloading", 38, message).await;
    }
    if guest.uses_apt() {
        run_command(state, apt_update_command()).await?;
    }
    run_command(state, php_install_command(&guest, label)).await
}

pub async fn install_mail(state: std::sync::Arc<AppState>, mail: MailSystem) {
    let result = async {
        let guest = require_installable_guest()?;
        state.log(
            format!(
                "Sistema invitado detectado: {} ({})",
                guest.label, guest.pretty_name
            ),
            "info",
        );
        if matches!(mail, MailSystem::Thunderbird) {
            run_command(
                &state,
                pkg_install(
                    &guest,
                    vec!["thunderbird"],
                    vec!["thunderbird"],
                    "Instalando Thunderbird",
                    DnfProgress {
                        download_start: 2,
                        download_end: 58,
                        install_start: 60,
                        install_end: 88,
                        label: "Thunderbird",
                    },
                ),
            )
            .await?;
            state
                .progress("testing", 92, "Comprobando Thunderbird")
                .await;
            run_command(
                &state,
                command(
                    "thunderbird",
                    vec!["--version"],
                    "Verificando la versiÃ³n instalada",
                    "testing",
                    96,
                ),
            )
            .await?;
        } else {
            install_webmail(&state, mail).await?;
            state
                .progress("testing", 92, format!("Comprobando {}", mail.label()))
                .await;
            run_command(
                &state,
                command(
                    "systemctl",
                    vec!["is-active", "--quiet", "cpn-webmail"],
                    "Verificando el servicio de webmail",
                    "testing",
                    94,
                ),
            )
            .await?;
            run_command(
                &state,
                command(
                    "curl",
                    vec![
                        "--fail",
                        "--silent",
                        "--show-error",
                        "--retry",
                        "10",
                        "--retry-connrefused",
                        "--retry-delay",
                        "1",
                        "--max-time",
                        "15",
                        "--output",
                        "/dev/null",
                        "http://127.0.0.1:8888/",
                    ],
                    "Comprobando la respuesta HTTP del webmail",
                    "testing",
                    97,
                ),
            )
            .await?;
        }
            {
                let mut status = state.status.write().await;
                status.mail_client_ready = true;
                status.mail_backend_ready = false;
                if matches!(mail, MailSystem::Thunderbird) {
                    status.access_note = Some(
                        "Thunderbird is a desktop mail client. It does not provision IMAP/SMTP on this host."
                            .into(),
                    );
                } else {
                    status.access_note = Some(
                        "Webmail client installed. IMAP/SMTP backend was not provisioned by CPN."
                            .into(),
                    );
                }
            }
        Ok::<_, String>(())
    }
    .await;
    finish(&state, result, mail.label(), false, true).await;
}

async fn finish(
    state: &AppState,
    result: Result<(), String>,
    label: &str,
    mark_server_ready: bool,
    mail_flow: bool,
) {
    let mut status = state.status.write().await;
    match result {
        Ok(()) => {
            if mark_server_ready {
                status.server_ready = true;
                status.external_ports_configured = status
                    .environment
                    .as_ref()
                    .and_then(|env| env.firewall.as_ref())
                    .is_some();
                if status.access_note.is_none() {
                    status.access_note = Some(
                        "Servicio verificado en loopback. Comprueba acceso externo desde otra máquina si el firewall estaba activo."
                            .into(),
                    );
                }
            }
            status.phase = "completed";
            status.progress = 100;
            if mail_flow && !status.mail_backend_ready {
                status.message = format!(
                    "{label} (mail client) installed. IMAP/SMTP backend was not provisioned by CPN."
                );
            } else {
                status.message = format!("{label} se instaló y verificó correctamente");
            }
            if let Err(error) = manifest::record_install(
                env!("CARGO_PKG_VERSION"),
                &format!("v{}", env!("CARGO_PKG_VERSION")),
                ManifestSource::Local,
                status.selected_server,
                status.selected_mail,
            ) {
                state.log(
                    format!("Warning: could not write install manifest: {error}"),
                    "error",
                );
            }
            let _ = state.events.send(InstallerEvent::Completed {
                status: status.clone(),
            });
        }
        Err(error) => {
            status.phase = "failed";
            status.error = Some(error.clone());
            status.message =
                "La instalación falló; pueden quedar cambios parciales. Revisa /var/lib/cpn/."
                    .into();
            state.log(error, "error");
            let _ = state.events.send(InstallerEvent::Error {
                status: status.clone(),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fraction;
    use crate::os_support::require_installable_guest;

    #[test]
    fn parses_dnf_download_and_transaction_fractions() {
        assert_eq!(fraction("(3/8): package.rpm"), Some((3, 8)));
        assert_eq!(fraction("Installing : package 5/7"), Some((5, 7)));
        assert_eq!(fraction("No package progress here"), None);
    }

    #[test]
    fn guest_os_detection_is_callable() {
        let _ = require_installable_guest();
    }
}
