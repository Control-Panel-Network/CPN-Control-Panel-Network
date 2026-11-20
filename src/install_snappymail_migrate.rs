//! Migrate SnappyMail-family user data (settings, identities, filters UI, PGP) into Tachyon.
//!
//! Mailbox messages live in Dovecot/Maildir and are NOT copied. This only imports
//! SnappyMail application storage and Contacts address books when discovered on disk
//! or under `/home/backup/cpn-*` after a CyberPanel wipe.

use crate::install_snappymail_contacts::ensure_contacts_defaults;
use crate::install_snappymail_lineage::{chown_data_tree, lineage_data_dirs};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const TACHYON_DATA_DIR: &str = "/var/lib/cpn-webmail/tachyon/";
const MARKER: &str = "_data_/_default_/.cpn-snappy-migrate-v1";
const SKIP_DIR_NAMES: &[&str] = &[".sessions", ".sign_me", "__nobody__"];

/// Discover CyberPanel / backup SnappyMail data roots that still hold `_data_/_default_/storage`.
pub fn discover_snappymail_data_sources() -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    let candidates = [
        "/usr/local/lscp/cyberpanel/snappymail/data",
        "/usr/local/CyberCP/public/snappymail/data",
        "/usr/local/CyberCPBak/public/snappymail/data",
        "/var/lib/cpn-webmail/snappymail",
    ];
    for c in candidates {
        push_if_storage(PathBuf::from(c), &mut out);
    }
    for pattern in [
        "/home/backup/cpn-full-cutover-*",
        "/home/backup/cpn-migrate-*",
    ] {
        if let Ok(paths) = glob_dirs(pattern) {
            for base in paths {
                push_if_storage(base.join("lscp/cyberpanel/snappymail/data"), &mut out);
                push_if_storage(base.join("cybercp/public/snappymail/data"), &mut out);
                push_if_storage(base.join("CyberCP/public/snappymail/data"), &mut out);
            }
        }
    }
    out
}

fn glob_dirs(pattern: &str) -> Result<Vec<PathBuf>, String> {
    // Avoid a glob crate dependency: expand the common /home/backup/cpn-* prefixes.
    let Some((dir, prefix)) = pattern.rsplit_once('/') else {
        return Ok(Vec::new());
    };
    let prefix = prefix.trim_end_matches('*');
    let mut out = Vec::new();
    let rd = fs::read_dir(dir).map_err(|e| e.to_string())?;
    for ent in rd.flatten() {
        let name = ent.file_name().to_string_lossy().to_string();
        if name.starts_with(prefix) && ent.path().is_dir() {
            out.push(ent.path());
        }
    }
    out.sort();
    out.reverse(); // newest backup name first when timestamp-suffixed
    Ok(out)
}

fn push_if_storage(path: PathBuf, out: &mut Vec<PathBuf>) {
    let storage = path.join("_data_/_default_/storage");
    if storage.is_dir() && !out.iter().any(|p| p == &path) {
        out.push(path);
    }
}

fn tachyon_data_dir() -> String {
    let preferred = TACHYON_DATA_DIR.to_string();
    if Path::new(&preferred).is_dir() {
        return preferred;
    }
    lineage_data_dirs()
        .into_iter()
        .find(|d| d.to_ascii_lowercase().contains("tachyon"))
        .unwrap_or(preferred)
}

/// Import SnappyMail storage + Contacts into Tachyon when sources exist (idempotent).
pub fn migrate_snappymail_into_tachyon() -> Result<String, String> {
    let dest_root = tachyon_data_dir();
    let dest = Path::new(&dest_root);
    if !dest.is_dir() {
        return Err(format!(
            "Tachyon data dir missing at {dest_root}. Install Tachyon first."
        ));
    }
    let marker = dest.join(MARKER);
    if marker.is_file() {
        return Ok(
            "SnappyMail → Tachyon migration already applied (marker present). Mailboxes stay on IMAP."
                .into(),
        );
    }

    let sources = discover_snappymail_data_sources();
    if sources.is_empty() {
        write_marker(&marker, "no-source")?;
        return Ok(
            "No SnappyMail data dirs found (live or /home/backup/cpn-*). Mailboxes still load via IMAP; users may need to re-enter contacts/settings."
                .into(),
        );
    }

    let source = &sources[0];
    let copied = copy_storage_tree(
        &source.join("_data_/_default_/storage"),
        &dest.join("_data_/_default_/storage"),
    )?;
    let _ = ensure_contacts_defaults(&dest_root);
    let contacts_msg = migrate_contacts_database(source, &dest_root).unwrap_or_else(|e| e);
    let _ = chown_data_tree(&dest_root);
    write_marker(
        &marker,
        &format!(
            "source={} files={copied} contacts={contacts_msg}",
            source.display()
        ),
    )?;
    Ok(format!(
        "Migrated SnappyMail user storage from {} into Tachyon ({copied} files). {contacts_msg} Mailbox messages were not copied (IMAP/Maildir).",
        source.display()
    ))
}

fn write_marker(path: &Path, detail: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(
        path,
        format!("cpn snappymail→tachyon migrate v1\n{detail}\n"),
    )
    .map_err(|e| e.to_string())
}

fn copy_storage_tree(src: &Path, dst: &Path) -> Result<u32, String> {
    if !src.is_dir() {
        return Ok(0);
    }
    fs::create_dir_all(dst).map_err(|e| e.to_string())?;
    let mut copied = 0u32;
    copy_dir_filtered(src, dst, &mut copied)?;
    Ok(copied)
}

fn copy_dir_filtered(src: &Path, dst: &Path, copied: &mut u32) -> Result<(), String> {
    for ent in fs::read_dir(src).map_err(|e| e.to_string())?.flatten() {
        let name = ent.file_name();
        let name_str = name.to_string_lossy();
        if SKIP_DIR_NAMES
            .iter()
            .any(|s| name_str.eq_ignore_ascii_case(s))
        {
            continue;
        }
        let from = ent.path();
        let to = dst.join(&name);
        let meta = ent.metadata().map_err(|e| e.to_string())?;
        if meta.is_dir() {
            fs::create_dir_all(&to).map_err(|e| e.to_string())?;
            copy_dir_filtered(&from, &to, copied)?;
        } else if meta.is_file() {
            if to.is_file() {
                // Keep Tachyon-local prefs when already present.
                continue;
            }
            if let Some(parent) = to.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            fs::copy(&from, &to).map_err(|e| format!("copy {}: {e}", from.display()))?;
            *copied += 1;
        }
    }
    Ok(())
}

/// Copy rainloop_* address book tables from a discovered SnappyMail MariaDB into Tachyon's AB DB.
fn migrate_contacts_database(source_data: &Path, dest_data: &str) -> Result<String, String> {
    let src_ini = source_data.join("_data_/_default_/configs/application.ini");
    if !src_ini.is_file() {
        return Ok("contacts: no source application.ini".into());
    }
    let raw = fs::read_to_string(&src_ini).map_err(|e| e.to_string())?;
    let Some(src_dsn) = ini_quoted(&raw, "pdo_dsn") else {
        return Ok("contacts: source has no pdo_dsn".into());
    };
    let src_user = ini_quoted(&raw, "pdo_user").unwrap_or_default();
    let src_pass = ini_quoted(&raw, "pdo_password").unwrap_or_default();
    if src_dsn.is_empty() || src_user.is_empty() {
        return Ok("contacts: incomplete source PDO settings".into());
    }

    let dest_secret = Path::new("/var/lib/cpn/webmail-contacts/tachyon.json");
    if !dest_secret.is_file() {
        return Ok("contacts: Tachyon contacts DB secret not ready yet".into());
    }
    let secret_raw = fs::read_to_string(dest_secret).map_err(|e| e.to_string())?;
    let secret: serde_json::Value =
        serde_json::from_str(&secret_raw).map_err(|e| format!("contacts secret json: {e}"))?;
    let dest_dsn = secret
        .get("pdo_dsn")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let dest_user = secret
        .get("db_user")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let dest_pass = secret
        .get("db_password")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if dest_dsn.is_empty() || dest_user.is_empty() {
        return Ok("contacts: Tachyon PDO incomplete".into());
    }

    let output = Command::new("php")
        .args([
            "-r",
            r#"
$srcDsn = getenv('CPN_SRC_DSN');
$srcUser = getenv('CPN_SRC_USER');
$srcPass = getenv('CPN_SRC_PASS');
$dstDsn = getenv('CPN_DST_DSN');
$dstUser = getenv('CPN_DST_USER');
$dstPass = getenv('CPN_DST_PASS');
try {
  $src = new PDO('mysql:' . $srcDsn, $srcUser, $srcPass, [PDO::ATTR_ERRMODE => PDO::ERRMODE_EXCEPTION]);
  $dst = new PDO('mysql:' . $dstDsn, $dstUser, $dstPass, [PDO::ATTR_ERRMODE => PDO::ERRMODE_EXCEPTION]);
} catch (Throwable $e) {
  fwrite(STDERR, 'connect: ' . $e->getMessage());
  exit(2);
}
$dstCount = (int)$dst->query('SELECT COUNT(*) FROM rainloop_ab_contacts')->fetchColumn();
if ($dstCount > 0) { echo "keep-dest:$dstCount"; exit(0); }
$srcCount = (int)$src->query('SELECT COUNT(*) FROM rainloop_ab_contacts')->fetchColumn();
if ($srcCount < 1) { echo "empty-source"; exit(0); }
$dst->beginTransaction();
foreach (['rainloop_users','rainloop_ab_contacts','rainloop_ab_properties'] as $table) {
  $rows = $src->query('SELECT * FROM ' . $table)->fetchAll(PDO::FETCH_ASSOC) ?: [];
  if (!$rows) { continue; }
  $cols = array_keys($rows[0]);
  $placeholders = implode(',', array_fill(0, count($cols), '?'));
  $colList = implode(',', array_map(fn($c) => '`' . str_replace('`','``',$c) . '`', $cols));
  $sql = "INSERT IGNORE INTO `$table` ($colList) VALUES ($placeholders)";
  $stmt = $dst->prepare($sql);
  foreach ($rows as $row) {
    $stmt->execute(array_values($row));
  }
}
$dst->commit();
$n = (int)$dst->query('SELECT COUNT(*) FROM rainloop_ab_contacts')->fetchColumn();
echo "imported:$n";
"#,
        ])
        .env("CPN_SRC_DSN", &src_dsn)
        .env("CPN_SRC_USER", &src_user)
        .env("CPN_SRC_PASS", &src_pass)
        .env("CPN_DST_DSN", &dest_dsn)
        .env("CPN_DST_USER", &dest_user)
        .env("CPN_DST_PASS", &dest_pass)
        .output()
        .map_err(|e| format!("contacts migrate failed to start: {e}"))?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        // Never echo PDO passwords; keep a short scrubbed reason.
        let scrubbed: String = err
            .chars()
            .take(160)
            .collect::<String>()
            .replace(&src_pass, "[redacted]")
            .replace(&dest_pass, "[redacted]");
        return Err(format!("contacts migrate failed: {scrubbed}"));
    }
    let msg = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let _ = dest_data;
    Ok(format!("contacts: {msg}"))
}

fn ini_quoted(raw: &str, key: &str) -> Option<String> {
    crate::install_snappymail_lineage::parse_ini_quoted_value(raw, key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("cpn-snappy-mig-{label}-{nanos}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn copy_skips_sessions_and_keeps_existing() {
        let root = temp_dir("copy");
        let src = root.join("src");
        let dst = root.join("dst");
        fs::create_dir_all(src.join("example.com/user/.sessions")).unwrap();
        fs::write(src.join("example.com/user/.sessions/a"), "sess").unwrap();
        fs::create_dir_all(src.join("example.com/user")).unwrap();
        fs::write(src.join("example.com/user/settings"), "{\"a\":1}").unwrap();
        fs::create_dir_all(dst.join("example.com/user")).unwrap();
        fs::write(dst.join("example.com/user/settings"), "{\"keep\":true}").unwrap();
        let n = copy_storage_tree(&src, &dst).unwrap();
        assert_eq!(n, 0); // settings already existed
        assert!(!dst.join("example.com/user/.sessions/a").exists());
        assert_eq!(
            fs::read_to_string(dst.join("example.com/user/settings")).unwrap(),
            "{\"keep\":true}"
        );
        fs::remove_file(dst.join("example.com/user/settings")).unwrap();
        let n2 = copy_storage_tree(&src, &dst).unwrap();
        assert_eq!(n2, 1);
        assert!(dst.join("example.com/user/settings").is_file());
        let _ = fs::remove_dir_all(root);
    }
}
