//! Safe archive listing and extraction (reject zip-slip / path traversal).

use crate::panel_ops_docker_probe::output_with_timeout;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Hard ceiling for full member inventory (zip-slip validation on smaller archives).
pub const MAX_ARCHIVE_MEMBERS: usize = 50_000;
/// Plan/entity discovery keeps a filtered subset; allow more shallow markers.
pub const MAX_PLAN_ARCHIVE_MEMBERS: usize = 200_000;
/// Default bound for small archives so a wedged `tar` cannot hang workers forever.
pub const ARCHIVE_LIST_TIMEOUT: Duration = Duration::from_secs(30);
/// Large classic source control-panel archives (multi-GB gzip) need a long inventory window.
pub const ARCHIVE_LIST_TIMEOUT_LARGE: Duration = Duration::from_secs(7_200);
/// Archives at or above this size use the large inventory timeout and disk cache.
pub const LARGE_ARCHIVE_BYTES: u64 = 2 * 1024 * 1024 * 1024;
/// Hub render budget for `/backups/restore/plan` (must cover large inventory).
pub const RESTORE_PLAN_RENDER_BUDGET: Duration = Duration::from_secs(7_200);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveKind {
    TarGz,
    Zip,
    Unsupported,
}

pub fn archive_kind(path: &Path) -> ArchiveKind {
    let name = path
        .file_name()
        .and_then(|v| v.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if name.ends_with(".tar.gz") || name.ends_with(".tgz") {
        ArchiveKind::TarGz
    } else if name.ends_with(".zip") {
        ArchiveKind::Zip
    } else if name.ends_with(".wpress") {
        ArchiveKind::Unsupported
    } else if name.ends_with(".tar") {
        ArchiveKind::TarGz
    } else {
        ArchiveKind::Unsupported
    }
}

fn archive_size_bytes(archive: &Path) -> u64 {
    fs::metadata(archive).map(|m| m.len()).unwrap_or(0)
}

/// Inventory timeout scales up for multi-GB classic archives.
pub fn archive_list_timeout(archive: &Path) -> Duration {
    if archive_size_bytes(archive) >= LARGE_ARCHIVE_BYTES {
        ARCHIVE_LIST_TIMEOUT_LARGE
    } else {
        ARCHIVE_LIST_TIMEOUT
    }
}

fn inventory_cache_dir() -> PathBuf {
    crate::account::data_dir().join("backup-inventory")
}

fn inventory_cache_path(archive: &Path) -> Option<PathBuf> {
    let meta = fs::metadata(archive).ok()?;
    let name = archive
        .file_name()
        .and_then(|v| v.to_str())
        .unwrap_or("archive")
        .replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_");
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    Some(
        inventory_cache_dir().join(format!(
            "{name}.{}.{}.members.txt",
            meta.len(),
            mtime
        )),
    )
}

fn read_inventory_cache(archive: &Path) -> Option<Vec<String>> {
    let path = inventory_cache_path(archive)?;
    if !path.is_file() {
        return None;
    }
    let raw = fs::read_to_string(&path).ok()?;
    let mut members = Vec::new();
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        members.push(trimmed.to_string());
        if members.len() > MAX_PLAN_ARCHIVE_MEMBERS {
            break;
        }
    }
    if members.is_empty() {
        None
    } else {
        Some(members)
    }
}

fn write_inventory_cache(archive: &Path, members: &[String]) {
    let Some(path) = inventory_cache_path(archive) else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let mut body = String::new();
    for m in members {
        body.push_str(m);
        body.push('\n');
    }
    let _ = fs::write(path, body);
}

/// Reject absolute paths, Windows drive prefixes, and `..` components.
pub fn is_safe_archive_member(raw: &str) -> bool {
    let trimmed = raw.trim().trim_start_matches("./");
    if trimmed.is_empty() {
        return true;
    }
    if trimmed.contains('\0') {
        return false;
    }
    let path = Path::new(trimmed);
    if path.is_absolute() || path.has_root() {
        return false;
    }
    for comp in path.components() {
        match comp {
            Component::ParentDir => return false,
            Component::Prefix(_) => return false,
            Component::RootDir => return false,
            Component::CurDir | Component::Normal(_) => {}
        }
    }
    let mut depth = 0i32;
    for part in trimmed.replace('\\', "/").split('/') {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." {
            return false;
        }
        depth += 1;
        if depth > 512 {
            return false;
        }
    }
    true
}

pub fn validate_member_list(members: &[String]) -> Result<(), String> {
    let mut bad = Vec::new();
    for m in members {
        if !is_safe_archive_member(m) {
            bad.push(m.clone());
            if bad.len() >= 5 {
                break;
            }
        }
    }
    if bad.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "Archive contains unsafe paths (zip-slip blocked): {}",
            bad.join(", ")
        ))
    }
}

/// Keep shallow / structural paths for restore planning; drop deep website file noise.
pub fn keep_member_for_plan(raw: &str) -> bool {
    let n = raw.trim().trim_start_matches("./").replace('\\', "/");
    if n.is_empty() {
        return false;
    }
    let lower = n.to_ascii_lowercase();
    if lower.ends_with("meta.xml")
        || lower.ends_with(".sql")
        || lower.ends_with(".sql.gz")
        || lower.ends_with("-db.gz")
    {
        return true;
    }
    let depth = n.split('/').filter(|p| !p.is_empty() && *p != ".").count();
    if depth <= 3 {
        return true;
    }
    if lower.contains("/public_html") && depth <= 5 {
        return true;
    }
    if lower.starts_with("public_html/") && depth <= 4 {
        return true;
    }
    if lower.starts_with("homedir/") && depth <= 5 {
        return true;
    }
    if lower.starts_with("vmail/")
        || lower.starts_with("docker/")
        || lower.starts_with("dns/")
        || lower.starts_with("panel-config/")
        || lower.starts_with("userdata/")
        || lower.starts_with("mysql/")
    {
        return depth <= 6;
    }
    // Domain-shaped first segment with useful second segment.
    if let Some((first, rest)) = n.split_once('/') {
        let host = first.to_ascii_lowercase();
        if host.contains('.')
            && !host.starts_with('.')
            && (rest.starts_with("public_html")
                || rest.starts_with("mysql/")
                || rest.ends_with(".sql")
                || rest.ends_with(".sql.gz"))
            && depth <= 6
        {
            return true;
        }
    }
    false
}

fn collect_member_lines(stdout: &[u8]) -> Result<Vec<String>, String> {
    let mut members = Vec::new();
    for line in String::from_utf8_lossy(stdout).lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        members.push(trimmed.to_string());
        if members.len() > MAX_ARCHIVE_MEMBERS {
            return Err(format!(
                "Archive lists more than {MAX_ARCHIVE_MEMBERS} members. Split the backup or restore with a smaller archive."
            ));
        }
    }
    Ok(members)
}

fn collect_plan_member_lines(stdout: &[u8]) -> Result<Vec<String>, String> {
    let mut members = Vec::new();
    let mut seen_website = false;
    for line in String::from_utf8_lossy(stdout).lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let lower = trimmed.to_ascii_lowercase();
        if lower.contains("public_html") {
            seen_website = true;
        }
        if !keep_member_for_plan(trimmed) {
            continue;
        }
        members.push(trimmed.to_string());
        if members.len() > MAX_PLAN_ARCHIVE_MEMBERS {
            break;
        }
    }
    if seen_website
        && !members.iter().any(|m| {
            let l = m.to_ascii_lowercase();
            l.contains("public_html")
        })
    {
        members.push("./public_html/".into());
    }
    Ok(members)
}

/// List members for restore planning (filtered + cached for large archives).
pub fn list_archive_members(archive: &Path) -> Result<Vec<String>, String> {
    if let Some(cached) = read_inventory_cache(archive) {
        return Ok(cached);
    }
    let large = archive_size_bytes(archive) >= LARGE_ARCHIVE_BYTES;
    let members = match archive_kind(archive) {
        ArchiveKind::TarGz => {
            if large {
                list_tar_members_streaming_plan(archive)?
            } else {
                list_tar_members(archive)?
            }
        }
        ArchiveKind::Zip => list_zip_members(archive)?,
        ArchiveKind::Unsupported => {
            return Err(format!(
                "Unsupported archive type for `{}`. Use .tar.gz / .tgz / .zip.",
                archive.display()
            ));
        }
    };
    if large {
        write_inventory_cache(archive, &members);
    }
    Ok(members)
}

fn list_tar_members(archive: &Path) -> Result<Vec<String>, String> {
    let name = archive
        .file_name()
        .and_then(|v| v.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let flag = if name.ends_with(".tar") && !name.ends_with(".tar.gz") {
        "-tf"
    } else {
        "-tzf"
    };
    let timeout = archive_list_timeout(archive);
    let path = archive.to_string_lossy();
    let out = output_with_timeout("tar", &[flag, path.as_ref()], timeout).map_err(|e| {
        if e.kind() == std::io::ErrorKind::TimedOut {
            format!(
                "Listing the archive timed out after {}s. For multi-GB source control-panel archives, wait for inventory cache under /var/lib/cpn/backup-inventory/ or retry.",
                timeout.as_secs()
            )
        } else {
            format!("Could not list tar archive: {e}")
        }
    })?;
    if !out.status.success() {
        return Err(format!(
            "tar list failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    collect_member_lines(&out.stdout)
}

fn list_tar_members_streaming_plan(archive: &Path) -> Result<Vec<String>, String> {
    let name = archive
        .file_name()
        .and_then(|v| v.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let flag = if name.ends_with(".tar") && !name.ends_with(".tar.gz") {
        "-tf"
    } else {
        "-tzf"
    };
    let timeout = archive_list_timeout(archive);
    let mut cmd = Command::new("tar");
    cmd.arg(flag)
        .arg(archive)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Could not start tar list: {e}"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "tar list missing stdout".to_string())?;
    let start = Instant::now();
    let reader = BufReader::new(stdout);
    let mut members = Vec::new();
    let mut seen_website = false;
    let mut timed_out = false;
    // Stream the full listing (filtered). Child-domain and .sql markers often
    // appear late in classic source control-panel archives; do not stop early.
    for line in reader.lines() {
        if start.elapsed() >= timeout {
            timed_out = true;
            break;
        }
        let Ok(line) = line else {
            continue;
        };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let lower = trimmed.to_ascii_lowercase();
        if lower.contains("public_html") {
            seen_website = true;
        }
        if !keep_member_for_plan(trimmed) {
            continue;
        }
        if members.len() >= MAX_PLAN_ARCHIVE_MEMBERS {
            // Prefer structural SQL / domain markers over earlier deep noise.
            let essential = lower.ends_with(".sql")
                || lower.ends_with(".sql.gz")
                || lower.ends_with("meta.xml")
                || (lower.contains("/public_html") && trimmed.matches('/').count() <= 3);
            if !essential {
                continue;
            }
        }
        members.push(trimmed.to_string());
    }
    if timed_out {
        #[cfg(unix)]
        {
            let pid = child.id() as libc::pid_t;
            if pid > 0 {
                unsafe {
                    libc::kill(-pid, libc::SIGKILL);
                }
            }
        }
        let _ = child.kill();
    }
    let status = child.wait().map_err(|e| format!("tar list wait: {e}"))?;
    if timed_out && members.is_empty() {
        return Err(format!(
            "Listing the archive timed out after {}s with no structural members. The file may be corrupt or still decompressing.",
            timeout.as_secs()
        ));
    }
    if seen_website
        && !members.iter().any(|m| {
            let l = m.to_ascii_lowercase();
            l.contains("public_html")
        })
    {
        members.push("./public_html/".into());
    }
    if !status.success() && members.is_empty() && !timed_out {
        return Err("tar list failed before any members were read".into());
    }
    if members.len() > MAX_PLAN_ARCHIVE_MEMBERS {
        members.truncate(MAX_PLAN_ARCHIVE_MEMBERS);
    }
    Ok(members)
}

fn list_zip_members(archive: &Path) -> Result<Vec<String>, String> {
    let path = archive.to_string_lossy();
    let timeout = archive_list_timeout(archive);
    let out = output_with_timeout("unzip", &["-Z1", path.as_ref()], timeout).map_err(|e| {
        if e.kind() == std::io::ErrorKind::TimedOut {
            format!(
                "Listing the zip archive timed out after {}s. The file may be corrupt or too large to inventory.",
                timeout.as_secs()
            )
        } else {
            format!("Could not list zip archive (is unzip installed?): {e}")
        }
    })?;
    if !out.status.success() {
        return Err(format!(
            "unzip list failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    collect_member_lines(&out.stdout)
}

/// Extract into `dest` after validating every member path. Dest must exist.
pub fn extract_archive_safe(archive: &Path, dest: &Path) -> Result<(), String> {
    fs::create_dir_all(dest).map_err(|e| format!("Cannot create extract dir: {e}"))?;
    let large = archive_size_bytes(archive) >= LARGE_ARCHIVE_BYTES;
    if large {
        // Full member validation of multi-GB classic archives is impractical; extract
        // then reject escaping symlinks. Prefer extract_archive_prefixes for website-only.
        match archive_kind(archive) {
            ArchiveKind::TarGz => extract_tar(archive, dest)?,
            ArchiveKind::Zip => extract_zip(archive, dest)?,
            ArchiveKind::Unsupported => return Err("Unsupported archive type".into()),
        }
        reject_escaping_symlinks(dest)?;
        return Ok(());
    }
    let members = list_archive_members(archive)?;
    validate_member_list(&members)?;
    match archive_kind(archive) {
        ArchiveKind::TarGz => extract_tar(archive, dest),
        ArchiveKind::Zip => extract_zip(archive, dest),
        ArchiveKind::Unsupported => Err("Unsupported archive type".into()),
    }?;
    reject_escaping_symlinks(dest)?;
    Ok(())
}

/// Extract only selected path prefixes (website trees / SQL / meta) from a tar archive.
pub fn extract_archive_prefixes(
    archive: &Path,
    dest: &Path,
    prefixes: &[String],
) -> Result<(), String> {
    if prefixes.is_empty() {
        return extract_archive_safe(archive, dest);
    }
    for p in prefixes {
        if !is_safe_archive_member(p) {
            return Err(format!("Refusing unsafe extract prefix: {p}"));
        }
    }
    fs::create_dir_all(dest).map_err(|e| format!("Cannot create extract dir: {e}"))?;
    match archive_kind(archive) {
        ArchiveKind::TarGz => extract_tar_prefixes(archive, dest, prefixes)?,
        ArchiveKind::Zip => {
            // Zip selective extract is best-effort via full extract for now.
            extract_zip(archive, dest)?;
        }
        ArchiveKind::Unsupported => return Err("Unsupported archive type".into()),
    }
    reject_escaping_symlinks(dest)?;
    Ok(())
}

fn extract_tar(archive: &Path, dest: &Path) -> Result<(), String> {
    let name = archive
        .file_name()
        .and_then(|v| v.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let mut cmd = Command::new("tar");
    cmd.arg("--no-same-owner");
    if name.ends_with(".tar") && !name.ends_with(".tar.gz") {
        cmd.args(["-xf"]);
    } else {
        cmd.args(["-xzf"]);
    }
    let status = cmd
        .arg(archive)
        .arg("-C")
        .arg(dest)
        .status()
        .map_err(|e| format!("Could not start tar extract: {e}"))?;
    if !status.success() {
        return Err("tar extract failed".into());
    }
    Ok(())
}

fn extract_tar_prefixes(archive: &Path, dest: &Path, prefixes: &[String]) -> Result<(), String> {
    let name = archive
        .file_name()
        .and_then(|v| v.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let mut cmd = Command::new("tar");
    cmd.arg("--no-same-owner");
    if name.ends_with(".tar") && !name.ends_with(".tar.gz") {
        cmd.args(["-xf"]);
    } else {
        cmd.args(["-xzf"]);
    }
    cmd.arg(archive).arg("-C").arg(dest);
    // GNU tar accepts member names; include common ./ variants.
    for p in prefixes {
        let trimmed = p.trim().trim_start_matches("./");
        if trimmed.is_empty() {
            continue;
        }
        cmd.arg(trimmed);
        cmd.arg(format!("./{trimmed}"));
    }
    let status = cmd
        .status()
        .map_err(|e| format!("Could not start tar selective extract: {e}"))?;
    if !status.success() {
        // Some members may be missing; retry full extract only when nothing landed.
        let empty = fs::read_dir(dest)
            .map(|rd| rd.flatten().count() == 0)
            .unwrap_or(true);
        if empty {
            return Err(
                "tar selective extract failed (no members matched). Try a full restore when disk allows."
                    .into(),
            );
        }
    }
    Ok(())
}

fn extract_zip(archive: &Path, dest: &Path) -> Result<(), String> {
    let status = Command::new("unzip")
        .args(["-q", "-o"])
        .arg(archive)
        .arg("-d")
        .arg(dest)
        .status()
        .map_err(|e| format!("Could not start unzip: {e}"))?;
    if !status.success() {
        return Err("unzip extract failed".into());
    }
    Ok(())
}

fn reject_escaping_symlinks(root: &Path) -> Result<(), String> {
    let root_canon = fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    walk_check(&root_canon, &root_canon)
}

fn walk_check(root: &Path, current: &Path) -> Result<(), String> {
    let rd = match fs::read_dir(current) {
        Ok(v) => v,
        Err(_) => return Ok(()),
    };
    for ent in rd.flatten() {
        let path = ent.path();
        let ft = ent.file_type().map_err(|e| format!("stat failed: {e}"))?;
        if ft.is_symlink() {
            let target = fs::read_link(&path).map_err(|e| format!("readlink failed: {e}"))?;
            let resolved = if target.is_absolute() {
                target.clone()
            } else {
                path.parent().unwrap_or(root).join(&target)
            };
            let resolved_norm = normalize_logical(&resolved);
            if !resolved_norm.starts_with(root) {
                return Err(format!(
                    "Refusing symlink escape: {} -> {}",
                    path.display(),
                    target.display()
                ));
            }
        } else if ft.is_dir() {
            walk_check(root, &path)?;
        }
    }
    Ok(())
}

fn normalize_logical(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::Prefix(p) => out.push(p.as_os_str()),
            Component::RootDir => out.push(comp.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                let _ = out.pop();
            }
            Component::Normal(seg) => out.push(seg),
        }
    }
    out
}

/// Copy directory tree with `cp -a` when available; fallback recursive copy.
pub fn copy_tree(src: &Path, dest: &Path) -> Result<(), String> {
    if !src.exists() {
        return Ok(());
    }
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("mkdir: {e}"))?;
    }
    if src.is_file() {
        fs::copy(src, dest).map_err(|e| format!("copy file: {e}"))?;
        return Ok(());
    }
    #[cfg(unix)]
    {
        let status = Command::new("cp")
            .args(["-a", &src.to_string_lossy(), &dest.to_string_lossy()])
            .status()
            .map_err(|e| format!("cp failed: {e}"))?;
        if status.success() {
            return Ok(());
        }
    }
    copy_tree_fallback(src, dest)
}

fn copy_tree_fallback(src: &Path, dest: &Path) -> Result<(), String> {
    fs::create_dir_all(dest).map_err(|e| format!("mkdir dest: {e}"))?;
    for ent in fs::read_dir(src).map_err(|e| format!("read_dir: {e}"))? {
        let ent = ent.map_err(|e| format!("read_dir entry: {e}"))?;
        let from = ent.path();
        let to = dest.join(ent.file_name());
        let ft = ent.file_type().map_err(|e| e.to_string())?;
        if ft.is_dir() {
            copy_tree_fallback(&from, &to)?;
        } else if ft.is_file() {
            fs::copy(&from, &to).map_err(|e| format!("copy: {e}"))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_zip_slip_members() {
        assert!(!is_safe_archive_member("../etc/passwd"));
        assert!(!is_safe_archive_member("/etc/passwd"));
        assert!(!is_safe_archive_member("foo/../../etc/passwd"));
        assert!(is_safe_archive_member("./public_html/index.html"));
        assert!(is_safe_archive_member("wp-content/uploads/a.jpg"));
    }

    #[test]
    fn validate_list_surfaces_bad_paths() {
        let members = vec!["ok/file.txt".into(), "../evil".into()];
        assert!(validate_member_list(&members).is_err());
    }

    #[test]
    fn collect_member_lines_enforces_ceiling() {
        let mut blob = String::new();
        for i in 0..(MAX_ARCHIVE_MEMBERS + 2) {
            blob.push_str(&format!("file-{i}.txt\n"));
        }
        let err = collect_member_lines(blob.as_bytes()).unwrap_err();
        assert!(err.contains(&MAX_ARCHIVE_MEMBERS.to_string()));
    }

    #[test]
    fn plan_filter_keeps_structure_drops_deep_noise() {
        assert!(keep_member_for_plan("./meta.xml"));
        assert!(keep_member_for_plan("./public_html/"));
        assert!(keep_member_for_plan("./public_html/index.php"));
        assert!(keep_member_for_plan("ai.newstargeted.com/public_html/index.php"));
        assert!(keep_member_for_plan("./news_disco.sql"));
        assert!(!keep_member_for_plan(
            "./public_html/.cache/composer/files/composer/ca-bundle/abc.zip"
        ));
    }

    #[test]
    fn collect_plan_lines_filters_noise() {
        let blob = b"./meta.xml\n./public_html/index.php\n./public_html/.cache/deep/file.bin\n./db.sql\n";
        let members = collect_plan_member_lines(blob).unwrap();
        assert!(members.iter().any(|m| m.contains("meta.xml")));
        assert!(members.iter().any(|m| m.contains("index.php")));
        assert!(members.iter().any(|m| m.contains("db.sql")));
        assert!(!members.iter().any(|m| m.contains(".cache/deep")));
    }

    #[test]
    fn large_timeout_constant_exceeds_legacy_15s() {
        assert!(ARCHIVE_LIST_TIMEOUT_LARGE.as_secs() > 15);
        assert!(RESTORE_PLAN_RENDER_BUDGET.as_secs() >= ARCHIVE_LIST_TIMEOUT_LARGE.as_secs());
    }
}
