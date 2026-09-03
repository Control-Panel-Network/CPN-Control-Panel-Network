//! Webmail download and activation helpers.
use crate::install_recipes::command;
use crate::installer::{AppState, install_php_runtime, run_command};
use crate::model::MailSystem;
use rand::{Rng, distr::Alphanumeric};
use std::{os::unix::fs::symlink, path::Path, process::Stdio};
use tokio::{io::AsyncReadExt, process::Command};

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

pub(crate) async fn install_webmail(state: &AppState, mail: MailSystem) -> Result<(), String> {
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
