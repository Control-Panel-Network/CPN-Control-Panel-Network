//! Active panel webmail client preference (SnappyMail / Tachyon / Roundcube / NextSnapMail).
//!
//! Stored under `/var/lib/cpn/active-webmail.json`. The `/opt/cpn-webmail/current` symlink
//! remains the runtime docroot for panel-proxied clients; this preference decides which
//! installed client Open Webmail and Host packages treat as active.

use crate::model::MailSystem;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

const PREF_FILE: &str = "/var/lib/cpn/active-webmail.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActiveWebmailPref {
    pub client: String,
}

fn pref_path() -> PathBuf {
    PathBuf::from(PREF_FILE)
}

pub fn mail_to_id(mail: MailSystem) -> &'static str {
    match mail {
        MailSystem::Snappymail => "snappymail",
        MailSystem::Tachyon => "tachyon",
        MailSystem::Roundcube => "roundcube",
        MailSystem::Nextsnapmail => "nextsnapmail",
        MailSystem::Sogo => "sogo",
        MailSystem::Thunderbird => "thunderbird",
    }
}

pub fn parse_client_id(raw: &str) -> Option<MailSystem> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "snappymail" | "snappy" => Some(MailSystem::Snappymail),
        "tachyon" => Some(MailSystem::Tachyon),
        "roundcube" => Some(MailSystem::Roundcube),
        "nextsnapmail" | "next-snapmail" => Some(MailSystem::Nextsnapmail),
        "sogo" => Some(MailSystem::Sogo),
        "thunderbird" => Some(MailSystem::Thunderbird),
        _ => None,
    }
}

pub fn load_active_pref() -> Option<MailSystem> {
    let path = pref_path();
    if !path.is_file() {
        return None;
    }
    let raw = fs::read_to_string(&path).ok()?;
    let pref: ActiveWebmailPref = serde_json::from_str(&raw).ok()?;
    parse_client_id(&pref.client)
}

pub fn save_active_pref(mail: MailSystem) -> Result<(), String> {
    let path = pref_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Could not create {}: {e}", parent.display()))?;
    }
    let pref = ActiveWebmailPref {
        client: mail_to_id(mail).to_string(),
    };
    let raw = serde_json::to_string_pretty(&pref)
        .map_err(|e| format!("Could not serialize active webmail pref: {e}"))?;
    fs::write(&path, raw).map_err(|e| format!("Could not write {}: {e}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

pub fn clear_active_pref() {
    let _ = fs::remove_file(pref_path());
}

pub fn client_files_present(mail: MailSystem) -> bool {
    match mail {
        MailSystem::Snappymail => Path::new("/opt/cpn-webmail/snappymail/index.php").is_file(),
        MailSystem::Tachyon => Path::new("/opt/cpn-webmail/tachyon/index.php").is_file(),
        MailSystem::Roundcube => {
            Path::new("/opt/cpn-webmail/roundcube/public_html/index.php").is_file()
                || Path::new("/opt/cpn-webmail/roundcube/index.php").is_file()
        }
        MailSystem::Nextsnapmail => crate::apps_nextcloud::nextsnapmail_app_present(),
        MailSystem::Sogo | MailSystem::Thunderbird => false,
    }
}

pub fn public_path_for(mail: MailSystem) -> &'static str {
    match mail {
        MailSystem::Tachyon => "/tachyon",
        MailSystem::Roundcube => "/roundcube",
        MailSystem::Snappymail => "/snappymail",
        // NextSnapMail is opened via Nextcloud; panel mount stays unused.
        MailSystem::Nextsnapmail | MailSystem::Sogo | MailSystem::Thunderbird => "/snappymail",
    }
}

pub fn docroot_for(mail: MailSystem) -> Option<&'static str> {
    match mail {
        MailSystem::Snappymail => Some("/opt/cpn-webmail/snappymail"),
        MailSystem::Tachyon => Some("/opt/cpn-webmail/tachyon"),
        MailSystem::Roundcube => {
            if Path::new("/opt/cpn-webmail/roundcube/public_html").is_dir() {
                Some("/opt/cpn-webmail/roundcube/public_html")
            } else {
                Some("/opt/cpn-webmail/roundcube")
            }
        }
        MailSystem::Nextsnapmail | MailSystem::Sogo | MailSystem::Thunderbird => None,
    }
}

/// Default client for fresh installs when nothing is active yet.
pub fn default_webmail_client() -> MailSystem {
    MailSystem::Tachyon
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_ids() {
        assert_eq!(parse_client_id("tachyon"), Some(MailSystem::Tachyon));
        assert_eq!(mail_to_id(MailSystem::Snappymail), "snappymail");
        assert_eq!(public_path_for(MailSystem::Tachyon), "/tachyon");
        assert_eq!(default_webmail_client(), MailSystem::Tachyon);
    }
}
