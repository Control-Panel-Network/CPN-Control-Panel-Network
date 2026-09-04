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
use std::sync::RwLock;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Command,
    sync::broadcast,
};

/// Matrix-friendly status snapshot path (readable even if HTTP workers stall).
pub const STATUS_SNAPSHOT_PATH: &str = "/tmp/cpn-installer-status.json";

pub struct AppState {
    pub status: RwLock<InstallerStatus>,
    pub events: broadcast::Sender<InstallerEvent>,
    pub token: String,
    /// HttpOnly cookie value (server-generated; never taken from the query string).
    pub session_id: String,
    /// TCP port this process actually bound (may differ from a saved preference).
    pub bind_port: u16,
    /// True when bound to 0.0.0.0 (--allow-remote).
    pub allow_remote: bool,
    /// Server-known Host/Origin authorities (never from the client Host header).
    pub allowed_hosts: Vec<String>,
    /// Set on SIGINT/SIGTERM so in-flight stages stop starting new work (issue #18).
    pub cancel_requested: std::sync::atomic::AtomicBool,
    /// Live child PIDs (process-group leaders) so cancel can reap them while running.
    pub active_child_pids: std::sync::Mutex<Vec<u32>>,
}

/// Best-effort JSON snapshot for docker-matrix / lab probes (never blocks install).
pub fn persist_status_snapshot(status: &InstallerStatus) {
    if let Ok(json) = serde_json::to_vec(status) {
        let _ = std::fs::write(STATUS_SNAPSHOT_PATH, json);
    }
}

impl AppState {
    pub fn cancel_requested(&self) -> bool {
        self.cancel_requested
            .load(std::sync::atomic::Ordering::SeqCst)
    }

    pub fn request_cancel(&self) {
        self.cancel_requested
            .store(true, std::sync::atomic::Ordering::SeqCst);
        // Best-effort: TERM any active process groups immediately (issue #18).
        #[cfg(unix)]
        {
            if let Ok(pids) = self.active_child_pids.lock() {
                for &pid in pids.iter() {
                    let _ = std::process::Command::new("kill")
                        .args(["-TERM", &format!("-{pid}")])
                        .status();
                }
            }
        }
    }

    fn register_child_pid(&self, pid: u32) {
        if let Ok(mut pids) = self.active_child_pids.lock() {
            if !pids.contains(&pid) {
                pids.push(pid);
            }
        }
    }

    fn unregister_child_pid(&self, pid: u32) {
        if let Ok(mut pids) = self.active_child_pids.lock() {
            pids.retain(|value| *value != pid);
        }
    }

    pub async fn progress(&self, phase: &'static str, progress: u8, message: impl Into<String>) {
        let snapshot = {
            let mut status = self.status.write().unwrap_or_else(|e| e.into_inner());
            status.phase = phase;
            status.progress = progress;
            status.message = message.into();
            status.error = None;
            status.clone()
        };
        persist_status_snapshot(&snapshot);
        let _ = self
            .events
            .send(InstallerEvent::Progress { status: snapshot });
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
    if state.cancel_requested() {
        return Err("Instalacion cancelada por el operador".into());
    }
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
    state.log(format!("> {}", spec.description), "info");
    let mut command = Command::new(spec.program);
    command
        .args(&spec.args)
        .env("LC_ALL", "C")
        .kill_on_drop(true)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    {
        // New process group so timeout/cancel can reap descendants (issue #18).
        // SAFETY: runs in the child after fork, before exec.
        unsafe {
            command.pre_exec(|| {
                if libc::setpgid(0, 0) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }
    let mut child = command
        .spawn()
        .map_err(|error| format!("No se pudo ejecutar {}: {error}", spec.program))?;
    let child_pid = child.id();
    if let Some(pid) = child_pid {
        state.register_child_pid(pid);
    }

    let description = spec.description;
    let pump = async {
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "No se pudo leer la salida del proceso".to_string())?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| "No se pudo leer el error del proceso".to_string())?;
        let mut out_lines = BufReader::new(stdout).lines();
        let mut err_lines = BufReader::new(stderr).lines();
        let (mut out_done, mut err_done, mut transaction) = (false, false, false);
        while !out_done || !err_done {
            if state.cancel_requested() {
                return Err("Instalacion cancelada por el operador".into());
            }
            tokio::select! {
                line = out_lines.next_line(), if !out_done => match line {
                    Ok(Some(line)) => {
                        if !line.trim().is_empty() { state.log(&line, "info"); }
                        if let Some(tracking) = spec.dnf {
                            process_dnf_line(state, tracking, &mut transaction, &line).await;
                        }
                    }
                    Ok(None) | Err(_) => out_done = true,
                },
                line = err_lines.next_line(), if !err_done => match line {
                    Ok(Some(line)) => {
                        if !line.trim().is_empty() { state.log(&line, "info"); }
                        if let Some(tracking) = spec.dnf {
                            process_dnf_line(state, tracking, &mut transaction, &line).await;
                        }
                    }
                    Ok(None) | Err(_) => err_done = true,
                },
                _ = tokio::time::sleep(std::time::Duration::from_millis(100)) => {}
            }
        }
        if state.cancel_requested() {
            return Err("Instalacion cancelada por el operador".into());
        }
        let exit = child.wait().await.map_err(|error| error.to_string())?;
        if !exit.success() {
            return Err(format!(
                "{description} terminó con código {}",
                exit.code().unwrap_or(-1)
            ));
        }
        Ok::<(), String>(())
    };

    let cancel_watch = async {
        loop {
            if state.cancel_requested() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    };

    let outcome = tokio::select! {
        result = tokio::time::timeout(std::time::Duration::from_secs(1800), pump) => {
            match result {
                Ok(inner) => inner,
                Err(_) => Err(format!("Tiempo de espera agotado en: {description}")),
            }
        }
        _ = cancel_watch => Err("Instalacion cancelada por el operador".into()),
    };

    if outcome.is_err() {
        #[cfg(unix)]
        {
            if let Some(pid) = child_pid {
                let _ = Command::new("kill")
                    .args(["-TERM", &format!("-{pid}")])
                    .kill_on_drop(true)
                    .status()
                    .await;
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                let _ = Command::new("kill")
                    .args(["-KILL", &format!("-{pid}")])
                    .kill_on_drop(true)
                    .status()
                    .await;
            }
        }
        // Child handle is dropped with the cancelled/timed-out pump (kill_on_drop).
    }
    if let Some(pid) = child_pid {
        state.unregister_child_pid(pid);
    }
    outcome?;
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

pub use crate::install_server::{install, install_with_database};

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
    run_command(state, php_install_command(&guest, label)).await?;
    if let Some(stream) = guest.php_module_stream() {
        let today = chrono_today_ymd();
        crate::php_lifecycle::assert_selected_runtime_ok(stream, &today)?;
    }
    // Refuse EOL runtimes even if a host already had an old php package (issue #4).
    let status = Command::new("bash")
        .args([
            "-c",
            "php -r 'exit(version_compare(PHP_VERSION,\"8.2.0\",\"<\")?1:0);'",
        ])
        .kill_on_drop(true)
        .status()
        .await
        .map_err(|error| format!("PHP version check failed: {error}"))?;
    if !status.success() {
        return Err(
            "Installed PHP is older than 8.2 (EOL). Enable php:8.2 or Remi remi-8.2 and retry."
                .into(),
        );
    }
    Ok(())
}

fn chrono_today_ymd() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs())
        .unwrap_or(0);
    // Approximate UTC date without chrono crate (good enough for EOL gate).
    // Days since 1970-01-01.
    let days = (secs / 86400) as i64;
    let mut y = 1970i32;
    let mut rem = days;
    loop {
        let leap = y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
        let diy = if leap { 366 } else { 365 };
        if rem < diy {
            break;
        }
        rem -= diy;
        y += 1;
    }
    let leap = y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
    let mdays = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut m = 1u32;
    for dim in mdays {
        if rem < dim {
            break;
        }
        rem -= dim;
        m += 1;
    }
    let d = (rem + 1) as u32;
    format!("{y:04}-{m:02}-{d:02}")
}

pub async fn install_mail(state: std::sync::Arc<AppState>, mail: MailSystem) {
    let result = async {
        let _run = install_journal::begin_install_run("mail")?;
        let report = tokio::task::spawn_blocking(|| install_journal::run_preflight(512))
            .await
            .map_err(|error| format!("preflight join failed: {error}"))??;
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
        if guest.is_windows() {
            return Err(crate::os_support::windows_linux_recipe_blocked_message(
                "Mail / webmail install",
            ));
        }
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
                    "Verificando la versión instalada",
                    "testing",
                    96,
                ),
            )
            .await?;
            {
                let mut status = state.status.write().unwrap_or_else(|e| e.into_inner());
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
                .unwrap_or_else(|e| e.into_inner())
                .selected_server
                .ok_or_else(|| {
                    "Web server selection missing; install and verify the web server before mail"
                        .to_string()
                })?;
            provision_local_mail_backend(&state).await?;
            verify_imap_smtp_listeners().await?;
            crate::install_mail_backend::verify_mail_roundtrip(&state).await?;
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
                let mut status = state.status.write().unwrap_or_else(|e| e.into_inner());
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
    let rollback = if result.is_err() {
        Some(
            install_journal::rollback_tracked_files().unwrap_or_else(|rollback_error| {
                state.log(format!("Rollback journal error: {rollback_error}"), "error");
                install_journal::RollbackReport {
                    restored: Vec::new(),
                    removed: Vec::new(),
                    skipped: vec![rollback_error],
                    kind: FailureKind::FailedPartial,
                }
            }),
        )
    } else {
        None
    };

    let (snapshot, manifest_error) = {
        let mut status = state.status.write().unwrap_or_else(|e| e.into_inner());
        let mut manifest_error: Option<String> = None;
        match &result {
            Ok(()) => {
                if mark_server_ready {
                    status.server_ready = true;
                    // external_ports_configured is set by open_service_ports success path.
                    if status.access_note.is_none() {
                        status.access_note = Some(
                            "Servicio verificado en loopback. Comprueba acceso externo desde otra máquina si el firewall estaba activo."
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
                    status.message = format!(
                        "{label} installed without a verified IMAP/SMTP backend (unexpected state)."
                    );
                } else {
                    status.message = format!("{label} se instaló y verificó correctamente");
                }
                install_journal::end_install_run();
                if let Err(error) = manifest::record_install(
                    env!("CARGO_PKG_VERSION"),
                    &format!("v{}", env!("CARGO_PKG_VERSION")),
                    ManifestSource::Local,
                    status.selected_server,
                    status.selected_mail,
                ) {
                    manifest_error = Some(error);
                }
            }
            Err(error) => {
                let report = rollback.as_ref().expect("rollback set for Err");
                status.phase = "failed";
                status.error = Some(error.clone());
                status.message = install_journal::failure_message(report.kind);
                status.access_note = Some(format!(
                    "failure_kind={:?}; restored={}; removed={}; skipped={}",
                    report.kind,
                    report.restored.len(),
                    report.removed.len(),
                    report.skipped.len()
                ));
                install_journal::end_install_run();
            }
        }
        (status.clone(), manifest_error)
    };
    persist_status_snapshot(&snapshot);

    if let Some(error) = manifest_error {
        state.log(
            format!("Warning: could not write install manifest: {error}"),
            "error",
        );
    }

    match result {
        Ok(()) => {
            let _ = state
                .events
                .send(InstallerEvent::Completed { status: snapshot });
        }
        Err(error) => {
            if let Some(report) = rollback {
                state.log(error, "error");
                state.log(
                    format!(
                        "rollback kind={:?} restored={} removed={} skipped={}",
                        report.kind,
                        report.restored.len(),
                        report.removed.len(),
                        report.skipped.len()
                    ),
                    "info",
                );
            }
            let _ = state
                .events
                .send(InstallerEvent::Error { status: snapshot });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{AppState, fraction};
    use crate::os_support::require_installable_guest;
    use std::sync::RwLock;
    use std::sync::atomic::AtomicBool;
    use tokio::sync::broadcast;

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

    #[test]
    fn cancel_flag_blocks_new_commands() {
        let (events, _) = broadcast::channel(8);
        let state = AppState {
            status: RwLock::new(Default::default()),
            events,
            token: "t".into(),
            session_id: "s".into(),
            bind_port: 2087,
            allow_remote: false,
            allowed_hosts: crate::http_helpers::build_allowed_hosts(2087, &[]),
            cancel_requested: AtomicBool::new(false),
            active_child_pids: std::sync::Mutex::new(Vec::new()),
        };
        assert!(!state.cancel_requested());
        state.request_cancel();
        assert!(state.cancel_requested());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn slow_child_dies_when_cancel_requested() {
        use crate::install_recipes::command;
        use std::sync::Arc;
        use tokio::process::Command;
        use tokio::time::{Duration, sleep};

        let (events, _) = broadcast::channel(8);
        let state = Arc::new(AppState {
            status: RwLock::new(Default::default()),
            events,
            token: "t".into(),
            session_id: "s".into(),
            bind_port: 2087,
            allow_remote: false,
            allowed_hosts: crate::http_helpers::build_allowed_hosts(2087, &[]),
            cancel_requested: AtomicBool::new(false),
            active_child_pids: std::sync::Mutex::new(Vec::new()),
        });
        let worker = state.clone();
        let join = tokio::spawn(async move {
            super::run_command(
                &worker,
                command(
                    "bash",
                    vec!["-c", "sleep 120 & sleep 120 & wait"],
                    "slow cancel probe",
                    "testing",
                    1,
                ),
            )
            .await
        });
        sleep(Duration::from_millis(300)).await;
        let pids: Vec<u32> = state
            .active_child_pids
            .lock()
            .map(|guard| guard.clone())
            .unwrap_or_default();
        assert!(
            !pids.is_empty(),
            "expected an active child pid before cancel"
        );
        let pgid = pids[0];
        state.request_cancel();
        let result = tokio::time::timeout(Duration::from_secs(8), join)
            .await
            .expect("join timed out")
            .expect("join failed");
        assert!(
            result
                .as_ref()
                .err()
                .map(|msg| msg.contains("cancelada"))
                .unwrap_or(false),
            "expected cancel error, got {result:?}"
        );
        sleep(Duration::from_millis(400)).await;
        let still = Command::new("bash")
            .args(["-c", &format!("ps -o pid= -g {pgid} | grep -q .")])
            .status()
            .await
            .map(|status| status.success())
            .unwrap_or(false);
        assert!(
            !still,
            "process group {pgid} still has members after AppState::request_cancel"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn slow_child_process_group_dies_on_timeout_kill() {
        use tokio::process::Command;
        use tokio::time::{Duration, timeout};

        // Mirror run_command process-group + kill -TERM -<pid> behaviour (issue #18).
        let mut command = Command::new("bash");
        command
            .args(["-c", "sleep 120 & sleep 120 & wait"])
            .kill_on_drop(true)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        unsafe {
            command.pre_exec(|| {
                if libc::setpgid(0, 0) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let mut child = command.spawn().expect("spawn slow group");
        let pid = child.id().expect("pid");
        let pump = async {
            let _ = child.wait().await;
        };
        let timed_out = timeout(Duration::from_millis(400), pump).await.is_err();
        assert!(timed_out, "expected timeout before sleep finishes");
        let _ = Command::new("kill")
            .args(["-TERM", &format!("-{pid}")])
            .kill_on_drop(true)
            .status()
            .await;
        let _ = child.kill().await;
        let _ = child.wait().await;
        // Process group leader and descendants must be gone.
        let still = Command::new("bash")
            .args(["-c", &format!("ps -o pid= -g {pid} | grep -q .")])
            .status()
            .await
            .map(|s| s.success())
            .unwrap_or(false);
        assert!(
            !still,
            "process group {pid} still has members after TERM+kill"
        );
    }
}
