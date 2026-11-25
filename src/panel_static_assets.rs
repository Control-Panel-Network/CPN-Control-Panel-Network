//! Cacheable panel chrome (CSS and JS) so HTML navigation does not resend it.
//!
//! Theme variables and color-mode boot stay inline. Private HTML stays `no-store`.

use actix_web::{HttpResponse, route};
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

// GET and HEAD: monitors, proxies, and `curl -I` probe with HEAD and must not see 404.
#[route("/cpn-panel.css", method = "GET", method = "HEAD")]
pub async fn panel_stylesheet() -> HttpResponse {
    cached_body("text/css; charset=utf-8", shared_panel_stylesheet())
}

#[route("/cpn-panel.js", method = "GET", method = "HEAD")]
pub async fn panel_script() -> HttpResponse {
    cached_body(
        "application/javascript; charset=utf-8",
        shared_panel_script(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::http::{Method, StatusCode, header::CACHE_CONTROL};
    use actix_web::{App, test as atest};

    fn assert_cached(resp: &actix_web::dev::ServiceResponse, content_type: &str) {
        assert_eq!(resp.status(), StatusCode::OK);
        let cache = resp
            .headers()
            .get(CACHE_CONTROL)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        assert_eq!(cache, "public, max-age=3600");
        let ct = resp
            .headers()
            .get(actix_web::http::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        assert!(ct.starts_with(content_type), "content-type was {ct}");
    }

    #[test]
    fn css_and_js_serve_for_get_and_head_with_cache_control() {
        actix_web::rt::System::new().block_on(async {
            let app =
                atest::init_service(App::new().service(panel_stylesheet).service(panel_script))
                    .await;
            for method in [Method::GET, Method::HEAD] {
                let css = atest::TestRequest::default()
                    .method(method.clone())
                    .uri("/cpn-panel.css?v=test")
                    .to_request();
                assert_cached(&atest::call_service(&app, css).await, "text/css");
                let js = atest::TestRequest::default()
                    .method(method)
                    .uri("/cpn-panel.js?v=test")
                    .to_request();
                assert_cached(
                    &atest::call_service(&app, js).await,
                    "application/javascript",
                );
            }
        });
    }
}
