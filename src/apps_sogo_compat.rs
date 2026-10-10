//! SOGo on EL10+: runtime libraries the Inverse EL9 build links against but the host no
//! longer ships under the same soname (today only `libsodium.so.23`; EL10 ships `.so.26`).
//!
//! dnf cannot help here: `libsodium` from EPEL 9 and `libsodium` from EL10 are the same
//! package name, so dnf always resolves the newer EL10 build. Instead CPN downloads the
//! signed EPEL 9 RPM through the scoped `[cpn-sogo-el9-compat]` repo section, verifies its
//! GPG signature with `rpm -K`, extracts only the shared library files, and places them in a
//! CPN-owned directory wired through `/etc/ld.so.conf.d/`. Nothing in `/usr/lib64` is
//! touched; uninstall removes the directory and the loader entry again.

use std::path::Path;
use std::process::{Command, Stdio};

/// Private directory for compat shared libraries (never overlaps distro files).
pub const COMPAT_LIB_DIR: &str = "/usr/lib64/cpn-sogo-compat";
/// Loader configuration that adds `COMPAT_LIB_DIR` to the search path.
pub const COMPAT_LD_CONF: &str = "/etc/ld.so.conf.d/cpn-sogo-compat.conf";
const COMPAT_REPO_ID: &str = "cpn-sogo-el9-compat";
const EPEL9_GPGKEY: &str = "https://dl.fedoraproject.org/pub/epel/RPM-GPG-KEY-EPEL-9";
const WORK_DIR: &str = "/var/lib/cpn/sogo/compat";

fn set_mode(path: &str, mode: u32) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode));
    }
    #[cfg(not(unix))]
    {
        let _ = (path, mode);
    }
}

fn symlink(target: &Path, link: &str) -> Result<(), String> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link).map_err(|e| format!("Could not link {link}: {e}"))
    }
    #[cfg(not(unix))]
    {
        let _ = target;
        Err(format!(
            "Symlinks for {link} are only created on Linux hosts."
        ))
    }
}

fn run(program: &str, args: &[&str]) -> Result<String, String> {
    let out = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("Could not start {program}: {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        let tail: String = err.lines().rev().take(3).collect::<Vec<_>>().join(" ");
        return Err(format!(
            "{program} {} failed: {}",
            args.join(" "),
            tail.chars().take(300).collect::<String>()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

/// Shared objects the SOGo daemon or its frameworks cannot resolve (`ldd` "not found").
pub fn missing_runtime_libs() -> Vec<String> {
    let mut targets = vec!["/usr/sbin/sogod".to_string()];
    for dir in ["/usr/lib64/GNUstep/Frameworks/SOGo.framework/Versions/5/sogo"] {
        if let Ok(rd) = std::fs::read_dir(dir) {
            for entry in rd.flatten() {
                let p = entry.path();
                if p.extension().is_some_and(|e| e == "so") {
                    targets.push(p.to_string_lossy().to_string());
                }
            }
        }
    }
    let mut missing: Vec<String> = Vec::new();
    for target in targets {
        if !Path::new(&target).is_file() {
            continue;
        }
        let Ok(out) = Command::new("ldd")
            .arg(&target)
            .stdin(Stdio::null())
            .output()
        else {
            continue;
        };
        for line in String::from_utf8_lossy(&out.stdout).lines() {
            if let Some(lib) = line.trim().strip_suffix("=> not found") {
                let lib = lib.trim().to_string();
                if !lib.is_empty() && !missing.contains(&lib) {
                    missing.push(lib);
                }
            }
        }
    }
    missing
}

/// Validate a URL returned by `dnf repoquery --location` before downloading it.
pub fn location_is_trusted(url: &str) -> bool {
    url.starts_with("https://dl.fedoraproject.org/pub/epel/9/")
        && url.ends_with(".rpm")
        && url.contains("/libsodium-")
        && !url.contains("..")
}

fn provide_from_epel9(pkg: &str) -> Result<String, String> {
    std::fs::create_dir_all(WORK_DIR).map_err(|e| format!("Could not create {WORK_DIR}: {e}"))?;
    set_mode(WORK_DIR, 0o700);
    let listing = run(
        "dnf",
        &[
            "-q",
            "repoquery",
            "--repo",
            COMPAT_REPO_ID,
            "--location",
            pkg,
        ],
    )?;
    let url = listing
        .lines()
        .map(str::trim)
        .find(|l| location_is_trusted(l))
        .ok_or_else(|| {
            format!("EPEL 9 did not offer {pkg} through the {COMPAT_REPO_ID} repo section.")
        })?
        .to_string();
    let rpm_path = format!("{WORK_DIR}/{pkg}.el9.rpm");
    run(
        "curl",
        &[
            "--fail",
            "--silent",
            "--location",
            "--proto",
            "=https",
            "--max-time",
            "120",
            "-o",
            &rpm_path,
            &url,
        ],
    )?;
    let _ = run("rpm", &["--import", EPEL9_GPGKEY]);
    let check = run("rpm", &["-K", &rpm_path])?;
    if !check.to_lowercase().contains("signatures ok") {
        let _ = std::fs::remove_file(&rpm_path);
        return Err(format!(
            "Signature check failed for {pkg} from EPEL 9; refusing to install an unverified runtime library."
        ));
    }
    let extract_dir = format!("{WORK_DIR}/{pkg}");
    let _ = std::fs::remove_dir_all(&extract_dir);
    std::fs::create_dir_all(&extract_dir)
        .map_err(|e| format!("Could not create {extract_dir}: {e}"))?;
    let script = format!(
        "set -o pipefail; cd '{extract_dir}' && rpm2cpio '{rpm_path}' | cpio -idm --quiet './usr/lib64/*.so.*'"
    );
    run("bash", &["-c", &script])?;
    std::fs::create_dir_all(COMPAT_LIB_DIR)
        .map_err(|e| format!("Could not create {COMPAT_LIB_DIR}: {e}"))?;
    let mut copied = Vec::new();
    let lib_src = format!("{extract_dir}/usr/lib64");
    for entry in std::fs::read_dir(&lib_src)
        .map_err(|e| format!("Extracted {pkg} has no usr/lib64: {e}"))?
        .flatten()
    {
        let src = entry.path();
        let Some(name) = src.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let dst = format!("{COMPAT_LIB_DIR}/{name}");
        let _ = std::fs::remove_file(&dst);
        if let Ok(target) = std::fs::read_link(&src) {
            symlink(&target, &dst)?;
        } else {
            std::fs::copy(&src, &dst).map_err(|e| format!("Could not copy {name}: {e}"))?;
            set_mode(&dst, 0o755);
        }
        copied.push(name.to_string());
    }
    let _ = std::fs::remove_dir_all(&extract_dir);
    let _ = std::fs::remove_file(&rpm_path);
    if copied.is_empty() {
        return Err(format!(
            "No shared libraries found inside the EPEL 9 {pkg} package."
        ));
    }
    std::fs::write(COMPAT_LD_CONF, format!("{COMPAT_LIB_DIR}\n"))
        .map_err(|e| format!("Could not write {COMPAT_LD_CONF}: {e}"))?;
    run("ldconfig", &[])?;
    copied.sort();
    Ok(copied.join(", "))
}

/// Resolve missing EL9-build runtime libraries on this host. No-op when `ldd` is clean.
pub fn ensure_runtime_libs() -> Result<Option<String>, String> {
    let missing = missing_runtime_libs();
    if missing.is_empty() {
        return Ok(None);
    }
    let mut notes = Vec::new();
    for lib in &missing {
        let pkg = if lib.starts_with("libsodium.so.") {
            "libsodium"
        } else {
            return Err(format!(
                "sogod needs {lib}, which this host does not provide and CPN has no verified EL9 source for. Set a repository with a native build in /var/lib/cpn/sogo/repo-override.json and retry."
            ));
        };
        let files = provide_from_epel9(pkg)?;
        notes.push(format!(
            "{lib} via signed EPEL 9 {pkg} ({files}) in {COMPAT_LIB_DIR}"
        ));
    }
    let still_missing = missing_runtime_libs();
    if !still_missing.is_empty() {
        return Err(format!(
            "sogod still cannot resolve {} after the compat step; see journalctl -u sogod.",
            still_missing.join(", ")
        ));
    }
    Ok(Some(notes.join("; ")))
}

/// Remove the compat directory and loader entry (uninstall path).
pub fn remove_runtime_libs() {
    let _ = std::fs::remove_dir_all(COMPAT_LIB_DIR);
    let _ = std::fs::remove_file(COMPAT_LD_CONF);
    let _ = std::fs::remove_dir_all(WORK_DIR);
    let _ = run("ldconfig", &[]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trusted_locations_are_epel9_libsodium_only() {
        assert!(location_is_trusted(
            "https://dl.fedoraproject.org/pub/epel/9/Everything/x86_64/Packages/l/libsodium-1.0.18-9.el9.x86_64.rpm"
        ));
        assert!(!location_is_trusted(
            "http://dl.fedoraproject.org/pub/epel/9/Everything/x86_64/Packages/l/libsodium-1.0.18-9.el9.x86_64.rpm"
        ));
        assert!(!location_is_trusted(
            "https://dl.fedoraproject.org/pub/epel/9/Everything/x86_64/Packages/o/openssl-3.rpm"
        ));
        assert!(!location_is_trusted(
            "https://mirror.example/pub/epel/9/Everything/x86_64/Packages/l/libsodium-1.0.18-9.el9.x86_64.rpm"
        ));
    }

    #[test]
    fn missing_libs_probe_does_not_panic() {
        let _ = missing_runtime_libs();
    }
}
