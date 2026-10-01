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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ColorMode {
    #[default]
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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DesignPreset {
    #[default]
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

/// Editable design tokens (Custom profile). Default values come from [`default_tokens`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DesignTokens {
    pub accent: String,
    pub accent_focus: String,
    pub radius_px: u8,
    pub density: String,
    pub font_scale: f32,
}

/// Optional catalog theme backgrounds (gradients / surface colors). Not used by manual Custom edits alone.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ThemeBackground {
    /// Preferred operator color mode when applying (`light` or `dark`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color_mode: Option<String>,
    /// CSS `background` value for `body` / `.panel-layout` (gradients preferred).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    /// CSS `background` for `.sidebar`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sidebar: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub surface: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canvas: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub surface_soft: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ink: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub muted: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hairline: Option<String>,
    /// Theme Store card swatch CSS background (falls back to body / accent gradient).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview: Option<String>,
}

impl ThemeBackground {
    pub fn is_empty(&self) -> bool {
        self.color_mode.is_none()
            && self.body.is_none()
            && self.sidebar.is_none()
            && self.surface.is_none()
            && self.canvas.is_none()
            && self.surface_soft.is_none()
            && self.ink.is_none()
            && self.muted.is_none()
            && self.hairline.is_none()
            && self.preview.is_none()
    }

    pub fn validate(mut self) -> Result<Self, String> {
        if let Some(mode) = self.color_mode.take() {
            let normalized = mode.trim().to_ascii_lowercase();
            if normalized != "light" && normalized != "dark" {
                return Err("background.color_mode must be light or dark".into());
            }
            self.color_mode = Some(normalized);
        }
        self.body = Self::validate_css_bg(self.body.take(), "background.body")?;
        self.sidebar = Self::validate_css_bg(self.sidebar.take(), "background.sidebar")?;
        self.preview = Self::validate_css_bg(self.preview.take(), "background.preview")?;
        self.surface = Self::validate_color_or_none(self.surface.take(), "background.surface")?;
        self.canvas = Self::validate_color_or_none(self.canvas.take(), "background.canvas")?;
        self.surface_soft =
            Self::validate_color_or_none(self.surface_soft.take(), "background.surface_soft")?;
        self.ink = Self::validate_color_or_none(self.ink.take(), "background.ink")?;
        self.muted = Self::validate_color_or_none(self.muted.take(), "background.muted")?;
        self.hairline = Self::validate_color_or_none(self.hairline.take(), "background.hairline")?;
        Ok(self)
    }

    fn validate_color_or_none(raw: Option<String>, field: &str) -> Result<Option<String>, String> {
        let Some(value) = raw else {
            return Ok(None);
        };
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Ok(None);
        }
        Ok(Some(normalize_hex_color(trimmed, field)?))
    }

    fn validate_css_bg(raw: Option<String>, field: &str) -> Result<Option<String>, String> {
        let Some(value) = raw else {
            return Ok(None);
        };
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Ok(None);
        }
        if trimmed.len() > 1200 {
            return Err(format!("{field} is too long"));
        }
        let lower = trimmed.to_ascii_lowercase();
        if lower.contains("url(")
            || lower.contains("expression(")
            || lower.contains("javascript:")
            || lower.contains("@import")
            || lower.contains("</")
            || lower.contains("behavior:")
            || trimmed.contains(';')
            || trimmed.contains('{')
            || trimmed.contains('}')
        {
            return Err(format!("{field} contains disallowed CSS"));
        }
        let allowed = lower.starts_with('#')
            || lower.starts_with("rgb(")
            || lower.starts_with("rgba(")
            || lower.starts_with("hsl(")
            || lower.starts_with("hsla(")
            || lower.starts_with("linear-gradient(")
            || lower.starts_with("radial-gradient(")
            || lower.starts_with("repeating-linear-gradient(")
            || lower.starts_with("repeating-radial-gradient(")
            || lower.starts_with("conic-gradient(");
        if !allowed {
            return Err(format!(
                "{field} must be a color or CSS gradient (no external urls)"
            ));
        }
        Ok(Some(trimmed.to_string()))
    }
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
        radius_px: 8,
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
    /// Catalog theme id currently applied as Custom (panel-wide).
    #[serde(default)]
    pub active_theme_id: Option<String>,
    /// Background package from the active catalog theme (cleared when leaving theme Custom).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_theme_background: Option<ThemeBackground>,
    /// Optional sanitized extra CSS from the active theme `theme.css` file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_theme_extra_css: Option<String>,
}

impl Default for PanelDesignFile {
    fn default() -> Self {
        Self {
            schema_version: 1,
            preset: DesignPreset::Default,
            custom: None,
            active_theme_id: None,
            active_theme_background: None,
            active_theme_extra_css: None,
        }
    }
}

fn design_path() -> PathBuf {
    data_dir().join("panel-design.json")
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
        DesignPreset::Custom => design.custom.clone().unwrap_or_else(default_tokens),
    }
}

pub fn apply_design_preset(preset: DesignPreset) -> Result<PanelDesignFile, String> {
    let mut design = load_panel_design();
    match preset {
        DesignPreset::Default => {
            // Keep any saved custom profile on disk, but activate Default.
            design.preset = DesignPreset::Default;
            design.active_theme_id = None;
            design.active_theme_background = None;
            design.active_theme_extra_css = None;
        }
        DesignPreset::Light | DesignPreset::Dark => {
            design.preset = preset;
            design.active_theme_id = None;
            design.active_theme_background = None;
            design.active_theme_extra_css = None;
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
    save_custom_tokens_with_theme(tokens, None, None, None)
}

/// Save custom tokens and optionally mark which installed catalog theme is active.
pub fn save_custom_tokens_with_theme(
    tokens: DesignTokens,
    active_theme_id: Option<&str>,
    background: Option<ThemeBackground>,
    extra_css: Option<String>,
) -> Result<PanelDesignFile, String> {
    let tokens = tokens.validate()?;
    let background = match background {
        Some(bg) => {
            let validated = bg.validate()?;
            if validated.is_empty() {
                None
            } else {
                Some(validated)
            }
        }
        None => None,
    };
    let extra_css = extra_css
        .map(|css| sanitize_theme_extra_css(&css))
        .filter(|css| !css.trim().is_empty());
    let mut design = load_panel_design();
    design.custom = Some(tokens);
    design.preset = DesignPreset::Custom;
    let theme_id = active_theme_id
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty());
    design.active_theme_id = theme_id.clone();
    if theme_id.is_some() {
        design.active_theme_background = background;
        design.active_theme_extra_css = extra_css;
    } else {
        design.active_theme_background = None;
        design.active_theme_extra_css = None;
    }
    save_panel_design(&design)?;
    Ok(design)
}

/// Strip risky constructs from optional theme.css (keep gradients and selectors).
pub fn sanitize_theme_extra_css(raw: &str) -> String {
    let mut out = String::new();
    for line in raw.lines() {
        let lower = line.to_ascii_lowercase();
        if lower.contains("@import")
            || lower.contains("javascript:")
            || lower.contains("expression(")
            || lower.contains("behavior:")
            || lower.contains("</")
            || lower.contains("url(http://")
            || lower.contains("url(https://")
            || lower.contains("url(//")
        {
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    if out.len() > 24_000 {
        out.truncate(24_000);
    }
    out
}

/// Wipe custom profile and return to immutable Default.
pub fn restore_default_design() -> Result<PanelDesignFile, String> {
    let design = PanelDesignFile::default();
    save_panel_design(&design)?;
    Ok(design)
}

/// Inline `:root` CSS variables for the active design (panel chrome + Manage).
pub fn design_css_vars(design: &PanelDesignFile) -> String {
    let tokens = resolve_tokens(design);
    let density_pad = if tokens.density == "compact" {
        "0.92"
    } else {
        "1"
    };
    let mut css = format!(
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
.sidebar nav a.active {{
  background:color-mix(in srgb, var(--cpn-accent, var(--blue)) 16%, transparent);
  color:var(--blue);
}}
.btn-primary, a.btn-primary {{ background:var(--cpn-accent, var(--blue)); }}
.site-manage {{
  --m-accent:var(--cpn-accent);
}}
"#,
        accent = tokens.accent,
        focus = tokens.accent_focus,
        radius = tokens.radius_px,
        density = density_pad,
        scale = tokens.font_scale,
    );
    if design.preset == DesignPreset::Custom {
        if let Some(bg) = design.active_theme_background.as_ref() {
            css.push_str(&theme_background_css(bg));
        }
        if let Some(extra) = design.active_theme_extra_css.as_deref() {
            let cleaned = sanitize_theme_extra_css(extra);
            if !cleaned.trim().is_empty() {
                css.push('\n');
                css.push_str(&cleaned);
                css.push('\n');
            }
        }
    }
    css
}

fn theme_background_css(bg: &ThemeBackground) -> String {
    let mut root_vars = String::new();
    if let Some(v) = bg.surface.as_deref() {
        root_vars.push_str(&format!("  --surface:{v};\n"));
    }
    if let Some(v) = bg.canvas.as_deref() {
        root_vars.push_str(&format!("  --canvas:{v};\n"));
    }
    if let Some(v) = bg.surface_soft.as_deref() {
        root_vars.push_str(&format!("  --surface-soft:{v};\n"));
    }
    if let Some(v) = bg.ink.as_deref() {
        root_vars.push_str(&format!("  --ink:{v};\n"));
    }
    if let Some(v) = bg.muted.as_deref() {
        root_vars.push_str(&format!("  --muted:{v};\n"));
    }
    if let Some(v) = bg.hairline.as_deref() {
        root_vars.push_str(&format!("  --hairline:{v};\n"));
    }
    let mut out = String::new();
    if !root_vars.is_empty() {
        // Beat color-mode attribute selectors that set surface tokens.
        out.push_str("html[data-color-mode=\"dark\"], html[data-color-mode=\"light\"], :root {\n");
        out.push_str(&root_vars);
        out.push_str("}\n");
        out.push_str("html[data-color-mode=\"dark\"], html[data-color-mode=\"light\"] {\n");
        out.push_str(&root_vars);
        out.push_str("}\n");
    }
    if let Some(body) = bg.body.as_deref() {
        // Higher specificity than `[data-color-mode] body` so theme gradients stick.
        out.push_str(&format!(
            "html[data-color-mode=\"dark\"] body,\n\
html[data-color-mode=\"light\"] body,\n\
html[data-color-mode=\"dark\"] .panel-layout,\n\
html[data-color-mode=\"light\"] .panel-layout,\n\
body, .panel-layout {{\n  background:{body};\n  background-attachment:fixed;\n}}\n"
        ));
    }
    if let Some(sidebar) = bg.sidebar.as_deref() {
        out.push_str(&format!(
            "html[data-color-mode=\"dark\"] .sidebar,\n\
html[data-color-mode=\"light\"] .sidebar,\n\
.sidebar {{\n  background:{sidebar};\n  border-right-color:var(--hairline);\n}}\n"
        ));
    }
    out
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
  background:color-mix(in srgb, var(--cpn-accent, var(--blue)) 22%, transparent);
  color:var(--blue);
}
[data-color-mode="dark"] .mobile-header,
html[data-color-mode="dark"] .mobile-header,
[data-color-mode="dark"] body .mobile-header {
  background:#161922; border-bottom-color:#2a2f3a; color:#f2f4f7;
}
[data-color-mode="dark"] .mobile-header strong,
[data-color-mode="dark"] .mobile-header .icon-btn {
  color:#f2f4f7;
}
[data-color-mode="dark"] .mobile-header .logout {
  color:#d0d5dd;
}
[data-color-mode="dark"] .mobile-header .logout:hover {
  color:#f2f4f7;
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
[data-color-mode="dark"] .hub-badge.live { background:rgba(6,118,71,.22); color:#6ce9a6; }
[data-color-mode="dark"] .hub-badge.scaffold { background:#2a2f3a; color:#98a2b3; }
[data-color-mode="dark"] .stack-form label,
html[data-color-mode="dark"] .stack-form label {
  color:#f2f4f7;
}
[data-color-mode="dark"] .stack-form input,
[data-color-mode="dark"] .stack-form select,
[data-color-mode="dark"] .stack-form textarea,
html[data-color-mode="dark"] .stack-form input,
html[data-color-mode="dark"] .stack-form select,
html[data-color-mode="dark"] .stack-form textarea {
  background:var(--canvas,#1a1d26); border-color:var(--hairline,#2a2f3a); color:#f2f4f7;
}
[data-color-mode="dark"] .mfa-codes-panel,
[data-color-mode="dark"] .mfa-totp-setup,
html[data-color-mode="dark"] .mfa-codes-panel,
html[data-color-mode="dark"] .mfa-totp-setup {
  background:var(--surface-soft,#161922); border-color:var(--hairline,#2a2f3a); color:#f2f4f7;
}
[data-color-mode="dark"] .mfa-codes-label,
[data-color-mode="dark"] .mfa-codes-panel ul,
[data-color-mode="dark"] .mfa-codes-panel code,
[data-color-mode="dark"] .mfa-totp-setup,
[data-color-mode="dark"] .mfa-totp-setup p,
[data-color-mode="dark"] .mfa-totp-setup strong,
[data-color-mode="dark"] .mfa-totp-setup code,
[data-color-mode="dark"] .mfa-totp-setup label,
[data-color-mode="dark"] .mfa-totp-setup .muted,
[data-color-mode="dark"] .mfa-totp-setup p.muted,
[data-color-mode="dark"] .mfa-secret,
html[data-color-mode="dark"] .mfa-codes-label,
html[data-color-mode="dark"] .mfa-totp-setup label,
html[data-color-mode="dark"] .mfa-secret {
  color:#f2f4f7;
}
[data-color-mode="dark"] .mfa-codes-actions .btn-secondary,
[data-color-mode="dark"] .btn-secondary,
html[data-color-mode="dark"] .mfa-codes-actions .btn-secondary,
html[data-color-mode="dark"] .btn-secondary {
  background:var(--canvas,#1a1d26); border-color:var(--hairline,#2a2f3a); color:#f2f4f7;
}
[data-color-mode="dark"] a.btn-primary,
html[data-color-mode="dark"] a.btn-primary { color:#fff; }
.password-policy-link {
  color: var(--blue, #0066cc);
  text-decoration: underline;
  text-underline-offset: 2px;
}
.password-policy-link:hover {
  color: var(--blue-focus, #3385d6);
}
[data-color-mode="dark"] .password-policy-link,
html[data-color-mode="dark"] .password-policy-link {
  color: #60a5fa;
}
[data-color-mode="dark"] .password-policy-link:hover,
html[data-color-mode="dark"] .password-policy-link:hover {
  color: #93c5fd;
}
.theme-toggle {
  display:inline-grid; place-items:center; cursor:pointer; color:var(--muted);
}
.theme-toggle:focus-visible { outline:2px solid var(--blue-focus); outline-offset:2px; }
.theme-toggle-icon { display:inline-grid; place-items:center; line-height:1; }
"#
}

pub fn design_public_json(design: &PanelDesignFile) -> serde_json::Value {
    let tokens = resolve_tokens(design);
    serde_json::json!({
        "preset": design.preset.as_str(),
        "tokens": tokens,
        "default_tokens": default_tokens(),
        "has_custom": design.custom.is_some(),
        "active_theme_id": design.active_theme_id,
        "has_theme_background": design.active_theme_background.is_some(),
        "active_theme_background": design.active_theme_background,
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
}
