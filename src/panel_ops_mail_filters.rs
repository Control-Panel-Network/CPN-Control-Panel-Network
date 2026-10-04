//! Email filters as Dovecot Sieve rules managed by the panel.

use crate::mail_postfix_maps::{can_manage_mail_domain, normalize_email};
use crate::panel_ops_mail_autorespond::rebuild_active_sieve;
use crate::panel_ops_mailbox_provision::local_part;
use crate::paths::join_data;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailFilter {
    pub id: String,
    pub address: String,
    /// `subject_contains`, `from_contains`, `to_contains`.
    pub match_field: String,
    pub match_value: String,
    /// `discard`, `fileinto`, `redirect`.
    pub action: String,
    #[serde(default)]
    pub action_arg: String,
    #[serde(default)]
    pub owner: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub created_at_unix: u64,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct Store {
    #[serde(default)]
    rules: Vec<EmailFilter>,
}

fn store_path() -> PathBuf {
    join_data("mail-filters.json")
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

pub fn list_filters() -> Vec<EmailFilter> {
    load_store().rules
}

pub fn count_for_owner(owner: &str) -> u64 {
    list_filters()
        .into_iter()
        .filter(|f| f.owner.trim().eq_ignore_ascii_case(owner.trim()))
        .count() as u64
}

fn sieve_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn render_sieve_for_address(address: &str, rules: &[EmailFilter]) -> String {
    let mut out = String::from("require [\"fileinto\", \"mailbox\"];\n");
    let mut any = false;
    for rule in rules
        .iter()
        .filter(|r| r.enabled && r.address.eq_ignore_ascii_case(address))
    {
        let header = match rule.match_field.as_str() {
            "from_contains" => "From",
            "to_contains" => "To",
            _ => "Subject",
        };
        let needle = sieve_escape(&rule.match_value);
        let action_block = match rule.action.as_str() {
            "discard" => "  discard;\n  stop;".to_string(),
            "redirect" => {
                let dest = sieve_escape(&rule.action_arg);
                format!("  redirect \"{dest}\";\n  stop;")
            }
            "fileinto" => {
                let folder = if rule.action_arg.trim().is_empty() {
                    "Junk".to_string()
                } else {
                    sieve_escape(rule.action_arg.trim())
                };
                format!("  fileinto :create \"{folder}\";\n  stop;")
            }
            _ => continue,
        };
        out.push_str(&format!(
            "if header :contains \"{header}\" \"{needle}\" {{\n{action_block}\n}}\n"
        ));
        any = true;
    }
    if !any {
        return "# CPN filters: none active\n".into();
    }
    out
}

fn write_filters_sieve(address: &str) -> Result<(), String> {
    let user = local_part(address)?;
    let home = PathBuf::from(format!("/home/{user}"));
    if !home.is_dir() {
        return Err(format!(
            "Mailbox system user `{user}` home is missing; create the mailbox first"
        ));
    }
    let sieve_dir = home.join("sieve");
    fs::create_dir_all(&sieve_dir).map_err(|e| format!("Cannot create sieve dir: {e}"))?;
    let rules = list_filters();
    let body = render_sieve_for_address(address, &rules);
    fs::write(sieve_dir.join("cpn-filters.sieve"), body)
        .map_err(|e| format!("Cannot write filter sieve: {e}"))?;
    rebuild_active_sieve(&home)?;
    #[cfg(unix)]
    {
        let _ = std::process::Command::new("chown")
            .args([
                "-R",
                &format!("{user}:{user}"),
                sieve_dir.to_string_lossy().as_ref(),
            ])
            .status();
    }
    Ok(())
}

pub fn add_filter(
    actor: &str,
    address: &str,
    match_field: &str,
    match_value: &str,
    action: &str,
    action_arg: &str,
) -> Result<String, String> {
    let address = normalize_email(address)?;
    let domain = address
        .rsplit_once('@')
        .map(|(_, d)| d)
        .ok_or_else(|| "Invalid address".to_string())?;
    if !can_manage_mail_domain(actor, domain) {
        return Err("You cannot manage filters for this domain".into());
    }
    let match_field = match match_field.trim() {
        "from_contains" | "to_contains" | "subject_contains" => match_field.trim().to_string(),
        _ => "subject_contains".into(),
    };
    let match_value = match_value.trim();
    if match_value.is_empty() || match_value.len() > 200 {
        return Err("Match value is required (max 200 characters)".into());
    }
    let action = match action.trim() {
        "discard" | "fileinto" | "redirect" => action.trim().to_string(),
        _ => return Err("Action must be discard, fileinto, or redirect".into()),
    };
    if action == "redirect" {
        let _ = normalize_email(action_arg)?;
    }
    let owner = crate::mail_postfix_maps::owner_for_mail_domain(domain)
        .unwrap_or_else(|| actor.to_string());

    let mut store = load_store();
    let id = format!("ef-{}", crate::account::now_unix());
    store.rules.push(EmailFilter {
        id: id.clone(),
        address: address.clone(),
        match_field,
        match_value: match_value.to_string(),
        action,
        action_arg: action_arg.trim().to_string(),
        owner,
        enabled: true,
        created_at_unix: crate::account::now_unix(),
    });
    save_store(&store)?;
    write_filters_sieve(&address)?;
    Ok(format!("Filter {id} saved for {address}."))
}

pub fn remove_filter(actor: &str, id: &str) -> Result<String, String> {
    let id = id.trim();
    let mut store = load_store();
    let Some(pos) = store.rules.iter().position(|r| r.id == id) else {
        return Err(format!("Filter `{id}` not found"));
    };
    let rule = store.rules.remove(pos);
    let domain = rule.address.rsplit_once('@').map(|(_, d)| d).unwrap_or("");
    if !can_manage_mail_domain(actor, domain) {
        return Err("You cannot delete this filter".into());
    }
    let address = rule.address.clone();
    save_store(&store)?;
    let _ = write_filters_sieve(&address);
    Ok("Filter removed.".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;

    #[test]
    fn render_discard_rule() {
        let rules = vec![EmailFilter {
            id: "ef-1".into(),
            address: "a@example.com".into(),
            match_field: "subject_contains".into(),
            match_value: "spam".into(),
            action: "discard".into(),
            action_arg: String::new(),
            owner: "ops".into(),
            enabled: true,
            created_at_unix: 1,
        }];
        let s = render_sieve_for_address("a@example.com", &rules);
        assert!(s.contains("Subject"));
        assert!(s.contains("discard"));
    }

    #[test]
    fn store_count() {
        with_test_data_dir(|| {
            let mut store = Store::default();
            store.rules.push(EmailFilter {
                id: "ef-1".into(),
                address: "a@example.com".into(),
                match_field: "subject_contains".into(),
                match_value: "x".into(),
                action: "discard".into(),
                action_arg: String::new(),
                owner: "ops".into(),
                enabled: true,
                created_at_unix: 1,
            });
            save_store(&store).unwrap();
            assert_eq!(count_for_owner("ops"), 1);
        });
    }
}
