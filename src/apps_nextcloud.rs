//! Nextcloud host package + NextSnapMail Nextcloud app install helpers.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub const NEXTCLOUD_ROOT: &str = "/opt/nextcloud";
const NEXTCLOUD_URL: &str = "https://download.nextcloud.com/server/releases/latest.tar.bz2";
const NEXTSNAP_ZIP: &str = "https://github.com/oe79/NextSnapMail/archive/refs/heads/master.zip";

pub fn nextcloud_present() -> bool {
    Path::new(NEXTCLOUD_ROOT).join("version.php").is_file()
        || Path::new("/var/www/nextcloud/version.php").is_file()
        || Path::new("/usr/share/nextcloud/version.php").is_file()
        || std::env::var_os("CPN_NEXTCLOUD_ROOT").is_some()
}

pub fn nextcloud_root() -> PathBuf {
    if let Ok(custom) = std::env::var("CPN_NEXTCLOUD_ROOT") {
        let p = PathBuf::from(custom);
        if p.join("version.php").is_file() {
            return p;
        }
    }
    for candidate in [NEXTCLOUD_ROOT, "/var/www/nextcloud", "/usr/share/nextcloud"] {
        let p = Path::new(candidate);
        if p.join("version.php").is_file() {
            return p.to_path_buf();
        }
    }
    PathBuf::from(NEXTCLOUD_ROOT)
}

pub fn nextsnapmail_app_present() -> bool {
    let root = nextcloud_root();
    root.join("apps/nextsnapmail/appinfo/info.xml").is_file()
        || root.join("apps/nextsnapmail/appinfo/info.php").is_file()
        || root.join("apps/nextsnapmail").is_dir()
            && root.join("apps/nextsnapmail/appinfo").is_dir()
}

pub fn detect_nextcloud_status() -> (bool, String) {
    if nextcloud_present() {
        (
            true,
            format!("Nextcloud detected at {}.", nextcloud_root().display()),
        )
    } else {
        (
            false,
            "Nextcloud not detected. CPN can install files under /opt/nextcloud (operator must finish OCC/web setup)."
                .into(),
        )
    }
}

fn run_checked(program: &str, args: &[&str], label: &str) -> Result<(), String> {
    let status = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .status()
        .map_err(|e| format!("{label}: failed to start {program}: {e}"))?;
    if !status.success() {
        return Err(format!("{label}: {program} exited with {status}"));
    }
    Ok(())
}

fn ephemeral_dir(prefix: &str) -> Result<PathBuf, String> {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let root = if Path::new("/var/tmp").is_dir() {
        Path::new("/var/tmp")
    } else {
        Path::new("/tmp")
    };
    let dir = root.join(format!("{prefix}-{suffix}"));
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&dir, fs::Permissions::from_mode(0o700));
    }
    Ok(dir)
}

/// Download Nextcloud server files under `/opt/nextcloud` (does not run OCC wizard).
pub fn install_nextcloud_files() -> Result<String, String> {
    if nextcloud_present() {
        return Ok(format!(
            "Nextcloud already present at {}.",
            nextcloud_root().display()
        ));
    }
    let work = ephemeral_dir("cpn-nc")?;
    let archive = work.join("nextcloud.tar.bz2");
    let archive_s = archive.to_string_lossy().to_string();
    run_checked(
        "curl",
        &[
            "--fail",
            "--location",
            "--proto",
            "=https",
            "--proto-redir",
            "=https",
            "--max-time",
            "900",
            "--connect-timeout",
            "30",
            "--output",
            &archive_s,
            NEXTCLOUD_URL,
        ],
        "Downloading Nextcloud",
    )?;
    let extract_parent = work.join("extract");
    fs::create_dir_all(&extract_parent).map_err(|e| e.to_string())?;
    run_checked(
        "tar",
        &["xjf", &archive_s, "-C", &extract_parent.to_string_lossy()],
        "Extracting Nextcloud",
    )?;
    let extracted = extract_parent.join("nextcloud");
    if !extracted.join("version.php").is_file() {
        let _ = fs::remove_dir_all(&work);
        return Err(
            "Nextcloud archive did not contain version.php after extract. Check download.nextcloud.com."
                .into(),
        );
    }
    if Path::new(NEXTCLOUD_ROOT).exists() {
        fs::remove_dir_all(NEXTCLOUD_ROOT)
            .map_err(|e| format!("Could not replace {NEXTCLOUD_ROOT}: {e}"))?;
    }
    if fs::rename(&extracted, NEXTCLOUD_ROOT).is_err() {
        // Cross-device rename fallback.
        let status = Command::new("cp")
            .args(["-a", &extracted.to_string_lossy(), NEXTCLOUD_ROOT])
            .status()
            .map_err(|e| format!("Could not copy Nextcloud into {NEXTCLOUD_ROOT}: {e}"))?;
        if !status.success() {
            return Err(format!("Could not move Nextcloud into {NEXTCLOUD_ROOT}"));
        }
    }
    let _ = fs::remove_dir_all(&work);
    Ok(format!(
        "Installed Nextcloud files under {NEXTCLOUD_ROOT}. Finish OCC/web setup (DB, admin) before enabling apps in production. NextSnapMail can be installed next."
    ))
}

/// Install NextSnapMail into the Nextcloud `apps/` tree (requires Nextcloud files).
pub fn install_nextsnapmail_app() -> Result<String, String> {
    let mut notes = Vec::new();
    if !nextcloud_present() {
        notes.push(install_nextcloud_files()?);
    }
    if nextsnapmail_app_present() {
        notes.push("NextSnapMail app already present under Nextcloud apps/.".into());
        return Ok(notes.join(" "));
    }
    let root = nextcloud_root();
    let apps = root.join("apps");
    fs::create_dir_all(&apps).map_err(|e| format!("Could not create {}: {e}", apps.display()))?;
    let work = ephemeral_dir("cpn-nsm")?;
    let zip = work.join("nextsnapmail.zip");
    let zip_s = zip.to_string_lossy().to_string();
    run_checked(
        "curl",
        &[
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
            "--output",
            &zip_s,
            NEXTSNAP_ZIP,
        ],
        "Downloading NextSnapMail",
    )?;
    let extract = work.join("extract");
    fs::create_dir_all(&extract).map_err(|e| e.to_string())?;
    run_checked(
        "unzip",
        &["-q", &zip_s, "-d", &extract.to_string_lossy()],
        "Extracting NextSnapMail",
    )
    .or_else(|_| {
        // Fallback when unzip is missing: use bsdtar / tar if available.
        run_checked(
            "tar",
            &["xf", &zip_s, "-C", &extract.to_string_lossy()],
            "Extracting NextSnapMail (tar)",
        )
    })?;
    let mut source: Option<PathBuf> = None;
    if let Ok(rd) = fs::read_dir(&extract) {
        for ent in rd.flatten() {
            let p = ent.path();
            if p.is_dir()
                && (p.join("appinfo").is_dir()
                    || p.file_name().is_some_and(|n| {
                        n.to_string_lossy()
                            .to_ascii_lowercase()
                            .contains("nextsnap")
                    }))
            {
                source = Some(p);
                break;
            }
        }
    }
    let source = source.ok_or_else(|| {
        "NextSnapMail zip did not contain an app directory with appinfo/.".to_string()
    })?;
    let dest = apps.join("nextsnapmail");
    if dest.exists() {
        fs::remove_dir_all(&dest).map_err(|e| e.to_string())?;
    }
    fs::rename(&source, &dest).or_else(|_| {
        let status = Command::new("cp")
            .args(["-a", &source.to_string_lossy(), &dest.to_string_lossy()])
            .status()
            .map_err(|e| e.to_string())?;
        if status.success() {
            Ok(())
        } else {
            Err(format!(
                "Could not copy NextSnapMail into {}",
                dest.display()
            ))
        }
    })?;
    let _ = fs::remove_dir_all(&work);
    // Best-effort enable via occ when available.
    let occ = root.join("occ");
    if occ.is_file() {
        let _ = Command::new("php")
            .args([occ.to_string_lossy().as_ref(), "app:enable", "nextsnapmail"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    notes.push(format!(
        "Installed NextSnapMail under {}/apps/nextsnapmail. Enable the app in Nextcloud (occ app:enable nextsnapmail) after Nextcloud setup completes.",
        root.display()
    ));
    Ok(notes.join(" "))
}

pub fn uninstall_nextsnapmail_app() -> Result<String, String> {
    let dest = nextcloud_root().join("apps/nextsnapmail");
    if dest.exists() {
        fs::remove_dir_all(&dest)
            .map_err(|e| format!("Could not remove {}: {e}", dest.display()))?;
        Ok(format!("Removed NextSnapMail from {}.", dest.display()))
    } else {
        Ok("NextSnapMail app was not present.".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roots_are_absolute() {
        assert!(NEXTCLOUD_ROOT.starts_with('/'));
        assert!(!NEXTCLOUD_URL.is_empty());
    }
}
