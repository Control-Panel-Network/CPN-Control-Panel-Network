//! Secret hygiene: strength checks and CSPRNG helpers shared by every secret generator.
//!
//! CPN never ships a static key, token, or password. Every secret is created from the
//! operating-system CSPRNG on first install and is re-checked on every panel start
//! (including after a tip upgrade) by [`crate::secret_hygiene_audit`]. A secret that is
//! empty, short, repeating, low-entropy, or that matches a well-known default is rotated.
//!
//! Nothing in this module prints or logs a secret value; reports carry lengths and
//! entropy estimates only.

use rand::{Rng, distr::Alphanumeric};
use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::path::Path;

/// Minimum accepted length for a text secret (session HMAC key, tokens, blowfish).
pub const MIN_SECRET_LEN: usize = 32;

/// Substrings and prefixes of well-known defaults. Matching is case-insensitive.
/// Entries of six or more characters match anywhere; shorter ones only as a prefix
/// (a random secret contains a six-character marker with negligible probability).
pub const KNOWN_BAD_MARKERS: &[&str] = &[
    "password",
    "passw0rd",
    "changeme",
    "change-me",
    "change_me",
    "secret",
    "admin123",
    "letmein",
    "qwerty",
    "default",
    "example",
    "sample",
    "insecure",
    "hunter2",
    "cpnlab",
    "bytheway",
    "youcanchange",
    "your-secret",
    "your_secret",
    "yoursecret",
    "123456",
    "abcdef",
    "admin",
    "root",
    "test",
    "demo",
    "cpn",
    "mra",
];

/// Why a secret was rejected. Carries no secret material.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeakReason {
    Empty,
    TooShort,
    KnownDefault,
    LowEntropy,
    Repeating,
}

impl WeakReason {
    pub fn label(self) -> &'static str {
        match self {
            Self::Empty => "empty",
            Self::TooShort => "too_short",
            Self::KnownDefault => "known_default",
            Self::LowEntropy => "low_entropy",
            Self::Repeating => "repeating_pattern",
        }
    }
}

/// Shannon entropy in bits per symbol of `data` (0.0 for empty input).
pub fn entropy_bits_per_symbol(data: &[u8]) -> f64 {
    if data.is_empty() {
        return 0.0;
    }
    let mut counts = [0usize; 256];
    for byte in data {
        counts[*byte as usize] += 1;
    }
    let len = data.len() as f64;
    counts
        .iter()
        .filter(|count| **count > 0)
        .map(|count| {
            let p = *count as f64 / len;
            -p * p.log2()
        })
        .sum()
}

fn has_short_period(data: &[u8]) -> bool {
    let n = data.len();
    (1..=8usize).any(|period| {
        n >= period * 3 && data.iter().enumerate().all(|(i, b)| *b == data[i % period])
    })
}

fn matches_known_default(lower: &str) -> bool {
    KNOWN_BAD_MARKERS.iter().any(|marker| {
        if marker.len() >= 6 {
            lower.contains(marker)
        } else {
            lower.starts_with(marker)
        }
    })
}

/// Classify a text secret. `None` means the secret passes every check.
pub fn weak_reason(secret: &str, min_len: usize) -> Option<WeakReason> {
    let value = secret.trim();
    if value.is_empty() {
        return Some(WeakReason::Empty);
    }
    if value.len() < min_len {
        return Some(WeakReason::TooShort);
    }
    if matches_known_default(&value.to_ascii_lowercase()) {
        return Some(WeakReason::KnownDefault);
    }
    let bytes = value.as_bytes();
    if has_short_period(bytes) {
        return Some(WeakReason::Repeating);
    }
    let distinct: HashSet<u8> = bytes.iter().copied().collect();
    if distinct.len() < 8 || entropy_bits_per_symbol(bytes) < 3.0 {
        return Some(WeakReason::LowEntropy);
    }
    None
}

/// Classify binary key material (for example a 32 byte AES key).
pub fn weak_key_bytes(key: &[u8], expected_len: usize) -> Option<WeakReason> {
    if key.is_empty() {
        return Some(WeakReason::Empty);
    }
    if key.len() != expected_len {
        return Some(WeakReason::TooShort);
    }
    if has_short_period(key) {
        return Some(WeakReason::Repeating);
    }
    let distinct: HashSet<u8> = key.iter().copied().collect();
    if distinct.len() < 8 {
        return Some(WeakReason::LowEntropy);
    }
    None
}

/// Random alphanumeric secret from the OS-seeded CSPRNG.
pub fn random_alnum(len: usize) -> String {
    rand::rng()
        .sample_iter(&Alphanumeric)
        .take(len)
        .map(char::from)
        .collect()
}

/// Random lowercase hex secret from `byte_len` random bytes (`2 * byte_len` characters).
pub fn random_hex(byte_len: usize) -> String {
    let mut out = String::with_capacity(byte_len * 2);
    let mut rng = rand::rng();
    for _ in 0..byte_len {
        let byte: u8 = rng.random();
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

/// Random alphanumeric secret that also passes [`weak_reason`] (retries on the rare
/// accidental match with a marker). `len` is raised to [`MIN_SECRET_LEN`] when smaller.
pub fn strong_alnum(len: usize) -> String {
    let len = len.max(MIN_SECRET_LEN);
    loop {
        let candidate = random_alnum(len);
        if weak_reason(&candidate, MIN_SECRET_LEN).is_none() {
            return candidate;
        }
    }
}

/// Random hex secret that also passes [`weak_reason`] (`byte_len` raised to 16 or more).
pub fn strong_hex(byte_len: usize) -> String {
    let byte_len = byte_len.max(MIN_SECRET_LEN / 2);
    loop {
        let candidate = random_hex(byte_len);
        if weak_reason(&candidate, MIN_SECRET_LEN).is_none() {
            return candidate;
        }
    }
}

/// Write `bytes` to `path` with mode 0600 from the first byte (no world-readable window).
/// The write goes through a sibling temp file and a rename so a crash never leaves a
/// half-written secret behind.
pub fn write_private(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Could not create {}: {e}", parent.display()))?;
    }
    let tmp = path.with_extension("tmp-secret");
    let _ = fs::remove_file(&tmp);
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(&tmp)
        .map_err(|e| format!("Could not create {}: {e}", tmp.display()))?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|e| format!("Could not write {}: {e}", tmp.display()))?;
    drop(file);
    fs::rename(&tmp, path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        format!("Could not install {}: {e}", path.display())
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

#[cfg(test)]
#[path = "secret_hygiene_tests.rs"]
mod tests;
