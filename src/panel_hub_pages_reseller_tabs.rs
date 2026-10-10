//! Reseller Center tab chrome (Overview / Resellers / Quotas / Branding).

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Normalize `?tab=` from the query string.
pub fn normalize_reseller_tab(raw: &str) -> &'static str {
    match raw.trim().to_ascii_lowercase().as_str() {
        "resellers" | "reseller" | "manage" | "hierarchy" => "resellers",
        "quotas" | "quota" | "limits" | "pool" => "quotas",
        "branding" | "brand" | "logo" => "branding",
        _ => "overview",
    }
}

/// Pill tab bar matching Plugins / Design Store styling (readable dark and light).
pub fn reseller_tab_bar(active: &str) -> String {
    let tab = normalize_reseller_tab(active);
    let link = |id: &str, label: &str| -> String {
        let active_cls = if tab == id { " active" } else { "" };
        let selected = if tab == id { "true" } else { "false" };
        format!(
            r#"<a class="plugin-tab{active_cls}" role="tab" aria-selected="{selected}" href="/account/users/reseller?tab={id}">{label}</a>"#,
            active_cls = active_cls,
            selected = selected,
            id = html_escape(id),
            label = html_escape(label),
        )
    };
    format!(
        r#"<style>
.reseller-tabs.plugin-tabs {{ display:flex; flex-wrap:wrap; gap:8px; margin:0 0 18px; }}
.reseller-tabs .plugin-tab {{
  display:inline-flex; align-items:center; justify-content:center;
  min-height:40px; padding:8px 14px; border-radius:10px;
  border:1px solid var(--hairline,#cbd5e1); background:transparent;
  color:inherit; text-decoration:none; font:inherit; font-size:13px; font-weight:600;
  white-space:nowrap;
}}
.reseller-tabs .plugin-tab:hover {{ border-color:var(--blue,#2563eb); }}
.reseller-tabs .plugin-tab.active {{ background:#e7f1ff; color:#0b3d91; border-color:#93c5fd; }}
[data-color-mode="dark"] .reseller-tabs .plugin-tab {{
  color:#e2e8f0; background:#1a1d26; border-color:#334155;
}}
[data-color-mode="dark"] .reseller-tabs .plugin-tab.active {{
  background:rgba(59,130,246,.25); color:#bfdbfe; border-color:#60a5fa;
}}
@media (max-width:719.98px) {{
  .reseller-tabs.plugin-tabs {{ flex-wrap:nowrap; overflow-x:auto; -webkit-overflow-scrolling:touch; }}
  .reseller-tabs .plugin-tab {{ min-height:44px; }}
}}
</style>
<nav class="plugin-tabs reseller-tabs" role="tablist" aria-label="Reseller Center sections">
  {overview}
  {resellers}
  {quotas}
  {branding}
</nav>"#,
        overview = link("overview", "Overview"),
        resellers = link("resellers", "Resellers"),
        quotas = link("quotas", "Quotas"),
        branding = link("branding", "Branding"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_maps_aliases() {
        assert_eq!(normalize_reseller_tab("overview"), "overview");
        assert_eq!(normalize_reseller_tab("hierarchy"), "resellers");
        assert_eq!(normalize_reseller_tab("pool"), "quotas");
        assert_eq!(normalize_reseller_tab("logo"), "branding");
        assert_eq!(normalize_reseller_tab(""), "overview");
    }

    #[test]
    fn tab_bar_marks_active_and_avoids_scaffold_brand() {
        let html = reseller_tab_bar("quotas");
        assert!(html.contains("aria-selected=\"true\""));
        assert!(html.contains("?tab=quotas"));
        assert!(html.contains("Overview"));
        assert!(html.contains("Resellers"));
        assert!(html.contains("Quotas"));
        assert!(html.contains("Branding"));
        assert!(!html.to_lowercase().contains("cyberpanel"));
        assert!(!html.contains("Not configured yet"));
    }
}
