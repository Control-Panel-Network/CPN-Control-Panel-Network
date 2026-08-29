use crate::{
    environment,
    model::{InstallerStatus, MailSystem, ServerEngine},
    oauth::CloudflareAuthorization,
    secrets,
};
use rand::{Rng, distr::Alphanumeric};
use scrypt::{Params, scrypt};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::Path,
    process::Stdio,
};
use tokio::process::Command;

pub const PANEL_PORT: u16 = 8090;
pub const PANEL_UPSTREAM_PORT: u16 = 8091;
pub const CONFIG_PATH: &str = "/etc/cpn/install.json";
pub const KEY_PATH: &str = "/etc/cpn/secret.key";
pub const CLOUDFLARE_PATH: &str = "/var/lib/cpn/secrets/cloudflare.enc";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PanelConfig {
    pub domain: String,
    pub server: ServerEngine,
    pub webmail: MailSystem,
    pub panel_url: String,
    pub webmail_url: String,
}

pub struct PanelInstallResult {
    pub url: String,
    pub email: String,
    pub password: String,
}

fn random_secret(length: usize) -> String {
    rand::rng()
        .sample_iter(&Alphanumeric)
        .take(length)
        .map(char::from)
        .collect()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn password_hash(password: &str) -> Result<String, String> {
    let mut salt = [0_u8; 16];
    rand::rng().fill(&mut salt);
    let mut output = [0_u8; 32];
    let params = Params::new(14, 8, 1, output.len()).map_err(|error| error.to_string())?;
    scrypt(password.as_bytes(), &salt, &params, &mut output).map_err(|error| error.to_string())?;
    Ok(format!("{}:{}", hex(&salt), hex(&output)))
}

fn write_file(path: &str, contents: &[u8], mode: u32) -> Result<(), String> {
    let path = Path::new(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(mode)
        .open(path)
        .map_err(|error| error.to_string())?;
    file.write_all(contents)
        .map_err(|error| error.to_string())?;
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).map_err(|error| error.to_string())
}

async fn checked(program: &str, args: &[&str]) -> Result<(), String> {
    let result = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|error| format!("No se pudo ejecutar {program}: {error}"))?;
    if result.status.success() {
        Ok(())
    } else {
        Err(format!(
            "{} {} falló: {}",
            program,
            args.join(" "),
            String::from_utf8_lossy(&result.stderr).trim()
        ))
    }
}

fn configure_proxy(server: ServerEngine) -> Result<(), String> {
    match server {
        ServerEngine::Nginx => write_file(
            "/etc/nginx/conf.d/cpn-panel.conf",
            format!(
                "server {{\n  listen {PANEL_PORT};\n  server_name _;\n  location / {{\n    proxy_pass http://127.0.0.1:{PANEL_UPSTREAM_PORT};\n    proxy_http_version 1.1;\n    proxy_set_header Host $http_host;\n    proxy_set_header X-Forwarded-Host $http_host;\n    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;\n    proxy_set_header X-Forwarded-Proto $scheme;\n    proxy_buffering off;\n  }}\n}}\n"
            )
            .as_bytes(),
            0o644,
        ),
        ServerEngine::Caddy => {
            let path = "/etc/caddy/Caddyfile";
            let mut config = fs::read_to_string(path).unwrap_or_default();
            if let Some(index) = config.find("# CPN_PANEL_BEGIN") {
                config.truncate(index);
            }
            config.push_str(&format!(
                "\n# CPN_PANEL_BEGIN\n:{PANEL_PORT} {{\n  reverse_proxy 127.0.0.1:{PANEL_UPSTREAM_PORT}\n}}\n"
            ));
            write_file(path, config.as_bytes(), 0o644)
        }
        ServerEngine::Openlitespeed => {
            let root = "/usr/local/lsws/conf/vhosts/cpn-panel";
            fs::create_dir_all(root).map_err(|error| error.to_string())?;
            write_file(
                &format!("{root}/vhconf.conf"),
                format!(
                    "docRoot /var/empty\n\nextprocessor cpnPanel {{\n  type proxy\n  address 127.0.0.1:{PANEL_UPSTREAM_PORT}\n  maxConns 20\n  initTimeout 60\n  retryTimeout 0\n}}\n\ncontext / {{\n  type proxy\n  handler cpnPanel\n  addDefaultCharset off\n}}\n"
                )
                .as_bytes(),
                0o644,
            )?;
            let path = "/usr/local/lsws/conf/httpd_config.conf";
            let mut config = fs::read_to_string(path).map_err(|error| error.to_string())?;
            if let Some(index) = config.find("# CPN_PANEL_BEGIN") {
                config.truncate(index);
            }
            config.push_str(&format!(
                "# CPN_PANEL_BEGIN\nvirtualhost cpn-panel {{\n  vhRoot {root}\n  configFile {root}/vhconf.conf\n  allowSymbolLink 0\n  enableScript 1\n  restrained 1\n}}\n\nlistener CPN_PANEL {{\n  address *:{PANEL_PORT}\n  secure 0\n  map cpn-panel *\n}}\n"
            ));
            write_file(path, config.as_bytes(), 0o600)
        }
    }
}

fn service_name(server: ServerEngine) -> &'static str {
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

pub async fn provision(
    status: &InstallerStatus,
    cloudflare: Option<&CloudflareAuthorization>,
) -> Result<PanelInstallResult, String> {
    let domain = status.domain.clone().ok_or("Falta el dominio del panel")?;
    let server = status.installed_server.ok_or("Falta el servidor web")?;
    let webmail = status.installed_mail.ok_or("Falta el cliente webmail")?;
    let environment = status.environment.clone().ok_or("Falta el entorno")?;
    let address = environment
        .addresses
        .first()
        .cloned()
        .unwrap_or_else(|| "127.0.0.1".into());
    let url = format!("http://{address}:{PANEL_PORT}");
    let email = format!("admin@{domain}");
    let password = random_secret(22);
    let webmail_token = random_secret(48);
    let session_secret = random_secret(64);
    let config = PanelConfig {
        domain,
        server,
        webmail,
        panel_url: url.clone(),
        webmail_url: format!("http://{address}:8888"),
    };

    write_file(
        CONFIG_PATH,
        &serde_json::to_vec_pretty(&config).map_err(|error| error.to_string())?,
        0o600,
    )?;
    let environment_file = format!(
        "NODE_ENV=production\nHOSTNAME=127.0.0.1\nPORT={PANEL_UPSTREAM_PORT}\nCPN_PANEL_WEBMAIL_TOKEN={webmail_token}\nCPN_PANEL_ADMIN_EMAIL={email}\nCPN_PANEL_ADMIN_PASSWORD_SCRYPT={}\nCPN_PANEL_SESSION_SECRET={session_secret}\n",
        password_hash(&password)?
    );
    write_file("/etc/cpn/panel.env", environment_file.as_bytes(), 0o600)?;
    write_file(
        "/etc/cpn/webmail-agent.token",
        webmail_token.as_bytes(),
        0o640,
    )?;
    if Path::new("/etc/cpn/webmail-agent.token").exists() {
        let _ = checked(
            "chown",
            &["root:cpn-webmail", "/etc/cpn/webmail-agent.token"],
        )
        .await;
    }

    let key = secrets::load_or_create_key(Path::new(KEY_PATH))?;
    if let Some(authorization) = cloudflare {
        secrets::seal(Path::new(CLOUDFLARE_PATH), &key, authorization)?;
        let verified: CloudflareAuthorization = secrets::open(Path::new(CLOUDFLARE_PATH), &key)?;
        if verified.zone_id != authorization.zone_id
            || verified.access_token != authorization.access_token
        {
            return Err("La verificación del secreto cifrado de Cloudflare falló".into());
        }
    }

    configure_proxy(server)?;
    checked("systemctl", &["daemon-reload"]).await?;
    checked("systemctl", &["enable", "cpn-panel"]).await?;
    checked("systemctl", &["restart", "cpn-panel"]).await?;
    checked("systemctl", &["restart", service_name(server)]).await?;
    environment::open_persistent_port(&environment, PANEL_PORT).await?;
    if webmail != MailSystem::Thunderbird {
        environment::open_persistent_port(&environment, 8888).await?;
    }
    let local_url = format!("http://127.0.0.1:{PANEL_PORT}");
    checked(
        "curl",
        &[
            "--fail",
            "--silent",
            "--retry",
            "15",
            "--retry-connrefused",
            "--retry-delay",
            "1",
            "--max-time",
            "30",
            &local_url,
        ],
    )
    .await?;
    if webmail != MailSystem::Thunderbird {
        checked(
            "curl",
            &[
                "--fail",
                "--silent",
                "--retry",
                "15",
                "--retry-connrefused",
                "--retry-delay",
                "1",
                "--max-time",
                "30",
                "http://127.0.0.1:8888",
            ],
        )
        .await?;
    }

    Ok(PanelInstallResult {
        url,
        email,
        password,
    })
}

#[cfg(test)]
mod tests {
    use super::password_hash;

    #[test]
    fn panel_password_uses_the_node_compatible_scrypt_format() {
        let password = ["correct", "horse", "battery", "staple"].join(" ");
        let encoded = password_hash(&password).unwrap();
        let (salt, hash) = encoded.split_once(':').unwrap();
        assert_eq!(salt.len(), 32);
        assert_eq!(hash.len(), 64);
        assert!(
            encoded
                .chars()
                .all(|character| character == ':' || character.is_ascii_hexdigit())
        );
    }
}
