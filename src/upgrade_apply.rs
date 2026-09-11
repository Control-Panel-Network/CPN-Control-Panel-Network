//! Guest-aware package download/apply for maintenance (upgrade / repair / downgrade).

use crate::installer::AppState;
use crate::manifest::{ManifestSource, installer_bin};
use crate::release_verify::{
    maybe_check_rpm_sig, verify_gpg_enabled, verify_gpg_sums, verify_release_enabled,
    verify_sha256_file,
};
use crate::releases::{self, CpnRelease, NativePackageKind};
use rand::{Rng, distr::Alphanumeric};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::process::Command;

fn ephemeral_path(filename: &str) -> Result<String, String> {
    let suffix: String = rand::rng()
        .sample_iter(&Alphanumeric)
        .take(12)
        .map(char::from)
        .collect();
    let dir = format!("/var/tmp/cpn-upgrade-{suffix}");
    std::fs::create_dir_all(&dir).map_err(|error| format!("Could not create temp dir: {error}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700));
    }
    Ok(format!("{dir}/{filename}"))
}

async fn download_file(url: &str, destination: &str) -> Result<(), String> {
    let status = Command::new("curl")
        .args([
            "--fail",
            "--location",
            "--silent",
            "--show-error",
            "--max-time",
            "120",
            "--output",
            destination,
            url,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .status()
        .await
        .map_err(|error| format!("Download failed: {error}"))?;
    if !status.success() {
        return Err("Download of package asset failed".into());
    }
    Ok(())
}

fn bundled_gpg_keyring() -> Option<PathBuf> {
    let candidates = [
        PathBuf::from("packaging/RPM-GPG-KEY-CPN"),
        PathBuf::from("/usr/share/cpn/RPM-GPG-KEY-CPN"),
        PathBuf::from("/etc/pki/rpm-gpg/RPM-GPG-KEY-CPN"),
    ];
    candidates.into_iter().find(|path| path.is_file())
}

async fn verify_downloaded_artifact(
    state: &AppState,
    release: &CpnRelease,
    artifact_path: &str,
) -> Result<(), String> {
    if !verify_release_enabled() {
        state.log(
            "CPN_VERIFY_RELEASE disabled; skipping release artifact verification",
            "info",
        );
        return Ok(());
    }

    let sums_asset = release.checksums_asset.as_ref().ok_or_else(|| {
        "Release is missing SHA256SUMS. Refuse install while CPN_VERIFY_RELEASE is enabled (set CPN_VERIFY_RELEASE=0 only for local lab builds)."
            .to_string()
    })?;

    state
        .progress("verifying", 35, "Downloading SHA256SUMS")
        .await;
    let sums_path = ephemeral_path("SHA256SUMS")?;
    download_file(&sums_asset.browser_download_url, &sums_path).await?;
    let sums_body = std::fs::read_to_string(&sums_path)
        .map_err(|error| format!("Could not read SHA256SUMS: {error}"))?;
    verify_sha256_file(Path::new(artifact_path), &sums_body)?;
    state.log(
        format!(
            "SHA-256 verified for {}",
            Path::new(artifact_path).display()
        ),
        "info",
    );

    if verify_gpg_enabled() {
        let asc = release.checksums_asc_asset.as_ref().ok_or_else(|| {
            "Release is missing SHA256SUMS.asc while CPN_VERIFY_GPG=1".to_string()
        })?;
        let asc_path = ephemeral_path("SHA256SUMS.asc")?;
        download_file(&asc.browser_download_url, &asc_path).await?;
        let keyring = bundled_gpg_keyring();
        verify_gpg_sums(
            Path::new(&sums_path),
            Path::new(&asc_path),
            keyring.as_deref(),
        )
        .await?;
        state.log("GPG verified SHA256SUMS.asc", "info");
        let _ = std::fs::remove_file(&asc_path);
    }

    maybe_check_rpm_sig(Path::new(artifact_path)).await?;
    let _ = std::fs::remove_file(&sums_path);
    Ok(())
}

/// Download and install the package that matches this guest OS.
///
/// Never uses `release.rpm_asset` alone: GitHub often lists `.el10` first, which
/// breaks AlmaLinux 9 labs (`libc.so.6(GLIBC_2.39)` / wrong `.elN`).
pub async fn apply_release(
    state: &AppState,
    release: &CpnRelease,
    force: bool,
    allow_oldpackage: bool,
) -> Result<ManifestSource, String> {
    let guest = crate::os_support::detect_guest_os().ok();
    if let Some(guest) = guest.as_ref()
        && let Some((asset, kind)) = releases::compatible_package_asset(release, guest)
    {
        match kind {
            NativePackageKind::Rpm => {
                state
                    .progress("downloading", 20, format!("Downloading {}", asset.name))
                    .await;
                let path = ephemeral_path(&asset.name)?;
                download_file(&asset.browser_download_url, &path).await?;
                verify_downloaded_artifact(state, release, &path).await?;
                state
                    .progress(
                        "installing",
                        60,
                        format!("Installing RPM ({})", asset.name),
                    )
                    .await;
                crate::upgrade_pkg::install_rpm(&path, force, allow_oldpackage).await?;
                let _ = std::fs::remove_file(&path);
                return Ok(ManifestSource::Rpm);
            }
            NativePackageKind::Deb => {
                return Err(format!(
                    "Release {} has a DEB for this host, but DEB apply is not wired yet. Use the binary asset or build from source.",
                    release.tag_name
                ));
            }
        }
    }
    if let Some(bin) = &release.binary_asset {
        state
            .progress("downloading", 20, format!("Downloading {}", bin.name))
            .await;
        let path = ephemeral_path("cpn-installer.bin")?;
        download_file(&bin.browser_download_url, &path).await?;
        verify_downloaded_artifact(state, release, &path).await?;
        state
            .progress("installing", 60, "Replacing cpn-installer binary")
            .await;
        crate::upgrade_pkg::install_binary(&path, installer_bin()).await?;
        let _ = std::fs::remove_file(&path);
        return Ok(ManifestSource::Binary);
    }
    let available: Vec<&str> = release
        .assets
        .iter()
        .map(|a| a.name.as_str())
        .filter(|n| n.ends_with(".rpm") || n.ends_with(".deb") || *n == "cpn-installer")
        .collect();
    let guest_note = guest
        .as_ref()
        .map(|g| format!("el{}/{}", g.major, g.label))
        .unwrap_or_else(|| "unknown guest".into());
    Err(format!(
        "Release {} has no compatible package for {guest_note}. Available: {}. See to-do/UPGRADE-REPAIR.md.",
        release.tag_name,
        if available.is_empty() {
            "(none)".into()
        } else {
            available.join(", ")
        }
    ))
}
