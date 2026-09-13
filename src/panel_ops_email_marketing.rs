//! Email marketing MVP: lists, recipients, send via local MTA with rate limits.

use crate::mail_outbound::{OutboundMessage, resolve_outbound_settings, send_mail_with_settings};
use crate::panel_ops_email_limits::load_send_limits;
use crate::paths::join_data;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketingList {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub recipients: Vec<String>,
    pub created_at_unix: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketingCampaign {
    pub id: String,
    pub list_id: String,
    pub subject: String,
    pub body: String,
    pub from_address: String,
    pub sent_count: u32,
    pub last_error: Option<String>,
    pub updated_at_unix: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct Store {
    #[serde(default)]
    lists: Vec<MarketingList>,
    #[serde(default)]
    campaigns: Vec<MarketingCampaign>,
}

fn store_path() -> PathBuf {
    join_data("mail-marketing.json")
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

fn valid_email(addr: &str) -> bool {
    let a = addr.trim();
    a.contains('@') && !a.contains(' ') && a.len() < 254
}

pub fn list_marketing_lists() -> Vec<MarketingList> {
    load_store().lists
}

pub fn list_campaigns() -> Vec<MarketingCampaign> {
    load_store().campaigns
}

pub fn create_list(name: &str) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() || name.len() > 120 {
        return Err("List name is required (max 120 characters)".into());
    }
    let mut store = load_store();
    let id = format!("list-{}", crate::account::now_unix());
    store.lists.push(MarketingList {
        id: id.clone(),
        name: name.to_string(),
        recipients: Vec::new(),
        created_at_unix: crate::account::now_unix(),
    });
    save_store(&store)?;
    Ok(format!("List `{name}` created ({id})."))
}

pub fn add_recipient(list_id: &str, email: &str) -> Result<String, String> {
    let email = email.trim().to_ascii_lowercase();
    if !valid_email(&email) {
        return Err("Recipient must be a valid email".into());
    }
    let mut store = load_store();
    let Some(list) = store.lists.iter_mut().find(|l| l.id == list_id.trim()) else {
        return Err("List not found".into());
    };
    if list.recipients.iter().any(|r| r == &email) {
        return Ok("Recipient already on the list.".into());
    }
    if list.recipients.len() >= 5000 {
        return Err("List is at the 5000 recipient cap".into());
    }
    list.recipients.push(email.clone());
    save_store(&store)?;
    Ok(format!("Added `{email}`."))
}

fn rate_delay_ms() -> u64 {
    let limits = load_send_limits();
    // Default ~1 msg/sec; tighten if panel limits exist.
    let min_per_min = limits
        .iter()
        .map(|l| {
            if l.window_minutes == 0 {
                60
            } else {
                (l.max_messages.max(1) * 60) / l.window_minutes.max(1)
            }
        })
        .min()
        .unwrap_or(60);
    let per_sec = (min_per_min as f64 / 60.0).max(0.2);
    ((1000.0 / per_sec) as u64).clamp(200, 5000)
}

pub fn send_campaign(
    list_id: &str,
    subject: &str,
    body: &str,
    from_address: &str,
) -> Result<String, String> {
    let subject = subject.trim();
    let body = body.trim();
    let from_address = from_address.trim();
    if subject.is_empty() || body.is_empty() {
        return Err("Subject and body are required".into());
    }
    if !valid_email(from_address) {
        return Err("From address must be a valid email".into());
    }
    let mut store = load_store();
    let Some(list) = store.lists.iter().find(|l| l.id == list_id.trim()).cloned() else {
        return Err("List not found".into());
    };
    if list.recipients.is_empty() {
        return Err("List has no recipients".into());
    }
    let mut settings = resolve_outbound_settings(Some(from_address))?;
    settings.from_address = from_address.to_string();
    let delay = Duration::from_millis(rate_delay_ms());
    let mut sent = 0u32;
    let mut last_error = None;
    for to in &list.recipients {
        match send_mail_with_settings(
            &settings,
            &OutboundMessage {
                to: to.clone(),
                subject: subject.to_string(),
                body: body.to_string(),
            },
        ) {
            Ok(()) => sent += 1,
            Err(e) => {
                last_error = Some(e);
                break;
            }
        }
        thread::sleep(delay);
    }
    let id = format!("camp-{}", crate::account::now_unix());
    store.campaigns.insert(
        0,
        MarketingCampaign {
            id: id.clone(),
            list_id: list.id,
            subject: subject.to_string(),
            body: body.to_string(),
            from_address: from_address.to_string(),
            sent_count: sent,
            last_error: last_error.clone(),
            updated_at_unix: crate::account::now_unix(),
        },
    );
    if store.campaigns.len() > 50 {
        store.campaigns.truncate(50);
    }
    save_store(&store)?;
    if let Some(err) = last_error {
        Ok(format!(
            "Campaign {id}: sent {sent}/{} then stopped: {err}",
            list.recipients.len()
        ))
    } else {
        Ok(format!(
            "Campaign {id}: sent {sent}/{} via local MTA/SMTP.",
            list.recipients.len()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;

    #[test]
    fn list_and_recipient() {
        with_test_data_dir(|| {
            create_list("Newsletter").unwrap();
            let id = list_marketing_lists()[0].id.clone();
            add_recipient(&id, "a@example.com").unwrap();
            assert_eq!(list_marketing_lists()[0].recipients.len(), 1);
        });
    }
}
