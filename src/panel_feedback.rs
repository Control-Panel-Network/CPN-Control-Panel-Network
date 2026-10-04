//! Authenticated sidebar Feedback button and viewport-centered mail dialog.

use crate::account::now_unix;
use crate::account_mgmt::find_account;
use crate::auth_api::panel_user_from_request;
use crate::http_helpers::VERSION;
use crate::installer::AppState;
use crate::mail_outbound::{OutboundMessage, send_mail_with_fallback};
use crate::panel_session::session_secret;
use crate::panel_site_tools_security::same_origin_ok;
use actix_web::{HttpRequest, HttpResponse, post, web};
use hmac::{Hmac, KeyInit, Mac};
use serde::Deserialize;
use sha2::Sha256;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

type HmacSha256 = Hmac<Sha256>;

const RECIPIENTS: [&str; 2] = ["info@newstargeted.com", "info@discord-bot-network.com"];
const RATE_WINDOW_SECS: u64 = 3600;
const RATE_MAX: u32 = 5;
const MAX_SUBJECT_CHARS: usize = 120;
const MAX_MESSAGE_CHARS: usize = 10_000;
static RATE: Mutex<Option<HashMap<String, (u64, u32)>>> = Mutex::new(None);

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn hmac_hex(secret: &str, payload: &str) -> String {
    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key length");
    mac.update(payload.as_bytes());
    mac.finalize()
        .into_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn constant_time_eq(left: &str, right: &str) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.bytes()
        .zip(right.bytes())
        .fold(0u8, |diff, (a, b)| diff | (a ^ b))
        == 0
}

pub fn feedback_csrf_token(username: &str) -> String {
    let hour = now_unix() / 3600;
    let payload = format!("panel-feedback|{username}|{hour}");
    format!("{hour}.{}", hmac_hex(&session_secret(None), &payload))
}

fn verify_feedback_csrf(username: &str, token: &str) -> bool {
    let Some((hour_raw, signature)) = token.split_once('.') else {
        return false;
    };
    let Ok(hour) = hour_raw.parse::<u64>() else {
        return false;
    };
    let current = now_unix() / 3600;
    if hour + 2 < current || hour > current + 1 {
        return false;
    }
    let payload = format!("panel-feedback|{username}|{hour}");
    constant_time_eq(&hmac_hex(&session_secret(None), &payload), signature)
}

fn check_rate_limit(username: &str) -> Result<(), String> {
    let now = now_unix();
    let mut guard = RATE
        .lock()
        .map_err(|_| "Feedback rate limiter is temporarily unavailable".to_string())?;
    let map = guard.get_or_insert_with(HashMap::new);
    let entry = map.entry(username.to_ascii_lowercase()).or_insert((now, 0));
    if now.saturating_sub(entry.0) >= RATE_WINDOW_SECS {
        *entry = (now, 0);
    }
    if entry.1 >= RATE_MAX {
        return Err("Too many feedback submissions. Try again later.".into());
    }
    entry.1 += 1;
    Ok(())
}

fn json_response(status: actix_web::http::StatusCode, value: serde_json::Value) -> HttpResponse {
    HttpResponse::build(status)
        .content_type("application/json; charset=utf-8")
        .body(
            serde_json::to_string_pretty(&value).unwrap_or_else(|_| {
                "{\"ok\":false,\"error\":\"Could not encode response\"}".into()
            }),
        )
}

fn json_error(status: actix_web::http::StatusCode, message: &str) -> HttpResponse {
    json_response(status, serde_json::json!({ "ok": false, "error": message }))
}

fn safe_host(http: &HttpRequest) -> String {
    http.headers()
        .get("host")
        .and_then(|value| value.to_str().ok())
        .filter(|value| {
            !value.is_empty()
                && value.len() <= 253
                && !value
                    .chars()
                    .any(|ch| ch.is_control() || ch == '/' || ch == '\\')
        })
        .unwrap_or("unknown")
        .to_string()
}

fn feedback_body(
    username: &str,
    sender_email: &str,
    category: &str,
    subject: &str,
    message: &str,
    host: &str,
) -> String {
    format!(
        "CPN Panel feedback\r\n\r\n\
Category: {category}\r\n\
Subject: {subject}\r\n\
User: {username}\r\n\
User email: {sender_email}\r\n\
Panel host: {host}\r\n\
Panel version: {VERSION}\r\n\r\n\
Message:\r\n{message}\r\n"
    )
}

#[derive(Debug, Deserialize)]
pub struct FeedbackBody {
    #[serde(default)]
    subject: String,
    #[serde(default)]
    category: String,
    #[serde(default)]
    message: String,
    #[serde(default)]
    csrf: String,
}

#[post("/api/panel/feedback")]
pub async fn panel_feedback_submit(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    body: web::Json<FeedbackBody>,
) -> HttpResponse {
    let Some(username) = panel_user_from_request(&state, &http) else {
        return json_error(actix_web::http::StatusCode::UNAUTHORIZED, "Login required");
    };
    if !same_origin_ok(&http) || !verify_feedback_csrf(&username, body.csrf.trim()) {
        return json_error(
            actix_web::http::StatusCode::FORBIDDEN,
            "Invalid or expired request token. Refresh the page and try again.",
        );
    }

    let subject = body.subject.trim();
    let message = body.message.trim();
    if subject.is_empty() || message.is_empty() {
        return json_error(
            actix_web::http::StatusCode::BAD_REQUEST,
            "Subject and message are required.",
        );
    }
    if subject.chars().count() > MAX_SUBJECT_CHARS || message.chars().count() > MAX_MESSAGE_CHARS {
        return json_error(
            actix_web::http::StatusCode::BAD_REQUEST,
            "Subject or message is too long.",
        );
    }
    let category = match body.category.trim() {
        "Bug" => "Bug",
        "Feature request" => "Feature request",
        "Usability" => "Usability",
        _ => "General",
    };
    if let Err(message) = check_rate_limit(&username) {
        return json_error(actix_web::http::StatusCode::TOO_MANY_REQUESTS, &message);
    }

    let sender_email = find_account(&username)
        .map(|(account, _)| account.recovery_email)
        .unwrap_or_default();
    let sender_email = if sender_email.trim().is_empty() {
        "Not set"
    } else {
        sender_email.trim()
    };
    let mail_body = feedback_body(
        &username,
        sender_email,
        category,
        subject,
        message,
        &safe_host(&http),
    );

    for recipient in RECIPIENTS {
        let outbound = OutboundMessage {
            to: recipient.to_string(),
            subject: format!("[CPN Feedback] {subject}"),
            body: mail_body.clone(),
        };
        if let Err(error) = send_mail_with_fallback(&outbound) {
            eprintln!("panel_feedback: mail delivery failed for a configured recipient");
            return json_error(actix_web::http::StatusCode::BAD_GATEWAY, &error);
        }
    }

    json_response(
        actix_web::http::StatusCode::OK,
        serde_json::json!({ "ok": true, "message": "Feedback sent. Thank you." }),
    )
}

fn message_svg() -> &'static str {
    r#"<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M21 15a4 4 0 0 1-4 4H8l-5 3V7a4 4 0 0 1 4-4h10a4 4 0 0 1 4 4z"/><path d="M8 9h8M8 13h5"/></svg>"#
}

pub fn feedback_markup(username: &str) -> String {
    let email = find_account(username)
        .map(|(account, _)| account.recovery_email)
        .unwrap_or_default();
    let sender = if email.trim().is_empty() {
        username.to_string()
    } else {
        format!("{} ({})", username, email.trim())
    };
    format!(
        r#"<button type="button" id="cpn-feedback-btn" class="footer-icon-btn"
          aria-haspopup="dialog" aria-controls="cpn-feedback-modal"
          aria-label="Send feedback" title="Feedback">{icon}</button>
        <div id="cpn-feedback-modal" class="feedback-modal" hidden>
          <div class="feedback-dialog" role="dialog" aria-modal="true"
            aria-labelledby="cpn-feedback-title" data-csrf="{csrf}">
            <header><h2 id="cpn-feedback-title">Send feedback</h2>
              <button type="button" id="cpn-feedback-close" class="feedback-close"
                aria-label="Close feedback dialog">&times;</button></header>
            <form id="cpn-feedback-form">
              <p class="feedback-sender">Sending as {sender}</p>
              <label>Category
                <select name="category">
                  <option>General</option><option>Bug</option>
                  <option>Feature request</option><option>Usability</option>
                </select>
              </label>
              <label>Subject
                <input name="subject" type="text" maxlength="{subject_max}" required autocomplete="off">
              </label>
              <label>Message
                <textarea name="message" maxlength="{message_max}" rows="7" required></textarea>
              </label>
              <div id="cpn-feedback-status" class="feedback-status" role="status" aria-live="polite"></div>
              <div class="feedback-actions">
                <button type="button" class="feedback-cancel">Cancel</button>
                <button type="submit" class="feedback-submit">Send feedback</button>
              </div>
            </form>
          </div>
        </div>"#,
        icon = message_svg(),
        csrf = html_escape(&feedback_csrf_token(username)),
        sender = html_escape(&sender),
        subject_max = MAX_SUBJECT_CHARS,
        message_max = MAX_MESSAGE_CHARS,
    )
}

pub fn feedback_styles() -> &'static str {
    r#"
/* Viewport overlay (JS portals to body). Aside overflow/transform must not clip this. */
.feedback-modal[hidden] { display:none !important; }
body > .feedback-modal, .feedback-modal {
  position:fixed !important; inset:0 !important; left:0; top:0; right:0; bottom:0;
  width:100vw; width:100dvw; height:100vh; height:100dvh; margin:0;
  z-index:400; display:flex; align-items:center; justify-content:center;
  padding:24px; background:rgba(15,23,42,.58); box-sizing:border-box;
  transform:none !important; filter:none !important;
}
.feedback-dialog {
  position:relative; flex:0 1 auto; margin:0 auto;
  width:min(560px, calc(100vw - 48px)); max-height:min(720px, calc(100dvh - 48px));
  overflow:auto; padding:22px 22px 18px; border:1px solid var(--hairline); border-radius:14px;
  background:var(--canvas); color:var(--ink); box-shadow:0 24px 60px rgba(0,0,0,.28);
}
.feedback-dialog header { display:flex; align-items:center; justify-content:space-between; gap:12px; }
.feedback-dialog h2 { margin:0; font-size:22px; color:var(--ink); }
.feedback-close { border:0; background:transparent; color:var(--muted); font-size:28px; line-height:1; cursor:pointer; }
.feedback-close:hover { color:var(--ink); }
#cpn-feedback-form { display:grid; gap:14px; margin-top:14px; }
#cpn-feedback-form label { display:grid; gap:6px; font-size:14px; font-weight:600; color:var(--ink); }
#cpn-feedback-form input, #cpn-feedback-form select, #cpn-feedback-form textarea {
  width:100%; padding:10px 12px; border:1px solid var(--hairline); border-radius:9px;
  background:var(--canvas); color:var(--ink); font:inherit; color-scheme:light dark; }
#cpn-feedback-form textarea { min-height:130px; resize:vertical; }
.feedback-sender { margin:0; color:var(--muted); font-size:13px; overflow-wrap:anywhere; }
.feedback-status { min-height:22px; font-size:14px; }
.feedback-status.error { color:#b42318; }
.feedback-status.ok { color:var(--green); }
.feedback-actions { display:flex; justify-content:flex-end; gap:10px; }
.feedback-actions button { min-height:40px; padding:8px 14px; border-radius:9px; }
.feedback-cancel { border:1px solid var(--hairline); background:var(--canvas); color:var(--ink); }
.feedback-submit { border:1px solid var(--blue); background:var(--blue); color:#fff; font-weight:600; }
.feedback-submit:disabled { opacity:.65; cursor:wait; }
html[data-color-mode="dark"] .feedback-dialog,
[data-color-mode="dark"] .feedback-dialog { background:#161b22; color:#e5e7eb; }
[data-color-mode="dark"] .feedback-status.error { color:#fda4af; }
@media (max-width:520px) {
  .feedback-modal { padding:12px; align-items:center; justify-content:center; }
  .feedback-dialog { width:min(560px, calc(100vw - 24px)); max-height:calc(100dvh - 24px); padding:16px; }
  .feedback-actions { flex-direction:column-reverse; }
  .feedback-actions button { width:100%; }
}
"#
}

pub fn feedback_script() -> &'static str {
    r#"
<script>
(function () {
  var openBtn=document.getElementById("cpn-feedback-btn");
  var modal=document.getElementById("cpn-feedback-modal");
  var dialog=modal&&modal.querySelector(".feedback-dialog");
  var form=document.getElementById("cpn-feedback-form");
  var closeBtn=document.getElementById("cpn-feedback-close");
  var cancelBtn=modal&&modal.querySelector(".feedback-cancel");
  var status=document.getElementById("cpn-feedback-status");
  if(!openBtn||!modal||!dialog||!form||!closeBtn||!status)return;
  if(modal.parentNode!==document.body)document.body.appendChild(modal);
  var previousFocus=null;
  function focusable(){return Array.prototype.slice.call(dialog.querySelectorAll(
    'button:not([disabled]),input:not([disabled]),select:not([disabled]),textarea:not([disabled])')); }
  function setOpen(open){
    modal.hidden=!open;
    if(open){previousFocus=document.activeElement;document.body.setAttribute("data-feedback-open","true");
      var first=form.querySelector('select');if(first)first.focus();}
    else{document.body.removeAttribute("data-feedback-open");if(previousFocus)previousFocus.focus();}
  }
  openBtn.addEventListener("click",function(){status.textContent="";status.className="feedback-status";setOpen(true);});
  closeBtn.addEventListener("click",function(){setOpen(false);});
  if(cancelBtn)cancelBtn.addEventListener("click",function(){setOpen(false);});
  modal.addEventListener("click",function(event){if(event.target===modal)setOpen(false);});
  document.addEventListener("keydown",function(event){
    if(modal.hidden)return;
    if(event.key==="Escape"){event.preventDefault();setOpen(false);return;}
    if(event.key!=="Tab")return;
    var items=focusable();if(!items.length)return;
    var first=items[0],last=items[items.length-1];
    if(event.shiftKey&&document.activeElement===first){event.preventDefault();last.focus();}
    else if(!event.shiftKey&&document.activeElement===last){event.preventDefault();first.focus();}
  });
  form.addEventListener("submit",function(event){
    event.preventDefault();
    var submit=form.querySelector('[type="submit"]');
    var data=new FormData(form);
    submit.disabled=true;status.textContent="Sending...";status.className="feedback-status";
    fetch("/api/panel/feedback",{method:"POST",credentials:"same-origin",
      headers:{"Content-Type":"application/json","Accept":"application/json"},
      body:JSON.stringify({category:data.get("category"),subject:data.get("subject"),
        message:data.get("message"),csrf:dialog.getAttribute("data-csrf")||""})
    }).then(function(response){return response.json().catch(function(){return{};}).then(function(payload){
      if(!response.ok)throw new Error(payload.error||"Feedback could not be sent.");
      return payload;
    });}).then(function(payload){
      status.textContent=payload.message||"Feedback sent. Thank you.";status.className="feedback-status ok";
      form.reset();window.setTimeout(function(){setOpen(false);},1200);
    }).catch(function(error){
      status.textContent=error.message||"Feedback could not be sent.";status.className="feedback-status error";
    }).finally(function(){submit.disabled=false;});
  });
})();
</script>
"#
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;

    #[test]
    fn feedback_csrf_is_user_bound() {
        with_test_data_dir(|| {
            let token = feedback_csrf_token("Admin");
            assert!(verify_feedback_csrf("Admin", &token));
            assert!(!verify_feedback_csrf("Other", &token));
        });
    }

    #[test]
    fn feedback_body_includes_context_and_message() {
        let body = feedback_body(
            "operator",
            "operator@example.com",
            "Bug",
            "Broken button",
            "Details here",
            "panel.example",
        );
        assert!(body.contains("operator@example.com"));
        assert!(body.contains("panel.example"));
        assert!(body.contains(VERSION));
        assert!(body.contains("Details here"));
    }

    #[test]
    fn feedback_ui_is_modal_and_accessible() {
        with_test_data_dir(|| {
            let html = feedback_markup("Admin");
            assert!(html.contains("cpn-feedback-btn"));
            assert!(html.contains("role=\"dialog\""));
            assert!(html.contains("aria-modal=\"true\""));
            assert!(feedback_script().contains("event.key===\"Escape\""));
            assert!(feedback_script().contains("/api/panel/feedback"));
            assert!(feedback_script().contains("document.body.appendChild(modal)"));
            let css = feedback_styles();
            assert!(css.contains("z-index:400"));
            assert!(css.contains("100vw") || css.contains("100dvw"));
            assert!(css.contains("align-items:center"));
            assert!(css.contains("justify-content:center"));
            assert!(!css.contains("align-items:end"));
        });
    }
}
