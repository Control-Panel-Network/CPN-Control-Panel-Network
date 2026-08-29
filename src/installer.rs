use crate::{
    model::{
        InstallerEvent, InstallerPhase, InstallerStatus, MailSystem, ServerEngine, SetupStage,
    },
    oauth::{CloudflareAuthorization, PendingOAuth},
    panel, secrets,
};
use rand::{Rng, distr::Alphanumeric};
use sha2::{Digest, Sha256};
use std::{
    os::unix::fs::{OpenOptionsExt, PermissionsExt, symlink},
    path::Path,
    process::Stdio,
    sync::atomic::AtomicBool,
    time::Duration,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    process::Command,
    sync::{RwLock, broadcast},
};

fn verify_release(release: &str) -> Result<(), String> {
    if !release
        .lines()
        .any(|line| line == "ID=almalinux" || line == "ID=\"almalinux\"")
    {
        return Err("Esta primera versión solo admite AlmaLinux".into());
    }
    let version = release
        .lines()
        .find_map(|line| {
            line.strip_prefix("VERSION_ID=")
                .map(|value| value.trim_matches('"'))
        })
        .ok_or("No se pudo determinar la versión de AlmaLinux")?;
    if version.split('.').next() != Some("9") {
        return Err(format!(
            "Esta versión requiere AlmaLinux 9; se detectó {version}"
        ));
    }
    Ok(())
}

pub fn verify_almalinux() -> Result<(), String> {
    let release = std::fs::read_to_string("/etc/os-release")
        .map_err(|_| "No se pudo identificar el sistema operativo".to_string())?;
    verify_release(&release)
}

pub struct AppState {
    pub status: RwLock<InstallerStatus>,
    pub events: broadcast::Sender<InstallerEvent>,
    pub token: String,
    pub bootstrap_used: AtomicBool,
    pub pending_oauth: RwLock<Option<PendingOAuth>>,
    pub cloudflare: RwLock<Option<CloudflareAuthorization>>,
}

impl AppState {
    pub async fn progress(&self, phase: InstallerPhase, progress: u8, message: impl Into<String>) {
        let mut status = self.status.write().await;
        status.phase = phase;
        status.progress = progress;
        status.message = message.into();
        status.error = None;
        status.failed_phase = None;
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
    args: Vec<String>,
    description: &'static str,
    phase: InstallerPhase,
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
                InstallerPhase::Installing,
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
                InstallerPhase::Installing,
                tracking.install_start,
                tracking.install_end,
                "Instalando",
            )
        } else {
            (
                InstallerPhase::Downloading,
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
                InstallerPhase::Downloading,
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
        .kill_on_drop(true)
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
                InstallerPhase::Installing,
                tracking.install_end,
                format!("{} instalado", tracking.label),
            )
            .await;
    }
    Ok(())
}

async fn download_for_phase(
    state: &AppState,
    url: &'static str,
    expected_sha256: &'static str,
    label: &'static str,
    start: u8,
    end: u8,
    phase: InstallerPhase,
) -> Result<tempfile::TempPath, String> {
    state
        .progress(phase, start, format!("Descargando {label}"))
        .await;
    let temporary = tempfile::Builder::new()
        .prefix("cpn-download-")
        .tempfile()
        .map_err(|error| format!("No se pudo crear el archivo temporal: {error}"))?;
    let destination = temporary.into_temp_path();
    let mut child = Command::new("curl")
        .args([
            "--fail",
            "--location",
            "--proto",
            "=https",
            "--proto-redir",
            "=https",
            "--progress-bar",
            "--max-time",
            "900",
            "--output",
        ])
        .arg(&destination)
        .arg(url)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
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
                    .progress(phase, progress.min(end), format!("Descargando {label}"))
                    .await;
            }
        }
    }
    let exit = child.wait().await.map_err(|error| error.to_string())?;
    if !exit.success() {
        return Err(format!("La descarga de {label} no pudo completarse"));
    }
    let bytes = std::fs::read(&destination)
        .map_err(|error| format!("No se pudo verificar {label}: {error}"))?;
    let actual = format!("{:x}", Sha256::digest(bytes));
    if actual != expected_sha256 {
        return Err(format!(
            "La integridad SHA-256 de {label} no coincide; el archivo no se utilizará"
        ));
    }
    state
        .progress(phase, end, format!("{label} descargado"))
        .await;
    Ok(destination)
}

async fn download(
    state: &AppState,
    url: &'static str,
    expected_sha256: &'static str,
    label: &'static str,
    start: u8,
    end: u8,
) -> Result<tempfile::TempPath, String> {
    download_for_phase(
        state,
        url,
        expected_sha256,
        label,
        start,
        end,
        InstallerPhase::Downloading,
    )
    .await
}

fn command(
    program: &'static str,
    args: Vec<&'static str>,
    description: &'static str,
    phase: InstallerPhase,
    progress: u8,
) -> CommandSpec {
    CommandSpec {
        program,
        args: args.into_iter().map(str::to_owned).collect(),
        description,
        phase,
        progress,
        dnf: None,
    }
}

fn owned_command(
    program: &'static str,
    args: Vec<String>,
    description: &'static str,
    phase: InstallerPhase,
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
        args: args.into_iter().map(str::to_owned).collect(),
        description,
        phase: InstallerPhase::Downloading,
        progress: tracking.download_start,
        dnf: Some(tracking),
    }
}

fn server_recipes(server: ServerEngine) -> Vec<CommandSpec> {
    match server {
        ServerEngine::Nginx => vec![dnf(
            vec!["install", "-y", "nginx"],
            "Instalando Nginx",
            DnfProgress {
                download_start: 2,
                download_end: 48,
                install_start: 50,
                install_end: 82,
                label: "Nginx",
            },
        )],
        ServerEngine::Caddy => vec![dnf(
            vec!["install", "-y", "caddy"],
            "Instalando Caddy",
            DnfProgress {
                download_start: 5,
                download_end: 48,
                install_start: 50,
                install_end: 82,
                label: "Caddy",
            },
        )],
        ServerEngine::Openlitespeed => vec![dnf(
            vec!["install", "-y", "openlitespeed", "procps-ng"],
            "Instalando OpenLiteSpeed",
            DnfProgress {
                download_start: 5,
                download_end: 48,
                install_start: 50,
                install_end: 78,
                label: "OpenLiteSpeed",
            },
        )],
    }
}

async fn prepare_caddy_repository(state: &AppState) -> Result<(), String> {
    state
        .progress(
            InstallerPhase::Configuring,
            5,
            "Configurando el repositorio verificado de Caddy",
        )
        .await;
    const REPOSITORY: &str = "[copr:copr.fedorainfracloud.org:group_caddy:caddy]\nname=Caddy official COPR\nbaseurl=https://download.copr.fedorainfracloud.org/results/@caddy/caddy/epel-9-$basearch/\ntype=rpm-md\nskip_if_unavailable=False\ngpgcheck=1\ngpgkey=https://download.copr.fedorainfracloud.org/results/@caddy/caddy/pubkey.gpg\nrepo_gpgcheck=0\nenabled=1\n";
    std::fs::write("/etc/yum.repos.d/caddy.repo", REPOSITORY)
        .map_err(|error| format!("No se pudo configurar el repositorio de Caddy: {error}"))
}

async fn prepare_openlitespeed_repository(state: &AppState) -> Result<(), String> {
    let script = download_for_phase(
        state,
        "https://repo.litespeed.sh",
        "45bb48ed6da20ba9970afe02ceca81f55a7418d97624062713a47b6f5f4f4895",
        "repositorio firmado de OpenLiteSpeed",
        1,
        4,
        InstallerPhase::Configuring,
    )
    .await?;
    run_command(
        state,
        owned_command(
            "bash",
            vec![script.to_string_lossy().into_owned()],
            "Configurando el repositorio verificado de OpenLiteSpeed",
            InstallerPhase::Configuring,
            5,
        ),
    )
    .await
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

async fn validate_openlitespeed_config(state: &AppState) -> Result<(), String> {
    const DESCRIPTION: &str = "Validando la configuración de OpenLiteSpeed";
    state
        .progress(InstallerPhase::Installing, 83, DESCRIPTION)
        .await;
    state.log(format!("› {DESCRIPTION}"), "info");
    let result = Command::new("/usr/local/lsws/bin/openlitespeed")
        .arg("-t")
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .output()
        .await
        .map_err(|error| format!("No se pudo ejecutar la validación de OpenLiteSpeed: {error}"))?;
    let output = format!(
        "{}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    for line in output.lines().filter(|line| !line.trim().is_empty()) {
        state.log(
            line,
            if line.contains("[ERROR]") || line.contains("[FATAL]") {
                "error"
            } else {
                "info"
            },
        );
    }
    if openlitespeed_config_is_valid(result.status.success(), &output) {
        return Ok(());
    }
    let detail = output
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("OpenLiteSpeed no entregó detalles");
    Err(format!("{DESCRIPTION} falló: {}", detail.trim()))
}

async fn configure_openlitespeed(state: &AppState) -> Result<(), String> {
    let root = Path::new("/var/www/cpn/html");
    std::fs::create_dir_all(root).map_err(|error| error.to_string())?;
    std::fs::write(
        root.join("index.html"),
        "<!doctype html><meta charset=\"utf-8\"><title>CPN</title><h1>CPN está listo</h1>",
    )
    .map_err(|error| error.to_string())?;
    std::fs::create_dir_all("/usr/local/lsws/conf/vhosts/cpn-default")
        .map_err(|error| error.to_string())?;
    let vhost = "docRoot /var/www/cpn/html\n\nindex {\n  useServer 0\n  indexFiles index.html\n}\n\ncontext / {\n  type static\n  location /var/www/cpn/html\n  allowBrowse 1\n}\n";
    std::fs::write("/usr/local/lsws/conf/vhosts/cpn-default/vhconf.conf", vhost)
        .map_err(|error| error.to_string())?;
    run_command(
        state,
        command(
            "chown",
            vec!["-R", "nobody:nobody", "/var/www/cpn"],
            "Ajustando permisos para OpenLiteSpeed",
            InstallerPhase::Installing,
            83,
        ),
    )
    .await?;
    let main_path = "/usr/local/lsws/conf/httpd_config.conf";
    let mut main = std::fs::read_to_string(main_path).map_err(|error| error.to_string())?;
    if !main.contains("# CPN_MANAGED_LISTENER") {
        main.push_str("\n# CPN_MANAGED_LISTENER\nvirtualhost cpn-default {\n  vhRoot /var/www/cpn\n  configFile /usr/local/lsws/conf/vhosts/cpn-default/vhconf.conf\n  allowSymbolLink 0\n  enableScript 1\n  restrained 1\n}\n\nlistener CPN_HTTP {\n  address *:80\n  secure 0\n  map cpn-default *\n}\n");
        std::fs::write(main_path, main).map_err(|error| error.to_string())?;
    }
    let admin_path = "/usr/local/lsws/admin/conf/admin_config.conf";
    if let Ok(admin) = std::fs::read_to_string(admin_path) {
        let restricted = admin.replace(
            "address                 *:7080",
            "address                 127.0.0.1:7080",
        );
        if restricted != admin {
            std::fs::write(admin_path, restricted).map_err(|error| error.to_string())?;
        }
    }
    validate_openlitespeed_config(state).await?;
    let vendor_source = Path::new("/usr/local/lsws/admin/misc/lshttpd.service");
    if !Path::new("/usr/lib/systemd/system/lsws.service").exists()
        && !Path::new("/usr/lib/systemd/system/lshttpd.service").exists()
        && vendor_source.exists()
    {
        std::fs::copy(vendor_source, "/usr/lib/systemd/system/lshttpd.service").map_err(
            |error| format!("No se pudo registrar el unit vendor de OpenLiteSpeed: {error}"),
        )?;
        run_command(
            state,
            command(
                "systemctl",
                vec!["daemon-reload"],
                "Registrando el unit vendor de OpenLiteSpeed",
                InstallerPhase::Installing,
                83,
            ),
        )
        .await?;
    }
    let vendor_unit = if Path::new("/usr/lib/systemd/system/lsws.service").exists() {
        "lsws"
    } else {
        "lshttpd"
    };
    run_command(
        state,
        owned_command(
            "systemctl",
            vec!["enable".into(), "--now".into(), vendor_unit.into()],
            "Activando OpenLiteSpeed",
            InstallerPhase::Installing,
            84,
        ),
    )
    .await
}

fn server_service(server: ServerEngine) -> &'static str {
    match server {
        ServerEngine::Nginx => "nginx",
        ServerEngine::Caddy => "caddy",
        ServerEngine::Openlitespeed => {
            if Path::new("/usr/lib/systemd/system/lsws.service").exists() {
                "lsws"
            } else {
                "lshttpd"
            }
        }
    }
}

fn server_url(server: ServerEngine) -> &'static str {
    match server {
        ServerEngine::Nginx | ServerEngine::Caddy => "http://127.0.0.1/__cpn_health",
        ServerEngine::Openlitespeed => "http://127.0.0.1/",
    }
}

fn http_response_is_valid(server: ServerEngine, status: u16, body: &str) -> bool {
    let marker = match server {
        ServerEngine::Nginx | ServerEngine::Caddy => "CPN health",
        ServerEngine::Openlitespeed => "CPN está listo",
    };
    (200..400).contains(&status) && body.contains(marker)
}

fn configure_server_http(server: ServerEngine) -> Result<(), String> {
    match server {
        ServerEngine::Nginx => std::fs::write(
            "/etc/nginx/conf.d/cpn-health.conf",
            r#"server {
  listen 80;
  server_name 127.0.0.1 localhost;

  location = /__cpn_health {
    default_type text/plain;
    return 200 "CPN health\n";
  }

  location / {
    default_type text/plain;
    return 200 "CPN está listo\n";
  }
}
"#,
        )
        .map_err(|error| format!("No se pudo configurar el endpoint HTTP de Nginx: {error}")),
        ServerEngine::Caddy => std::fs::write(
            "/etc/caddy/Caddyfile",
            r#"{
  auto_https off
}

:80 {
  respond /__cpn_health "CPN health" 200
  respond "CPN está listo" 200
}
"#,
        )
        .map_err(|error| format!("No se pudo configurar el endpoint HTTP de Caddy: {error}")),
        ServerEngine::Openlitespeed => Ok(()),
    }
}

async fn wait_for_server_http(state: &AppState, server: ServerEngine) -> Result<(), String> {
    const ATTEMPTS: u8 = 30;
    let url = server_url(server);
    state
        .progress(
            InstallerPhase::Testing,
            96,
            "Esperando la respuesta HTTP local real",
        )
        .await;
    state.log(format!("› Comprobando {url}"), "info");
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(2))
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|error| format!("No se pudo preparar la comprobación HTTP: {error}"))?;
    let mut last_error = "el servicio todavía no acepta conexiones".to_string();
    for attempt in 1..=ATTEMPTS {
        match client.get(url).send().await {
            Ok(response) => {
                let status = response.status().as_u16();
                match response.text().await {
                    Ok(body) if http_response_is_valid(server, status, &body) => {
                        state.log(format!("HTTP {status} recibido desde {url}"), "info");
                        return Ok(());
                    }
                    Ok(_) => {
                        last_error = if matches!(server, ServerEngine::Openlitespeed) {
                            format!("HTTP {status}, pero la página no pertenece a CPN")
                        } else {
                            format!("HTTP {status}")
                        };
                    }
                    Err(error) => last_error = format!("no se pudo leer la respuesta: {error}"),
                }
            }
            Err(error) => last_error = error.to_string(),
        }
        if attempt < ATTEMPTS {
            if attempt == 1 || attempt % 5 == 0 {
                state.log(
                    format!("El puerto aún no está listo; intento {attempt}/{ATTEMPTS}"),
                    "info",
                );
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }
    Err(format!(
        "El servicio está activo, pero {url} no respondió correctamente después de {ATTEMPTS} intentos: {last_error}"
    ))
}

pub async fn install(state: std::sync::Arc<AppState>, server: ServerEngine) {
    let result = async {
        verify_almalinux()?;
        state
            .progress(
                InstallerPhase::Configuring,
                0,
                "Preparando la configuración del servidor",
            )
            .await;
        if matches!(server, ServerEngine::Caddy) {
            prepare_caddy_repository(&state).await?;
        }
        if matches!(server, ServerEngine::Openlitespeed) {
            prepare_openlitespeed_repository(&state).await?;
        }
        for item in server_recipes(server) {
            run_command(&state, item).await?;
        }
        if matches!(server, ServerEngine::Openlitespeed) {
            configure_openlitespeed(&state).await?;
        } else {
            configure_server_http(server)?;
            run_command(
                &state,
                command(
                    "systemctl",
                    vec!["enable", "--now", server_service(server)],
                    "Activando el servidor web con su configuración de CPN",
                    InstallerPhase::Installing,
                    84,
                ),
            )
            .await?;
        }
        state
            .progress(
                InstallerPhase::Testing,
                90,
                "Comprobando que el servicio está activo",
            )
            .await;
        run_command(
            &state,
            command(
                "systemctl",
                vec!["is-active", "--quiet", server_service(server)],
                "Verificando el servicio con systemd",
                InstallerPhase::Testing,
                92,
            ),
        )
        .await?;
        wait_for_server_http(&state, server).await?;
        if let Some(environment) = state.status.read().await.environment.clone() {
            crate::environment::open_web_services(&environment).await?;
        }
        Ok::<_, String>(())
    }
    .await;
    finish(&state, result, server.label()).await;
}

const PHP_PACKAGES: &[&str] = &[
    "install",
    "-y",
    "php-cli",
    "php-fpm",
    "php-mbstring",
    "php-intl",
    "php-xml",
    "php-pdo",
    "php-sqlite3",
    "php-process",
    "php-gd",
    "php-opcache",
    "php-pecl-zip",
    "unzip",
    "tar",
    "sqlite",
];

async fn install_php_runtime(state: &AppState, label: &'static str) -> Result<(), String> {
    run_command(
        state,
        dnf(
            vec![
                "install",
                "-y",
                "epel-release",
                "https://rpms.remirepo.net/enterprise/remi-release-9.rpm",
            ],
            "Habilitando el repositorio firmado de PHP",
            DnfProgress {
                download_start: 51,
                download_end: 53,
                install_start: 53,
                install_end: 54,
                label: "repositorio PHP",
            },
        ),
    )
    .await?;
    run_command(
        state,
        command(
            "dnf",
            vec!["module", "enable", "-y", "php:remi-8.3"],
            "Preparando PHP 8.3 con soporte de seguridad",
            InstallerPhase::Downloading,
            54,
        ),
    )
    .await?;
    run_command(
        state,
        dnf(
            PHP_PACKAGES.to_vec(),
            "Instalando PHP y sus extensiones",
            DnfProgress {
                download_start: 55,
                download_end: 67,
                install_start: 68,
                install_end: 79,
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

async fn configure_webmail_service(
    state: &AppState,
    root: &'static str,
    mail: MailSystem,
) -> Result<(), String> {
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
                InstallerPhase::Installing,
                83,
            ),
        )
        .await?;
    }
    run_command(
        state,
        command(
            "chown",
            vec!["root:cpn-webmail", "/etc/cpn"],
            "Protegiendo el directorio de integración del webmail",
            InstallerPhase::Installing,
            83,
        ),
    )
    .await?;
    run_command(
        state,
        command(
            "chmod",
            vec!["0750", "/etc/cpn"],
            "Limitando el acceso al directorio de integración",
            InstallerPhase::Installing,
            83,
        ),
    )
    .await?;
    reset_current_link(Path::new(root))?;
    let pool = "[cpn-webmail]\nuser = cpn-webmail\ngroup = cpn-webmail\nlisten = 127.0.0.1:9001\npm = ondemand\npm.max_children = 8\npm.process_idle_timeout = 10s\nclear_env = yes\nphp_admin_value[open_basedir] = /opt/cpn-webmail:/etc/cpn/webmail-agent.token:/tmp\nphp_admin_value[session.save_path] = /opt/cpn-webmail/runtime/sessions\nphp_admin_flag[expose_php] = off\n";
    std::fs::create_dir_all("/opt/cpn-webmail/runtime/sessions")
        .map_err(|error| error.to_string())?;
    std::fs::write("/etc/php-fpm.d/cpn-webmail.conf", pool).map_err(|error| error.to_string())?;
    run_command(
        state,
        command(
            "chown",
            vec!["-R", "root:root", "/opt/cpn-webmail"],
            "Protegiendo el código del webmail",
            InstallerPhase::Installing,
            84,
        ),
    )
    .await?;
    let mut writable = vec!["/opt/cpn-webmail/runtime"];
    match mail {
        MailSystem::Snappymail => writable.push("/opt/cpn-webmail/snappymail/data"),
        MailSystem::Rainloop => writable.push("/opt/cpn-webmail/rainloop/data"),
        MailSystem::Roundcube => writable.extend([
            "/opt/cpn-webmail/roundcube/temp",
            "/opt/cpn-webmail/roundcube/logs",
        ]),
        MailSystem::Thunderbird => {}
    }
    run_command(
        state,
        owned_command(
            "chown",
            std::iter::once("-R".to_owned())
                .chain(std::iter::once("cpn-webmail:cpn-webmail".to_owned()))
                .chain(writable.into_iter().map(str::to_owned))
                .collect(),
            "Habilitando solo los directorios de runtime",
            InstallerPhase::Installing,
            85,
        ),
    )
    .await?;

    let server = state
        .status
        .read()
        .await
        .installed_server
        .ok_or("No existe un servidor web instalado")?;
    match server {
        ServerEngine::Nginx => {
            let config = format!(
                "server {{\n  listen 8888;\n  server_name _;\n  root {root};\n  index index.php;\n  location / {{ try_files $uri $uri/ /index.php?$query_string; }}\n  location ~ \\.php$ {{ include fastcgi_params; fastcgi_param SCRIPT_FILENAME $document_root$fastcgi_script_name; fastcgi_pass 127.0.0.1:9001; }}\n}}\n"
            );
            std::fs::write("/etc/nginx/conf.d/cpn-webmail.conf", config)
                .map_err(|error| error.to_string())?;
        }
        ServerEngine::Caddy => {
            let path = "/etc/caddy/Caddyfile";
            let mut config = std::fs::read_to_string(path).unwrap_or_default();
            if let Some(index) = config.find("# CPN_WEBMAIL_BEGIN") {
                config.truncate(index);
            }
            config.push_str(&format!("\n# CPN_WEBMAIL_BEGIN\n:8888 {{\n  root * {root}\n  php_fastcgi 127.0.0.1:9001\n  file_server\n}}\n"));
            std::fs::write(path, config).map_err(|error| error.to_string())?;
        }
        ServerEngine::Openlitespeed => {
            let vhost = format!(
                "docRoot {root}\n\nindex {{\n  useServer 0\n  indexFiles index.php\n}}\n\nextprocessor cpnWebmailFpm {{\n  type fcgi\n  address 127.0.0.1:9001\n  path /usr/sbin/php-fpm\n  autoStart 0\n  maxConns 8\n  initTimeout 60\n  retryTimeout 0\n}}\n\nscripthandler {{\n  add fcgi:cpnWebmailFpm php\n}}\n\ncontext / {{\n  type null\n  location {root}\n  allowBrowse 1\n}}\n"
            );
            std::fs::create_dir_all("/usr/local/lsws/conf/vhosts/cpn-webmail")
                .map_err(|error| error.to_string())?;
            std::fs::write("/usr/local/lsws/conf/vhosts/cpn-webmail/vhconf.conf", vhost)
                .map_err(|error| error.to_string())?;
            let path = "/usr/local/lsws/conf/httpd_config.conf";
            let mut config = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
            if let Some(index) = config.find("# CPN_WEBMAIL_BEGIN") {
                config.truncate(index);
            }
            config.push_str("# CPN_WEBMAIL_BEGIN\nvirtualhost cpn-webmail {\n  vhRoot /opt/cpn-webmail\n  configFile /usr/local/lsws/conf/vhosts/cpn-webmail/vhconf.conf\n  allowSymbolLink 1\n  enableScript 1\n  restrained 1\n}\n\nlistener CPN_WEBMAIL {\n  address *:8888\n  secure 0\n  map cpn-webmail *\n}\n");
            std::fs::write(path, config).map_err(|error| error.to_string())?;
            run_command(
                state,
                command(
                    "sh",
                    vec![
                        "-c",
                        "/usr/local/lsws/bin/openlitespeed -t > /tmp/cpn-webmail-ols-check 2>&1; test $? -eq 0; ! grep -q '\\[ERROR\\]' /tmp/cpn-webmail-ols-check",
                    ],
                    "Validando la configuración del webmail en OpenLiteSpeed",
                    InstallerPhase::Testing,
                    86,
                ),
            )
            .await?;
        }
    }
    run_command(
        state,
        command(
            "systemctl",
            vec!["enable", "--now", "php-fpm"],
            "Activando PHP-FPM",
            InstallerPhase::Installing,
            86,
        ),
    )
    .await?;
    run_command(
        state,
        command(
            "systemctl",
            vec!["restart", server_service(server)],
            "Activando el webmail detrás del servidor elegido",
            InstallerPhase::Installing,
            88,
        ),
    )
    .await
}

async fn install_webmail(state: &AppState, mail: MailSystem) -> Result<(), String> {
    std::fs::create_dir_all("/opt/cpn-webmail").map_err(|error| error.to_string())?;
    std::fs::create_dir_all("/opt/cpn-webmail/runtime")
        .map_err(|error| format!("No se pudo preparar el runtime del webmail: {error}"))?;
    match mail {
        MailSystem::Snappymail => {
            std::fs::create_dir_all("/opt/cpn-webmail/snappymail")
                .map_err(|error| error.to_string())?;
            let archive = download(state, "https://github.com/the-djmaze/snappymail/releases/download/v2.38.2/snappymail-2.38.2.tar.gz", "71f1d8a9065cc9cf7ddd064f5c47cc7b255cb70e6a56713647fc73d4b79e33ec", "SnappyMail", 35, 50).await?;
            install_php_runtime(state, "PHP para SnappyMail").await?;
            run_command(
                state,
                owned_command(
                    "tar",
                    vec![
                        "xzf".into(),
                        archive.to_string_lossy().into_owned(),
                        "-C".into(),
                        "/opt/cpn-webmail/snappymail".into(),
                    ],
                    "Extrayendo SnappyMail",
                    InstallerPhase::Installing,
                    80,
                ),
            )
            .await?;
            configure_webmail_service(state, "/opt/cpn-webmail/snappymail", mail).await?;
        }
        MailSystem::Rainloop => {
            std::fs::create_dir_all("/opt/cpn-webmail/rainloop")
                .map_err(|error| error.to_string())?;
            let archive = download(
                state,
                "https://github.com/RainLoop/rainloop-webmail/releases/download/v1.17.0/rainloop-legacy-1.17.0.zip",
                "782dcabacadab5d7176f7701dd23319a040b2cfbf974fac6df068600cf69c50a",
                "RainLoop",
                35,
                50,
            )
            .await?;
            install_php_runtime(state, "PHP para RainLoop").await?;
            run_command(
                state,
                owned_command(
                    "unzip",
                    vec![
                        "-q".into(),
                        archive.to_string_lossy().into_owned(),
                        "-d".into(),
                        "/opt/cpn-webmail/rainloop".into(),
                    ],
                    "Extrayendo RainLoop",
                    InstallerPhase::Installing,
                    80,
                ),
            )
            .await?;
            configure_webmail_service(state, "/opt/cpn-webmail/rainloop", mail).await?;
        }
        MailSystem::Roundcube => {
            std::fs::create_dir_all("/opt/cpn-webmail/roundcube")
                .map_err(|error| error.to_string())?;
            let archive = download(state, "https://github.com/roundcube/roundcubemail/releases/download/1.7.1/roundcubemail-1.7.1-complete.tar.gz", "1e0382bcefd627ab0b6285d3181ddfba5b444fdcf6d49f33f5ea15fbf97864ef", "Roundcube", 35, 50).await?;
            install_php_runtime(state, "PHP para Roundcube").await?;
            run_command(
                state,
                owned_command(
                    "tar",
                    vec![
                        "xzf".into(),
                        archive.to_string_lossy().into_owned(),
                        "-C".into(),
                        "/opt/cpn-webmail/roundcube".into(),
                        "--strip-components=1".into(),
                    ],
                    "Extrayendo Roundcube",
                    InstallerPhase::Installing,
                    80,
                ),
            )
            .await?;
            let key: String = rand::rng()
                .sample_iter(&Alphanumeric)
                .take(24)
                .map(char::from)
                .collect();
            let config = format!(
                "<?php\n$config = [];\n$config['db_dsnw'] = 'sqlite:////opt/cpn-webmail/runtime/roundcube.sqlite';\n$config['imap_host'] = 'localhost:143';\n$config['smtp_host'] = 'localhost:587';\n$config['smtp_user'] = '%u';\n$config['smtp_pass'] = '%p';\n$config['product_name'] = 'Roundcube Webmail';\n$config['des_key'] = '{key}';\n$config['plugins'] = ['archive', 'zipdownload', 'cpn_sso'];\n$config['skin'] = 'elastic';\n"
            );
            std::fs::write("/opt/cpn-webmail/roundcube/config/config.inc.php", config)
                .map_err(|error| error.to_string())?;
            std::fs::create_dir_all("/opt/cpn-webmail/roundcube/plugins/cpn_sso")
                .map_err(|error| error.to_string())?;
            let plugin = r#"<?php
class cpn_sso extends rcube_plugin
{
    public $task = 'login';
    public function init()
    {
        $this->add_hook('startup', [$this, 'startup']);
        $this->add_hook('authenticate', [$this, 'authenticate']);
    }
    public function startup($args)
    {
        if (empty($_SESSION['user_id']) && !empty($_GET['_cpn_sso'])) $args['action'] = 'login';
        return $args;
    }
    public function authenticate($args)
    {
        if (empty($_GET['_cpn_sso'])) return $args;
        $code = preg_replace('/[^A-Za-z0-9_-]/', '', $_GET['_cpn_sso']);
        $token = trim(@file_get_contents('/etc/cpn/webmail-agent.token'));
        if (!$code || !$token) return $args;
        $context = stream_context_create(['http' => [
            'method' => 'POST',
            'header' => "Authorization: Bearer {$token}\r\nContent-Length: 0\r\n",
            'timeout' => 5,
            'ignore_errors' => true,
        ]]);
        $body = @file_get_contents("http://127.0.0.1:8091/api/internal/webmail/redeem/{$code}", false, $context);
        $login = json_decode($body ?: '', true);
        if (empty($login['username']) || empty($login['password'])) return $args;
        $args['user'] = $login['username'];
        $args['pass'] = $login['password'];
        $args['host'] = '127.0.0.1:143';
        $args['cookiecheck'] = false;
        $args['valid'] = true;
        return $args;
    }
}
"#;
            std::fs::write(
                "/opt/cpn-webmail/roundcube/plugins/cpn_sso/cpn_sso.php",
                plugin,
            )
            .map_err(|error| error.to_string())?;
            std::fs::OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .mode(0o600)
                .open("/opt/cpn-webmail/runtime/roundcube.sqlite")
                .map_err(|error| error.to_string())?;
            run_command(
                state,
                command(
                    "php",
                    vec!["-r", "exit(extension_loaded('pdo_sqlite') ? 0 : 1);"],
                    "Verificando PDO SQLite",
                    InstallerPhase::Testing,
                    81,
                ),
            )
            .await?;
            run_command(
                state,
                command(
                    "sqlite3",
                    vec![
                        "/opt/cpn-webmail/runtime/roundcube.sqlite",
                        ".read /opt/cpn-webmail/roundcube/SQL/sqlite.initial.sql",
                    ],
                    "Inicializando la base de datos de Roundcube",
                    InstallerPhase::Installing,
                    82,
                ),
            )
            .await?;
            run_command(
                state,
                command(
                    "sqlite3",
                    vec![
                        "/opt/cpn-webmail/runtime/roundcube.sqlite",
                        "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='users';",
                    ],
                    "Verificando el esquema de Roundcube",
                    InstallerPhase::Testing,
                    83,
                ),
            )
            .await?;
            configure_webmail_service(state, "/opt/cpn-webmail/roundcube/public_html", mail)
                .await?;
        }
        MailSystem::Thunderbird => unreachable!(),
    }
    Ok(())
}

async fn install_mail_backend(state: &AppState) -> Result<(), String> {
    run_command(
        state,
        dnf(
            vec!["install", "-y", "postfix", "dovecot", "dovecot-pigeonhole"],
            "Instalando el servidor de correo",
            DnfProgress {
                download_start: 2,
                download_end: 14,
                install_start: 15,
                install_end: 25,
                label: "Postfix y Dovecot",
            },
        ),
    )
    .await?;
    let user_exists = Command::new("id")
        .args(["-u", "vmail"])
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
                    "/var/vmail",
                    "--create-home",
                    "--shell",
                    "/sbin/nologin",
                    "vmail",
                ],
                "Creando el usuario aislado para buzones",
                InstallerPhase::Installing,
                26,
            ),
        )
        .await?;
    }
    std::fs::create_dir_all("/var/vmail").map_err(|error| error.to_string())?;
    std::fs::create_dir_all("/var/lib/cpn").map_err(|error| error.to_string())?;
    for (path, contents, mode) in [
        ("/etc/dovecot/cpn-users", "", 0o600),
        ("/etc/postfix/cpn-domains", "", 0o600),
        ("/etc/postfix/cpn-mailboxes", "", 0o600),
        ("/var/lib/cpn/mailboxes.json", "[]\n", 0o600),
    ] {
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .mode(mode)
            .open(path)
            .map_err(|error| error.to_string())?;
        if file.metadata().map_err(|error| error.to_string())?.len() == 0 {
            use std::io::Write;
            file.write_all(contents.as_bytes())
                .map_err(|error| error.to_string())?;
        }
    }
    let key = secrets::load_or_create_key(Path::new(panel::KEY_PATH))?;
    let master_path = Path::new("/var/lib/cpn/secrets/mail-master.enc");
    let master_password: String = if master_path.exists() {
        secrets::open(master_path, &key)?
    } else {
        let value: String = rand::rng()
            .sample_iter(&Alphanumeric)
            .take(48)
            .map(char::from)
            .collect();
        secrets::seal(master_path, &key, &value)?;
        value
    };
    let mut child = Command::new("openssl")
        .args(["passwd", "-6", "-stdin"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| error.to_string())?;
    child
        .stdin
        .as_mut()
        .ok_or("No se pudo proteger la clave maestra")?
        .write_all(format!("{master_password}\n").as_bytes())
        .await
        .map_err(|error| error.to_string())?;
    drop(child.stdin.take());
    let output = child
        .wait_with_output()
        .await
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err("No se pudo proteger la clave maestra de acceso webmail".into());
    }
    let master_hash = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    std::fs::write(
        "/etc/dovecot/cpn-master-users",
        format!("cpn-master:{{SHA512-CRYPT}}{master_hash}\n"),
    )
    .map_err(|error| error.to_string())?;
    std::fs::set_permissions(
        "/etc/dovecot/cpn-master-users",
        std::fs::Permissions::from_mode(0o640),
    )
    .map_err(|error| error.to_string())?;
    std::fs::set_permissions(
        "/etc/dovecot/cpn-users",
        std::fs::Permissions::from_mode(0o640),
    )
    .map_err(|error| error.to_string())?;
    run_command(
        state,
        command(
            "chown",
            vec![
                "root:dovecot",
                "/etc/dovecot/cpn-users",
                "/etc/dovecot/cpn-master-users",
            ],
            "Protegiendo las credenciales de Dovecot",
            InstallerPhase::Installing,
            30,
        ),
    )
    .await?;
    let dovecot = "protocols = imap lmtp\nlisten = 127.0.0.1\ndisable_plaintext_auth = no\nauth_mechanisms = plain login\nauth_master_user_separator = *\nmail_home = /var/vmail/%d/%n\nmail_location = maildir:~/Maildir\nfirst_valid_uid = 1\n\npassdb {\n  driver = passwd-file\n  master = yes\n  pass = yes\n  args = scheme=SHA512-CRYPT /etc/dovecot/cpn-master-users\n}\npassdb {\n  driver = passwd-file\n  args = scheme=SHA512-CRYPT username_format=%u /etc/dovecot/cpn-users\n}\nuserdb {\n  driver = static\n  args = uid=vmail gid=vmail home=/var/vmail/%d/%n\n}\nservice lmtp {\n  unix_listener /var/spool/postfix/private/dovecot-lmtp {\n    mode = 0600\n    user = postfix\n    group = postfix\n  }\n}\nservice auth {\n  unix_listener /var/spool/postfix/private/auth {\n    mode = 0660\n    user = postfix\n    group = postfix\n  }\n}\nprotocol lmtp {\n  postmaster_address = postmaster@localhost\n}\n";
    std::fs::write("/etc/dovecot/conf.d/99-cpn.conf", dovecot)
        .map_err(|error| error.to_string())?;
    for setting in [
        "inet_interfaces = all",
        "mydestination = localhost",
        "virtual_mailbox_domains = hash:/etc/postfix/cpn-domains",
        "virtual_mailbox_maps = hash:/etc/postfix/cpn-mailboxes",
        "virtual_transport = lmtp:unix:private/dovecot-lmtp",
        "smtpd_sasl_type = dovecot",
        "smtpd_sasl_path = private/auth",
        "smtpd_sasl_auth_enable = yes",
        "smtpd_recipient_restrictions = permit_mynetworks,permit_sasl_authenticated,reject_unauth_destination",
    ] {
        run_command(
            state,
            owned_command(
                "postconf",
                vec!["-e".into(), setting.into()],
                "Configurando el transporte de correo",
                InstallerPhase::Installing,
                28,
            ),
        )
        .await?;
    }
    for map in ["/etc/postfix/cpn-domains", "/etc/postfix/cpn-mailboxes"] {
        run_command(
            state,
            owned_command(
                "postmap",
                vec![map.into()],
                "Preparando los mapas de buzones",
                InstallerPhase::Installing,
                29,
            ),
        )
        .await?;
    }
    run_command(
        state,
        command(
            "chown",
            vec!["-R", "vmail:vmail", "/var/vmail"],
            "Protegiendo el almacenamiento de buzones",
            InstallerPhase::Installing,
            30,
        ),
    )
    .await?;
    run_command(
        state,
        command(
            "doveconf",
            vec!["-n"],
            "Validando Dovecot",
            InstallerPhase::Testing,
            31,
        ),
    )
    .await?;
    run_command(
        state,
        command(
            "postfix",
            vec!["check"],
            "Validando Postfix",
            InstallerPhase::Testing,
            32,
        ),
    )
    .await?;
    run_command(
        state,
        command(
            "systemctl",
            vec!["enable", "--now", "postfix", "dovecot"],
            "Activando el servidor de correo",
            InstallerPhase::Installing,
            34,
        ),
    )
    .await
}

pub async fn install_mail(state: std::sync::Arc<AppState>, mail: MailSystem) {
    let result = async {
        verify_almalinux()?;
        install_mail_backend(&state).await?;
        if matches!(mail, MailSystem::Thunderbird) {
            run_command(
                &state,
                dnf(
                    vec!["install", "-y", "thunderbird"],
                    "Instalando Thunderbird",
                    DnfProgress {
                        download_start: 35,
                        download_end: 58,
                        install_start: 60,
                        install_end: 88,
                        label: "Thunderbird",
                    },
                ),
            )
            .await?;
            state
                .progress(InstallerPhase::Testing, 92, "Comprobando Thunderbird")
                .await;
            run_command(
                &state,
                command(
                    "thunderbird",
                    vec!["--version"],
                    "Verificando la versión instalada",
                    InstallerPhase::Testing,
                    96,
                ),
            )
            .await?;
        } else {
            install_webmail(&state, mail).await?;
            state
                .progress(
                    InstallerPhase::Testing,
                    92,
                    format!("Comprobando {}", mail.label()),
                )
                .await;
            run_command(
                &state,
                command(
                    "systemctl",
                    vec!["is-active", "--quiet", "php-fpm"],
                    "Verificando PHP-FPM del cliente webmail",
                    InstallerPhase::Testing,
                    94,
                ),
            )
            .await?;
            let marker = match mail {
                MailSystem::Snappymail => "SnappyMail",
                MailSystem::Rainloop => "RainLoop",
                MailSystem::Roundcube => "Roundcube",
                MailSystem::Thunderbird => unreachable!(),
            };
            run_command(
                &state,
                owned_command(
                    "sh",
                    vec![
                        "-c".into(),
                        format!(
                            "curl --fail --silent --show-error --retry 10 --retry-all-errors --retry-delay 1 --max-time 20 http://127.0.0.1:8888/ | grep -Fqi {marker}"
                        ),
                    ],
                    "Comprobando el contenido HTTP real del webmail",
                    InstallerPhase::Testing,
                    97,
                ),
            )
            .await?;
        }
        Ok::<_, String>(())
    }
    .await;
    finish(&state, result, mail.label()).await;
}

async fn finish(state: &AppState, result: Result<(), String>, label: &str) {
    let mut status = state.status.write().await;
    match result {
        Ok(()) => {
            status.phase = InstallerPhase::Completed;
            status.progress = 100;
            status.message = format!("{label} se instaló y verificó correctamente");
            if let Some(server) = status.selected_server
                && status.stage == SetupStage::Server
            {
                status.installed_server = Some(server);
                status.stage = SetupStage::Mail;
            }
            if let Some(mail) = status.selected_mail
                && status.stage == SetupStage::Mail
            {
                status.installed_mail = Some(mail);
                status.stage = SetupStage::Domain;
            }
            let _ = state.events.send(InstallerEvent::Completed {
                status: status.clone(),
            });
        }
        Err(error) => {
            status.failed_phase = Some(status.phase);
            status.phase = InstallerPhase::FailedPartial;
            status.error = Some(error.clone());
            status.message =
                "La instalación falló; el sistema puede conservar cambios parciales".into();
            state.log(error, "error");
            let _ = state.events.send(InstallerEvent::Error {
                status: status.clone(),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{fraction, http_response_is_valid, openlitespeed_config_is_valid, verify_release};
    use crate::model::ServerEngine;

    #[test]
    fn parses_dnf_download_and_transaction_fractions() {
        assert_eq!(fraction("(3/8): package.rpm"), Some((3, 8)));
        assert_eq!(fraction("Installing : package 5/7"), Some((5, 7)));
        assert_eq!(fraction("No package progress here"), None);
    }

    #[test]
    fn accepts_openlitespeed_warning_exit_but_rejects_real_errors() {
        assert!(openlitespeed_config_is_valid(
            false,
            "[WARN] License validation failed - module features disabled"
        ));
        assert!(!openlitespeed_config_is_valid(
            false,
            "[ERROR] listener CPN_HTTP has an invalid address"
        ));
        assert!(!openlitespeed_config_is_valid(false, "unexpected failure"));
        assert!(!openlitespeed_config_is_valid(
            false,
            "[WARN] optional module disabled\ninvalid configuration"
        ));
    }

    #[test]
    fn accepts_only_almalinux_nine() {
        assert!(verify_release("ID=almalinux\nVERSION_ID=\"9.8\"\n").is_ok());
        assert!(verify_release("ID=almalinux\nVERSION_ID=\"8.10\"\n").is_err());
        assert!(verify_release("ID=almalinux\nVERSION_ID=\"10.0\"\n").is_err());
        assert!(verify_release("ID=rocky\nVERSION_ID=\"9.8\"\n").is_err());
    }

    #[test]
    fn validates_real_http_response_for_each_server() {
        assert!(http_response_is_valid(
            ServerEngine::Nginx,
            200,
            "CPN health"
        ));
        assert!(http_response_is_valid(
            ServerEngine::Caddy,
            200,
            "CPN health"
        ));
        assert!(http_response_is_valid(
            ServerEngine::Openlitespeed,
            200,
            "<h1>CPN está listo</h1>"
        ));
        assert!(!http_response_is_valid(
            ServerEngine::Openlitespeed,
            200,
            "LiteSpeed default page"
        ));
        assert!(!http_response_is_valid(ServerEngine::Nginx, 200, "welcome"));
        assert!(!http_response_is_valid(ServerEngine::Nginx, 503, ""));
    }
}
