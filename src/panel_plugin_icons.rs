//! Visual thumbnails for plugin cards (Store and Installed, Host / CPN-only / Site).
//!
//! Resolution order per card:
//! 1. Catalog icon URL (`<icon>` in `meta.xml` or bundled `icon.*`), layered over a fallback tile.
//! 2. Bundled brand mark by plugin id (Docker whale, MariaDB, Redis, phpMyAdmin, ...).
//! 3. Lucide-style glyph on a per-plugin gradient (id-derived hue) when the id or category is known.
//! 4. Lettermark (initials) on the same per-plugin gradient so every card has a unique visual.

use crate::panel_icons_svg::svg_owned;
use crate::panel_plugin_icons_brand::{BrandMark, brand_mark};
use crate::panel_plugins_markup::html_escape;

fn normalized_id(id: &str) -> String {
    id.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// Map a plugin / host package id to a bundled brand mark slug.
pub(crate) fn brand_slug_for(id: &str) -> Option<&'static str> {
    let n = normalized_id(id);
    let rules: &[(&[&str], &str)] = &[
        (&["docker", "podman"], "docker"),
        (&["mariadb", "mysql"], "mariadb"),
        (&["postgres", "pgsql"], "postgresql"),
        (&["redis"], "redis"),
        (&["rabbit"], "rabbitmq"),
        (&["phpmyadmin"], "phpmyadmin"),
        (&["nextcloud", "nextsnapmail"], "nextcloud"),
        (&["roundcube"], "roundcube"),
        (&["proton"], "proton"),
        (&["discord"], "discord"),
        (&["paypal"], "paypal"),
        (&["googletagmanager", "gtm"], "googletagmanager"),
        (&["pm2"], "pm2"),
        (&["contabo"], "contabo"),
        (&["cloudflare"], "cloudflare"),
        (&["dovecot"], "dovecot"),
        (&["letsencrypt", "certbot"], "letsencrypt"),
    ];
    for (needles, slug) in rules {
        if needles.iter().any(|needle| n.contains(needle)) {
            return Some(slug);
        }
    }
    if n.starts_with("php") {
        return Some("php");
    }
    None
}

/// Glyph key (see [`svg_owned`]) chosen from the plugin id, then from its category.
pub(crate) fn glyph_for(id: &str, category: &str) -> Option<&'static str> {
    let n = normalized_id(id);
    let rules: &[(&[&str], &str)] = &[
        (&["clam", "antivirus", "virus"], "bug"),
        (&["fail2ban"], "shield"),
        (&["autoban", "securityalert"], "shield-alert"),
        (&["bimi"], "image"),
        (&["mtasts"], "lock"),
        (&["mragent", "agent", "assistant"], "bot"),
        (&["filegator", "filemanager", "files"], "folder"),
        (&["csp"], "lock"),
        (&["marketing", "newsletter"], "megaphone"),
        (&["memcache"], "cpu"),
        (&["panelaccess", "access", "auth"], "key-round"),
        (&["port"], "plug"),
        (&["malware", "scan"], "scan"),
        (&["billing", "commerce", "invoice"], "credit-card"),
        (&["snappymail"], "send"),
        (&["tachyon"], "inbox"),
        (&["sogo", "calendar"], "calendar"),
        (&["webhook"], "send"),
        (&["snapshot", "backup"], "hard-drive"),
        (&["email", "mail", "postfix"], "mail"),
        (&["premium"], "tags"),
        (&["dns"], "dns"),
        (&["ssl", "cert"], "certificate"),
    ];
    for (needles, key) in rules {
        if needles.iter().any(|needle| n.contains(needle)) {
            return Some(key);
        }
    }
    glyph_for_category(category)
}

fn glyph_for_category(category: &str) -> Option<&'static str> {
    let c = category.trim().to_ascii_lowercase();
    let key = match c.as_str() {
        "security" => "shield",
        "email" | "mail" => "mail",
        "database" | "databases" => "database",
        "utility" | "utilities" | "tools" => "settings",
        "apps" | "app" | "applications" => "boxes",
        "analytics" | "monitoring" => "activity",
        "network" | "networking" | "dns" => "network",
        "files" | "storage" => "folder",
        "payments" | "billing" | "commerce" => "credit-card",
        "ai" | "assistant" => "bot",
        _ => return None,
    };
    Some(key)
}

/// FNV-1a hash so a plugin id always yields the same hue (0..360).
pub(crate) fn hue_for(id: &str) -> u32 {
    let mut hash: u32 = 0x811c_9dc5;
    for byte in normalized_id(id).bytes() {
        hash ^= u32::from(byte);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    hash % 360
}

fn gradient_style(id: &str) -> String {
    let hue = hue_for(id);
    let hue2 = (hue + 36) % 360;
    format!("background:linear-gradient(135deg,hsl({hue} 72% 46%),hsl({hue2} 70% 34%));")
}

/// Up to two initials from a display name (falls back to the id).
pub(crate) fn initials_for(name: &str, id: &str) -> String {
    let source = if name.trim().is_empty() { id } else { name };
    let words: Vec<&str> = source
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();
    let mut out = String::new();
    match words.len() {
        0 => out.push('?'),
        1 => {
            for c in words[0].chars().take(2) {
                out.push(c.to_ascii_uppercase());
            }
        }
        _ => {
            for word in words.iter().take(2) {
                if let Some(c) = word.chars().next() {
                    out.push(c.to_ascii_uppercase());
                }
            }
        }
    }
    out
}

fn brand_svg(mark: &BrandMark) -> String {
    format!(
        r#"<svg viewBox="0 0 24 24" aria-hidden="true" focusable="false"><path fill="{fill}" d="{path}"/></svg>"#,
        fill = mark.fill,
        path = mark.path,
    )
}

/// Fallback tile parts (no catalog image): `(kind class, inline style, aria label, inner html)`.
fn fallback_tile(id: &str, name: &str, category: &str) -> (&'static str, String, String, String) {
    let label_name = if name.trim().is_empty() { id } else { name };
    if let Some(mark) = brand_slug_for(id).and_then(brand_mark) {
        return (
            "plugin-thumb-brand",
            String::new(),
            format!("{} logo", mark.title),
            brand_svg(mark),
        );
    }
    let style = gradient_style(id);
    if let Some(key) = glyph_for(id, category) {
        return (
            "plugin-thumb-glyph",
            style,
            format!("{label_name} icon"),
            svg_owned(key),
        );
    }
    (
        "plugin-thumb-letter",
        style,
        format!("{label_name} icon"),
        format!(
            r#"<span class="plugin-thumb-text">{}</span>"#,
            html_escape(&initials_for(name, id))
        ),
    )
}

fn thumb_with_optional_image(
    id: &str,
    name: &str,
    category: &str,
    icon_url: Option<&str>,
    extra_class: &str,
) -> String {
    let (kind, style, label, mut inner) = fallback_tile(id, name, category);
    let mut classes = format!("plugin-thumb {kind}{extra_class}");
    if let Some(url) = icon_url
        .map(str::trim)
        .filter(|u| u.to_ascii_lowercase().starts_with("https://"))
    {
        // Layer the catalog image over the fallback tile: a failed load still shows a visual.
        classes.push_str(" plugin-thumb-img");
        inner.push_str(&format!(
            r#"<img src="{src}" alt="" loading="lazy" decoding="async" referrerpolicy="no-referrer" width="56" height="56">"#,
            src = html_escape(url),
        ));
    }
    let style_attr = if style.is_empty() {
        String::new()
    } else {
        format!(r#" style="{style}""#)
    };
    format!(
        r#"<span class="{classes}"{style_attr} role="img" aria-label="{label}" title="{label}">{inner}</span>"#,
        classes = classes,
        style_attr = style_attr,
        label = html_escape(&label),
        inner = inner,
    )
}

/// Card-size thumbnail (56px) for Store and Installed cards.
pub(crate) fn plugin_thumb(id: &str, name: &str, category: &str, icon_url: Option<&str>) -> String {
    thumb_with_optional_image(id, name, category, icon_url, "")
}

/// Compact thumbnail (30px) for table rows.
pub(crate) fn plugin_thumb_small(
    id: &str,
    name: &str,
    category: &str,
    icon_url: Option<&str>,
) -> String {
    thumb_with_optional_image(id, name, category, icon_url, " sm")
}

/// Card header: thumbnail beside the `<h3>` title (title is escaped here).
pub(crate) fn card_head(id: &str, name: &str, category: &str, icon_url: Option<&str>) -> String {
    format!(
        r#"<div class="plugin-card-head">{thumb}<h3>{name}</h3></div>"#,
        thumb = plugin_thumb(id, name, category, icon_url),
        name = html_escape(name),
    )
}

/// CSS for thumbnails (light + dark), appended to the Plugins hub style block.
pub(crate) fn plugin_thumb_styles() -> &'static str {
    r#"
        .plugin-card-head { display:flex; align-items:center; gap:12px; min-width:0; }
        .plugin-card-head h3 { flex:1 1 auto; min-width:0; overflow-wrap:anywhere; }
        .plugin-thumb {
          position:relative; flex:0 0 auto; display:inline-flex; align-items:center; justify-content:center;
          width:56px; height:56px; border-radius:14px; overflow:hidden; color:#ffffff;
          box-shadow:inset 0 0 0 1px rgba(15,23,42,.10); vertical-align:middle;
        }
        .plugin-thumb svg { width:58%; height:58%; display:block; }
        .plugin-thumb-glyph svg { stroke:#ffffff; }
        .plugin-thumb-brand { background:#ffffff; border:1px solid var(--hairline); }
        .plugin-thumb-brand svg { width:66%; height:66%; }
        .plugin-thumb-text { font-weight:800; font-size:20px; letter-spacing:.02em; line-height:1; color:#ffffff; }
        .plugin-thumb img {
          position:absolute; inset:0; width:100%; height:100%; object-fit:contain; padding:6px;
          box-sizing:border-box; background:#ffffff;
        }
        .plugin-thumb.sm { width:30px; height:30px; border-radius:8px; }
        .plugin-thumb.sm .plugin-thumb-text { font-size:11px; }
        .plugin-thumb.sm img { padding:3px; }
        .plugin-thumb-cell { display:inline-flex; align-items:center; gap:10px; min-width:0; }
        [data-color-mode="dark"] .plugin-thumb { box-shadow:inset 0 0 0 1px rgba(255,255,255,.12); }
        [data-color-mode="dark"] .plugin-thumb-brand { background:#f1f5f9; border-color:#334155; }
        [data-color-mode="dark"] .plugin-thumb img { background:#f1f5f9; }
        @media (max-width:520px) {
          .plugin-thumb { width:48px; height:48px; border-radius:12px; }
          .plugin-thumb-text { font-size:17px; }
        }"#
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brand_lookup_by_id() {
        assert_eq!(brand_slug_for("docker"), Some("docker"));
        assert_eq!(brand_slug_for("limitedPhpmyAdmin"), Some("phpmyadmin"));
        assert_eq!(brand_slug_for("postgresManager"), Some("postgresql"));
        assert_eq!(brand_slug_for("discordWebhooks"), Some("discord"));
        assert_eq!(brand_slug_for("contaboAutoSnapshot"), Some("contabo"));
        assert_eq!(brand_slug_for("nextsnapmail"), Some("nextcloud"));
        assert_eq!(brand_slug_for("clamav"), None);
        assert_eq!(brand_slug_for("phpVersions"), Some("php"));
    }

    #[test]
    fn glyph_lookup_by_id_then_category() {
        assert_eq!(glyph_for("clamav", "Security"), Some("bug"));
        assert_eq!(glyph_for("mtaSts", "Email"), Some("lock"));
        assert_eq!(glyph_for("mrAgent", "Utility"), Some("bot"));
        assert_eq!(glyph_for("unknownThing", "Security"), Some("shield"));
        assert_eq!(glyph_for("unknownThing", "Weird"), None);
    }

    #[test]
    fn hue_is_stable_and_bounded() {
        assert_eq!(hue_for("clamav"), hue_for("ClamAV"));
        assert!(hue_for("clamav") < 360);
        assert_ne!(hue_for("clamav"), hue_for("bimi"));
    }

    #[test]
    fn initials_from_name_or_id() {
        assert_eq!(initials_for("Auto Ban Security Alerts", "x"), "AB");
        assert_eq!(initials_for("Fail2ban", "x"), "FA");
        assert_eq!(initials_for("", "testPlugin"), "TE");
        assert_eq!(initials_for("", ""), "?");
    }

    #[test]
    fn docker_card_uses_brand_mark_not_generic_square() {
        let html = plugin_thumb("docker", "Docker", "Utility", None);
        assert!(html.contains("plugin-thumb-brand"));
        assert!(html.contains("#2496ED"));
        assert!(html.contains(r#"aria-label="Docker logo""#));
        assert!(!html.contains("plugin-thumb-letter"));
    }

    #[test]
    fn clamav_and_bimi_get_distinct_glyph_tiles() {
        let clam = plugin_thumb("clamav", "ClamAV", "Security", None);
        let bimi = plugin_thumb("bimi", "BIMI", "Email", None);
        assert!(clam.contains("plugin-thumb-glyph"));
        assert!(bimi.contains("plugin-thumb-glyph"));
        assert!(clam.contains("linear-gradient"));
        assert_ne!(clam, bimi);
        assert!(clam.contains(r#"aria-label="ClamAV icon""#));
    }

    #[test]
    fn unknown_plugin_gets_lettermark() {
        let html = plugin_thumb("examplePlugin", "Example Plugin", "Misc", None);
        assert!(html.contains("plugin-thumb-letter"));
        assert!(html.contains(">EP<"));
    }

    #[test]
    fn catalog_icon_layers_image_over_fallback() {
        let html = plugin_thumb(
            "clamav",
            "ClamAV",
            "Security",
            Some("https://example.com/clamav.svg"),
        );
        assert!(html.contains("plugin-thumb-img"));
        assert!(html.contains(r#"<img src="https://example.com/clamav.svg" alt="""#));
        assert!(html.contains("plugin-thumb-glyph"));
        assert!(html.ends_with("</span>"));
        // Non-https icon URLs are ignored (fallback only).
        let plain = plugin_thumb("clamav", "ClamAV", "Security", Some("http://x/y.png"));
        assert!(!plain.contains("<img"));
    }

    #[test]
    fn small_variant_and_card_head() {
        let small = plugin_thumb_small("redis", "Redis", "Utility", None);
        assert!(small.contains("plugin-thumb-brand sm"));
        let head = card_head("bimi", "BIMI <x>", "Email", None);
        assert!(head.starts_with(r#"<div class="plugin-card-head">"#));
        assert!(head.contains("<h3>BIMI &lt;x&gt;</h3>"));
    }
}
