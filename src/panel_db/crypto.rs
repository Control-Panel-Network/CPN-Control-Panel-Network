//! Application-level encryption for secrets stored in MariaDB.
//!
//! The key lives only in `panel-db.key` (root 0600, outside the database), so
//! a database dump or a phpMyAdmin view of the table alone never exposes
//! secrets. Envelope: `cpn1:<key id>:<base64(nonce || ciphertext+tag)>`.
//! The row identity (namespace and key) is bound in as AEAD associated data,
//! so ciphertext cannot be moved between rows undetected.

use super::config::key_path;
use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
use hmac::{Hmac, Mac};
use rand::Rng;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use std::path::Path;

const PREFIX: &str = "cpn1";

type HmacSha256 = Hmac<Sha256>;

#[derive(Clone)]
pub struct PanelKey([u8; 32]);

impl std::fmt::Debug for PanelKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PanelKey([redacted])")
    }
}

impl PanelKey {
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Short non-secret identifier so a wrong or rotated key is detected early.
    pub fn id(&self) -> String {
        let digest = Sha256::digest(self.0);
        data_encoding::HEXLOWER.encode(&digest[..4])
    }

    /// Load the key; create it (exclusive, 0600) only when `create` is set.
    pub fn load(create: bool) -> Result<Self, String> {
        Self::load_at(&key_path(), create)
    }

    pub fn load_at(path: &Path, create: bool) -> Result<Self, String> {
        match fs::read(path) {
            Ok(raw) => {
                let bytes: [u8; 32] = raw
                    .as_slice()
                    .try_into()
                    .map_err(|_| "Panel database key file has the wrong length".to_string())?;
                Ok(Self(bytes))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound && create => {
                let bytes: [u8; 32] = rand::rng().random();
                create_key_file(path, &bytes)?;
                Ok(Self(bytes))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                Err("Panel database key file is missing".into())
            }
            Err(e) => Err(format!("Could not read panel database key: {e}")),
        }
    }

    pub fn encrypt(&self, aad: &str, plain: &[u8]) -> Result<String, String> {
        let cipher = Aes256Gcm::new_from_slice(&self.0).map_err(|_| "AES key error")?;
        let nonce_bytes: [u8; 12] = rand::rng().random();
        let nonce = Nonce::from(nonce_bytes);
        let sealed = cipher
            .encrypt(
                &nonce,
                Payload {
                    msg: plain,
                    aad: aad.as_bytes(),
                },
            )
            .map_err(|_| "Could not encrypt panel secret".to_string())?;
        let mut blob = nonce_bytes.to_vec();
        blob.extend_from_slice(&sealed);
        Ok(format!("{PREFIX}:{}:{}", self.id(), B64.encode(blob)))
    }

    pub fn decrypt(&self, aad: &str, envelope: &str) -> Result<Vec<u8>, String> {
        let mut parts = envelope.trim().splitn(3, ':');
        let (Some(prefix), Some(key_id), Some(payload)) =
            (parts.next(), parts.next(), parts.next())
        else {
            return Err("Corrupt encrypted panel value".into());
        };
        if prefix != PREFIX {
            return Err("Unknown encrypted panel value format".into());
        }
        if key_id != self.id() {
            return Err("Encrypted panel value was written with a different key".into());
        }
        let blob = B64
            .decode(payload.as_bytes())
            .map_err(|_| "Corrupt encrypted panel value".to_string())?;
        if blob.len() < 12 + 16 {
            return Err("Corrupt encrypted panel value".into());
        }
        let (nonce_bytes, sealed) = blob.split_at(12);
        let nonce_arr: [u8; 12] = nonce_bytes.try_into().map_err(|_| "Corrupt nonce")?;
        let cipher = Aes256Gcm::new_from_slice(&self.0).map_err(|_| "AES key error")?;
        cipher
            .decrypt(
                &Nonce::from(nonce_arr),
                Payload {
                    msg: sealed,
                    aad: aad.as_bytes(),
                },
            )
            .map_err(|_| "Could not decrypt panel secret (wrong key or moved row)".to_string())
    }

    /// Keyed fingerprint for encrypted rows (plain SHA-256 would let a dump
    /// holder confirm guesses of low-entropy secrets).
    pub fn fingerprint(&self, plain: &[u8]) -> String {
        let mut mac =
            <HmacSha256 as KeyInit>::new_from_slice(&self.0).expect("HMAC accepts any key");
        mac.update(plain);
        data_encoding::HEXLOWER.encode(&mac.finalize().into_bytes())
    }
}

/// Unkeyed fingerprint for non-secret rows.
pub fn sha256_hex(plain: &[u8]) -> String {
    data_encoding::HEXLOWER.encode(&Sha256::digest(plain))
}

fn create_key_file(path: &Path, bytes: &[u8; 32]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Could not create {}: {e}", parent.display()))?;
    }
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|e| format!("Could not create panel database key: {e}"))?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|e| format!("Could not write panel database key: {e}"))
}

/// Random key per label, generated once per test process: stable for a label
/// (encrypt then decrypt), distinct across labels, and never hard-coded.
#[cfg(test)]
pub(crate) fn panel_key_test_fixture(label: &str) -> PanelKey {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};
    static KEYS: OnceLock<Mutex<HashMap<String, [u8; 32]>>> = OnceLock::new();
    let mut keys = KEYS
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let bytes = *keys
        .entry(label.to_string())
        .or_insert_with(|| rand::rng().random());
    PanelKey::from_bytes(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key() -> PanelKey {
        panel_key_test_fixture("panel-db-crypto-primary")
    }

    #[test]
    fn encrypt_decrypt_round_trip() {
        let sealed = key().encrypt("mfa\0ntowner.json", b"totp-secret").unwrap();
        assert!(sealed.starts_with("cpn1:"));
        assert!(!sealed.contains("totp-secret"));
        let plain = key().decrypt("mfa\0ntowner.json", &sealed).unwrap();
        assert_eq!(plain, b"totp-secret");
    }

    #[test]
    fn ciphertext_is_bound_to_its_row() {
        let sealed = key().encrypt("a\0one", b"x").unwrap();
        assert!(key().decrypt("a\0two", &sealed).is_err());
    }

    #[test]
    fn wrong_key_is_rejected_with_clear_error() {
        let sealed = key().encrypt("a\0b", b"x").unwrap();
        let other = panel_key_test_fixture("panel-db-crypto-other");
        let err = other.decrypt("a\0b", &sealed).unwrap_err();
        assert!(err.contains("different key"));
    }

    #[test]
    fn tampering_is_detected() {
        let sealed = key().encrypt("a\0b", b"hello world").unwrap();
        let mut chars: Vec<char> = sealed.chars().collect();
        let last = chars.len() - 3;
        chars[last] = if chars[last] == 'A' { 'B' } else { 'A' };
        let tampered: String = chars.into_iter().collect();
        assert!(key().decrypt("a\0b", &tampered).is_err());
    }

    #[test]
    fn fingerprints_are_keyed_for_secrets() {
        let a = key().fingerprint(b"1234");
        let b = panel_key_test_fixture("panel-db-crypto-fingerprint-b").fingerprint(b"1234");
        assert_ne!(a, b);
        assert_ne!(a, sha256_hex(b"1234"));
        assert_eq!(a.len(), 64);
    }

    #[test]
    fn key_file_is_created_once_and_reused() {
        let dir = std::env::temp_dir().join(format!("cpn-pdb-key-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let path = dir.join("panel-db.key");
        assert!(PanelKey::load_at(&path, false).is_err());
        let first = PanelKey::load_at(&path, true).unwrap();
        let again = PanelKey::load_at(&path, false).unwrap();
        assert_eq!(first.id(), again.id());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        let _ = fs::remove_dir_all(&dir);
    }
}
