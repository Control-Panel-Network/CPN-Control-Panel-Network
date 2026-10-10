//! `GET /api/version-branches`: branch options for the Version Management
//! Branch picker (configured repo). Read access for any signed-in panel user;
//! saving the branch goes through `POST /api/version-source` (admin only).

use crate::installer::AppState;
use crate::maintenance_api::version_read_authorized;
use crate::model::TokenQuery;
use crate::releases;
use crate::releases_branches::branch_listing;
use crate::releases_source::load_update_source;
use actix_web::{HttpRequest, HttpResponse, get, web};
use std::sync::Arc;

#[get("/api/version-branches")]
pub async fn api_version_branches(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<TokenQuery>,
) -> HttpResponse {
    if !version_read_authorized(&state, &query, &http) {
        return HttpResponse::Unauthorized().finish();
    }
    let cfg = load_update_source();
    let repo = releases::github_repo();
    let listing = branch_listing(&repo, &cfg.branch).await;
    HttpResponse::Ok().json(listing)
}

#[cfg(test)]
mod tests {
    #[test]
    fn route_path_is_hyphenated() {
        // Panel routes avoid spaces and encoded characters.
        let path = "/api/version-branches";
        assert!(!path.contains(' '));
        assert!(!path.contains("%20"));
        assert!(!path.contains('\u{2014}'));
    }
}
