use crate::install_journal::{self, FailureKind};
use crate::install_mail_backend::{provision_local_mail_backend, verify_imap_smtp_listeners};
use crate::install_recipes::{
    CommandSpec, DnfProgress, apt_update_command, command, php_install_command,
    php_module_enable_command, pkg_install,
};
use crate::install_webmail::install_webmail;
use crate::install_webmail_health::verify_webmail_http_surface;
use crate::manifest::{self, ManifestSource};
use crate::model::{InstallerEvent, InstallerStatus, MailSystem};
use crate::os_support::require_installable_guest;
use std::{
    fs::{self, OpenOptions},
    io::Write,
    process::Stdio,
    sync::{Mutex, RwLock},
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Command,
    sync::broadcast,
};

/// Matrix-friendly status snapshot path (readable even if HTTP workers stall).
pub const STATUS_SNAPSHOT_PATH: &str = "/tmp/cpn-installer-status.json";
/// Persistent installer transcript. This is deliberately separate from the
/// rollback journal: it contains operator-facing command output and progress.
pub const INSTALLATION_LOG_FILE: &str = "installation.log";

static INSTALLATION_LOG_LOCK: Mutex<()> = Mutex::new(());

pub fn installation_log_path() -> std::path::PathBuf {
    crate::paths::join_data(INSTALLATION_LOG_FILE)
}

fn append_installation_log(level: &str, line: &str) {
    let Ok(_guard) = INSTALLATION_LOG_LOCK.lock() else {
        return;
    };
    let path = installation_log_path();
    let Some(parent) = path.parent() else {
        return;
    };
    if fs::create_dir_all(parent).is_err() {
        return;
    }
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "[{timestamp}] [{level}] {line}");
        let _ = file.flush();
    }
}

/// Package-manager output policy. New runs always use Full so support reports
/// have a complete transcript; Minimal remains only for serialized compatibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InstallLogDetail {
    #[default]
    Full,
    Minimal,
}

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
    /// Always Full for new runs (kept in state for compatibility).
    pub install_log_detail: std::sync::Mutex<InstallLogDetail>,
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
                    // ESRCH is harmless: the process may have exited already.
                    unsafe {
                        libc::kill(-(pid as i32), libc::SIGTERM);
                    }
                }
            }
        }
    }

    pub fn set_install_log_detail(&self, detail: InstallLogDetail) {
        if let Ok(mut guard) = self.install_log_detail.lock() {
            *guard = detail;
        }
    }

    pub fn install_log_detail(&self) -> InstallLogDetail {
        self.install_log_detail
            .lock()
            .map(|guard| *guard)
            .unwrap_or(InstallLogDetail::Full)
    }

    pub fn install_log_is_full(&self) -> bool {
        matches!(self.install_log_detail(), InstallLogDetail::Full)
    }

    fn register_child_pid(&self, pid: u32) {
        if let Ok(mut pids) = self.active_child_pids.lock()
            && !pids.contains(&pid)
        {
            pids.push(pid);
        }
    }

    fn unregister_child_pid(&self, pid: u32) {
        if let Ok(mut pids) = self.active_child_pids.lock() {
            pids.retain(|value| *value != pid);
        }
    }

    pub async fn progress(&self, phase: &'static str, progress: u8, message: impl Into<String>) {
        let message = message.into();
        let snapshot = {
            let mut status = self.status.write().unwrap_or_else(|e| e.into_inner());
            status.phase = phase;
            status.progress = progress;
            status.message = message;
            status.error = None;
            status.clone()
        };
        append_installation_log(
            "progress",
            &format!(
                "phase={} progress={} message={}",
                snapshot.phase, snapshot.progress, snapshot.message
            ),
        );
        persist_status_snapshot(&snapshot);
        let _ = self
            .events
            .send(InstallerEvent::Progress { status: snapshot });
    }

    pub fn log(&self, line: impl Into<String>, level: &'static str) {
        let line = line.into();
        append_installation_log(level, &line);
        let _ = self.events.send(InstallerEvent::Log { line, level });
    }

    /// Always write to `installation.log`; new runs also mirror all output.
    pub fn log_command_output(&self, line: impl Into<String>, level: &'static str) {
        let line = line.into();
        append_installation_log(level, &line);
        if level == "error" || self.install_log_is_full() {
            let _ = self.events.send(InstallerEvent::Log { line, level });
        }
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
                format!("Installing {}", tracking.label),
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
                "Installing",
            )
        } else {
            (
                "downloading",
                tracking.download_start,
                tracking.download_end,
                "Downloading",
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
        return Err("Installation cancelled by the operator".into());
    }
    if let Some(tracking) = spec.dnf {
        state
            .progress(
                "downloading",
                tracking.download_start,
                format!("Downloading {}", tracking.label),
            )
            .await;
    } else {
        state
            .progress(spec.phase, spec.progress, spec.description)
            .await;
    }
    state.log(
        format!(
            "> {}: {} {}",
            spec.description,
            spec.program,
            spec.args.join(" ")
        ),
        "info",
    );
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
        .map_err(|error| format!("Failed to run {}: {error}", spec.program))?;
    let child_pid = child.id();
    if let Some(pid) = child_pid {
        state.register_child_pid(pid);
    }

    let description = spec.description;
    let pump = async {
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "Failed to read process stdout".to_string())?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| "Failed to read process stderr".to_string())?;
        let mut out_lines = BufReader::new(stdout).lines();
        let mut err_lines = BufReader::new(stderr).lines();
        let (mut out_done, mut err_done, mut transaction) = (false, false, false);
        let mut heartbeat = tokio::time::interval(std::time::Duration::from_secs(20));
        heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        heartbeat.tick().await;
        let mut diagnostic = std::collections::VecDeque::with_capacity(12);
        while !out_done || !err_done {
            if state.cancel_requested() {
                return Err("Installation cancelled by the operator".into());
            }
            tokio::select! {
                line = out_lines.next_line(), if !out_done => match line {
                    Ok(Some(line)) => {
                        if diagnostic.len() == 12 { diagnostic.pop_front(); }
                        diagnostic.push_back(line.clone());
                        if !line.trim().is_empty() {
                            state.log_command_output(&line, "info");
                        }
                        if let Some(tracking) = spec.dnf {
                            process_dnf_line(state, tracking, &mut transaction, &line).await;
                        }
                    }
                    Ok(None) | Err(_) => out_done = true,
                },
                line = err_lines.next_line(), if !err_done => match line {
                    Ok(Some(line)) => {
                        if diagnostic.len() == 12 { diagnostic.pop_front(); }
                        diagnostic.push_back(line.clone());
                        if !line.trim().is_empty() {
                            state.log_command_output(&line, "info");
                        }
                        if let Some(tracking) = spec.dnf {
                            process_dnf_line(state, tracking, &mut transaction, &line).await;
                        }
                    }
                    Ok(None) | Err(_) => err_done = true,
                },
                _ = heartbeat.tick() => {
                    let (phase, progress) = {
                        let status = state.status.read().unwrap_or_else(|e| e.into_inner());
                        (status.phase, status.progress)
                    };
                    // Heartbeats must look healthy, not failed: percent may stay low while dnf is quiet.
                    let wait_msg = format!(
                        "{description} still running (normal): waiting for package-manager output; this can take several minutes"
                    );
                    state.log(&wait_msg, "info");
                    state.progress(phase, progress, wait_msg).await;
                }
            }
        }
        if state.cancel_requested() {
            return Err("Installation cancelled by the operator".into());
        }
        let exit = child.wait().await.map_err(|error| error.to_string())?;
        if !exit.success() {
            return Err(format!(
                "{description} exited with code {}\n{}",
                exit.code().unwrap_or(-1),
                diagnostic.into_iter().collect::<Vec<_>>().join("\n")
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
                Err(_) => Err(format!("Timed out waiting for: {description}")),
            }
        }
        _ = cancel_watch => Err("Installation cancelled by the operator".into()),
    };

    if outcome.is_err() && child.try_wait().ok().flatten().is_none() {
        #[cfg(unix)]
        {
            if let Some(pid) = child_pid {
                unsafe {
                    libc::kill(-(pid as i32), libc::SIGTERM);
                }
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                unsafe {
                    libc::kill(-(pid as i32), libc::SIGKILL);
                }
            }
        }
        // Child handle is dropped with the cancelled/timed-out pump (kill_on_drop).
    }
    if let Some(pid) = child_pid {
        state.unregister_child_pid(pid);
    }
    if let Err(error) = outcome {
        state.log(
            format!("✗ Verification failed: {description}: {error}"),
            "error",
        );
        return Err(error);
    }
    state.log(format!("✓ Verified: {description}"), "success");
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
                "Guest OS detected: {} ({})",
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
                    "Installing Thunderbird",
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
                .progress("testing", 92, "Checking Thunderbird")
                .await;
            run_command(
                &state,
                command(
                    "thunderbird",
                    vec!["--version"],
                    "Verifying the installed version",
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
                .progress("testing", 92, format!("Checking {}", mail.label()))
                .await;
            run_command(
                &state,
                command(
                    "systemctl",
                    vec!["is-active", "--quiet", "php-fpm"],
                    "Verifying PHP-FPM for webmail",
                    "testing",
                    94,
                ),
            )
            .await?;
            state
                .progress(
                    "testing",
                    97,
                    format!("Validating HTTP UI and data/temp/logs denial ({})", mail.label()),
                )
                .await;
            // Keep blocking work off the Actix worker (curl + body checks).
            let mail_key = mail.label().to_string();
            tokio::task::spawn_blocking(move || verify_webmail_http_surface(&mail_key))
                .await
                .map_err(|error| format!("webmail health join failed: {error}"))??;
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
                            "Service verified on loopback. Check external access from another host if a firewall was active."
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
                    status.message = format!("{label} installed and verified successfully");
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
            crate::motd::ensure_motd_installed();
            let allow_remote = state.allow_remote || crate::panel_service::allow_remote_requested();
            crate::panel_service::ensure_panel_service_best_effort(
                crate::panel_service::PanelServiceMode::EnablePreferRunning,
                allow_remote,
            );
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
    use super::{AppState, InstallLogDetail, fraction};
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
            install_log_detail: std::sync::Mutex::new(InstallLogDetail::Full),
        };
        assert!(!state.cancel_requested());
        state.request_cancel();
        assert!(state.cancel_requested());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn failed_command_includes_stderr_and_unregisters_child() {
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
            install_log_detail: std::sync::Mutex::new(InstallLogDetail::Full),
        };
        let error = super::run_command(
            &state,
            crate::install_recipes::command(
                "sh",
                vec!["-c", "echo 'missing dependency libgd.so.103' >&2; exit 1"],
                "dependency probe",
                "configuring",
                0,
            ),
        )
        .await
        .unwrap_err();
        assert!(error.contains("missing dependency libgd.so.103"), "{error}");
        assert!(error.contains("code 1"), "{error}");
        assert!(state.active_child_pids.lock().unwrap().is_empty());
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
            install_log_detail: std::sync::Mutex::new(InstallLogDetail::Full),
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
                .map(|msg| {
                    let lower = msg.to_ascii_lowercase();
                    lower.contains("cancelled") || lower.contains("cancelada")
                })
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
