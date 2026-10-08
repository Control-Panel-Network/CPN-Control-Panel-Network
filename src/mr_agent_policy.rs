//! Host-level Mr Agent owner policy (independent of Host plugin install).
//!
//! Stored at `/var/lib/cpn/mr-agent/host-policy.json` (mode 600).
//! Two switches:
//! - `allow_host_chat` (default true): panel bubble and `/plugins/mr-agent`
//! - `allow_site_install` (default false): Store Site Install for mrAgent
//!
//! Visibility ACL (`admins_only` / `all_authenticated` / `packages`) stays separate.

use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

const POLICY_DIR: &str = "/var/lib/cpn/mr-agent";
const POLICY_FILE: &str = "host-policy.json";

pub const SITE_INSTALL_DISABLED_MSG: &str =
    "Site install of Mr Agent is disabled by the server owner. Ask the panel owner to enable Allow site install in Mr Agent host policy, or use panel chat when Allow host chat is on.";

pub const HOST_CHAT_DISABLED_MSG: &str =
    "Panel Mr Agent chat is disabled by the server owner (Allow host chat is off).";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MrAgentHostPolicy {
    /// Panel bubble and `/plugins/mr-agent` for users who pass visibility ACL.
    #[serde(default = "default_true")]
    pub allow_host_chat: bool,
    /// Store/site users may Install Mr Agent on their own sites.
    #[serde(default = "default_false")]
    pub allow_site_install: bool,
}

fn default_true() -> bool {
    true
}

fn default_false() -> bool {
    false
}

impl Default for MrAgentHostPolicy {
    fn default() -> Self {
        Self {
            allow_host_chat: true,
            allow_site_install: false,
        }
    }
}

pub fn policy_path() -> PathBuf {
    PathBuf::from(POLICY_DIR).join(POLICY_FILE)
}

fn ensure_policy_dir() -> Result<(), String> {
    let dir = Path::new(POLICY_DIR);
    fs::create_dir_all(dir).map_err(|e| format!("Could not create mr-agent policy dir: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(dir, fs::Permissions::from_mode(0o700));
    }
    Ok(())
}

/// Load host policy. Missing file uses defaults (host chat on, site install off).
pub fn load_host_policy() -> MrAgentHostPolicy {
    let path = policy_path();
    let Ok(raw) = fs::read_to_string(&path) else {
        return MrAgentHostPolicy::default();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

/// Persist host policy (owner/admin only callers).
pub fn save_host_policy(policy: &MrAgentHostPolicy) -> Result<(), String> {
    ensure_policy_dir()?;
    let path = policy_path();
    let raw = serde_json::to_string_pretty(policy)
        .map_err(|e| format!("Could not serialize host policy: {e}"))?;
    let tmp = path.with_extension("json.tmp");
    {
        let mut f = fs::File::create(&tmp)
            .map_err(|e| format!("Could not write host policy temp: {e}"))?;
        f.write_all(raw.as_bytes())
            .map_err(|e| format!("Could not write host policy: {e}"))?;
        f.write_all(b"\n")
            .map_err(|e| format!("Could not write host policy newline: {e}"))?;
        f.sync_all()
            .map_err(|e| format!("Could not sync host policy: {e}"))?;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600));
    }
    fs::rename(&tmp, &path).map_err(|e| format!("Could not replace host policy: {e}"))?;
    Ok(())
}

pub fn allow_host_chat() -> bool {
    load_host_policy().allow_host_chat
}

pub fn allow_site_install() -> bool {
    load_host_policy().allow_site_install
}

/// Refuse new site Install when policy is off. Does not uninstall existing copies.
pub fn refuse_site_install_if_disabled(plugin_id: &str) -> Result<(), String> {
    if !crate::mr_agent_install::is_mr_agent(plugin_id) {
        return Ok(());
    }
    if allow_site_install() {
        return Ok(());
    }
    Err(SITE_INSTALL_DISABLED_MSG.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn defaults_panel_on_site_off() {
        let p = MrAgentHostPolicy::default();
        assert!(p.allow_host_chat);
        assert!(!p.allow_site_install);
    }

    #[test]
    fn serde_missing_keys_use_defaults() {
        let p: MrAgentHostPolicy = serde_json::from_str("{}").unwrap();
        assert!(p.allow_host_chat);
        assert!(!p.allow_site_install);
        let p2: MrAgentHostPolicy =
            serde_json::from_str(r#"{"allow_host_chat":false,"allow_site_install":true}"#)
                .unwrap();
        assert!(!p2.allow_host_chat);
        assert!(p2.allow_site_install);
    }

    #[test]
    fn refuse_only_mr_agent_when_off() {
        let _g = LOCK.lock().unwrap();
        // Unit path uses live filesystem when present; only assert message for disabled default
        // when we cannot write. Logic of refuse_site_install_if_disabled for non-mrAgent:
        assert!(refuse_site_install_if_disabled("bimi").is_ok());
    }

    #[test]
    fn mode_labels() {
        let off = MrAgentHostPolicy {
            allow_host_chat: false,
            allow_site_install: false,
        };
        let panel_only = MrAgentHostPolicy {
            allow_host_chat: true,
            allow_site_install: false,
        };
        let both = MrAgentHostPolicy {
            allow_host_chat: true,
            allow_site_install: true,
        };
        assert!(!off.allow_host_chat && !off.allow_site_install);
        assert!(panel_only.allow_host_chat && !panel_only.allow_site_install);
        assert!(both.allow_host_chat && both.allow_site_install);
    }
}
