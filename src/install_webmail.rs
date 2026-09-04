//! Webmail download and activation helpers.
use crate::install_webmail_runtime::configure_webmail_runtime;
use crate::installer::{AppState, install_php_runtime};
use crate::model::{MailSystem, ServerEngine};
use rand::{Rng, distr::Alphanumeric};
use sha2::{Digest, Sha256};
use std::{fs::OpenOptions, io::Write, path::Path, process::Stdio};
use tokio::{io::AsyncReadExt, process::Command};

const SNAPPYMAIL_SHA256: &str = "71f1d8a9065cc9cf7ddd064f5c47cc7b255cb70e6a56713647fc73d4b79e33ec";
const ROUNDCUBE_SHA256: &str = "443cde2ea03b840ce4701fe23c273f01e68702f176d282e60248236bbb5f5f85";

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
    let path = format!("{dir}/{filename}");
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options
        .open(&path)
        .map_err(|error| format!("No se pudo crear el archivo temporal de forma segura: {error}"))?
        .write_all(b"")
        .map_err(|error| error.to_string())?;
    Ok(path)
}

fn verify_sha256(path: &str, expected_hex: &str) -> Result<(), String> {
    let bytes = std::fs::read(path).map_err(|error| format!("No se pudo leer {path}: {error}"))?;
    let digest = Sha256::digest(&bytes);
    let actual = digest
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    if !actual.eq_ignore_ascii_case(expected_hex) {
        return Err(format!(
            "Integridad fallida para {path}: sha256 esperado {expected_hex}, obtenido {actual}"
        ));
    }
    Ok(())
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
    let work = async {
        let mut child = Command::new("curl")
            .args([
                "--fail",
                "--location",
                "--proto",
                "=https",
                "--proto-redir",
                "=https",
                "--max-time",
                "600",
                "--connect-timeout",
                "30",
                "--progress-bar",
                "--output",
                destination,
                url,
            ])
            .kill_on_drop(true)
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
        Ok::<(), String>(())
    };
    match tokio::time::timeout(std::time::Duration::from_secs(620), work).await {
        Ok(result) => result?,
        Err(_) => return Err(format!("Tiempo de espera agotado descargando {label}")),
    }
    state
        .progress("downloading", end, format!("{label} descargado"))
        .await;
    Ok(())
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

pub(crate) async fn install_webmail(
    state: &AppState,
    mail: MailSystem,
    engine: ServerEngine,
) -> Result<(), String> {
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
            verify_sha256(&archive, SNAPPYMAIL_SHA256)?;
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
            configure_webmail_runtime(state, "/opt/cpn-webmail/snappymail", engine).await?;
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
            verify_sha256(&archive, ROUNDCUBE_SHA256)?;
            install_php_runtime(state, "PHP para Roundcube").await?;
            let pdo = Command::new("bash")
                .args(["-c", "php -m | grep -qi pdo_sqlite"])
                .kill_on_drop(true)
                .status()
                .await
                .map_err(|error| error.to_string())?;
            if !pdo.success() {
                return Err("Falta la extensión PHP pdo_sqlite requerida por Roundcube".into());
            }
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
            // Hosts match provisioned Postfix submission (:587) and Dovecot IMAP (:143).
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
            let sql = "/opt/cpn-webmail/roundcube/SQL/sqlite.initial.sql";
            if !Path::new(sql).exists() {
                return Err("No se encontró SQL/sqlite.initial.sql de Roundcube".into());
            }
            let init = Command::new("php")
                .args([
                    "-r",
                    "$db=new PDO('sqlite:/opt/cpn-webmail/roundcube/db.sqlite'); $n=$db->query(\"SELECT name FROM sqlite_master WHERE type='table' AND name='users'\")->fetchColumn(); if(!$n){ $sql=file_get_contents('/opt/cpn-webmail/roundcube/SQL/sqlite.initial.sql'); $db->exec($sql); $n=$db->query(\"SELECT name FROM sqlite_master WHERE type='table' AND name='users'\")->fetchColumn(); } if(!$n){fwrite(STDERR,\"missing users table\\n\"); exit(1);}",
                ])
                .kill_on_drop(true)
                .status()
                .await
                .map_err(|error| error.to_string())?;
            if !init.success() {
                return Err("No se pudo inicializar el esquema SQLite de Roundcube".into());
            }
            configure_webmail_runtime(state, "/opt/cpn-webmail/roundcube/public_html", engine)
                .await?;
        }
        MailSystem::Thunderbird => unreachable!(),
    }
    Ok(())
}
