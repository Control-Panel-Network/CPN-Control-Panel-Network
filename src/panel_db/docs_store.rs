//! Document store: namespaced blobs (JSON, text, key material) in `panel_documents`.
//!
//! [`DocStore`] is what later store cutovers call. The MariaDB implementation
//! encrypts secret rows with the application key and binds ciphertext to the
//! row identity; the in-memory one backs the round-trip tests.

use super::client::SqlExec;
use super::crypto::{PanelKey, sha256_hex};
use super::sqlfmt::{decode_hex, hex_lit, text_lit};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Doc {
    pub namespace: String,
    pub key: String,
    /// Always plaintext at this layer; encryption happens inside the store.
    pub body: Vec<u8>,
    pub encrypt: bool,
    pub mode: u32,
    pub phase: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocMeta {
    pub namespace: String,
    pub key: String,
    pub fingerprint: String,
    pub encrypted: bool,
    pub revision: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PutOutcome {
    Inserted,
    Updated,
    Unchanged,
}

pub trait DocStore {
    /// Fingerprint the store would record for doc (keyed for encrypted rows).
    fn fingerprint(&self, doc: &Doc) -> String;
    fn list(&self) -> Result<Vec<DocMeta>, String>;
    fn get(&self, namespace: &str, key: &str) -> Result<Option<Doc>, String>;
    /// Write `doc`; skips the write when the stored fingerprint already matches.
    fn put(&self, doc: &Doc) -> Result<PutOutcome, String>;
    fn delete(&self, namespace: &str, key: &str) -> Result<bool, String>;
}

fn aad(namespace: &str, key: &str) -> String {
    format!("{namespace}\0{key}")
}

pub struct MariaDocStore<'a> {
    sql: &'a dyn SqlExec,
    db: String,
    key: PanelKey,
}

impl<'a> MariaDocStore<'a> {
    pub fn new(sql: &'a dyn SqlExec, db: &str, key: PanelKey) -> Self {
        Self {
            sql,
            db: db.to_string(),
            key,
        }
    }

    fn fp(&self, doc_encrypted: bool, plain: &[u8]) -> String {
        if doc_encrypted {
            self.key.fingerprint(plain)
        } else {
            sha256_hex(plain)
        }
    }

    pub fn fingerprint_for(&self, doc: &Doc) -> String {
        self.fp(doc.encrypt, &doc.body)
    }

    fn existing(&self, ns: &str, key: &str) -> Result<Option<DocMeta>, String> {
        Ok(self
            .list_where(&format!(
                "WHERE namespace = {} AND doc_key = {}",
                text_lit(ns),
                text_lit(key)
            ))?
            .into_iter()
            .next())
    }

    fn list_where(&self, clause: &str) -> Result<Vec<DocMeta>, String> {
        let sql = format!(
            "SELECT HEX(namespace), HEX(doc_key), content_sha256, encrypted, revision FROM panel_documents {clause} ORDER BY namespace, doc_key"
        );
        let mut out = Vec::new();
        for row in self.sql.query(Some(&self.db), &sql)? {
            if row.len() < 5 {
                return Err("Unexpected panel_documents row shape".into());
            }
            let text = |i: usize| -> Result<String, String> {
                String::from_utf8(decode_hex(&row[i])?)
                    .map_err(|_| "Non UTF-8 document name".to_string())
            };
            out.push(DocMeta {
                namespace: text(0)?,
                key: text(1)?,
                fingerprint: row[2].clone(),
                encrypted: row[3] == "1",
                revision: row[4].parse().unwrap_or(0),
            });
        }
        Ok(out)
    }
}

impl DocStore for MariaDocStore<'_> {
    fn fingerprint(&self, doc: &Doc) -> String {
        self.fingerprint_for(doc)
    }

    fn list(&self) -> Result<Vec<DocMeta>, String> {
        self.list_where("")
    }

    fn get(&self, namespace: &str, key: &str) -> Result<Option<Doc>, String> {
        let sql = format!(
            "SELECT HEX(body), encrypted, file_mode, phase FROM panel_documents WHERE namespace = {} AND doc_key = {}",
            text_lit(namespace),
            text_lit(key)
        );
        let rows = self.sql.query(Some(&self.db), &sql)?;
        let Some(row) = rows.into_iter().next() else {
            return Ok(None);
        };
        if row.len() < 4 {
            return Err("Unexpected panel_documents row shape".into());
        }
        let stored = decode_hex(&row[0])?;
        let encrypted = row[1] == "1";
        let body = if encrypted {
            let envelope = String::from_utf8(stored)
                .map_err(|_| "Corrupt encrypted document body".to_string())?;
            self.key.decrypt(&aad(namespace, key), &envelope)?
        } else {
            stored
        };
        Ok(Some(Doc {
            namespace: namespace.to_string(),
            key: key.to_string(),
            body,
            encrypt: encrypted,
            mode: row[2].parse().unwrap_or(0o600),
            phase: row[3].parse().unwrap_or(3),
        }))
    }

    fn put(&self, doc: &Doc) -> Result<PutOutcome, String> {
        let fp = self.fingerprint_for(doc);
        let prior = self.existing(&doc.namespace, &doc.key)?;
        if let Some(meta) = &prior
            && meta.fingerprint == fp
            && meta.encrypted == doc.encrypt
        {
            return Ok(PutOutcome::Unchanged);
        }
        let stored: Vec<u8> = if doc.encrypt {
            self.key
                .encrypt(&aad(&doc.namespace, &doc.key), &doc.body)?
                .into_bytes()
        } else {
            doc.body.clone()
        };
        let sql = format!(
            "INSERT INTO panel_documents (namespace, doc_key, body, encrypted, content_sha256, size_bytes, file_mode, phase) \
             VALUES ({ns}, {key}, {body}, {enc}, '{fp}', {size}, {mode}, {phase}) \
             ON DUPLICATE KEY UPDATE body = VALUES(body), encrypted = VALUES(encrypted), \
             content_sha256 = VALUES(content_sha256), size_bytes = VALUES(size_bytes), \
             file_mode = VALUES(file_mode), phase = VALUES(phase), revision = revision + 1;",
            ns = text_lit(&doc.namespace),
            key = text_lit(&doc.key),
            body = hex_lit(&stored),
            enc = u8::from(doc.encrypt),
            size = doc.body.len(),
            mode = doc.mode & 0o777,
            phase = doc.phase,
        );
        self.sql.run(Some(&self.db), &sql)?;
        Ok(if prior.is_some() {
            PutOutcome::Updated
        } else {
            PutOutcome::Inserted
        })
    }

    fn delete(&self, namespace: &str, key: &str) -> Result<bool, String> {
        let existed = self.existing(namespace, key)?.is_some();
        self.sql.run(
            Some(&self.db),
            &format!(
                "DELETE FROM panel_documents WHERE namespace = {} AND doc_key = {}",
                text_lit(namespace),
                text_lit(key)
            ),
        )?;
        Ok(existed)
    }
}

#[cfg(test)]
pub(crate) mod mem {
    use super::*;
    use std::cell::RefCell;
    use std::collections::BTreeMap;

    /// Test double that applies the same encryption and fingerprint rules.
    pub struct MemDocStore {
        pub key: PanelKey,
        pub rows: RefCell<BTreeMap<(String, String), (Vec<u8>, bool, u32, u8, u64)>>,
    }

    impl MemDocStore {
        pub fn new(key: PanelKey) -> Self {
            Self {
                key,
                rows: RefCell::default(),
            }
        }

        fn fp(&self, enc: bool, plain: &[u8]) -> String {
            if enc {
                self.key.fingerprint(plain)
            } else {
                sha256_hex(plain)
            }
        }

        /// Raw stored bytes, to assert secrets are not stored in the clear.
        pub fn stored_bytes(&self, ns: &str, key: &str) -> Option<Vec<u8>> {
            self.rows
                .borrow()
                .get(&(ns.to_string(), key.to_string()))
                .map(|r| r.0.clone())
        }
    }

    impl DocStore for MemDocStore {
        fn fingerprint(&self, doc: &Doc) -> String {
            self.fp(doc.encrypt, &doc.body)
        }

        fn list(&self) -> Result<Vec<DocMeta>, String> {
            let mut out = Vec::new();
            for ((ns, k), (stored, enc, _, _, rev)) in self.rows.borrow().iter() {
                let plain = if *enc {
                    self.key
                        .decrypt(&aad(ns, k), std::str::from_utf8(stored).unwrap())?
                } else {
                    stored.clone()
                };
                out.push(DocMeta {
                    namespace: ns.clone(),
                    key: k.clone(),
                    fingerprint: self.fp(*enc, &plain),
                    encrypted: *enc,
                    revision: *rev,
                });
            }
            Ok(out)
        }

        fn get(&self, namespace: &str, key: &str) -> Result<Option<Doc>, String> {
            let rows = self.rows.borrow();
            let Some((stored, enc, mode, phase, _)) =
                rows.get(&(namespace.to_string(), key.to_string()))
            else {
                return Ok(None);
            };
            let body = if *enc {
                self.key
                    .decrypt(&aad(namespace, key), std::str::from_utf8(stored).unwrap())?
            } else {
                stored.clone()
            };
            Ok(Some(Doc {
                namespace: namespace.into(),
                key: key.into(),
                body,
                encrypt: *enc,
                mode: *mode,
                phase: *phase,
            }))
        }

        fn put(&self, doc: &Doc) -> Result<PutOutcome, String> {
            let id = (doc.namespace.clone(), doc.key.clone());
            let fp = self.fp(doc.encrypt, &doc.body);
            let prior = self
                .list()?
                .into_iter()
                .find(|m| m.namespace == doc.namespace && m.key == doc.key);
            if let Some(m) = &prior
                && m.fingerprint == fp
                && m.encrypted == doc.encrypt
            {
                return Ok(PutOutcome::Unchanged);
            }
            let stored = if doc.encrypt {
                self.key
                    .encrypt(&aad(&doc.namespace, &doc.key), &doc.body)?
                    .into_bytes()
            } else {
                doc.body.clone()
            };
            let rev = prior.as_ref().map(|m| m.revision + 1).unwrap_or(1);
            self.rows
                .borrow_mut()
                .insert(id, (stored, doc.encrypt, doc.mode, doc.phase, rev));
            Ok(if prior.is_some() {
                PutOutcome::Updated
            } else {
                PutOutcome::Inserted
            })
        }

        fn delete(&self, namespace: &str, key: &str) -> Result<bool, String> {
            Ok(self
                .rows
                .borrow_mut()
                .remove(&(namespace.to_string(), key.to_string()))
                .is_some())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::client::fake::FakeSql;
    use super::mem::MemDocStore;
    use super::*;

    fn key() -> PanelKey {
        super::super::crypto::panel_key_test_fixture("panel-db-docs-store")
    }

    fn doc(encrypt: bool, body: &[u8]) -> Doc {
        Doc {
            namespace: "mail-accounts".into(),
            key: "mail-accounts.json".into(),
            body: body.to_vec(),
            encrypt,
            mode: 0o600,
            phase: 2,
        }
    }

    #[test]
    fn mem_store_put_get_unchanged_update() {
        let store = MemDocStore::new(key());
        assert_eq!(store.put(&doc(false, b"{}")).unwrap(), PutOutcome::Inserted);
        assert_eq!(
            store.put(&doc(false, b"{}")).unwrap(),
            PutOutcome::Unchanged
        );
        assert_eq!(
            store.put(&doc(false, b"{\"a\":1}")).unwrap(),
            PutOutcome::Updated
        );
        let got = store
            .get("mail-accounts", "mail-accounts.json")
            .unwrap()
            .unwrap();
        assert_eq!(got.body, b"{\"a\":1}");
        assert_eq!(store.list().unwrap()[0].revision, 2);
    }

    #[test]
    fn secrets_are_stored_encrypted_and_read_back_plain() {
        let store = MemDocStore::new(key());
        store.put(&doc(true, b"hunter2-secret")).unwrap();
        let raw = store
            .stored_bytes("mail-accounts", "mail-accounts.json")
            .unwrap();
        assert!(!String::from_utf8_lossy(&raw).contains("hunter2"));
        let back = store
            .get("mail-accounts", "mail-accounts.json")
            .unwrap()
            .unwrap();
        assert_eq!(back.body, b"hunter2-secret");
    }

    #[test]
    fn maria_put_emits_hex_literals_and_no_plain_secret() {
        let sql = FakeSql::default();
        let store = MariaDocStore::new(&sql, "cpn_panel", key());
        store.put(&doc(true, b"super-secret-value")).unwrap();
        let stmts = sql.statements();
        let insert = stmts.iter().find(|s| s.starts_with("INSERT")).unwrap();
        assert!(insert.contains("X'"));
        assert!(!insert.contains("super-secret-value"));
        let plain_hex = data_encoding::HEXUPPER.encode(b"super-secret-value");
        assert!(!insert.contains(&plain_hex));
        assert!(insert.contains("ON DUPLICATE KEY UPDATE"));
    }

    #[test]
    fn maria_put_skips_when_fingerprint_matches() {
        let sql = FakeSql::default();
        let store = MariaDocStore::new(&sql, "cpn_panel", key());
        let d = doc(false, b"{}");
        let fp = store.fingerprint_for(&d);
        let ns = data_encoding::HEXUPPER.encode(b"mail-accounts");
        let k = data_encoding::HEXUPPER.encode(b"mail-accounts.json");
        sql.reply(
            "FROM panel_documents WHERE",
            Ok(&format!("{ns}\t{k}\t{fp}\t0\t3\n")),
        );
        assert_eq!(store.put(&d).unwrap(), PutOutcome::Unchanged);
        assert!(!sql.statements().iter().any(|s| s.starts_with("INSERT")));
    }

    #[test]
    fn maria_get_decrypts_envelope() {
        let sql = FakeSql::default();
        let k = key();
        let envelope = k.encrypt(&aad("ns", "k"), b"plain-bytes").unwrap();
        let hex = data_encoding::HEXUPPER.encode(envelope.as_bytes());
        sql.reply("SELECT HEX(body)", Ok(&format!("{hex}\t1\t384\t4\n")));
        let store = MariaDocStore::new(&sql, "cpn_panel", k);
        let got = store.get("ns", "k").unwrap().unwrap();
        assert_eq!(got.body, b"plain-bytes");
        assert_eq!(got.phase, 4);
    }
}
