//! Bootstrap configuration for the MariaDB panel-state store.
//!
//! This file (plus `panel-db.key`) is the only state that must stay on disk:
//! the panel needs it to find and open MariaDB. Mode 0600, root only.

use crate::paths::join_data;
use rand::{Rng, distr::Alphanumeric};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

pub const CONFIG_FILE: &str = "panel-db.json";
pub const KEY_FILE: &str = "panel-db.key";
pub const DISABLE_MARKER: &str = "panel-db.disabled";
pub const SCHEMA_VERSION: u32 = 1;

pub const DEFAULT_PANEL_DB: &str = "cpn_panel";
pub const DEFAULT_MAIL_DB: &str = "cpn_mail";
pub const DEFAULT_PANEL_USER: &str = "cpn_panel";
pub const DEFAULT_MAIL_AUTH_USER: &str = "cpn_mail_auth";

/// Which backend is authoritative for panel state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum StoreMode {
    /// Files are authoritative; MariaDB is an imported mirror (default).
    #[default]
    Files,
    /// Writes go to both; reads prefer files until verified.
    DualWrite,
    /// MariaDB is authoritative; files are a read-only fallback snapshot.
    DbPrimary,
}

impl StoreMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Files => "files",
            Self::DualWrite => "dual_write",
            Self::DbPrimary => "db_primary",
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct PanelDbConfig {
    #[serde(default)]
    pub schema: u32,
    #[serde(default)]
    pub mode: StoreMode,
    pub panel_db: String,
    pub mail_db: String,
    pub panel_user: String,
    pub panel_password: String,
    pub mail_auth_user: String,
    pub mail_auth_password: String,
    /// Legacy mailbox database name when a classic passdb was detected.
    #[serde(default)]
    pub legacy_mail_db: Option<String>,
    /// When true, upgrades switch Dovecot to `cpn_mail` after a clean verify.
    #[serde(default)]
    pub auto_mail_cutover: bool,
    #[serde(default)]
    pub created_at_unix: u64,
}

// Passwords must never reach logs through `{:?}`.
impl fmt::Debug for PanelDbConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PanelDbConfig")
            .field("mode", &self.mode)
            .field("panel_db", &self.panel_db)
            .field("mail_db", &self.mail_db)
            .field("panel_user", &self.panel_user)
            .field("mail_auth_user", &self.mail_auth_user)
            .field("secrets", &"[redacted]")
            .finish()
    }
}

pub fn config_path() -> PathBuf {
    join_data(CONFIG_FILE)
}

pub fn key_path() -> PathBuf {
    join_data(KEY_FILE)
}

/// Operator opt-out: `touch /var/lib/cpn/panel-db.disabled` or `CPN_PANEL_DB=off`.
pub fn disabled_by_operator() -> bool {
    if matches!(
        std::env::var("CPN_PANEL_DB").ok().as_deref(),
        Some("off" | "0" | "false")
    ) {
        return true;
    }
    join_data(DISABLE_MARKER).exists()
}

pub fn random_password() -> String {
    rand::rng()
        .sample_iter(&Alphanumeric)
        .take(32)
        .map(char::from)
        .collect()
}

impl PanelDbConfig {
    pub fn generate(now_unix: u64) -> Self {
        Self {
            schema: SCHEMA_VERSION,
            mode: StoreMode::Files,
            panel_db: DEFAULT_PANEL_DB.into(),
            mail_db: DEFAULT_MAIL_DB.into(),
            panel_user: DEFAULT_PANEL_USER.into(),
            panel_password: random_password(),
            mail_auth_user: DEFAULT_MAIL_AUTH_USER.into(),
            mail_auth_password: random_password(),
            legacy_mail_db: None,
            auto_mail_cutover: false,
            created_at_unix: now_unix,
        }
    }

    pub fn load() -> Result<Option<Self>, String> {
        load_from(&config_path())
    }

    pub fn save(&self) -> Result<(), String> {
        write_private_atomic(&config_path(), self.to_json()?.as_bytes())
    }

    fn to_json(&self) -> Result<String, String> {
        serde_json::to_string_pretty(self)
            .map_err(|e| format!("Could not serialize panel database config: {e}"))
    }
}

fn load_from(path: &Path) -> Result<Option<PanelDbConfig>, String> {
    let raw = match fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("Could not read panel database config: {e}")),
    };
    serde_json::from_str(&raw)
        .map(Some)
        .map_err(|_| "Panel database config is corrupt (not valid JSON)".to_string())
}

/// Write `bytes` with mode 0600 through a temp file and rename (never half-written).
pub fn write_private_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Could not create {}: {e}", parent.display()))?;
    }
    if let Ok(meta) = fs::symlink_metadata(path)
        && meta.file_type().is_symlink()
    {
        return Err(format!(
            "Refusing to write through symlink {}",
            path.display()
        ));
    }
    let tmp = path.with_extension(format!("tmp{}", std::process::id()));
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
        .and_then(|()| file.sync_all())
        .map_err(|e| format!("Could not write {}: {e}", tmp.display()))?;
    fs::rename(&tmp, path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        format!("Could not replace {}: {e}", path.display())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_config_has_distinct_strong_passwords() {
        let cfg = PanelDbConfig::generate(1);
        assert_ne!(cfg.panel_password, cfg.mail_auth_password);
        assert_eq!(cfg.panel_password.len(), 32);
        assert!(super::super::sqlfmt::safe_password(&cfg.panel_password).is_ok());
    }

    #[test]
    fn debug_output_hides_passwords() {
        let cfg = PanelDbConfig::generate(1);
        let shown = format!("{cfg:?}");
        assert!(!shown.contains(&cfg.panel_password));
        assert!(!shown.contains(&cfg.mail_auth_password));
    }

    #[test]
    fn save_and_load_round_trip() {
        let dir = std::env::temp_dir().join(format!("cpn-pdb-cfg-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let path = dir.join(CONFIG_FILE);
        let cfg = PanelDbConfig::generate(7);
        write_private_atomic(&path, cfg.to_json().unwrap().as_bytes()).unwrap();
        let back = load_from(&path).unwrap().unwrap();
        assert_eq!(back.panel_password, cfg.panel_password);
        assert_eq!(back.mode, StoreMode::Files);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
        }
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_config_is_none_and_corrupt_is_error() {
        let dir = std::env::temp_dir().join(format!("cpn-pdb-cfg2-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join(CONFIG_FILE);
        assert!(load_from(&path).unwrap().is_none());
        fs::write(&path, "{nope").unwrap();
        assert!(load_from(&path).is_err());
        let _ = fs::remove_dir_all(&dir);
    }
}
