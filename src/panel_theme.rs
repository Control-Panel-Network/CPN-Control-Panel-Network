//! Panel-wide design tokens and per-user color mode preferences.
//!
//! Design profiles live under `/var/lib/cpn/panel-design.json` (panel-global, not per-site).
//! **Default** is an immutable built-in baseline; custom edits never overwrite it.
//! Per-user light/dark color mode is stored under `/var/lib/cpn/user-prefs/<user>.json`
//! and mirrored in `localStorage` (`cpn-color-mode`).

use crate::account::data_dir;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

pub const COLOR_MODE_STORAGE_KEY: &str = "cpn-color-mode";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ColorMode {
    Light,
    Dark,
}

impl ColorMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "light" => Some(Self::Light),
            "dark" => Some(Self::Dark),
            _ => None,
        }
    }

    pub fn toggle(self) -> Self {
        match self {
            Self::Light => Self::Dark,
            Self::Dark => Self::Light,
        }
    }
}

impl Default for ColorMode {
    fn default() -> Self {
        Self::Light
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DesignPreset {
    Default,
    Light,
    Dark,
    Custom,
}

impl DesignPreset {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Light => "light",
            Self::Dark => "dark",
            Self::Custom => "custom",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "default" => Some(Self::Default),
            "light" => Some(Self::Light),
            "dark" => Some(Self::Dark),
            "custom" => Some(Self::Custom),
            _ => None,
        }
    }
}

impl Default for DesignPreset {
    fn default() -> Self {
        Self::Default
    }
}

/// Editable design tokens (Custom profile). Default values come from [`default_tokens`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DesignTokens {
    pub accent: String,
    pub accent_focus: String,
    pub radius_px: u8,
    pub density: String,
    pub font_scale: f32,
}

impl DesignTokens {
    pub fn validate(mut self) -> Result<Self, String> {
        self.accent = normalize_hex_color(&self.accent, "accent")?;
        self.accent_focus = normalize_hex_color(&self.accent_focus, "accent_focus")?;
        if !(8..=28).contains(&self.radius_px) {
            return Err("radius_px must be between 8 and 28".into());
        }
        let density = self.density.trim().to_ascii_lowercase();
        if density != "comfortable" && density != "compact" {
            return Err("density must be comfortable or compact".into());
        }
        self.density = density;
        if !(0.9..=1.25).contains(&self.font_scale) {
            return Err("font_scale must be between 0.9 and 1.25".into());
        }
        // Keep one decimal place for stable JSON.
        self.font_scale = (self.font_scale * 100.0).round() / 100.0;
        Ok(self)
    }
}

/// Immutable built-in CPN look. Never written to disk as a mutable baseline.
pub fn default_tokens() -> DesignTokens {
    DesignTokens {
        accent: "#0066cc".into(),
        accent_focus: "#0071e3".into(),
        radius_px: 18,
        density: "comfortable".into(),
        font_scale: 1.0,
    }
}

fn light_preset_tokens() -> DesignTokens {
    DesignTokens {
        accent: "#0a84ff".into(),
        accent_focus: "#409cff".into(),
        radius_px: 16,
        density: "comfortable".into(),
        font_scale: 1.0,
    }
}

fn dark_preset_tokens() -> DesignTokens {
    DesignTokens {
        accent: "#3b82f6".into(),
        accent_focus: "#60a5fa".into(),
        radius_px: 14,
        density: "compact".into(),
        font_scale: 0.98,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PanelDesignFile {
    pub schema_version: u32,
    pub preset: DesignPreset,
    #[serde(default)]
    pub custom: Option<DesignTokens>,
}

impl Default for PanelDesignFile {
    fn default() -> Self {
        Self {
            schema_version: 1,
            preset: DesignPreset::Default,
            custom: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UserUiPrefs {
    #[serde(default)]
    pub color_mode: ColorMode,
}

fn design_path() -> PathBuf {
    data_dir().join("panel-design.json")
}

fn user_prefs_path(username: &str) -> PathBuf {
    data_dir()
        .join("user-prefs")
        .join(format!("{}.json", safe_username_key(username)))
}

fn safe_username_key(username: &str) -> String {
    let trimmed = username.trim();
    let mut out = String::with_capacity(trimmed.len());
    for ch in trimmed.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.' {
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        "user".into()
    } else {
        out.chars().take(128).collect()
    }
}

fn normalize_hex_color(raw: &str, field: &str) -> Result<String, String> {
    let value = raw.trim();
    let hex = value.strip_prefix('#').unwrap_or(value);
    if hex.len() != 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!("{field} must be a #RRGGBB color"));
    }
    Ok(format!("#{}", hex.to_ascii_lowercase()))
}

fn write_json(path: &PathBuf, value: &impl Serialize) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Could not create prefs dir: {e}"))?;
    }
    let raw = serde_json::to_string_pretty(value)
        .map_err(|e| format!("Could not serialize prefs: {e}"))?;
    fs::write(path, raw).map_err(|e| format!("Could not write prefs: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

pub fn load_panel_design() -> PanelDesignFile {
    let Ok(raw) = fs::read_to_string(design_path()) else {
        return PanelDesignFile::default();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

pub fn save_panel_design(design: &PanelDesignFile) -> Result<(), String> {
    write_json(&design_path(), design)
}

/// Resolve the active token set. Default is always the built-in baseline.
pub fn resolve_tokens(design: &PanelDesignFile) -> DesignTokens {
    match design.preset {
        DesignPreset::Default => default_tokens(),
        DesignPreset::Light => light_preset_tokens(),
        DesignPreset::Dark => dark_preset_tokens(),
        DesignPreset::Custom => design
            .custom
            .clone()
            .unwrap_or_else(default_tokens),
    }
}

pub fn apply_design_preset(preset: DesignPreset) -> Result<PanelDesignFile, String> {
    let mut design = load_panel_design();
    match preset {
        DesignPreset::Default => {
            // Keep any saved custom profile on disk, but activate Default.
            design.preset = DesignPreset::Default;
        }
        DesignPreset::Light | DesignPreset::Dark => {
            design.preset = preset;
        }
        DesignPreset::Custom => {
            if design.custom.is_none() {
                design.custom = Some(default_tokens());
            }
            design.preset = DesignPreset::Custom;
        }
    }
    save_panel_design(&design)?;
    Ok(design)
}

pub fn save_custom_tokens(tokens: DesignTokens) -> Result<PanelDesignFile, String> {
    let tokens = tokens.validate()?;
    let mut design = load_panel_design();
    design.custom = Some(tokens);
    design.preset = DesignPreset::Custom;
    save_panel_design(&design)?;
    Ok(design)
}

/// Wipe custom profile and return to immutable Default.
pub fn restore_default_design() -> Result<PanelDesignFile, String> {
    let design = PanelDesignFile::default();
    save_panel_design(&design)?;
    Ok(design)
}

pub fn load_user_color_mode(username: &str) -> ColorMode {
    let Ok(raw) = fs::read_to_string(user_prefs_path(username)) else {
        return ColorMode::default();
    };
    serde_json::from_str::<UserUiPrefs>(&raw)
        .map(|p| p.color_mode)
        .unwrap_or_default()
}

pub fn save_user_color_mode(username: &str, mode: ColorMode) -> Result<ColorMode, String> {
    let prefs = UserUiPrefs { color_mode: mode };
    write_json(&user_prefs_path(username), &prefs)?;
    Ok(mode)
}

/// Inline `:root` CSS variables for the active design (panel chrome + Manage).
pub fn design_css_vars(design: &PanelDesignFile) -> String {
    let tokens = resolve_tokens(design);
    let density_pad = if tokens.density == "compact" {
        "0.92"
    } else {
        "1"
    };
    format!(
        r#":root {{
  --cpn-accent:{accent};
  --cpn-accent-focus:{focus};
  --cpn-radius:{radius}px;
  --cpn-density:{density};
  --cpn-font-scale:{scale};
  --blue:var(--cpn-accent);
  --blue-focus:var(--cpn-accent-focus);
}}
body {{ font-size:calc(17px * var(--cpn-font-scale)); }}
.resource-card, .status-card, .activity-card, .section-card, .server-summary {{
  border-radius:var(--cpn-radius);
}}
.site-manage {{
  --m-accent:var(--cpn-accent);
}}
"#,
        accent = tokens.accent,
        focus = tokens.accent_focus,
        radius = tokens.radius_px,
        density = density_pad,
        scale = tokens.font_scale,
    )
}

/// Dark color-mode surface overrides (extends existing light `:root` tokens).
pub fn color_mode_styles() -> &'static str {
    r#"
[data-color-mode="dark"] {
  --canvas:#1a1d26; --surface:#12141a; --surface-soft:#161922; --ink:#f2f4f7;
  --muted:#98a2b3; --hairline:#2a2f3a; --green:#3dd68c;
}
[data-color-mode="dark"] body,
[data-color-mode="dark"] .panel-layout { background:var(--surface); color:var(--ink); }
[data-color-mode="dark"] .sidebar {
  background:rgba(22,25,34,.96); border-right-color:var(--hairline);
}
[data-color-mode="dark"] .sidebar nav a { color:#c5cad3; }
[data-color-mode="dark"] .sidebar nav a.active {
  background:rgba(59,130,246,.18); color:var(--blue);
}
[data-color-mode="dark"] .mobile-header {
  background:rgba(22,25,34,.94); border-bottom-color:var(--hairline);
}
[data-color-mode="dark"] .gauge-track { stroke:#2a2f3a; }
[data-color-mode="dark"] .status-card li,
[data-color-mode="dark"] .activity-card > div,
[data-color-mode="dark"] .data-table th,
[data-color-mode="dark"] .data-table td,
[data-color-mode="dark"] .kv-list li {
  border-top-color:#2a2f3a;
}
[data-color-mode="dark"] .panel-notice.ok { background:#052e1c; border-color:#0f7a45; color:#6ce9a6; }
[data-color-mode="dark"] .panel-notice.error { background:#3f1d22; border-color:#912018; color:#fda29b; }
[data-color-mode="dark"] .btn-danger { background:#3f1d22; color:#fda29b; }
[data-color-mode="dark"] .stack-form input {
  background:#1a1d26; border-color:#2a2f3a; color:var(--ink);
}
.theme-toggle {
  display:flex; align-items:center; gap:10px; width:100%; min-height:44px;
  padding:0 12px; margin:0 0 8px; border:1px solid var(--hairline);
  border-radius:12px; background:var(--canvas); color:var(--ink); font:inherit; font-weight:600;
  font-size:13px; text-align:left; cursor:pointer;
}
.theme-toggle:focus-visible { outline:2px solid var(--blue-focus); outline-offset:2px; }
.theme-toggle-icon {
  width:28px; height:28px; border-radius:8px; display:inline-grid; place-items:center;
  background:var(--surface); border:1px solid var(--hairline); font-size:14px; line-height:1;
}
.sidebar-footer { flex-direction:column; align-items:stretch; gap:4px; }
.sidebar-footer .logout { margin-left:0; width:100%; justify-content:flex-start; }
"#
}

pub fn design_public_json(design: &PanelDesignFile) -> serde_json::Value {
    let tokens = resolve_tokens(design);
    serde_json::json!({
        "preset": design.preset.as_str(),
        "tokens": tokens,
        "default_tokens": default_tokens(),
        "has_custom": design.custom.is_some(),
        "scope": "panel-global",
        "note": "Design applies to panel chrome and Manage dashboard look for all operators. Not per-site branding."
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;

    #[test]
    fn default_tokens_are_stable() {
        let a = default_tokens();
        let b = default_tokens();
        assert_eq!(a, b);
        assert_eq!(a.accent, "#0066cc");
    }

    #[test]
    fn parse_color_mode_and_preset() {
        assert_eq!(ColorMode::parse("Dark"), Some(ColorMode::Dark));
        assert_eq!(ColorMode::parse("nope"), None);
        assert_eq!(DesignPreset::parse("custom"), Some(DesignPreset::Custom));
        assert_eq!(DesignPreset::parse("x"), None);
    }

    #[test]
    fn validate_rejects_bad_hex_and_radius() {
        let bad = DesignTokens {
            accent: "blue".into(),
            accent_focus: "#0071e3".into(),
            radius_px: 18,
            density: "comfortable".into(),
            font_scale: 1.0,
        };
        assert!(bad.validate().is_err());
        let wide = DesignTokens {
            accent: "#0066cc".into(),
            accent_focus: "#0071e3".into(),
            radius_px: 99,
            density: "comfortable".into(),
            font_scale: 1.0,
        };
        assert!(wide.validate().is_err());
    }

    #[test]
    fn custom_save_and_restore_default() {
        with_test_data_dir(|| {
            let tokens = DesignTokens {
                accent: "#112233".into(),
                accent_focus: "#445566".into(),
                radius_px: 12,
                density: "compact".into(),
                font_scale: 1.1,
            };
            let saved = save_custom_tokens(tokens.clone()).expect("save custom");
            assert_eq!(saved.preset, DesignPreset::Custom);
            assert_eq!(resolve_tokens(&saved).accent, "#112233");

            let restored = restore_default_design().expect("restore");
            assert_eq!(restored.preset, DesignPreset::Default);
            assert!(restored.custom.is_none());
            assert_eq!(resolve_tokens(&restored), default_tokens());
        });
    }

    #[test]
    fn switching_to_default_keeps_custom_on_disk_until_restore() {
        with_test_data_dir(|| {
            let _ = save_custom_tokens(DesignTokens {
                accent: "#abcdef".into(),
                accent_focus: "#fedcba".into(),
                radius_px: 10,
                density: "compact".into(),
                font_scale: 0.95,
            })
            .unwrap();
            let design = apply_design_preset(DesignPreset::Default).unwrap();
            assert_eq!(design.preset, DesignPreset::Default);
            assert!(design.custom.is_some());
            assert_eq!(resolve_tokens(&design), default_tokens());
        });
    }

    #[test]
    fn user_color_mode_persists() {
        with_test_data_dir(|| {
            assert_eq!(load_user_color_mode("Admin"), ColorMode::Light);
            save_user_color_mode("Admin", ColorMode::Dark).unwrap();
            assert_eq!(load_user_color_mode("Admin"), ColorMode::Dark);
            assert_eq!(safe_username_key("Admin/../x"), "admin_.._x");
        });
    }
}
