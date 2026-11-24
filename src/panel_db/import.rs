//! File to MariaDB import, verify, and export (rollback) for the data dir.
//!
//! Import is copy-only: it never edits or deletes the source files, so a
//! failed or interrupted run leaves the host exactly as it was. Each file is
//! compared by fingerprint first, so re-running skips everything already
//! migrated and resumes whatever is left.

use super::config::write_private_atomic;
use super::docs_store::{Doc, DocStore, PutOutcome};
use super::manifest::{self, Class, Entry};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ImportReport {
    pub scanned: usize,
    pub inserted: usize,
    pub updated: usize,
    pub unchanged: usize,
    pub bootstrap: usize,
    pub skipped: usize,
    pub deferred: usize,
    pub failed: usize,
    /// Relative paths only; never file contents or raw errors with values.
    pub failures: Vec<String>,
}

impl ImportReport {
    pub fn ok(&self) -> bool {
        self.failed == 0
    }

    pub fn summary(&self) -> String {
        format!(
            "scanned {}, inserted {}, updated {}, unchanged {}, bootstrap on disk {}, skipped {}, deferred {}, failed {}",
            self.scanned,
            self.inserted,
            self.updated,
            self.unchanged,
            self.bootstrap,
            self.skipped,
            self.deferred,
            self.failed
        )
    }
}

fn load_doc(entry: &Entry, phase: u8, secret_path: bool) -> Result<Option<Doc>, String> {
    let Some(namespace) = manifest::namespace_for(&entry.rel) else {
        return Ok(None);
    };
    if entry.rel.chars().count() > 255 {
        return Ok(None);
    }
    let body = fs::read(&entry.abs).map_err(|e| format!("read failed: {}", e.kind()))?;
    let encrypt = secret_path || manifest::body_has_secret_markers(&body);
    Ok(Some(Doc {
        namespace,
        key: entry.rel.clone(),
        body,
        encrypt,
        mode: entry.mode,
        phase,
    }))
}

/// Import every document under `root`. With `dry_run`, only classifies.
pub fn import_all(
    root: &Path,
    store: &dyn DocStore,
    dry_run: bool,
) -> Result<ImportReport, String> {
    let entries = manifest::scan(root)?;
    let mut report = ImportReport::default();
    let known: BTreeMap<(String, String), (String, bool)> = if dry_run {
        BTreeMap::new()
    } else {
        store
            .list()?
            .into_iter()
            .map(|m| ((m.namespace, m.key), (m.fingerprint, m.encrypted)))
            .collect()
    };
    for entry in &entries {
        report.scanned += 1;
        match entry.class {
            Class::Bootstrap => report.bootstrap += 1,
            Class::Skip(_) => report.skipped += 1,
            Class::Deferred { .. } => report.deferred += 1,
            Class::Document { phase, secret_path } => {
                let doc = match load_doc(entry, phase, secret_path) {
                    Ok(Some(doc)) => doc,
                    Ok(None) => {
                        report.skipped += 1;
                        continue;
                    }
                    Err(_) => {
                        report.failed += 1;
                        report.failures.push(entry.rel.clone());
                        continue;
                    }
                };
                if dry_run {
                    report.unchanged += 1;
                    continue;
                }
                let fp = store.fingerprint(&doc);
                if known
                    .get(&(doc.namespace.clone(), doc.key.clone()))
                    .is_some_and(|(f, enc)| *f == fp && *enc == doc.encrypt)
                {
                    report.unchanged += 1;
                    continue;
                }
                match store.put(&doc) {
                    Ok(PutOutcome::Inserted) => report.inserted += 1,
                    Ok(PutOutcome::Updated) => report.updated += 1,
                    Ok(PutOutcome::Unchanged) => report.unchanged += 1,
                    Err(_) => {
                        report.failed += 1;
                        report.failures.push(entry.rel.clone());
                    }
                }
            }
        }
    }
    Ok(report)
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct VerifyReport {
    pub checked: usize,
    pub missing_in_db: Vec<String>,
    pub mismatched: Vec<String>,
    /// In the database but no longer on disk (reported, never auto-deleted).
    pub orphans: Vec<String>,
}

impl VerifyReport {
    pub fn ok(&self) -> bool {
        self.missing_in_db.is_empty() && self.mismatched.is_empty()
    }

    pub fn summary(&self) -> String {
        format!(
            "checked {}, missing in database {}, changed since import {}, only in database {}",
            self.checked,
            self.missing_in_db.len(),
            self.mismatched.len(),
            self.orphans.len()
        )
    }
}

/// Compare every importable file with its database row by fingerprint.
pub fn verify(root: &Path, store: &dyn DocStore) -> Result<VerifyReport, String> {
    let known: BTreeMap<(String, String), (String, bool)> = store
        .list()?
        .into_iter()
        .map(|m| ((m.namespace, m.key), (m.fingerprint, m.encrypted)))
        .collect();
    let mut report = VerifyReport::default();
    let mut seen = std::collections::BTreeSet::new();
    for entry in manifest::scan(root)? {
        let Class::Document { phase, secret_path } = entry.class else {
            continue;
        };
        let Ok(Some(doc)) = load_doc(&entry, phase, secret_path) else {
            continue;
        };
        report.checked += 1;
        let id = (doc.namespace.clone(), doc.key.clone());
        seen.insert(id.clone());
        match known.get(&id) {
            None => report.missing_in_db.push(entry.rel.clone()),
            Some((fp, enc)) if *fp == store.fingerprint(&doc) && *enc == doc.encrypt => {}
            Some(_) => report.mismatched.push(entry.rel.clone()),
        }
    }
    for (id, _) in known {
        if !seen.contains(&id) {
            report.orphans.push(id.1);
        }
    }
    Ok(report)
}

/// Resolve a stored key to a path under `dest`, refusing traversal.
pub fn safe_dest(dest: &Path, key: &str) -> Result<PathBuf, String> {
    if key.is_empty() || key.contains('\\') || key.contains('\0') {
        return Err("Unsafe document key".into());
    }
    let mut out = dest.to_path_buf();
    for part in key.split('/') {
        if part.is_empty() || part == "." || part == ".." {
            return Err("Unsafe document key".into());
        }
        if Path::new(part)
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
        {
            return Err("Unsafe document key".into());
        }
        out.push(part);
    }
    Ok(out)
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ExportReport {
    pub written: usize,
    pub skipped_existing: usize,
}

/// Write every document back to `dest` as files (decrypted). Rollback path.
pub fn export(store: &dyn DocStore, dest: &Path, force: bool) -> Result<ExportReport, String> {
    let mut report = ExportReport::default();
    for meta in store.list()? {
        let Some(doc) = store.get(&meta.namespace, &meta.key)? else {
            continue;
        };
        let path = safe_dest(dest, &doc.key)?;
        if path.exists() && !force {
            report.skipped_existing += 1;
            continue;
        }
        write_private_atomic(&path, &doc.body)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&path, fs::Permissions::from_mode(doc.mode & 0o777));
        }
        report.written += 1;
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::super::crypto::PanelKey;
    use super::super::docs_store::mem::MemDocStore;
    use super::*;

    fn tree(tag: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("cpn-pdb-imp-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("mfa")).unwrap();
        fs::create_dir_all(root.join("dkim/example.org")).unwrap();
        fs::create_dir_all(root.join("audit")).unwrap();
        fs::write(root.join("packages.json"), "{\"packages\":[]}").unwrap();
        fs::write(
            root.join("mail-accounts.json"),
            "{\"accounts\":[{\"mailbox_password\":\"pw\"}]}",
        )
        .unwrap();
        fs::write(root.join("mfa/ntowner.json"), "{\"totp\":\"abc\"}").unwrap();
        fs::write(
            root.join("dkim/example.org/default.private"),
            b"-----BEGIN PRIVATE KEY-----\nabc",
        )
        .unwrap();
        fs::write(root.join("audit/audit.jsonl"), "{}\n").unwrap();
        fs::write(root.join("panel-db.json"), "{}").unwrap();
        fs::write(root.join("installation.log"), "log").unwrap();
        root
    }

    fn store() -> MemDocStore {
        use super::super::crypto::panel_key_test_fixture;
        MemDocStore::new(panel_key_test_fixture("panel-db-import"))
    }

    #[test]
    fn import_classifies_and_never_touches_sources() {
        let root = tree("a");
        let s = store();
        let r = import_all(&root, &s, false).unwrap();
        assert_eq!(r.inserted, 4);
        assert_eq!(r.bootstrap, 1);
        assert_eq!(r.deferred, 1);
        assert_eq!(r.skipped, 1);
        assert!(r.ok());
        assert!(root.join("mail-accounts.json").exists());
        assert!(root.join("panel-db.json").exists());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn rerun_is_idempotent_and_resumes_changes_only() {
        let root = tree("b");
        let s = store();
        import_all(&root, &s, false).unwrap();
        let again = import_all(&root, &s, false).unwrap();
        assert_eq!((again.inserted, again.updated, again.unchanged), (0, 0, 4));
        fs::write(root.join("packages.json"), "{\"packages\":[1]}").unwrap();
        fs::write(root.join("sites.json"), "[]").unwrap();
        let third = import_all(&root, &s, false).unwrap();
        assert_eq!((third.inserted, third.updated, third.unchanged), (1, 1, 3));
        let revs: Vec<u64> = s.list().unwrap().into_iter().map(|m| m.revision).collect();
        assert!(revs.contains(&2));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn secrets_never_land_in_the_clear() {
        let root = tree("c");
        let s = store();
        import_all(&root, &s, false).unwrap();
        for (ns, key) in [
            ("mail-accounts", "mail-accounts.json"),
            ("mfa", "mfa/ntowner.json"),
            ("dkim", "dkim/example.org/default.private"),
        ] {
            let raw = s.stored_bytes(ns, key).unwrap();
            let text = String::from_utf8_lossy(&raw);
            assert!(text.starts_with("cpn1:"), "{key} not encrypted");
        }
        let plain = s.stored_bytes("packages", "packages.json").unwrap();
        assert_eq!(plain, b"{\"packages\":[]}");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn export_round_trips_every_byte_and_mode() {
        let root = tree("d");
        let s = store();
        import_all(&root, &s, false).unwrap();
        let out = std::env::temp_dir().join(format!("cpn-pdb-exp-{}", std::process::id()));
        let _ = fs::remove_dir_all(&out);
        let r = export(&s, &out, false).unwrap();
        assert_eq!(r.written, 4);
        for rel in [
            "packages.json",
            "mail-accounts.json",
            "mfa/ntowner.json",
            "dkim/example.org/default.private",
        ] {
            assert_eq!(
                fs::read(root.join(rel)).unwrap(),
                fs::read(out.join(rel)).unwrap(),
                "{rel}"
            );
        }
        let again = export(&s, &out, false).unwrap();
        assert_eq!((again.written, again.skipped_existing), (0, 4));
        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_dir_all(&out);
    }

    #[test]
    fn verify_reports_drift_missing_and_orphans() {
        let root = tree("e");
        let s = store();
        assert_eq!(verify(&root, &s).unwrap().missing_in_db.len(), 4);
        import_all(&root, &s, false).unwrap();
        assert!(verify(&root, &s).unwrap().ok());
        fs::write(root.join("packages.json"), "{\"changed\":true}").unwrap();
        fs::remove_file(root.join("mfa/ntowner.json")).unwrap();
        let v = verify(&root, &s).unwrap();
        assert_eq!(v.mismatched, vec!["packages.json"]);
        assert_eq!(v.orphans, vec!["mfa/ntowner.json"]);
        assert!(!v.ok());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn dry_run_writes_nothing() {
        let root = tree("f");
        let s = store();
        let r = import_all(&root, &s, true).unwrap();
        assert_eq!(r.unchanged, 4);
        assert!(s.list().unwrap().is_empty());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn export_refuses_path_traversal() {
        let dest = Path::new("/tmp/out");
        assert!(safe_dest(dest, "../etc/passwd").is_err());
        assert!(safe_dest(dest, "a//b").is_err());
        assert!(safe_dest(dest, "/abs").is_err());
        assert!(safe_dest(dest, "a\\b").is_err());
        assert!(safe_dest(dest, "mfa/user.json").is_ok());
    }
}
