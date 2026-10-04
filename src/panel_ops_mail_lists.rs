//! CPN mailing lists: distribution addresses expanded via Postfix virtual aliases.

use crate::mail_postfix_maps::{
    apply_virtual_alias_map, can_manage_mail_domain, normalize_email, owner_for_mail_domain,
};
use crate::paths::join_data;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MailingList {
    pub id: String,
    /// List address (list@domain).
    pub address: String,
    pub name: String,
    #[serde(default)]
    pub members: Vec<String>,
    #[serde(default)]
    pub owner: String,
    pub created_at_unix: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct Store {
    #[serde(default)]
    lists: Vec<MailingList>,
}

fn store_path() -> PathBuf {
    join_data("mail-lists.json")
}

fn map_path() -> PathBuf {
    join_data("mail/virtual_lists")
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

pub fn list_mailing_lists() -> Vec<MailingList> {
    load_store().lists
}

pub fn count_for_owner(owner: &str) -> u64 {
    list_mailing_lists()
        .into_iter()
        .filter(|l| l.owner.trim().eq_ignore_ascii_case(owner.trim()))
        .count() as u64
}

pub fn apply_list_maps() -> Result<String, String> {
    let lists = list_mailing_lists();
    let mut body = String::from("# CPN mailing lists (distribution aliases)\n");
    for list in &lists {
        if list.members.is_empty() {
            continue;
        }
        let members = list.members.join(",");
        body.push_str(&format!("{}\t{}\n", list.address, members));
    }
    apply_virtual_alias_map(&map_path(), &body)
}

pub fn create_mailing_list(actor: &str, address: &str, name: &str) -> Result<String, String> {
    let address = normalize_email(address)?;
    let domain = address
        .rsplit_once('@')
        .map(|(_, d)| d)
        .ok_or_else(|| "Invalid list address".to_string())?;
    if !can_manage_mail_domain(actor, domain) {
        return Err("You cannot create mailing lists for this domain".into());
    }
    let name = name.trim();
    if name.is_empty() || name.len() > 120 {
        return Err("List name is required (max 120 characters)".into());
    }
    let owner = owner_for_mail_domain(domain).unwrap_or_else(|| actor.to_string());
    let mut store = load_store();
    if store
        .lists
        .iter()
        .any(|l| l.address.eq_ignore_ascii_case(&address))
    {
        return Err(format!("Mailing list `{address}` already exists"));
    }
    let id = format!("ml-{}", crate::account::now_unix());
    store.lists.push(MailingList {
        id: id.clone(),
        address: address.clone(),
        name: name.to_string(),
        members: Vec::new(),
        owner,
        created_at_unix: crate::account::now_unix(),
    });
    save_store(&store)?;
    let apply = apply_list_maps()?;
    Ok(format!("Mailing list `{address}` created ({id}). {apply}"))
}

pub fn add_list_member(actor: &str, list_id: &str, member: &str) -> Result<String, String> {
    let member = normalize_email(member)?;
    let mut store = load_store();
    let Some(list) = store.lists.iter_mut().find(|l| l.id == list_id.trim()) else {
        return Err("Mailing list not found".into());
    };
    let domain = list
        .address
        .rsplit_once('@')
        .map(|(_, d)| d)
        .unwrap_or("");
    if !can_manage_mail_domain(actor, domain) {
        return Err("You cannot edit this mailing list".into());
    }
    if list.members.iter().any(|m| m == &member) {
        return Ok("Member already on the list.".into());
    }
    if list.members.len() >= 500 {
        return Err("List is at the 500 member cap".into());
    }
    list.members.push(member.clone());
    save_store(&store)?;
    let apply = apply_list_maps()?;
    Ok(format!("Added `{member}`. {apply}"))
}

pub fn remove_list_member(actor: &str, list_id: &str, member: &str) -> Result<String, String> {
    let member = member.trim().to_ascii_lowercase();
    let mut store = load_store();
    let Some(list) = store.lists.iter_mut().find(|l| l.id == list_id.trim()) else {
        return Err("Mailing list not found".into());
    };
    let domain = list
        .address
        .rsplit_once('@')
        .map(|(_, d)| d)
        .unwrap_or("");
    if !can_manage_mail_domain(actor, domain) {
        return Err("You cannot edit this mailing list".into());
    }
    let before = list.members.len();
    list.members.retain(|m| m != &member);
    if list.members.len() == before {
        return Err("Member not found on this list".into());
    }
    save_store(&store)?;
    let apply = apply_list_maps()?;
    Ok(format!("Removed `{member}`. {apply}"))
}

pub fn delete_mailing_list(actor: &str, list_id: &str) -> Result<String, String> {
    let mut store = load_store();
    let Some(pos) = store.lists.iter().position(|l| l.id == list_id.trim()) else {
        return Err("Mailing list not found".into());
    };
    let list = &store.lists[pos];
    let domain = list
        .address
        .rsplit_once('@')
        .map(|(_, d)| d)
        .unwrap_or("");
    if !can_manage_mail_domain(actor, domain) {
        return Err("You cannot delete this mailing list".into());
    }
    let address = list.address.clone();
    store.lists.remove(pos);
    save_store(&store)?;
    let apply = apply_list_maps()?;
    Ok(format!("Deleted mailing list `{address}`. {apply}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;

    #[test]
    fn list_store_roundtrip() {
        with_test_data_dir(|| {
            let mut store = Store::default();
            store.lists.push(MailingList {
                id: "ml-1".into(),
                address: "news@example.com".into(),
                name: "News".into(),
                members: vec!["a@example.com".into()],
                owner: "ops".into(),
                created_at_unix: 1,
            });
            save_store(&store).unwrap();
            assert_eq!(count_for_owner("ops"), 1);
            assert_eq!(list_mailing_lists()[0].members.len(), 1);
        });
    }
}
