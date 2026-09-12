//! CPN brand assets: favicon links and sidebar icon mark (not CyberPanel).

/// Favicon / touch-icon tags for every panel HTML <head>.
pub fn brand_favicon_links() -> &'static str {
    concat!(
        r#"<link rel="icon" href="/favicon.ico" sizes="any">"#,
        "\n  ",
        r#"<link rel="icon" href="/favicon.svg" type="image/svg+xml">"#,
        "\n  ",
        r#"<link rel="apple-touch-icon" href="/apple-touch-icon.png">"#,
    )
}

/// CPN icon mark: panel window + network nodes (sampled from cpn-logo.png).
/// Blue #006CFA, charcoal #161F2B. Sized for the sidebar (28×28).
pub fn brand_mark_svg() -> &'static str {
    r##"<svg class="cpn-brand-mark" viewBox="0 0 32 32" width="28" height="28" aria-hidden="true" focusable="false" fill="none" xmlns="http://www.w3.org/2000/svg">
  <rect x="2.5" y="3.5" width="21" height="19" rx="3.2" stroke="#161F2B" stroke-width="1.7"/>
  <path d="M2.5 8.2V6.7c0-1.77 1.43-3.2 3.2-3.2h14.6c1.77 0 3.2 1.43 3.2 3.2v1.5H2.5z" fill="#006CFA"/>
  <circle cx="5.6" cy="5.85" r="0.65" fill="#BFD9FF" opacity=".9"/>
  <circle cx="7.7" cy="5.85" r="0.65" fill="#BFD9FF" opacity=".7"/>
  <circle cx="9.8" cy="5.85" r="0.65" fill="#BFD9FF" opacity=".5"/>
  <rect x="5.2" y="10.2" width="3.4" height="3.4" rx="0.7" fill="#006CFA"/>
  <rect x="5.2" y="14.4" width="3.4" height="3.4" rx="0.7" fill="#C8CDD5"/>
  <rect x="5.2" y="18.6" width="3.4" height="2.4" rx="0.7" fill="#C8CDD5"/>
  <rect x="10.6" y="11.1" width="9.2" height="2.2" rx="1.1" fill="#E4E7EC"/>
  <circle cx="17.8" cy="12.2" r="1.35" fill="#006CFA"/>
  <rect x="10.6" y="15.5" width="9.2" height="2.2" rx="1.1" fill="#E4E7EC"/>
  <circle cx="12.6" cy="16.6" r="1.35" fill="#fff" stroke="#006CFA" stroke-width="1"/>
  <g stroke="#161F2B" stroke-width="1.15" stroke-linecap="round">
    <line x1="20.2" y1="22.4" x2="24.6" y2="19.6"/>
    <line x1="20.2" y1="22.4" x2="26.8" y2="24.8"/>
    <line x1="24.6" y1="19.6" x2="28.4" y2="21.2"/>
    <line x1="24.6" y1="19.6" x2="26.8" y2="24.8"/>
    <line x1="28.4" y1="21.2" x2="26.8" y2="24.8"/>
  </g>
  <circle cx="20.2" cy="22.4" r="2.15" fill="#006CFA"/>
  <circle cx="24.6" cy="19.6" r="1.55" fill="#161F2B"/>
  <circle cx="28.4" cy="21.2" r="1.35" fill="#161F2B"/>
  <circle cx="26.8" cy="24.8" r="1.55" fill="#161F2B"/>
</svg>"##
}

#[cfg(test)]
mod tests {
    use super::{brand_favicon_links, brand_mark_svg};

    #[test]
    fn favicon_links_point_at_dedicated_routes() {
        let links = brand_favicon_links();
        assert!(links.contains("/favicon.ico"));
        assert!(links.contains("/favicon.svg"));
        assert!(links.contains("/apple-touch-icon.png"));
        assert!(!links.to_lowercase().contains("cyberpanel"));
    }

    #[test]
    fn brand_mark_is_window_and_nodes() {
        let svg = brand_mark_svg();
        assert!(svg.contains("cpn-brand-mark"));
        assert!(svg.contains("#006CFA"));
        assert!(svg.contains("#161F2B"));
        assert!(!svg.to_lowercase().contains("cyberpanel"));
        assert!(!svg.contains("lightning"));
        assert!(!svg.contains("cpnBrandGrad"));
    }
}
