//! Extra mail features: forwarding, catch-all, DKIM file stores when Postfix exists.

use crate::mail_postfix_maps::{
    apply_virtual_alias_map, can_manage_mail_domain, normalize_email, owner_for_mail_domain,
};
use crate::panel_ops_dkim_keys::{dkim_root, dkim_status_detail, ensure_dkim_root};
use crate::paths::join_data;
use crate::postfix_fallback::postfix_is_ready;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MailForward {
    pub from: String,
    pub to: String,
    /// Panel account that owns the source domain (for package metering).
    #[serde(default)]
    pub owner: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CatchAll {
    pub domain: String,
    pub target: String,
    #[serde(default)]
    pub owner: String,
}

fn forwards_path() -> PathBuf {
    join_data("mail-forwards.json")
}

fn catchall_path() -> PathBuf {
    join_data("mail-catchall.json")
}

fn forwards_map_path() -> PathBuf {
    join_data("mail/virtual_forwards")
}

fn catchall_map_path() -> PathBuf {
    join_data("mail/virtual_catchall")
}

pub fn mail_stack_note() -> String {
    if postfix_is_ready() {
        "Postfix is ready. Forwarders and catch-all rules apply to virtual alias maps under the CPN data dir."
            .into()
    } else {
        "Postfix not detected. Rules persist in the panel; maps apply when Postfix is up."
            .into()
    }
}

pub fn load_forwards() -> Vec<MailForward> {
    fs::read_to_string(forwards_path())
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

pub fn save_forwards(rows: &[MailForward]) -> Result<(), String> {
    if let Some(parent) = forwards_path().parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let raw = serde_json::to_string_pretty(rows).map_err(|e| e.to_string())?;
    fs::write(forwards_path(), raw).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(forwards_path(), fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

pub fn count_forwards_for_owner(owner: &str) -> u64 {
    load_forwards()
        .into_iter()
        .filter(|f| {
            if !f.owner.trim().is_empty() {
                return f.owner.trim().eq_ignore_ascii_case(owner.trim());
            }
            f.from
                .rsplit_once('@')
                .and_then(|(_, d)| owner_for_mail_domain(d))
                .map(|o| o.trim().eq_ignore_ascii_case(owner.trim()))
                .unwrap_or(false)
        })
        .count() as u64
}

pub fn apply_forward_maps() -> Result<String, String> {
    let rows = load_forwards();
    let mut body = String::from("# CPN address forwarders\n");
    for row in &rows {
        body.push_str(&format!("{}\t{}\n", row.from, row.to));
    }
    apply_virtual_alias_map(&forwards_map_path(), &body)
}

pub fn add_forward_for(actor: &str, from: &str, to: &str) -> Result<String, String> {
    let from = normalize_email(from)?;
    let to = normalize_email(to)?;
    let domain = from
        .rsplit_once('@')
        .map(|(_, d)| d)
        .ok_or_else(|| "Invalid from address".to_string())?;
    if !can_manage_mail_domain(actor, domain) {
        return Err("You cannot create forwarders for this domain".into());
    }
    let owner = owner_for_mail_domain(domain).unwrap_or_else(|| actor.to_string());
    let mut rows = load_forwards();
    if rows.iter().any(|r| r.from.eq_ignore_ascii_case(&from)) {
        return Err(format!("Forwarder `{from}` already exists"));
    }
    rows.push(MailForward {
        from: from.clone(),
        to,
        owner,
    });
    save_forwards(&rows)?;
    let apply = apply_forward_maps()?;
    Ok(format!("Forwarder `{from}` saved. {apply}"))
}

pub fn remove_forward(actor: &str, from: &str) -> Result<String, String> {
    let from = from.trim().to_ascii_lowercase();
    let mut rows = load_forwards();
    let Some(pos) = rows.iter().position(|r| r.from.eq_ignore_ascii_case(&from)) else {
        return Err(format!("Forwarder `{from}` not found"));
    };
    let domain = rows[pos]
        .from
        .rsplit_once('@')
        .map(|(_, d)| d)
        .unwrap_or("");
    if !can_manage_mail_domain(actor, domain) {
        return Err("You cannot delete this forwarder".into());
    }
    rows.remove(pos);
    save_forwards(&rows)?;
    let apply = apply_forward_maps()?;
    Ok(format!("Forwarder removed. {apply}"))
}

pub fn load_catchall() -> Vec<CatchAll> {
    fs::read_to_string(catchall_path())
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

pub fn save_catchall(rows: &[CatchAll]) -> Result<(), String> {
    if let Some(parent) = catchall_path().parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let raw = serde_json::to_string_pretty(rows).map_err(|e| e.to_string())?;
    fs::write(catchall_path(), raw).map_err(|e| e.to_string())
}

pub fn apply_catchall_maps() -> Result<String, String> {
    let rows = load_catchall();
    let mut body = String::from("# CPN catch-all domain aliases\n");
    for row in &rows {
        let domain = row.domain.trim().to_ascii_lowercase();
        if domain.is_empty() {
            continue;
        }
        body.push_str(&format!("@{domain}\t{}\n", row.target.trim().to_ascii_lowercase()));
    }
    apply_virtual_alias_map(&catchall_map_path(), &body)
}

pub fn add_catchall_for(actor: &str, domain: &str, target: &str) -> Result<String, String> {
    let domain = domain.trim().to_ascii_lowercase();
    if domain.is_empty() || !domain.contains('.') {
        return Err("Domain is required".into());
    }
    if !can_manage_mail_domain(actor, &domain) {
        return Err("You cannot create catch-all for this domain".into());
    }
    let target = normalize_email(target)?;
    let owner = owner_for_mail_domain(&domain).unwrap_or_else(|| actor.to_string());
    let mut rows = load_catchall();
    if rows.iter().any(|r| r.domain.eq_ignore_ascii_case(&domain)) {
        return Err(format!("Catch-all for `{domain}` already exists"));
    }
    rows.push(CatchAll {
        domain: domain.clone(),
        target,
        owner,
    });
    save_catchall(&rows)?;
    let apply = apply_catchall_maps()?;
    Ok(format!("Catch-all for `{domain}` saved. {apply}"))
}

pub fn dkim_status() -> (bool, String) {
    dkim_status_detail()
}

pub fn ensure_dkim_dir() -> Result<PathBuf, String> {
    ensure_dkim_root()
}

pub fn ensure_dkim_store_ready() -> Result<String, String> {
    let dir = ensure_dkim_root()?;
    Ok(format!(
        "DKIM directory ready at {}. Keys are generated per domain when you create a site or run Ensure DKIM.",
        dir.display()
    ))
}

pub fn dkim_store_path_display() -> String {
    dkim_root().display().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;

    #[test]
    fn forwards_roundtrip() {
        with_test_data_dir(|| {
            save_forwards(&[MailForward {
                from: "a@example.com".into(),
                to: "b@example.com".into(),
                owner: "ops".into(),
            }])
            .unwrap();
            let rows = load_forwards();
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0].from, "a@example.com");
            assert_eq!(count_forwards_for_owner("ops"), 1);
        });
    }
}
