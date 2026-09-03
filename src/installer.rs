use crate::model::{InstallerEvent, InstallerStatus, MailSystem, ServerEngine};
use rand::{Rng, distr::Alphanumeric};
use std::{os::unix::fs::symlink, path::Path, process::Stdio};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, BufReader},
    process::Command,
    sync::{RwLock, broadcast},
};

fn almalinux_major() -> Result<u32, String> {
    let release = std::fs::read_to_string("/etc/os-release")
        .map_err(|_| "No se pudo identificar el sistema operativo".to_string())?;
    let is_almalinux = release
        .lines()
        .any(|line| line == "ID=almalinux" || line == "ID=\"almalinux\"");
    if !is_almalinux {
        return Err("Esta versión solo admite AlmaLinux 9 o AlmaLinux 10 \
             (bare metal, VM, o contenedor Docker/Podman privilegiado con systemd; \
             ver to-do/DOCKER-INSTALL.md)"
            .into());
    }
    let version = release
        .lines()
        .find_map(|line| line.strip_prefix("VERSION_ID="))
        .map(|value| value.trim_matches('"'))
        .ok_or_else(|| "No se pudo leer VERSION_ID en /etc/os-release".to_string())?;
    let major = version
        .split('.')
        .next()
        .and_then(|part| part.parse::<u32>().ok())
        .ok_or_else(|| format!("VERSION_ID no válida: {version}"))?;
    if major != 9 && major != 10 {
        return Err(format!(
            "AlmaLinux {major} aún no está soportado (se admite 9 y 10, \
             incluyendo contenedores basados en esas versiones)"
        ));
    }
    Ok(major)
}

fn verify_almalinux() -> Result<(), String> {
    almalinux_major().map(|_| ())
}

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

#[derive(Clone, Copy)]
struct DnfProgress {
    download_start: u8,
    download_end: u8,
    install_start: u8,
    install_end: u8,
    label: &'static str,
}

struct CommandSpec {
    program: &'static str,
    args: Vec<&'static str>,
    description: &'static str,
    phase: &'static str,
    progress: u8,
    dnf: Option<DnfProgress>,
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

async fn run_command(state: &AppState, spec: CommandSpec) -> Result<(), String> {
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

fn ephemeral_download_path(filename: &str) -> Result<String, String> {
    let suffix: String = rand::rng()
        .sample_iter(&Alphanumeric)
        .take(16)
        .map(char::from)
        .collect();
    let dir = format!("/var/tmp/cpn-dl-{suffix}");
    std::fs::create_dir_all(&dir)
        .map_err(|error| format!("No se pudo crear el directorio temporal: {error}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700));
    }
    Ok(format!("{dir}/{filename}"))
}

async fn download(
    state: &AppState,
    url: &str,
    destination: &str,
    label: &str,
    start: u8,
    end: u8,
) -> Result<(), String> {
    state
        .progress("downloading", start, format!("Descargando {label}"))
        .await;
    let mut child = Command::new("curl")
        .args([
            "--fail",
            "--location",
            "--progress-bar",
            "--output",
            destination,
            url,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("No se pudo descargar {label}: {error}"))?;
    let mut stderr = child
        .stderr
        .take()
        .ok_or("No se pudo leer el progreso de la descarga")?;
    let mut buffer = [0_u8; 1024];
    let mut pending = String::new();
    loop {
        let read = stderr
            .read(&mut buffer)
            .await
            .map_err(|error| error.to_string())?;
        if read == 0 {
            break;
        }
        pending.push_str(&String::from_utf8_lossy(&buffer[..read]));
        let parts = pending
            .split(['\r', '\n'])
            .map(str::to_owned)
            .collect::<Vec<_>>();
        pending = parts.last().cloned().unwrap_or_default();
        for part in parts.iter().take(parts.len().saturating_sub(1)) {
            if let Some(percent) = part
                .split_whitespace()
                .find_map(|token| token.strip_suffix('%')?.parse::<f32>().ok())
            {
                let progress = start + ((end - start) as f32 * (percent / 100.0)).round() as u8;
                state
                    .progress(
                        "downloading",
                        progress.min(end),
                        format!("Descargando {label}"),
                    )
                    .await;
            }
        }
    }
    let exit = child.wait().await.map_err(|error| error.to_string())?;
    if !exit.success() {
        return Err(format!("La descarga de {label} no pudo completarse"));
    }
    state
        .progress("downloading", end, format!("{label} descargado"))
        .await;
    Ok(())
}

fn command(
    program: &'static str,
    args: Vec<&'static str>,
    description: &'static str,
    phase: &'static str,
    progress: u8,
) -> CommandSpec {
    CommandSpec {
        program,
        args,
        description,
        phase,
        progress,
        dnf: None,
    }
}

fn dnf(args: Vec<&'static str>, description: &'static str, tracking: DnfProgress) -> CommandSpec {
    CommandSpec {
        program: "dnf",
        args,
        description,
        phase: "downloading",
        progress: tracking.download_start,
        dnf: Some(tracking),
    }
}

fn server_recipes(server: ServerEngine) -> Vec<CommandSpec> {
    match server {
        ServerEngine::Nginx => vec![
            dnf(
                vec!["install", "-y", "nginx"],
                "Instalando Nginx",
                DnfProgress {
                    download_start: 2,
                    download_end: 48,
                    install_start: 50,
                    install_end: 82,
                    label: "Nginx",
                },
            ),
            command(
                "systemctl",
                vec!["enable", "--now", "nginx"],
                "Activando Nginx",
                "installing",
                84,
            ),
        ],
        ServerEngine::Caddy => vec![
            dnf(
                vec!["install", "-y", "caddy"],
                "Instalando Caddy",
                DnfProgress {
                    download_start: 5,
                    download_end: 48,
                    install_start: 50,
                    install_end: 82,
                    label: "Caddy",
                },
            ),
            command(
                "systemctl",
                vec!["enable", "--now", "caddy"],
                "Activando Caddy",
                "installing",
                84,
            ),
        ],
        ServerEngine::Openlitespeed => vec![
            command(
                "bash",
                vec!["-c", "curl -fsSL https://repo.litespeed.sh | bash"],
                "Preparando el repositorio de OpenLiteSpeed",
                "downloading",
                2,
            ),
            dnf(
                vec!["install", "-y", "openlitespeed"],
                "Instalando OpenLiteSpeed",
                DnfProgress {
                    download_start: 5,
                    download_end: 48,
                    install_start: 50,
                    install_end: 78,
                    label: "OpenLiteSpeed",
                },
            ),
        ],
    }
}

fn prepare_caddy_repository() -> Result<(), String> {
    let major = almalinux_major()?;
    let repository = format!(
        "[copr:copr.fedorainfracloud.org:group_caddy:caddy]\nname=Caddy official COPR\nbaseurl=https://download.copr.fedorainfracloud.org/results/@caddy/caddy/epel-{major}-$basearch/\ntype=rpm-md\nskip_if_unavailable=False\ngpgcheck=1\ngpgkey=https://download.copr.fedorainfracloud.org/results/@caddy/caddy/pubkey.gpg\nrepo_gpgcheck=0\nenabled=1\n"
    );
    std::fs::write("/etc/yum.repos.d/caddy.repo", repository)
        .map_err(|error| format!("No se pudo configurar el repositorio de Caddy: {error}"))
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
        verify_almalinux()?;
        if matches!(server, ServerEngine::Caddy) {
            prepare_caddy_repository()?;
        }
        for item in server_recipes(server) {
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

const PHP_PACKAGES: &[&str] = &[
    "install",
    "-y",
    "php-cli",
    "php-mbstring",
    "php-intl",
    "php-xml",
    "php-pdo",
    "php-process",
    "php-gd",
    "php-opcache",
    "php-pecl-zip",
    "unzip",
    "tar",
];

async fn install_php_runtime(state: &AppState, label: &'static str) -> Result<(), String> {
    let major = almalinux_major()?;
    // AlmaLinux 9 uses modular PHP streams. AlmaLinux 10 ships PHP from AppStream
    // without requiring `dnf module enable php:8.1`.
    if major == 9 {
        run_command(
            state,
            command(
                "dnf",
                vec!["module", "enable", "-y", "php:8.1"],
                "Preparando PHP 8.1",
                "downloading",
                38,
            ),
        )
        .await?;
    } else {
        state
            .progress(
                "downloading",
                38,
                "Usando PHP de AppStream en AlmaLinux 10",
            )
            .await;
    }
    run_command(
        state,
        dnf(
            PHP_PACKAGES.to_vec(),
            "Instalando PHP y sus extensiones",
            DnfProgress {
                download_start: 40,
                download_end: 58,
                install_start: 60,
                install_end: 76,
                label,
            },
        ),
    )
    .await
}

fn reset_current_link(target: &Path) -> Result<(), String> {
    let current = Path::new("/opt/cpn-webmail/current");
    if current.symlink_metadata().is_ok() {
        std::fs::remove_file(current).map_err(|error| error.to_string())?;
    }
    symlink(target, current).map_err(|error| format!("No se pudo activar el webmail: {error}"))
}

async fn configure_webmail_service(state: &AppState, root: &'static str) -> Result<(), String> {
    let _ = Command::new("systemctl")
        .args(["stop", "cpn-webmail"])
        .status()
        .await;
    let user_exists = Command::new("id")
        .args(["-u", "cpn-webmail"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .is_ok_and(|status| status.success());
    if !user_exists {
        run_command(
            state,
            command(
                "useradd",
                vec![
                    "--system",
                    "--home-dir",
                    "/opt/cpn-webmail",
                    "--shell",
                    "/sbin/nologin",
                    "cpn-webmail",
                ],
                "Creando el usuario aislado del webmail",
                "installing",
                83,
            ),
        )
        .await?;
    }
    reset_current_link(Path::new(root))?;
    const UNIT: &str = "[Unit]\nDescription=CPN Webmail\nAfter=network.target\n\n[Service]\nType=simple\nUser=cpn-webmail\nGroup=cpn-webmail\nWorkingDirectory=/opt/cpn-webmail/current\nExecStart=/usr/bin/php -S 127.0.0.1:8888 -t /opt/cpn-webmail/current\nRestart=on-failure\nPrivateTmp=true\nNoNewPrivileges=true\n\n[Install]\nWantedBy=multi-user.target\n";
    std::fs::write("/etc/systemd/system/cpn-webmail.service", UNIT)
        .map_err(|error| error.to_string())?;
    run_command(
        state,
        command(
            "chown",
            vec!["-R", "cpn-webmail:cpn-webmail", "/opt/cpn-webmail"],
            "Ajustando permisos del webmail",
            "installing",
            84,
        ),
    )
    .await?;
    run_command(
        state,
        command(
            "systemctl",
            vec!["daemon-reload"],
            "Registrando el servicio de correo",
            "installing",
            86,
        ),
    )
    .await?;
    run_command(
        state,
        command(
            "systemctl",
            vec!["enable", "--now", "cpn-webmail"],
            "Activando el sistema de correo",
            "installing",
            88,
        ),
    )
    .await
}

async fn extract_archive(
    state: &AppState,
    program: &str,
    args: &[&str],
    description: &str,
    progress: u8,
) -> Result<(), String> {
    state
        .progress("installing", progress, description.to_string())
        .await;
    let status = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .map_err(|error| format!("{description}: {error}"))?;
    if !status.success() {
        return Err(format!("{description} falló"));
    }
    Ok(())
}

async fn install_webmail(state: &AppState, mail: MailSystem) -> Result<(), String> {
    std::fs::create_dir_all("/opt/cpn-webmail").map_err(|error| error.to_string())?;
    match mail {
        MailSystem::Snappymail => {
            std::fs::create_dir_all("/opt/cpn-webmail/snappymail")
                .map_err(|error| error.to_string())?;
            let archive = ephemeral_download_path("snappymail.tar.gz")?;
            download(
                state,
                "https://github.com/the-djmaze/snappymail/releases/download/v2.38.2/snappymail-2.38.2.tar.gz",
                &archive,
                "SnappyMail",
                2,
                36,
            )
            .await?;
            install_php_runtime(state, "PHP para SnappyMail").await?;
            extract_archive(
                state,
                "tar",
                &["xzf", &archive, "-C", "/opt/cpn-webmail/snappymail"],
                "Extrayendo SnappyMail",
                80,
            )
            .await?;
            let _ = std::fs::remove_file(&archive);
            configure_webmail_service(state, "/opt/cpn-webmail/snappymail").await?;
        }
        MailSystem::Rainloop => {
            state.log(
                "RainLoop legacy está marcado como opción heredada; prefer SnappyMail o Roundcube",
                "info",
            );
            std::fs::create_dir_all("/opt/cpn-webmail/rainloop")
                .map_err(|error| error.to_string())?;
            let archive = ephemeral_download_path("rainloop.zip")?;
            download(
                state,
                "https://github.com/RainLoop/rainloop-webmail/releases/download/v1.17.0/rainloop-legacy-1.17.0.zip",
                &archive,
                "RainLoop",
                2,
                36,
            )
            .await?;
            install_php_runtime(state, "PHP para RainLoop").await?;
            extract_archive(
                state,
                "unzip",
                &["-o", &archive, "-d", "/opt/cpn-webmail/rainloop"],
                "Extrayendo RainLoop",
                80,
            )
            .await?;
            let _ = std::fs::remove_file(&archive);
            configure_webmail_service(state, "/opt/cpn-webmail/rainloop").await?;
        }
        MailSystem::Roundcube => {
            std::fs::create_dir_all("/opt/cpn-webmail/roundcube")
                .map_err(|error| error.to_string())?;
            let archive = ephemeral_download_path("roundcube.tar.gz")?;
            download(
                state,
                "https://github.com/roundcube/roundcubemail/releases/download/1.7.3/roundcubemail-1.7.3-complete.tar.gz",
                &archive,
                "Roundcube",
                2,
                36,
            )
            .await?;
            install_php_runtime(state, "PHP para Roundcube").await?;
            extract_archive(
                state,
                "tar",
                &[
                    "xzf",
                    &archive,
                    "-C",
                    "/opt/cpn-webmail/roundcube",
                    "--strip-components=1",
                ],
                "Extrayendo Roundcube",
                80,
            )
            .await?;
            let _ = std::fs::remove_file(&archive);
            let key: String = rand::rng()
                .sample_iter(&Alphanumeric)
                .take(24)
                .map(char::from)
                .collect();
            let config = format!(
                "<?php\n$config = [];\n$config['db_dsnw'] = 'sqlite:////opt/cpn-webmail/roundcube/db.sqlite?mode=0600';\n$config['imap_host'] = 'localhost:143';\n$config['smtp_host'] = 'localhost:587';\n$config['smtp_user'] = '%u';\n$config['smtp_pass'] = '%p';\n$config['product_name'] = 'Roundcube Webmail';\n$config['des_key'] = '{key}';\n$config['plugins'] = ['archive', 'zipdownload'];\n$config['skin'] = 'elastic';\n"
            );
            std::fs::write("/opt/cpn-webmail/roundcube/config/config.inc.php", config)
                .map_err(|error| error.to_string())?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let db = Path::new("/opt/cpn-webmail/roundcube/db.sqlite");
                if !db.exists() {
                    let _ = std::fs::File::create(db);
                }
                let _ = std::fs::set_permissions(db, std::fs::Permissions::from_mode(0o600));
                let _ = std::fs::set_permissions(
                    "/opt/cpn-webmail/roundcube/config/config.inc.php",
                    std::fs::Permissions::from_mode(0o640),
                );
            }
            configure_webmail_service(state, "/opt/cpn-webmail/roundcube/public_html").await?;
        }
        MailSystem::Thunderbird => unreachable!(),
    }
    Ok(())
}

pub async fn install_mail(state: std::sync::Arc<AppState>, mail: MailSystem) {
    let result = async {
        verify_almalinux()?;
        if matches!(mail, MailSystem::Thunderbird) {
            run_command(
                &state,
                dnf(
                    vec!["install", "-y", "thunderbird"],
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
    use super::{almalinux_major, fraction};

    #[test]
    fn parses_dnf_download_and_transaction_fractions() {
        assert_eq!(fraction("(3/8): package.rpm"), Some((3, 8)));
        assert_eq!(fraction("Installing : package 5/7"), Some((5, 7)));
        assert_eq!(fraction("No package progress here"), None);
    }

    #[test]
    fn almalinux_major_is_callable() {
        let _ = almalinux_major();
    }
}
