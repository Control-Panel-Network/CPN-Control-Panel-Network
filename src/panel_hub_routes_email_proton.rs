//! Email → Proton Mail routes (external / Bridge plugin gate).

use crate::installer::AppState;
use crate::panel_hub_http::{html_blocking, login_redirect, redirect_notice, require_panel_user};
use crate::panel_hub_pages_proton_mail::{email_proton_page, save_proton_mail_form};
use crate::panel_pages::panel_shell;
use actix_web::{HttpRequest, HttpResponse, get, post, web};
use std::sync::Arc;

#[get("/email/proton")]
pub async fn email_proton_route(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    let notice = query.get("notice").cloned();
    let error = query.get("error").cloned();
    html_blocking(move || {
        if !crate::panel_feature_gate::proton_mail_unlocked() {
            return panel_shell(
                &user,
                "email",
                "Proton Mail",
                &crate::panel_feature_gate::email_auth_plugin_required_page(
                    "Proton Mail",
                    "protonMail",
                ),
            );
        }
        panel_shell(
            &user,
            "email",
            "Proton Mail",
            &email_proton_page(notice.as_deref(), error.as_deref()),
        )
    })
    .await
}

#[derive(Debug, serde::Deserialize)]
pub struct ProtonMailForm {
    #[serde(default)]
    account_email: String,
    #[serde(default)]
    display_name: String,
    #[serde(default)]
    open_button_enabled: Option<String>,
    #[serde(default)]
    open_button_label: String,
    #[serde(default)]
    open_url: String,
    #[serde(default)]
    bridge_host: String,
    #[serde(default)]
    bridge_imap_port: String,
    #[serde(default)]
    bridge_smtp_port: String,
    #[serde(default)]
    notes: String,
}

#[post("/email/proton/save")]
pub async fn email_proton_save(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<ProtonMailForm>,
) -> HttpResponse {
    let Some(_user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if !crate::panel_feature_gate::proton_mail_unlocked() {
        return redirect_notice(
            "/email/proton",
            None,
            Some("Install the free protonMail plugin from the Plugin Store first."),
        );
    }
    let enabled = form
        .open_button_enabled
        .as_deref()
        .map(|v| v == "1" || v.eq_ignore_ascii_case("on") || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    match save_proton_mail_form(
        &form.account_email,
        &form.display_name,
        enabled,
        &form.open_button_label,
        &form.open_url,
        &form.bridge_host,
        &form.bridge_imap_port,
        &form.bridge_smtp_port,
        &form.notes,
    ) {
        Ok(()) => redirect_notice("/email/proton", Some("Proton Mail settings saved."), None),
        Err(err) => redirect_notice("/email/proton", None, Some(&err)),
    }
}
