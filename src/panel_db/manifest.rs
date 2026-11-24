//! Classify every entry under the CPN data dir for the MariaDB cutover.
//!
//! Four outcomes: stays on disk as bootstrap, skipped (cache or generated),
//! deferred (large append-only logs that get their own tables later), or
//! imported as a document. Secrets are encrypted at the application level.

use std::fs;
use std::path::{Path, PathBuf};

/// Largest single document imported (hex literal doubles it in the statement).
pub const MAX_DOC_BYTES: u64 = 4 * 1024 * 1024;
const MAX_DEPTH: usize = 8;

/// Files that must remain on disk so the panel can reach MariaDB or restore.
pub const BOOTSTRAP_FILES: &[&str] = &[
    "panel-db.json",
    "panel-db.key",
    "panel-db-state.json",
    "panel-db.disabled",
    "panel-db-auto.lock",
    "installer-bootstrap.token",
    "mariadb-admin.secret",
    "mariadb-client.cnf",
    ".my.cnf",
    "mfa/mfa-encryption.key",
];

const SKIP_DIRS: &[&str] = &[
    "cache",
    "tmp",
    "site-previews",
    "theme-catalog-assets",
    "installed-themes",
    "acme-webroot",
    "selinux",
    "install-backups",
    "rollback",
    "snapshots",
    "panel-db-snapshots",
];

const DEFERRED_LOG_DIRS: &[&str] = &["audit"];

const SECRET_DIRS: &[&str] = &[
    "dkim",
    "mfa",
    "passkeys",
    "accounts",
    "phonehome",
    "phpmyadmin",
    "webmail-contacts",
];

const SECRET_NAME_MARKERS: &[&str] = &[
    "secret",
    "token",
    "password",
    "session",
    "credential",
    "oauth",
    "cloudflare",
    "smtp",
    "ftp-accounts",
    "backup-destinations",
    "databases-registry",
    "mail-accounts",
    "zerossl",
    "proton",
    ".key",
    ".pem",
];

const SECRET_BODY_MARKERS: &[&str] = &[
    "password", "secret", "token", "private", "begin ", "api_key", "apikey", "passwd",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Class {
    Bootstrap,
    Skip(&'static str),
    Deferred { phase: u8, reason: &'static str },
    Document { phase: u8, secret_path: bool },
}

#[derive(Debug, Clone)]
pub struct Entry {
    pub rel: String,
    pub abs: PathBuf,
    pub size: u64,
    pub mode: u32,
    pub class: Class,
}

fn file_name(rel: &str) -> &str {
    rel.rsplit('/').next().unwrap_or(rel)
}

fn top(rel: &str) -> &str {
    rel.split('/').next().unwrap_or(rel)
}

/// Classify a relative path (forward slashes) that is a regular file.
pub fn classify(rel: &str, size: u64) -> Class {
    let lower = rel.to_ascii_lowercase();
    let name = file_name(&lower);
    if BOOTSTRAP_FILES.contains(&rel) || name.starts_with("panel-db.tmp") {
        return Class::Bootstrap;
    }
    if SKIP_DIRS.contains(&top(rel)) && rel.contains('/') {
        return Class::Skip("cache or generated directory");
    }
    if name.ends_with(".log")
        || name.ends_with(".stamp")
        || name.ends_with(".lock")
        || name.ends_with(".pid")
        || name.contains(".tmp")
    {
        return Class::Skip("log, stamp, lock or temp file");
    }
    if name.ends_with("-cache.json")
        || name == "host-metrics-ring.json"
        || name == "lab-deploy-meta.json"
    {
        return Class::Skip("cache or host metrics");
    }
    if name.ends_with(".php") {
        return Class::Skip("generated PHP (rebuilt by heal)");
    }
    if DEFERRED_LOG_DIRS.contains(&top(rel)) && rel.contains('/') {
        return Class::Deferred {
            phase: 5,
            reason: "audit logs move to append-friendly tables",
        };
    }
    if name.ends_with(".jsonl") {
        return Class::Deferred {
            phase: 5,
            reason: "append-only journal",
        };
    }
    if name.ends_with(".db") || name.ends_with(".sqlite") {
        return Class::Deferred {
            phase: 3,
            reason: "SQLite store moves with its own table set",
        };
    }
    if size > MAX_DOC_BYTES {
        return Class::Skip("larger than the document size limit");
    }
    let secret_path = SECRET_DIRS.contains(&top(rel)) && rel.contains('/')
        || SECRET_NAME_MARKERS.iter().any(|m| name.contains(m));
    Class::Document {
        phase: phase_for(&lower, name),
        secret_path,
    }
}

fn phase_for(lower: &str, name: &str) -> u8 {
    if name.starts_with("mail-") || name.starts_with("mail.") || top(lower) == "dkim" {
        return 2;
    }
    if top(lower) == "mfa"
        || top(lower) == "passkeys"
        || top(lower) == "accounts"
        || name.contains("session")
        || name.contains("token")
        || name.contains("oauth")
        || name.contains("secret")
        || name.contains("password")
    {
        return 4;
    }
    3
}

/// True when the content looks like it carries a secret (encrypt to be safe).
pub fn body_has_secret_markers(body: &[u8]) -> bool {
    let lower = String::from_utf8_lossy(body).to_ascii_lowercase();
    SECRET_BODY_MARKERS.iter().any(|m| lower.contains(m))
}

/// Namespace for a relative path: first component, extension dropped, sanitized.
pub fn namespace_for(rel: &str) -> Option<String> {
    let first = top(rel);
    let stem = if rel.contains('/') {
        first
    } else {
        first.rsplit_once('.').map(|(s, _)| s).unwrap_or(first)
    };
    let cleaned: String = stem
        .trim_start_matches('.')
        .to_ascii_lowercase()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();
    if cleaned.is_empty() || cleaned.len() > 64 {
        None
    } else {
        Some(cleaned)
    }
}

/// Walk `root` without following symlinks; returns one entry per regular file.
pub fn scan(root: &Path) -> Result<Vec<Entry>, String> {
    let mut out = Vec::new();
    walk(root, root, 0, &mut out)?;
    out.sort_by(|a, b| a.rel.cmp(&b.rel));
    Ok(out)
}

fn walk(root: &Path, dir: &Path, depth: usize, out: &mut Vec<Entry>) -> Result<(), String> {
    if depth > MAX_DEPTH {
        return Ok(());
    }
    let read = match fs::read_dir(dir) {
        Ok(r) => r,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(format!("Could not read {}: {e}", dir.display())),
    };
    for ent in read.flatten() {
        let path = ent.path();
        let Ok(rel_path) = path.strip_prefix(root) else {
            continue;
        };
        let rel = rel_path
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join("/");
        let Ok(meta) = fs::symlink_metadata(&path) else {
            continue;
        };
        if meta.file_type().is_symlink() {
            out.push(Entry {
                rel,
                abs: path,
                size: 0,
                mode: 0o600,
                class: Class::Skip("symlink"),
            });
            continue;
        }
        if meta.is_dir() {
            if SKIP_DIRS.contains(&rel.as_str()) {
                continue;
            }
            walk(root, &path, depth + 1, out)?;
            continue;
        }
        if !meta.is_file() {
            continue;
        }
        #[cfg(unix)]
        let mode = {
            use std::os::unix::fs::PermissionsExt;
            meta.permissions().mode() & 0o777
        };
        #[cfg(not(unix))]
        let mode = 0o600;
        let class = classify(&rel, meta.len());
        out.push(Entry {
            rel,
            abs: path,
            size: meta.len(),
            mode,
            class,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(rel: &str) -> Class {
        classify(rel, 100)
    }

    #[test]
    fn bootstrap_files_never_import() {
        for f in BOOTSTRAP_FILES {
            assert_eq!(classify(f, 10), Class::Bootstrap, "{f}");
        }
    }

    #[test]
    fn caches_logs_and_generated_files_are_skipped() {
        assert!(matches!(doc("github-releases-cache.json"), Class::Skip(_)));
        assert!(matches!(doc("installation.log"), Class::Skip(_)));
        assert!(matches!(doc("cache/x.json"), Class::Skip(_)));
        assert!(matches!(doc("phpmyadmin/signon.php"), Class::Skip(_)));
        assert!(matches!(doc("tachyon-release-check.stamp"), Class::Skip(_)));
    }

    #[test]
    fn journals_and_sqlite_are_deferred_not_imported() {
        assert!(matches!(
            doc("audit/audit.jsonl"),
            Class::Deferred { phase: 5, .. }
        ));
        assert!(matches!(
            doc("install-journal.jsonl"),
            Class::Deferred { phase: 5, .. }
        ));
        assert!(matches!(doc("panel.db"), Class::Deferred { phase: 3, .. }));
    }

    #[test]
    fn documents_get_phase_and_secret_hints() {
        assert_eq!(
            doc("mail-accounts.json"),
            Class::Document {
                phase: 2,
                secret_path: true
            }
        );
        assert_eq!(
            doc("packages.json"),
            Class::Document {
                phase: 3,
                secret_path: false
            }
        );
        assert_eq!(
            doc("mfa/ntowner.json"),
            Class::Document {
                phase: 4,
                secret_path: true
            }
        );
        assert_eq!(
            doc("dkim/example.org/default.private"),
            Class::Document {
                phase: 2,
                secret_path: true
            }
        );
        assert_eq!(
            doc("panel-session.secret"),
            Class::Document {
                phase: 4,
                secret_path: true
            }
        );
    }

    #[test]
    fn oversize_files_are_skipped() {
        assert!(matches!(
            classify("big.json", MAX_DOC_BYTES + 1),
            Class::Skip(_)
        ));
    }

    #[test]
    fn namespaces_are_sanitized() {
        assert_eq!(
            namespace_for("mail-accounts.json").as_deref(),
            Some("mail-accounts")
        );
        assert_eq!(namespace_for("mfa/ntowner.json").as_deref(), Some("mfa"));
        assert_eq!(namespace_for("Odd Name.json").as_deref(), Some("odd-name"));
        assert_eq!(namespace_for(".my.cnf").as_deref(), Some("my"));
    }

    #[test]
    fn body_markers_catch_credentials() {
        assert!(body_has_secret_markers(b"{\"smtp_password\":\"x\"}"));
        assert!(body_has_secret_markers(b"-----BEGIN PRIVATE KEY-----"));
        assert!(!body_has_secret_markers(b"{\"port\":2086}"));
    }

    #[test]
    fn scan_walks_without_following_symlinks() {
        let root = std::env::temp_dir().join(format!("cpn-pdb-scan-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("mfa")).unwrap();
        fs::create_dir_all(root.join("cache")).unwrap();
        fs::write(root.join("packages.json"), "{}").unwrap();
        fs::write(root.join("mfa/u.json"), "{}").unwrap();
        fs::write(root.join("cache/c.json"), "{}").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(root.join("packages.json"), root.join("link.json")).unwrap();
        let entries = scan(&root).unwrap();
        let rels: Vec<&str> = entries.iter().map(|e| e.rel.as_str()).collect();
        assert!(rels.contains(&"packages.json"));
        assert!(rels.contains(&"mfa/u.json"));
        assert!(!rels.iter().any(|r| r.starts_with("cache/")));
        #[cfg(unix)]
        assert!(matches!(
            entries.iter().find(|e| e.rel == "link.json").unwrap().class,
            Class::Skip("symlink")
        ));
        let _ = fs::remove_dir_all(&root);
    }
}
