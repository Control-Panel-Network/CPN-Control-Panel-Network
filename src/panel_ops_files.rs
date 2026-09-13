//! Mutating filesystem ops for Root File Manager (admin-only callers).

use crate::panel_ops_path::{
    is_protected_path, join_child, resolve_under_allowlist, validate_entry_name,
};
use crate::panel_session::session_secret;
use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;
use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

type HmacSha256 = Hmac<Sha256>;

pub const MAX_EDIT_BYTES: u64 = 2 * 1024 * 1024;
pub const MAX_UPLOAD_BYTES: u64 = 64 * 1024 * 1024;
const RATE_WINDOW_SECS: u64 = 60;
const RATE_MAX_OPS: u32 = 120;

static RATE: Mutex<Option<HashMap<String, (u64, u32)>>> = Mutex::new(None);

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn hmac_hex(secret: &str, payload: &str) -> String {
    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key length");
    mac.update(payload.as_bytes());
    mac.finalize()
        .into_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn verify_hmac_hex(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.bytes().zip(b.bytes()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// CSRF token for file manager POST actions (bound to user + hour bucket).
pub fn files_csrf_token(username: &str) -> String {
    let secret = session_secret(None);
    let hour = now_unix() / 3600;
    let payload = format!("files|{username}|{hour}");
    format!("{hour}.{}", hmac_hex(&secret, &payload))
}

pub fn verify_files_csrf(username: &str, token: &str) -> bool {
    let secret = session_secret(None);
    let Some((hour_s, sig)) = token.split_once('.') else {
        return false;
    };
    let Ok(hour) = hour_s.parse::<u64>() else {
        return false;
    };
    let current = now_unix() / 3600;
    if hour + 2 < current || hour > current + 1 {
        return false;
    }
    let payload = format!("files|{username}|{hour}");
    let expected = hmac_hex(&secret, &payload);
    expected == sig && verify_hmac_hex(&expected, sig)
}

pub fn check_rate_limit(username: &str) -> Result<(), String> {
    let now = now_unix();
    let mut guard = RATE.lock().map_err(|_| "Rate limiter busy".to_string())?;
    let map = guard.get_or_insert_with(HashMap::new);
    let entry = map.entry(username.to_string()).or_insert((now, 0));
    if now.saturating_sub(entry.0) >= RATE_WINDOW_SECS {
        *entry = (now, 0);
    }
    if entry.1 >= RATE_MAX_OPS {
        return Err("Too many file operations; wait a minute and try again".into());
    }
    entry.1 += 1;
    Ok(())
}

fn ensure_writable_target(path: &Path) -> Result<(), String> {
    if is_protected_path(path) {
        return Err(format!(
            "Refusing to modify protected path {}",
            path.display()
        ));
    }
    Ok(())
}

pub fn mkdir(parent: &str, name: &str) -> Result<String, String> {
    let parent = resolve_under_allowlist(parent)?;
    let dest = join_child(&parent, name)?;
    ensure_writable_target(&dest)?;
    fs::create_dir(&dest).map_err(|e| format!("mkdir failed: {e}"))?;
    Ok(format!("Created folder {}", dest.display()))
}

pub fn create_file(parent: &str, name: &str) -> Result<String, String> {
    let parent = resolve_under_allowlist(parent)?;
    let dest = join_child(&parent, name)?;
    ensure_writable_target(&dest)?;
    if dest.exists() {
        return Err("File already exists".into());
    }
    fs::File::create(&dest).map_err(|e| format!("create failed: {e}"))?;
    Ok(format!("Created file {}", dest.display()))
}

pub fn read_text(path: &str) -> Result<String, String> {
    let path = resolve_under_allowlist(path)?;
    let meta = fs::metadata(&path).map_err(|e| format!("Cannot read: {e}"))?;
    if meta.is_dir() {
        return Err("Cannot edit a directory".into());
    }
    if meta.len() > MAX_EDIT_BYTES {
        return Err(format!(
            "File too large to edit (max {} bytes)",
            MAX_EDIT_BYTES
        ));
    }
    let mut f = fs::File::open(&path).map_err(|e| format!("open failed: {e}"))?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf)
        .map_err(|e| format!("read failed: {e}"))?;
    if buf.iter().any(|&b| b == 0) {
        return Err("Binary files cannot be edited in the text editor".into());
    }
    String::from_utf8(buf).map_err(|_| "File is not valid UTF-8".to_string())
}

pub fn write_text(path: &str, content: &str) -> Result<String, String> {
    let path = resolve_under_allowlist(path)?;
    ensure_writable_target(&path)?;
    if content.len() as u64 > MAX_EDIT_BYTES {
        return Err("Content exceeds edit size limit".into());
    }
    let mut f = fs::File::create(&path).map_err(|e| format!("write failed: {e}"))?;
    f.write_all(content.as_bytes())
        .map_err(|e| format!("write failed: {e}"))?;
    Ok(format!("Saved {}", path.display()))
}

pub fn upload_bytes(parent: &str, filename: &str, data: &[u8]) -> Result<String, String> {
    if data.len() as u64 > MAX_UPLOAD_BYTES {
        return Err(format!(
            "Upload exceeds {} byte limit",
            MAX_UPLOAD_BYTES
        ));
    }
    let parent = resolve_under_allowlist(parent)?;
    let name = validate_entry_name(filename)?;
    let dest = join_child(&parent, name)?;
    ensure_writable_target(&dest)?;
    let mut f = fs::File::create(&dest).map_err(|e| format!("upload failed: {e}"))?;
    f.write_all(data)
        .map_err(|e| format!("upload failed: {e}"))?;
    Ok(format!("Uploaded {}", dest.display()))
}

pub fn delete_names(parent: &str, names: &[String]) -> Result<String, String> {
    if names.is_empty() {
        return Err("Nothing selected".into());
    }
    let parent = resolve_under_allowlist(parent)?;
    let mut done = 0usize;
    for name in names {
        let target = join_child(&parent, name)?;
        ensure_writable_target(&target)?;
        if target.is_dir() {
            fs::remove_dir_all(&target).map_err(|e| format!("delete {}: {e}", target.display()))?;
        } else {
            fs::remove_file(&target).map_err(|e| format!("delete {}: {e}", target.display()))?;
        }
        done += 1;
    }
    Ok(format!("Deleted {done} item(s)"))
}

pub fn rename_entry(parent: &str, from: &str, to: &str) -> Result<String, String> {
    let parent = resolve_under_allowlist(parent)?;
    let src = join_child(&parent, from)?;
    let dest = join_child(&parent, to)?;
    ensure_writable_target(&src)?;
    ensure_writable_target(&dest)?;
    if dest.exists() {
        return Err("Target name already exists".into());
    }
    fs::rename(&src, &dest).map_err(|e| format!("rename failed: {e}"))?;
    Ok(format!("Renamed to {}", dest.display()))
}

fn copy_recursive(src: &Path, dest: &Path) -> Result<(), String> {
    if src.is_dir() {
        fs::create_dir_all(dest).map_err(|e| format!("copy mkdir: {e}"))?;
        for ent in fs::read_dir(src).map_err(|e| format!("copy read: {e}"))? {
            let ent = ent.map_err(|e| format!("copy entry: {e}"))?;
            let name = ent.file_name();
            copy_recursive(&ent.path(), &dest.join(name))?;
        }
    } else {
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("copy parent: {e}"))?;
        }
        fs::copy(src, dest).map_err(|e| format!("copy failed: {e}"))?;
    }
    Ok(())
}

pub fn copy_entries(parent: &str, names: &[String], dest_dir: &str) -> Result<String, String> {
    if names.is_empty() {
        return Err("Nothing selected".into());
    }
    let parent = resolve_under_allowlist(parent)?;
    let dest_dir = resolve_under_allowlist(dest_dir)?;
    if !dest_dir.is_dir() {
        return Err("Destination must be a directory".into());
    }
    let mut done = 0usize;
    for name in names {
        let src = join_child(&parent, name)?;
        let dest = join_child(&dest_dir, name)?;
        ensure_writable_target(&dest)?;
        if dest.exists() {
            return Err(format!("{} already exists in destination", name));
        }
        copy_recursive(&src, &dest)?;
        done += 1;
    }
    Ok(format!("Copied {done} item(s) to {}", dest_dir.display()))
}

pub fn move_entries(parent: &str, names: &[String], dest_dir: &str) -> Result<String, String> {
    if names.is_empty() {
        return Err("Nothing selected".into());
    }
    let parent = resolve_under_allowlist(parent)?;
    let dest_dir = resolve_under_allowlist(dest_dir)?;
    if !dest_dir.is_dir() {
        return Err("Destination must be a directory".into());
    }
    let mut done = 0usize;
    for name in names {
        let src = join_child(&parent, name)?;
        let dest = join_child(&dest_dir, name)?;
        ensure_writable_target(&src)?;
        ensure_writable_target(&dest)?;
        if dest.exists() {
            return Err(format!("{} already exists in destination", name));
        }
        match fs::rename(&src, &dest) {
            Ok(()) => {}
            Err(_) => {
                copy_recursive(&src, &dest)?;
                if src.is_dir() {
                    fs::remove_dir_all(&src).map_err(|e| format!("move cleanup: {e}"))?;
                } else {
                    fs::remove_file(&src).map_err(|e| format!("move cleanup: {e}"))?;
                }
            }
        }
        done += 1;
    }
    Ok(format!("Moved {done} item(s) to {}", dest_dir.display()))
}

pub fn resolve_cwd(path: &str) -> Result<PathBuf, String> {
    resolve_under_allowlist(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csrf_roundtrip() {
        let t = files_csrf_token("admin");
        assert!(verify_files_csrf("admin", &t));
        assert!(!verify_files_csrf("other", &t));
    }
}
