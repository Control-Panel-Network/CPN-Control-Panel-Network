//! Email scaffold stubs (not yet fully wired backends).

use crate::installer::AppState;
use crate::panel_hub_http::{html_ok, login_redirect, require_panel_user};
use crate::panel_hub_pages_hosting::scaffold_feature;
use crate::panel_pages::panel_shell;
use actix_web::{HttpRequest, HttpResponse, get, web};
use std::sync::Arc;

macro_rules! email_scaffold {
    ($name:ident, $path:literal, $title:literal, $sub:literal, $detail:literal) => {
        #[get($path)]
        pub async fn $name(http: HttpRequest, state: web::Data<Arc<AppState>>) -> HttpResponse {
            let Some(user) = require_panel_user(&state, &http) else {
                return login_redirect();
            };
            html_ok(panel_shell(
                &user,
                "email",
                $title,
                &scaffold_feature("Email", "/email", $title, $sub, $detail),
            ))
        }
    };
}

email_scaffold!(
    email_pattern_fwd,
    "/email/pattern-forwarding",
    "Pattern Forwarding",
    "Rule-based forwarding",
    "Pattern rules are not wired yet."
);
email_scaffold!(
    email_limits,
    "/email/limits",
    "Email Limits",
    "Sending limits",
    "Per-mailbox send limits are not configured yet."
);
email_scaffold!(
    email_password,
    "/email/password",
    "Change Password",
    "Reset mailbox password",
    "Mailbox password reset UI is not wired yet."
);
email_scaffold!(
    email_debugger,
    "/email/debugger",
    "Email Debugger",
    "Diagnose mail issues",
    "Mail debugger is not configured yet."
);
email_scaffold!(
    email_queue,
    "/email/queue",
    "Mail Queue",
    "Inspect the queue",
    "Mail queue inspection is not configured yet."
);
email_scaffold!(
    email_spamassassin,
    "/email/spamassassin",
    "SpamAssassin",
    "Spam filtering",
    "SpamAssassin is not installed or not configured."
);
email_scaffold!(
    email_rspamd,
    "/email/rspamd",
    "Rspamd",
    "Spam filtering",
    "Rspamd is not installed or not configured."
);
email_scaffold!(
    email_mailscanner,
    "/email/mailscanner",
    "MailScanner",
    "Mail scanning",
    "MailScanner is not installed or not configured."
);
email_scaffold!(
    email_marketing,
    "/email/marketing",
    "Email Marketing",
    "Campaigns and lists",
    "Email marketing is not configured yet."
);
email_scaffold!(
    email_plus,
    "/email/plus-addressing",
    "Plus-Addressing",
    "user+tag addressing",
    "Plus-addressing controls are not configured yet."
);
