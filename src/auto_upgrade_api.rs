//! HTTP API for Owner automatic-update settings and dry-run checks.

use crate::auth_api::panel_user_from_request;
use crate::auto_upgrade::{
    AutoUpgradeMode, load_auto_upgrade_config, public_status, save_auto_upgrade_config,
};
use crate::auto_upgrade_run::run_auto_upgrade_check;
use crate::http_helpers::authorized_request;
use crate::installer::AppState;
use crate::model::TokenQuery;
use crate::panel_admin::is_panel_admin;
use actix_web::{HttpRequest, HttpResponse, get, post, web};
use serde::Deserialize;
use std::sync::Arc;

fn maintenance_authorized(state: &AppState, query: &TokenQuery, http: &HttpRequest) -> bool {
    if authorized_request(state, query, http) {
        return true;
    }
    matches!(
        panel_user_from_request(state, http),
        Some(user) if is_panel_admin(&user)
    )
}

fn version_read_authorized(state: &AppState, query: &TokenQuery, http: &HttpRequest) -> bool {
    if authorized_request(state, query, http) {
        return true;
    }
    panel_user_from_request(state, http).is_some()
}

#[derive(Debug, Deserialize)]
pub struct AutoUpgradeSaveRequest {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub mode: Option<String>,
    #[serde(default)]
    pub include_prerelease: bool,
    #[serde(default)]
    pub check_interval_secs: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct AutoUpgradeCheckRequest {
    /// When true, apply an upgrade if the Owner toggle is on and a newer release exists.
    /// Default false: dry-run / status only (never starts an upgrade while off).
    #[serde(default)]
    pub apply: bool,
}

fn parse_mode(raw: Option<&str>) -> Result<AutoUpgradeMode, String> {
    match raw.map(|s| s.trim().to_ascii_lowercase()).as_deref() {
        None | Some("") | Some("all") | Some("newer") | Some("all_releases") => {
            Ok(AutoUpgradeMode::All)
        }
        Some("security") | Some("security_only") | Some("sec") => Ok(AutoUpgradeMode::Security),
        Some(other) => Err(format!("Unknown mode '{other}'. Use 'all' or 'security'.")),
    }
}

#[get("/api/auto-upgrade")]
pub async fn api_auto_upgrade_get(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<TokenQuery>,
) -> HttpResponse {
    if !version_read_authorized(&state, &query, &http) {
        return HttpResponse::Unauthorized().finish();
    }
    let cfg = load_auto_upgrade_config();
    HttpResponse::Ok().json(public_status(&cfg))
}

#[post("/api/auto-upgrade")]
pub async fn api_auto_upgrade_post(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<TokenQuery>,
    body: web::Json<AutoUpgradeSaveRequest>,
) -> HttpResponse {
    if !maintenance_authorized(&state, &query, &http) {
        return HttpResponse::Unauthorized().finish();
    }
    let req = body.into_inner();
    let mode = match parse_mode(req.mode.as_deref()) {
        Ok(m) => m,
        Err(error) => {
            return HttpResponse::BadRequest().json(serde_json::json!({ "error": error }));
        }
    };
    let mut cfg = load_auto_upgrade_config();
    cfg.enabled = req.enabled;
    cfg.mode = mode;
    cfg.include_prerelease = req.include_prerelease;
    if let Some(secs) = req.check_interval_secs {
        cfg.check_interval_secs = secs;
    }
    if let Err(error) = save_auto_upgrade_config(&cfg) {
        return HttpResponse::BadRequest().json(serde_json::json!({ "error": error }));
    }
    crate::upgrade_tip_log::log_tagged_main_only(
        "auto_upgrade",
        "info",
        &format!(
            "Owner saved automatic updates: enabled={}, mode={:?}, prerelease={}",
            cfg.enabled, cfg.mode, cfg.include_prerelease
        ),
    );
    HttpResponse::Ok().json(public_status(&cfg))
}

/// Dry-run (default) or apply when armed. Toggle OFF never starts an upgrade.
#[post("/api/auto-upgrade/check")]
pub async fn api_auto_upgrade_check(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<TokenQuery>,
    body: web::Json<AutoUpgradeCheckRequest>,
) -> HttpResponse {
    if !maintenance_authorized(&state, &query, &http) {
        return HttpResponse::Unauthorized().finish();
    }
    let apply = body.apply;
    match run_auto_upgrade_check(state.get_ref().clone(), apply).await {
        Ok(report) => HttpResponse::Ok().json(report),
        Err(error) => HttpResponse::BadGateway().json(serde_json::json!({ "error": error })),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auto_upgrade::AutoUpgradeConfig;

    #[test]
    fn parses_modes() {
        assert_eq!(parse_mode(None).unwrap(), AutoUpgradeMode::All);
        assert_eq!(
            parse_mode(Some("security")).unwrap(),
            AutoUpgradeMode::Security
        );
        assert!(parse_mode(Some("weird")).is_err());
    }

    #[test]
    fn sample_default_on_all() {
        let cfg = AutoUpgradeConfig::default();
        assert!(cfg.enabled);
        assert_eq!(cfg.mode, AutoUpgradeMode::All);
    }
}
