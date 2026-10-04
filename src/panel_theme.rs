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
    /// Relative package image layered under the body scrim (e.g. `assets/bg.webp`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    /// Relative package image for Theme Store cards (e.g. `assets/preview.webp`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview_image: Option<String>,
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
            && self.image.is_none()
            && self.preview_image.is_none()
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
        self.image = validate_theme_asset_path(self.image.take(), "background.image")?;
        self.preview_image =
            validate_theme_asset_path(self.preview_image.take(), "background.preview_image")?;
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

/// Validate a package-relative theme asset path (`assets/name.ext` only).
pub fn validate_theme_asset_path(
    raw: Option<String>,
    field: &str,
) -> Result<Option<String>, String> {
    let Some(value) = raw else {
        return Ok(None);
    };
    let trimmed = value.trim().replace('\\', "/");
    if trimmed.is_empty() {
        return Ok(None);
    }
    if trimmed.contains("..") || trimmed.starts_with('/') || trimmed.contains(':') {
        return Err(format!("{field} must be a relative assets/ path"));
    }
    let Some((prefix, name)) = trimmed.split_once('/') else {
        return Err(format!("{field} must live under assets/"));
    };
    if !prefix.eq_ignore_ascii_case("assets") || name.contains('/') {
        return Err(format!("{field} must be assets/<filename>"));
    }
    if name.is_empty()
        || !name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' || ch == '.')
    {
        return Err(format!("{field} filename is invalid"));
    }
    let lower = name.to_ascii_lowercase();
    let ok_ext = lower.ends_with(".webp")
        || lower.ends_with(".jpg")
        || lower.ends_with(".jpeg")
        || lower.ends_with(".png")
        || lower.ends_with(".svg");
    if !ok_ext {
        return Err(format!("{field} must be .webp, .jpg, .jpeg, .png, or .svg"));
    }
    Ok(Some(format!("assets/{name}")))
}

/// Same-origin URL for an installed or catalog-cached theme asset.
pub fn theme_asset_public_url(theme_id: &str, asset_path: &str) -> Option<String> {
    let id = theme_id.trim().to_ascii_lowercase();
    if id.is_empty()
        || !id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
    {
        return None;
    }
    let Ok(Some(path)) = validate_theme_asset_path(Some(asset_path.to_string()), "asset") else {
        return None;
    };
    let file = path.trim_start_matches("assets/");
    Some(format!("/api/panel/themes/assets/{id}/{file}"))
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
        .map(|css| {
            let rewritten = if let Some(id) = active_theme_id {
                rewrite_theme_asset_urls(&css, id)
            } else {
                css
            };
            sanitize_theme_extra_css(&rewritten)
        })
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

/// Strip risky constructs from optional theme.css (keep gradients, selectors, same-origin assets).
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
            || lower.contains("url(data:")
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

/// Rewrite relative `url(assets/...)` references to authenticated panel asset routes.
pub fn rewrite_theme_asset_urls(css: &str, theme_id: &str) -> String {
    let id = theme_id.trim().to_ascii_lowercase();
    if id.is_empty() {
        return css.to_string();
    }
    let base = format!("/api/panel/themes/assets/{id}/");
    css.replace("url(./assets/", &format!("url({base}"))
        .replace("url('./assets/", &format!("url('{base}"))
        .replace("url(\"./assets/", &format!("url(\"{base}"))
        .replace("url(assets/", &format!("url({base}"))
        .replace("url('assets/", &format!("url('{base}"))
        .replace("url(\"assets/", &format!("url(\"{base}"))
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
            css.push_str(&theme_background_css(bg, design.active_theme_id.as_deref()));
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

/// CSS selector scope for catalog theme surfaces.
/// Prefer the theme's declared `color_mode` so Light/Dark toggle can fall back to
/// built-in tokens in the opposite mode (avoids pale ink on white nav tiles).
fn theme_surface_selector(bg: &ThemeBackground) -> &'static str {
    match bg.color_mode.as_deref() {
        Some("light") => "html[data-color-mode=\"light\"]",
        Some("dark") => "html[data-color-mode=\"dark\"]",
        // Legacy packages without color_mode: keep previous dual-mode apply.
        _ => "html[data-color-mode=\"dark\"], html[data-color-mode=\"light\"]",
    }
}

fn theme_background_css(bg: &ThemeBackground, theme_id: Option<&str>) -> String {
    let scope = theme_surface_selector(bg);
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
        // Do not write theme ink/muted onto bare :root: that leaks pale text into
        // the opposite personal color mode (white nav tiles + light --ink).
        out.push_str(scope);
        out.push_str(" {\n");
        out.push_str(&root_vars);
        out.push_str("}\n");
    }
    let image_url = theme_id
        .zip(bg.image.as_deref())
        .and_then(|(id, path)| theme_asset_public_url(id, path));
    if image_url.is_some() || bg.body.is_some() {
        let layers = match (bg.body.as_deref(), image_url.as_deref()) {
            (Some(scrim), Some(url)) => format!("{scrim}, url({url})"),
            (Some(scrim), None) => scrim.to_string(),
            (None, Some(url)) => format!("url({url})"),
            (None, None) => String::new(),
        };
        let (size, position, repeat, attachment) = if image_url.is_some() && bg.body.is_some() {
            (
                "auto, cover",
                "center, center",
                "no-repeat, no-repeat",
                "fixed, fixed",
            )
        } else if image_url.is_some() {
            ("cover", "center", "no-repeat", "fixed")
        } else {
            ("auto", "center", "no-repeat", "fixed")
        };
        out.push_str(&format!(
            "{scope} body,\n\
{scope} .panel-layout {{\n  background-image:{layers};\n  background-size:{size};\n  background-position:{position};\n  background-repeat:{repeat};\n  background-attachment:{attachment};\n}}\n"
        ));
    }
    if let Some(sidebar) = bg.sidebar.as_deref() {
        out.push_str(&format!(
            "{scope} .sidebar {{\n  background:{sidebar};\n  border-right-color:var(--hairline);\n}}\n"
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
    fn dark_theme_background_does_not_paint_light_mode_ink() {
        let bg = ThemeBackground {
            color_mode: Some("dark".into()),
            sidebar: Some("linear-gradient(#111,#222)".into()),
            ink: Some("#e8edf4".into()),
            muted: Some("#9aa8b8".into()),
            surface: Some("#0f141b".into()),
            ..ThemeBackground::default()
        };
        let css = theme_background_css(&bg, Some("slate-pro"));
        assert!(
            css.contains("html[data-color-mode=\"dark\"]"),
            "dark theme surfaces must target dark mode"
        );
        assert!(
            !css.contains("html[data-color-mode=\"light\"]"),
            "dark theme ink/sidebar must not apply in light mode"
        );
        assert!(css.contains("--ink:#e8edf4"));
        assert!(css.contains(".sidebar"));
    }

    #[test]
    fn light_theme_background_does_not_paint_dark_mode_ink() {
        let bg = ThemeBackground {
            color_mode: Some("light".into()),
            sidebar: Some("#ffffff".into()),
            ink: Some("#134e4a".into()),
            ..ThemeBackground::default()
        };
        let css = theme_background_css(&bg, Some("arctic-mint"));
        assert!(css.contains("html[data-color-mode=\"light\"]"));
        assert!(
            !css.contains("html[data-color-mode=\"dark\"]"),
            "light theme ink/sidebar must not apply in dark mode"
        );
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
    fn theme_asset_path_and_url_helpers() {
        assert!(
            validate_theme_asset_path(Some("assets/bg.webp".into()), "image")
                .unwrap()
                .is_some()
        );
        assert!(validate_theme_asset_path(Some("../etc/passwd".into()), "image").is_err());
        assert!(validate_theme_asset_path(Some("assets/bg.gif".into()), "image").is_err());
        assert_eq!(
            theme_asset_public_url("aurora-teal", "assets/bg.webp").as_deref(),
            Some("/api/panel/themes/assets/aurora-teal/bg.webp")
        );
        let rewritten =
            rewrite_theme_asset_urls("body{background-image:url(assets/bg.webp);}", "ocean-blue");
        assert!(rewritten.contains("/api/panel/themes/assets/ocean-blue/bg.webp"));
    }
}
