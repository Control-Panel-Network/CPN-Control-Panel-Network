use crate::install_journal::{self, FailureKind};
use crate::install_mail_backend::{provision_local_mail_backend, verify_imap_smtp_listeners};
use crate::install_recipes::{
    CommandSpec, DnfProgress, apt_update_command, command, php_install_command,
    php_module_enable_command, pkg_install,
};
use crate::install_webmail::install_webmail;
use crate::install_webmail_runtime::webmail_health_url;
use crate::manifest::{self, ManifestSource};
use crate::model::{InstallerEvent, InstallerStatus, MailSystem};
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
    /// TCP port this process actually bound (may differ from a saved preference).
    pub bind_port: u16,
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
    state.log(format!("ÃƒÂ¢Ã¢â€šÂ¬Ã‚Âº {}", spec.description), "info");
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
                "{} terminÃƒÂ³ con cÃƒÂ³digo {}",
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

pub use crate::install_server::install;

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
        if matches!(mail, MailSystem::Thunderbird) {
            // Desktop client only: never claim IMAP/SMTP backend success (issue #9).
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
                    "Verificando la versiÃƒÂ³n instalada",
                    "testing",
                    96,
                ),
            )
            .await?;
            {
                let mut status = state.status.write().await;
                status.mail_client_ready = true;
                status.mail_backend_ready = false;
                status.access_note = Some(
                    "Thunderbird is a desktop mail client only. No IMAP/SMTP server was provisioned on this host."
                        .into(),
                );
            }
        } else {
            let engine = state
                .status
                .read()
                .await
                .selected_server
                .ok_or_else(|| {
                    "Web server selection missing; install and verify the web server before mail"
                        .to_string()
                })?;
            provision_local_mail_backend(&state).await?;
            verify_imap_smtp_listeners().await?;
            install_webmail(&state, mail, engine).await?;
            state
                .progress("testing", 92, format!("Comprobando {}", mail.label()))
                .await;
            run_command(
                &state,
                command(
                    "systemctl",
                    vec!["is-active", "--quiet", "php-fpm"],
                    "Verificando PHP-FPM para webmail",
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
                        webmail_health_url(),
                    ],
                    "Comprobando la respuesta HTTP del webmail",
                    "testing",
                    97,
                ),
            )
            .await?;
            // Re-check IMAP/SMTP after webmail so success means real mail stack (issue #9).
            verify_imap_smtp_listeners().await?;
            {
                let mut status = state.status.write().await;
                status.mail_client_ready = true;
                status.mail_backend_ready = true;
                status.access_note = Some(
                    "Webmail served via PHP-FPM + reverse proxy. Local Postfix/Dovecot listeners verified on IMAP :143 and SMTP :587/:25."
                        .into(),
                );
            }
        }
        Ok::<_, String>(())
    }
    .await;
    finish(&state, result, mail.label(), false, true).await;
}

pub(crate) async fn finish(
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
                        "Servicio verificado en loopback. Comprueba acceso externo desde otra mÃƒÂ¡quina si el firewall estaba activo."
                            .into(),
                    );
                }
            }
            status.phase = "completed";
            status.progress = 100;
            if mail_flow && matches!(status.selected_mail, Some(MailSystem::Thunderbird)) {
                status.message = format!(
                    "{label} (desktop client) installed. No IMAP/SMTP backend was provisioned."
                );
            } else if mail_flow && status.mail_backend_ready {
                status.message =
                    format!("{label} webmail + local IMAP/SMTP backend verified successfully");
            } else if mail_flow && !status.mail_backend_ready {
                // Should not reach completed for webmail without backend; keep honest fallback.
                status.message = format!(
                    "{label} installed without a verified IMAP/SMTP backend (unexpected state)."
                );
            } else {
                status.message = format!("{label} se instalÃƒÂ³ y verificÃƒÂ³ correctamente");
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
            let rollback =
                install_journal::rollback_tracked_files().unwrap_or_else(|rollback_error| {
                    state.log(format!("Rollback journal error: {rollback_error}"), "error");
                    install_journal::RollbackReport {
                        restored: Vec::new(),
                        removed: Vec::new(),
                        skipped: vec![rollback_error],
                        kind: FailureKind::FailedPartial,
                    }
                });
            status.phase = "failed";
            status.error = Some(error.clone());
            status.message = install_journal::failure_message(rollback.kind).into();
            status.access_note = Some(format!(
                "failure_kind={:?}; restored={}; removed={}; skipped={}",
                rollback.kind,
                rollback.restored.len(),
                rollback.removed.len(),
                rollback.skipped.len()
            ));
            state.log(error, "error");
            state.log(
                format!(
                    "rollback kind={:?} restored={} removed={} skipped={}",
                    rollback.kind,
                    rollback.restored.len(),
                    rollback.removed.len(),
                    rollback.skipped.len()
                ),
                "info",
            );
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
