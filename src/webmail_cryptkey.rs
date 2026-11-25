//! Webmail client-side crypto key heal for SnappyMail-family clients (Tachyon, SnappyMail).
//!
//! The client seals a per-mailbox `.cryptkey` with the mailbox login password. When the
//! IMAP password changes outside the client (panel reset, `cpn mail passwd`, migrate), the
//! key can no longer be opened and webmail shows "Your login passphrase has changed. Please
//! fill in your old login passphrase to decrypt data".
//!
//! Heal: back up the sealed key under `/home/backup/webmail-cryptkey/`, remove it, and drop
//! the mailbox's cached sessions. The client regenerates a fresh key sealed with the current
//! password on the next login. Mail on the IMAP server is never touched. Only client-side
//! secrets sealed by the old key (additional-account passwords, stored PGP passphrases) must
//! be entered again.

use std::fs;
use std::path::{Path, PathBuf};

/// Backup root for sealed keys removed by this heal.
pub const BACKUP_ROOT: &str = "/home/backup/webmail-cryptkey";

/// Split and validate `local@domain` for use as storage path segments (no traversal).
pub fn split_address(address: &str) -> Result<(String, String), String> {
    let addr = address.trim().to_ascii_lowercase();
    let Some((local, domain)) = addr.split_once('@') else {
        return Err("Mailbox address must look like user@domain".into());
    };
    let ok = |s: &str| {
        !s.is_empty()
            && !s.starts_with('.')
            && !s.contains("..")
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '+'))
    };
    if !ok(local) || !ok(domain) || domain.contains('+') {
        return Err("Mailbox address contains unsupported characters".into());
    }
    Ok((local.to_string(), domain.to_string()))
}

/// Per-mailbox storage dirs (`_data_/<instance>/storage/<domain>/<local>`) that exist.
pub fn mailbox_storage_dirs(data_dir: &Path, address: &str) -> Result<Vec<PathBuf>, String> {
    let (local, domain) = split_address(address)?;
    let mut out = Vec::new();
    let Ok(rd) = fs::read_dir(data_dir.join("_data_")) else {
        return Ok(out);
    };
    for ent in rd.flatten() {
        let dir = ent.path().join("storage").join(&domain).join(&local);
        if dir.is_dir() {
            out.push(dir);
        }
    }
    Ok(out)
}

/// Remove the sealed `.cryptkey` for one mailbox in one client data dir.
/// Returns the number of keys moved to backup.
pub fn reset_in_data_dir(
    data_dir: &Path,
    backup_root: &Path,
    address: &str,
    stamp: &str,
) -> Result<usize, String> {
    let (local, domain) = split_address(address)?;
    let client = data_dir
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "webmail".into());
    let mut moved = 0usize;
    for dir in mailbox_storage_dirs(data_dir, address)? {
        let key = dir.join(".cryptkey");
        if !key.is_file() {
            continue;
        }
        let dest_dir = backup_root
            .join(stamp)
            .join(&client)
            .join(&domain)
            .join(&local);
        fs::create_dir_all(&dest_dir).map_err(|e| format!("backup dir: {e}"))?;
        let dest = dest_dir.join("cryptkey.old");
        fs::copy(&key, &dest).map_err(|e| format!("backup cryptkey: {e}"))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&dest_dir, fs::Permissions::from_mode(0o700));
            let _ = fs::set_permissions(&dest, fs::Permissions::from_mode(0o600));
        }
        // Verify the backup before removing the original.
        let same =
            fs::metadata(&dest).map(|m| m.len()).ok() == fs::metadata(&key).map(|m| m.len()).ok();
        if !same {
            return Err("cryptkey backup verification failed; key left in place".into());
        }
        fs::remove_file(&key).map_err(|e| format!("remove cryptkey: {e}"))?;
        for sub in [".sessions", ".sign_me"] {
            clear_dir_files(&dir.join(sub));
        }
        moved += 1;
    }
    Ok(moved)
}

fn clear_dir_files(dir: &Path) {
    if let Ok(rd) = fs::read_dir(dir) {
        for ent in rd.flatten() {
            let p = ent.path();
            if p.is_file() {
                let _ = fs::remove_file(p);
            }
        }
    }
}

/// Heal every installed SnappyMail-family client for `address`. Safe to call repeatedly.
pub fn reset_client_cryptkey(address: &str) -> Result<String, String> {
    let (_, _) = split_address(address)?;
    let stamp = crate::account::now_unix().to_string();
    let mut total = 0usize;
    for data_dir in crate::install_snappymail_lineage::lineage_data_dirs() {
        total += reset_in_data_dir(
            Path::new(&data_dir),
            Path::new(BACKUP_ROOT),
            address,
            &stamp,
        )?;
    }
    if total == 0 {
        return Ok("No sealed webmail key found; nothing to reset.".into());
    }
    Ok(format!(
        "Webmail client key reset for {total} client store(s); backup under {BACKUP_ROOT}/{stamp}. \
         Webmail will create a new key at next login."
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("cpn-cryptkey-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn rejects_traversal() {
        assert!(split_address("../x@evil.test").is_err());
        assert!(split_address("a@b/c").is_err());
        assert!(split_address("nodomain").is_err());
        assert_eq!(
            split_address("Info@Example.TEST").unwrap(),
            ("info".into(), "example.test".into())
        );
    }

    #[test]
    fn moves_key_to_backup_and_keeps_other_files() {
        let root = tmp("move");
        let data = root.join("tachyon");
        let mbox = data.join("_data_/_default_/storage/example.test/info");
        fs::create_dir_all(mbox.join(".sessions")).unwrap();
        fs::write(mbox.join(".cryptkey"), b"sealed").unwrap();
        fs::write(mbox.join("identities"), b"keep").unwrap();
        fs::write(mbox.join(".sessions/abc"), b"s").unwrap();
        let backup = root.join("backup");
        let n = reset_in_data_dir(&data, &backup, "info@example.test", "1").unwrap();
        assert_eq!(n, 1);
        assert!(!mbox.join(".cryptkey").exists());
        assert!(mbox.join("identities").exists());
        assert!(!mbox.join(".sessions/abc").exists());
        assert!(
            backup
                .join("1/tachyon/example.test/info/cryptkey.old")
                .is_file()
        );
        // Second run is a no-op.
        assert_eq!(
            reset_in_data_dir(&data, &backup, "info@example.test", "2").unwrap(),
            0
        );
        let _ = fs::remove_dir_all(&root);
    }
}
