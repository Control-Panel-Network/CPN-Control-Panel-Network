use crate::install_recipes::{
    CommandSpec, DnfProgress, apt_update_command, command, php_install_command,
    php_module_enable_command, pkg_install, prepare_caddy_apt_command, prepare_caddy_repository,
    server_recipes,
};
use crate::model::{InstallerEvent, InstallerStatus, MailSystem, ServerEngine};
use crate::install_webmail::install_webmail;
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
    state.log(format!("› {}", spec.description), "info");
    let mut child = Command::new(spec.program)
        .args(&spec.args)
        .env("LC_ALL", "C")
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

async fn configure_openlitespeed(state: &AppState) -> Result<(), String> {
    const UNIT: &str = "[Unit]\nDescription=OpenLiteSpeed HTTP Server\nAfter=network.target\n\n[Service]\nType=forking\nExecStart=/usr/local/lsws/bin/lswsctrl start\nExecStop=/usr/local/lsws/bin/lswsctrl stop\nExecReload=/usr/local/lsws/bin/lswsctrl restart\nRemainAfterExit=yes\n\n[Install]\nWantedBy=multi-user.target\n";
    std::fs::write("/etc/systemd/system/openlitespeed.service", UNIT)
        .map_err(|error| format!("No se pudo crear el servicio de OpenLiteSpeed: {error}"))?;
    run_command(
        state,
        command(
            "systemctl",
            vec!["daemon-reload"],
            "Registrando OpenLiteSpeed en systemd",
            "installing",
            80,
        ),
    )
    .await?;
    run_command(
        state,
        command(
            "systemctl",
            vec!["enable", "--now", "openlitespeed"],
            "Activando OpenLiteSpeed",
            "installing",
            84,
        ),
    )
    .await
}

fn server_service(server: ServerEngine) -> &'static str {
    match server {
        ServerEngine::Nginx => "nginx",
        ServerEngine::Caddy => "caddy",
        ServerEngine::Openlitespeed => "openlitespeed",
    }
}

fn server_url(server: ServerEngine) -> &'static str {
    match server {
        ServerEngine::Openlitespeed => "http://127.0.0.1:8088/",
        ServerEngine::Nginx | ServerEngine::Caddy => "http://127.0.0.1/",
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
        for item in server_recipes(&guest, server) {
            run_command(&state, item).await?;
        }
        if matches!(server, ServerEngine::Openlitespeed) {
            configure_openlitespeed(&state).await?;
        }
        state
            .progress("testing", 90, "Comprobando que el servicio está activo")
            .await;
        run_command(
            &state,
            command(
                "systemctl",
                vec!["is-active", "--quiet", server_service(server)],
                "Verificando el servicio con systemd",
                "testing",
                92,
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
        Ok::<_, String>(())
    }
    .await;
    finish(&state, result, server.label(), true).await;
}

async fn install_php_runtime(state: &AppState, label: &'static str) -> Result<(), String> {
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
                    "Verificando la versión instalada",
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
        Ok::<_, String>(())
    }
    .await;
    finish(&state, result, mail.label(), false).await;
}

async fn finish(
    state: &AppState,
    result: Result<(), String>,
    label: &str,
    mark_server_ready: bool,
) {
    let mut status = state.status.write().await;
    match result {
        Ok(()) => {
            if mark_server_ready {
                status.server_ready = true;
            }
            status.phase = "completed";
            status.progress = 100;
            status.message = format!("{label} se instaló y verificó correctamente");
            let _ = state.events.send(InstallerEvent::Completed {
                status: status.clone(),
            });
        }
        Err(error) => {
            status.phase = "failed";
            status.error = Some(error.clone());
            status.message = "La instalación se detuvo de forma segura".into();
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
