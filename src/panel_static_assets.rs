//! Cacheable panel chrome (CSS and JS) so HTML navigation does not resend it.
//!
//! Theme variables and color-mode boot stay inline. Private HTML stays `no-store`.

use actix_web::{HttpResponse, get};
use std::sync::OnceLock;

pub fn asset_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

fn strip_script_wrappers(raw: &str) -> String {
    raw.replace("<script>", "")
        .replace("</script>", "")
        .replace("<script defer>", "")
}

/// Shared panel CSS (not per-user). Built once per process.
pub fn shared_panel_stylesheet() -> &'static str {
    static CSS: OnceLock<String> = OnceLock::new();
    CSS.get_or_init(|| {
        format!(
            "{}{}{}{}{}{}{}{}{}{}{}{}{}{}{}{}",
            crate::panel_pages::panel_styles(),
            crate::panel_controls::control_styles(),
            crate::panel_nav_tree::nav_tree_styles(),
            crate::panel_sidebar::sidebar_extra_styles(),
            crate::panel_nav_chrome::sidebar_collapse_styles(),
            crate::panel_footer_chrome::sidebar_footer_styles(),
            crate::panel_feedback::feedback_styles(),
            crate::panel_hubs::hub_styles_with_icons(),
            crate::panel_dashboard_tools::dashboard_tools_styles(),
            crate::panel_dashboard_layout::dashboard_layout_styles(),
            crate::panel_dashboard_stats::dashboard_stats_styles(),
            crate::panel_dashboard_activity::activity_board_styles(),
            crate::panel_theme::color_mode_styles(),
            crate::panel_chrome_scheme::chrome_scheme_styles(),
            crate::panel_nav_tree::sidebar_nav_contrast_styles(),
            crate::panel_spa_nav::spa_nav_styles(),
        )
    })
    .as_str()
}

/// Shared panel behavior scripts (not per-user). Built once per process.
pub fn shared_panel_script() -> &'static str {
    static JS: OnceLock<String> = OnceLock::new();
    JS.get_or_init(|| {
        strip_script_wrappers(&format!(
            "{}{}{}{}{}{}{}{}",
            crate::panel_nav_chrome::panel_nav_script(),
            crate::panel_nav_tree::nav_tree_script(),
            crate::panel_sidebar::sidebar_search_and_ip_script(),
            crate::panel_theme_chrome::color_mode_toggle_script(),
            crate::panel_footer_chrome::notifications_popover_script(),
            crate::panel_chrome_scheme::chrome_scheme_popover_script(),
            crate::panel_feedback::feedback_script(),
            crate::panel_spa_nav::spa_nav_script(),
        ))
    })
    .as_str()
}

fn cached_body(content_type: &'static str, body: &'static str) -> HttpResponse {
    HttpResponse::Ok()
        .content_type(content_type)
        .insert_header(("Cache-Control", "public, max-age=3600"))
        .insert_header(("X-Content-Type-Options", "nosniff"))
        .body(body)
}

#[get("/cpn-panel.css")]
pub async fn panel_stylesheet() -> HttpResponse {
    cached_body("text/css; charset=utf-8", shared_panel_stylesheet())
}

#[get("/cpn-panel.js")]
pub async fn panel_script() -> HttpResponse {
    cached_body(
        "application/javascript; charset=utf-8",
        shared_panel_script(),
    )
}
