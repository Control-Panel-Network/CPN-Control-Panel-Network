//! Postfix queue inspect / flush / delete (allowlisted postqueue / postsuper).

use crate::panel_ops_security::which_exists;
use std::process::Command;

fn validate_queue_id(id: &str) -> Result<&str, String> {
    let id = id.trim();
    if id.is_empty() || id.len() > 32 {
        return Err("Queue id is invalid".into());
    }
    if !id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
    {
        return Err("Queue id may only contain letters, digits, ., -".into());
    }
    Ok(id)
}

pub fn queue_available() -> bool {
    which_exists("postqueue") || which_exists("postsuper")
}

pub fn list_queue() -> Result<String, String> {
    if !which_exists("postqueue") {
        return Err("postqueue is not installed (Postfix tools missing)".into());
    }
    let out = Command::new("postqueue")
        .arg("-p")
        .output()
        .map_err(|e| format!("Could not run postqueue: {e}"))?;
    let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
    if !out.status.success() && stdout.is_empty() {
        return Err(if stderr.is_empty() {
            "postqueue -p failed".into()
        } else {
            stderr
        });
    }
    if stdout.is_empty() {
        Ok("Mail queue is empty.".into())
    } else {
        Ok(stdout)
    }
}

pub fn flush_queue() -> Result<String, String> {
    if !which_exists("postqueue") {
        return Err("postqueue is not installed".into());
    }
    let out = Command::new("postqueue")
        .arg("-f")
        .output()
        .map_err(|e| format!("Could not run postqueue -f: {e}"))?;
    if out.status.success() {
        Ok("Queue flush requested (postqueue -f).".into())
    } else {
        let err = String::from_utf8_lossy(&out.stderr);
        Err(format!("postqueue -f failed: {}", err.trim()))
    }
}

pub fn delete_queue_id(id: &str) -> Result<String, String> {
    let id = validate_queue_id(id)?;
    if !which_exists("postsuper") {
        return Err("postsuper is not installed".into());
    }
    let out = Command::new("postsuper")
        .args(["-d", id])
        .output()
        .map_err(|e| format!("Could not run postsuper: {e}"))?;
    if out.status.success() {
        Ok(format!("Deleted queue id `{id}`."))
    } else {
        let err = String::from_utf8_lossy(&out.stderr);
        Err(format!("postsuper -d failed: {}", err.trim()))
    }
}

pub fn delete_all_deferred() -> Result<String, String> {
    if !which_exists("postsuper") {
        return Err("postsuper is not installed".into());
    }
    let out = Command::new("postsuper")
        .args(["-d", "ALL"])
        .output()
        .map_err(|e| format!("Could not run postsuper: {e}"))?;
    if out.status.success() {
        Ok("Deleted all messages from the Postfix queue (postsuper -d ALL).".into())
    } else {
        let err = String::from_utf8_lossy(&out.stderr);
        Err(format!("postsuper -d ALL failed: {}", err.trim()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_bad_queue_id() {
        assert!(validate_queue_id("../etc").is_err());
        assert!(validate_queue_id("ABC123").is_ok());
    }
}
