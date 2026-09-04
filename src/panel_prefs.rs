//! Panel UI preferences (document root visibility and similar).

use crate::account::data_dir;
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PanelUiPrefs {
    /// When true, Websites table shows document root paths (default for admins).
    #[serde(default = "default_true")]
    pub show_document_roots: bool,
}

fn default_true() -> bool {
    true
}

impl Default for PanelUiPrefs {
    fn default() -> Self {
        Self {
            show_document_roots: true,
        }
    }
}

fn prefs_path() -> std::path::PathBuf {
    data_dir().join("panel-ui.json")
}

pub fn load_panel_ui_prefs() -> PanelUiPrefs {
    let path = prefs_path();
    let Ok(raw) = fs::read_to_string(&path) else {
        return PanelUiPrefs::default();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

pub fn save_panel_ui_prefs(prefs: &PanelUiPrefs) -> Result<(), String> {
    let path = prefs_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Could not create prefs dir: {error}"))?;
    }
    let raw = serde_json::to_string_pretty(prefs)
        .map_err(|error| format!("Could not serialize panel UI prefs: {error}"))?;
    fs::write(&path, raw).map_err(|error| format!("Could not write panel UI prefs: {error}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

pub fn set_show_document_roots(show: bool) -> Result<PanelUiPrefs, String> {
    let mut prefs = load_panel_ui_prefs();
    prefs.show_document_roots = show;
    save_panel_ui_prefs(&prefs)?;
    Ok(prefs)
}
