//! Autoresponders (vacation) via Dovecot Sieve scripts under mailbox homes.

use crate::mail_postfix_maps::{can_manage_mail_domain, normalize_email};
use crate::panel_ops_mailbox_provision::local_part;
use crate::paths::join_data;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Autoresponder {
    pub id: String,
    pub address: String,
    pub subject: String,
    pub body: String,
    /// Owner panel account (site owner of the mailbox domain).
    #[serde(default)]
    pub owner: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub updated_at_unix: u64,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct Store {
    #[serde(default)]
    items: Vec<Autoresponder>,
}

fn store_path() -> PathBuf {
    join_data("mail-autoresponders.json")
}

fn load_store() -> Store {
    fs::read_to_string(store_path())
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

fn save_store(store: &Store) -> Result<(), String> {
    let path = store_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let raw = serde_json::to_string_pretty(store).map_err(|e| e.to_string())?;
    fs::write(&path, raw).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

pub fn list_autoresponders() -> Vec<Autoresponder> {
    load_store().items
}

pub fn count_for_owner(owner: &str) -> u64 {
    list_autoresponders()
        .into_iter()
        .filter(|a| a.owner.trim().eq_ignore_ascii_case(owner.trim()))
        .count() as u64
}

fn sieve_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn write_vacation_sieve(item: &Autoresponder) -> Result<(), String> {
    let user = local_part(&item.address)?;
    let home = PathBuf::from(format!("/home/{user}"));
    if !home.is_dir() {
        return Err(format!(
            "Mailbox system user `{user}` home is missing; create the mailbox first"
        ));
    }
    let sieve_dir = home.join("sieve");
    fs::create_dir_all(&sieve_dir).map_err(|e| format!("Cannot create sieve dir: {e}"))?;
    let script = if item.enabled {
        format!(
            "require [\"vacation\"];\nvacation :days 7 :addresses [\"{addr}\"] :subject \"{subj}\"\n\"{body}\";\n",
            addr = sieve_escape(&item.address),
            subj = sieve_escape(&item.subject),
            body = sieve_escape(&item.body),
        )
    } else {
        "# CPN autoresponder disabled\n".into()
    };
    let vacation_path = sieve_dir.join("cpn-vacation.sieve");
    fs::write(&vacation_path, script).map_err(|e| format!("Cannot write vacation sieve: {e}"))?;
    rebuild_active_sieve(&home)?;
    #[cfg(unix)]
    {
        let _ = std::process::Command::new("chown")
            .args(["-R", &format!("{user}:{user}"), sieve_dir.to_string_lossy().as_ref()])
            .status();
        let active = home.join(".dovecot.sieve");
        let _ = std::process::Command::new("chown")
            .args([&format!("{user}:{user}"), active.to_string_lossy().as_ref()])
            .status();
    }
    Ok(())
}

/// Merge CPN vacation + filters into `~/.dovecot.sieve`.
pub fn rebuild_active_sieve(home: &Path) -> Result<(), String> {
    let sieve_dir = home.join("sieve");
    let mut parts = String::from("# Managed by CPN (autoresponder + filters). Edit via the panel.\n");
    let vacation = sieve_dir.join("cpn-vacation.sieve");
    let filters = sieve_dir.join("cpn-filters.sieve");
    if vacation.is_file() {
        let body = fs::read_to_string(&vacation).unwrap_or_default();
        if body.lines().any(|l| !l.trim().is_empty() && !l.trim().starts_with('#')) {
            parts.push_str(&body);
            if !parts.ends_with('\n') {
                parts.push('\n');
            }
        }
    }
    if filters.is_file() {
        let body = fs::read_to_string(&filters).unwrap_or_default();
        if body.lines().any(|l| !l.trim().is_empty() && !l.trim().starts_with('#')) {
            parts.push_str(&body);
            if !parts.ends_with('\n') {
                parts.push('\n');
            }
        }
    }
    if parts.lines().all(|l| l.trim().is_empty() || l.trim().starts_with('#')) {
        parts.push_str("keep;\n");
    }
    let active = home.join(".dovecot.sieve");
    fs::write(&active, parts).map_err(|e| format!("Cannot write active sieve: {e}"))
}

pub fn upsert_autoresponder(
    actor: &str,
    address: &str,
    subject: &str,
    body: &str,
    enabled: bool,
) -> Result<String, String> {
    let address = normalize_email(address)?;
    let domain = address
        .rsplit_once('@')
        .map(|(_, d)| d)
        .ok_or_else(|| "Invalid address".to_string())?;
    if !can_manage_mail_domain(actor, domain) {
        return Err("You cannot manage autoresponders for this domain".into());
    }
    let subject = subject.trim();
    let body = body.trim();
    if subject.is_empty() || subject.len() > 200 {
        return Err("Subject is required (max 200 characters)".into());
    }
    if body.is_empty() || body.len() > 4000 {
        return Err("Message body is required (max 4000 characters)".into());
    }
    let owner = crate::mail_postfix_maps::owner_for_mail_domain(domain)
        .unwrap_or_else(|| actor.to_string());

    let mut store = load_store();
    if let Some(existing) = store
        .items
        .iter_mut()
        .find(|a| a.address.eq_ignore_ascii_case(&address))
    {
        existing.subject = subject.to_string();
        existing.body = body.to_string();
        existing.enabled = enabled;
        existing.owner = owner;
        existing.updated_at_unix = crate::account::now_unix();
        let item = existing.clone();
        save_store(&store)?;
        write_vacation_sieve(&item)?;
        return Ok(format!("Autoresponder updated for {address}."));
    }
    let id = format!("ar-{}", crate::account::now_unix());
    let item = Autoresponder {
        id: id.clone(),
        address: address.clone(),
        subject: subject.to_string(),
        body: body.to_string(),
        owner,
        enabled,
        updated_at_unix: crate::account::now_unix(),
    };
    store.items.push(item.clone());
    save_store(&store)?;
    write_vacation_sieve(&item)?;
    Ok(format!("Autoresponder created for {address} ({id})."))
}

pub fn remove_autoresponder(actor: &str, id: &str) -> Result<String, String> {
    let id = id.trim();
    let mut store = load_store();
    let Some(pos) = store.items.iter().position(|a| a.id == id) else {
        return Err(format!("Autoresponder `{id}` not found"));
    };
    let item = store.items.remove(pos);
    let domain = item
        .address
        .rsplit_once('@')
        .map(|(_, d)| d)
        .unwrap_or("");
    if !can_manage_mail_domain(actor, domain) {
        return Err("You cannot delete this autoresponder".into());
    }
    save_store(&store)?;
    let disabled = Autoresponder {
        enabled: false,
        ..item
    };
    let _ = write_vacation_sieve(&disabled);
    Ok("Autoresponder removed.".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;

    #[test]
    fn store_roundtrip_without_home() {
        with_test_data_dir(|| {
            let mut store = Store::default();
            store.items.push(Autoresponder {
                id: "ar-1".into(),
                address: "a@example.com".into(),
                subject: "Away".into(),
                body: "Back later".into(),
                owner: "ops".into(),
                enabled: true,
                updated_at_unix: 1,
            });
            save_store(&store).unwrap();
            assert_eq!(list_autoresponders().len(), 1);
            assert_eq!(count_for_owner("ops"), 1);
        });
    }
}
