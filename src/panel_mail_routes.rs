//! Email mailbox create/enable/disable panel routes.

use crate::account::{default_password_policy, password_meets_policy};
use crate::auth_api::panel_user_from_request;
use crate::backups::is_subdomain_site;
use crate::installer::AppState;
use crate::login_next::login_redirect;
use crate::mail_accounts::{MailAccountInput, MailSmtpMode, create_account, set_account_enabled};
use crate::mail_accounts_lifecycle::{modify_mailbox, restore_mailbox, soft_delete_mailbox};
use crate::mail_postfix_maps::can_manage_mail_domain;
use crate::packages::{QuotaResource, require_quota};
use crate::sites::{load_site, normalize_domain};
use crate::smtp_settings::SmtpTlsMode;
use actix_web::{HttpRequest, HttpResponse, post, web};
use std::sync::Arc;

fn urlencoding_simple(value: &str) -> String {
    let mut out = String::with_capacity(value.len() * 3);
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

fn safe_email_return_path(raw: &str) -> String {
    let path = raw.trim();
    if path.starts_with("/email/") && !path.contains("..") && !path.contains('\n') {
        path.to_string()
    } else {
        "/email/accounts".into()
    }
}

fn email_redirect(return_to: Option<&str>, notice: Option<&str>, error: Option<&str>) -> String {
    let mut url = return_to
        .map(safe_email_return_path)
        .unwrap_or_else(|| "/email/accounts".into());
    if let Some(notice) = notice {
        url.push_str(&format!("?notice={}", urlencoding_simple(notice)));
    } else if let Some(error) = error {
        url.push_str(&format!("?error={}", urlencoding_simple(error)));
    }
    url
}

#[derive(Debug, serde::Deserialize)]
pub struct MailAccountCreateForm {
    #[serde(default)]
    address: String,
    #[serde(default)]
    local_part: String,
    #[serde(default)]
    domain: String,
    #[serde(default)]
    domain_website: String,
    #[serde(default)]
    domain_subdomain: String,
    #[serde(default)]
    domain_scope: String,
    #[serde(default)]
    enabled: String,
    #[serde(default)]
    smtp_mode: String,
    #[serde(default)]
    smtp_host: String,
    #[serde(default)]
    smtp_port: String,
    #[serde(default)]
    smtp_tls: String,
    #[serde(default)]
    smtp_username: String,
    #[serde(default)]
    smtp_password: String,
    #[serde(default)]
    mailbox_password: String,
    #[serde(default)]
    mailbox_password_confirm: String,
    #[serde(default)]
    return_to: String,
}

impl MailAccountCreateForm {
    fn selected_domain(&self) -> Result<String, String> {
        let scope = self.domain_scope.trim().to_ascii_lowercase();
        let from_scope = match scope.as_str() {
            "subdomain" | "sub" => self.domain_subdomain.trim(),
            "website" | "apex" | "main" => self.domain_website.trim(),
            "" => {
                if !self.domain.trim().is_empty() {
                    self.domain.trim()
                } else if !self.domain_website.trim().is_empty() {
                    self.domain_website.trim()
                } else {
                    self.domain_subdomain.trim()
                }
            }
            other => {
                return Err(format!(
                    "Unknown domain scope `{other}`. Use website or subdomain."
                ));
            }
        };
        let raw = if !from_scope.is_empty() {
            from_scope
        } else {
            self.domain.trim()
        };
        if raw.is_empty() {
            return Err("Select a website or sub-domain for this mailbox".into());
        }
        let domain = normalize_domain(raw)?;
        let _ = load_site(&domain).map_err(|_| {
            format!("`{domain}` is not a hosted website or sub-domain on this panel")
        })?;
        match scope.as_str() {
            "subdomain" | "sub" if !is_subdomain_site(&domain) => {
                return Err(format!("`{domain}` is a main website, not a sub-domain"));
            }
            "website" | "apex" | "main" if is_subdomain_site(&domain) => {
                return Err(format!(
                    "`{domain}` is a sub-domain; choose Website scope or pick a main domain"
                ));
            }
            _ => {}
        }
        Ok(domain)
    }

    fn resolved_address(&self) -> Result<String, String> {
        let domain = self.selected_domain()?;
        let local = self.local_part.trim().to_ascii_lowercase();
        if !local.is_empty() {
            if local.contains('@') || local.contains(' ') {
                return Err("Email username must not include @ or spaces".into());
            }
            if !local
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '+' | '-'))
            {
                return Err("Email username may only use letters, numbers, and . _ + -".into());
            }
            return Ok(format!("{local}@{domain}"));
        }
        let address = self.address.trim().to_ascii_lowercase();
        if address.is_empty() {
            return Err("Enter an email username (before @)".into());
        }
        let Some((local_part, addr_domain)) = address.split_once('@') else {
            return Err("Mailbox address must be a valid email".into());
        };
        if local_part.is_empty() {
            return Err("Mailbox address must be a valid email".into());
        }
        if addr_domain != domain {
            return Err(format!(
                "Mailbox domain must match the selected site (`{domain}`)"
            ));
        }
        Ok(address)
    }

    fn to_input(&self) -> Result<MailAccountInput, String> {
        if self.mailbox_password_confirm.is_empty() {
            return Err("Confirm the mailbox password".into());
        }
        if self.mailbox_password != self.mailbox_password_confirm {
            return Err("Password and confirmation do not match".into());
        }
        let policy = default_password_policy();
        password_meets_policy(self.mailbox_password.trim(), &policy)?;
        let domain = self.selected_domain()?;
        let address = self.resolved_address()?;

        // Create UI always provisions local Postfix/Dovecot. External SMTP fields remain
        // accepted for API compatibility but are ignored unless smtp_mode is explicitly external.
        let smtp_mode = match self.smtp_mode.trim().to_ascii_lowercase().as_str() {
            "external" | "smtp" => MailSmtpMode::External,
            "postfix" | "postfix_local" | "local" | "" => MailSmtpMode::PostfixLocal,
            other => {
                return Err(format!(
                    "Unknown SMTP mode `{other}`. Use external or postfix_local."
                ));
            }
        };
        let enabled = match self.enabled.trim().to_ascii_lowercase().as_str() {
            "" => true,
            value => matches!(value, "1" | "true" | "yes" | "on"),
        };
        let (smtp_host, smtp_port, smtp_tls, smtp_username, smtp_password) =
            if smtp_mode == MailSmtpMode::External {
                let smtp_port = if self.smtp_port.trim().is_empty() {
                    None
                } else {
                    Some(
                        self.smtp_port
                            .trim()
                            .parse::<u16>()
                            .map_err(|_| "SMTP port must be a number".to_string())?,
                    )
                };
                let smtp_tls = match self.smtp_tls.trim().to_ascii_lowercase().as_str() {
                    "" | "starttls" => Some(SmtpTlsMode::Starttls),
                    "tls" => Some(SmtpTlsMode::Tls),
                    "none" => Some(SmtpTlsMode::None),
                    other => return Err(format!("Unknown TLS mode `{other}`")),
                };
                (
                    self.smtp_host.clone(),
                    smtp_port,
                    smtp_tls,
                    self.smtp_username.clone(),
                    self.smtp_password.clone(),
                )
            } else {
                (
                    String::new(),
                    Some(587),
                    Some(SmtpTlsMode::Starttls),
                    String::new(),
                    String::new(),
                )
            };
        Ok(MailAccountInput {
            address,
            domain,
            enabled,
            smtp_mode,
            smtp_host,
            smtp_port,
            smtp_tls,
            smtp_username,
            smtp_password,
            mailbox_password: self.mailbox_password.clone(),
        })
    }
}

#[derive(Debug, serde::Deserialize)]
pub struct MailAccountIdForm {
    #[serde(default)]
    id: String,
    #[serde(default)]
    return_to: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct MailAccountModifyForm {
    #[serde(default)]
    id: String,
    #[serde(default)]
    local_part: String,
    #[serde(default)]
    mailbox_password: String,
    #[serde(default)]
    mailbox_password_confirm: String,
    #[serde(default)]
    return_to: String,
}

#[post("/email/accounts/create")]
pub async fn email_account_create(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<MailAccountCreateForm>,
) -> HttpResponse {
    let Some(user) = panel_user_from_request(&state, &http) else {
        return login_redirect(&http);
    };
    let domain_for_acl = form.selected_domain().ok();
    if let Some(ref domain) = domain_for_acl
        && !can_manage_mail_domain(&user, domain)
    {
        return HttpResponse::SeeOther()
            .append_header((
                "Location",
                email_redirect(
                    Some(&form.return_to),
                    None,
                    Some("You cannot create mailboxes for that domain"),
                ),
            ))
            .finish();
    }
    let owner = if crate::packages::is_panel_admin(&user) {
        domain_for_acl
            .as_ref()
            .and_then(|d| load_site(d).ok())
            .map(|s| s.owner)
            .unwrap_or_else(|| user.clone())
    } else {
        user.clone()
    };
    match require_quota(&owner, QuotaResource::Emails)
        .and_then(|_| form.to_input())
        .and_then(create_account)
    {
        Ok(account) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                email_redirect(
                    Some(&form.return_to),
                    Some(&format!("Created mailbox {}", account.address)),
                    None,
                ),
            ))
            .finish(),
        Err(error) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                email_redirect(Some(&form.return_to), None, Some(&error)),
            ))
            .finish(),
    }
}

#[post("/email/accounts/enable")]
pub async fn email_account_enable(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<MailAccountIdForm>,
) -> HttpResponse {
    if panel_user_from_request(&state, &http).is_none() {
        return login_redirect(&http);
    }
    let ret = Some(form.return_to.as_str()).filter(|s| !s.is_empty());
    match set_account_enabled(&form.id, true) {
        Ok(account) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                email_redirect(ret, Some(&format!("Enabled {}", account.address)), None),
            ))
            .finish(),
        Err(error) => HttpResponse::SeeOther()
            .append_header(("Location", email_redirect(ret, None, Some(&error))))
            .finish(),
    }
}

#[post("/email/accounts/disable")]
pub async fn email_account_disable(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<MailAccountIdForm>,
) -> HttpResponse {
    if panel_user_from_request(&state, &http).is_none() {
        return login_redirect(&http);
    }
    let ret = Some(form.return_to.as_str()).filter(|s| !s.is_empty());
    match set_account_enabled(&form.id, false) {
        Ok(account) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                email_redirect(ret, Some(&format!("Disabled {}", account.address)), None),
            ))
            .finish(),
        Err(error) => HttpResponse::SeeOther()
            .append_header(("Location", email_redirect(ret, None, Some(&error))))
            .finish(),
    }
}

#[post("/email/accounts/delete")]
pub async fn email_account_delete(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<MailAccountIdForm>,
) -> HttpResponse {
    let Some(user) = panel_user_from_request(&state, &http) else {
        return login_redirect(&http);
    };
    let ret = Some(form.return_to.as_str()).filter(|s| !s.is_empty());
    match soft_delete_mailbox(&form.id, &user) {
        Ok(account) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                email_redirect(
                    ret,
                    Some(&format!(
                        "Deleted {}. Panel owner can restore for 14 days.",
                        account.address
                    )),
                    None,
                ),
            ))
            .finish(),
        Err(error) => HttpResponse::SeeOther()
            .append_header(("Location", email_redirect(ret, None, Some(&error))))
            .finish(),
    }
}

#[post("/email/accounts/restore")]
pub async fn email_account_restore(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<MailAccountIdForm>,
) -> HttpResponse {
    let Some(user) = panel_user_from_request(&state, &http) else {
        return login_redirect(&http);
    };
    let ret = Some(form.return_to.as_str()).filter(|s| !s.is_empty());
    match restore_mailbox(&form.id, &user) {
        Ok(account) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                email_redirect(ret, Some(&format!("Restored {}", account.address)), None),
            ))
            .finish(),
        Err(error) => HttpResponse::SeeOther()
            .append_header(("Location", email_redirect(ret, None, Some(&error))))
            .finish(),
    }
}

#[post("/email/accounts/modify")]
pub async fn email_account_modify(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<MailAccountModifyForm>,
) -> HttpResponse {
    let Some(user) = panel_user_from_request(&state, &http) else {
        return login_redirect(&http);
    };
    let ret = Some(form.return_to.as_str()).filter(|s| !s.is_empty());
    if !form.mailbox_password.is_empty() && form.mailbox_password != form.mailbox_password_confirm {
        return HttpResponse::SeeOther()
            .append_header((
                "Location",
                email_redirect(ret, None, Some("Password and confirmation do not match")),
            ))
            .finish();
    }
    let local = form.local_part.trim();
    let password = form.mailbox_password.trim();
    match modify_mailbox(
        &form.id,
        if local.is_empty() { None } else { Some(local) },
        if password.is_empty() {
            None
        } else {
            Some(password)
        },
        &user,
    ) {
        Ok(msg) => HttpResponse::SeeOther()
            .append_header(("Location", email_redirect(ret, Some(&msg), None)))
            .finish(),
        Err(error) => HttpResponse::SeeOther()
            .append_header(("Location", email_redirect(ret, None, Some(&error))))
            .finish(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_form_defaults_to_local_postfix() {
        let form = MailAccountCreateForm {
            address: String::new(),
            local_part: "info".into(),
            domain: "example.com".into(),
            domain_website: "example.com".into(),
            domain_subdomain: String::new(),
            domain_scope: "website".into(),
            enabled: "1".into(),
            smtp_mode: "postfix_local".into(),
            smtp_host: "should-be-ignored.example".into(),
            smtp_port: "2525".into(),
            smtp_tls: "none".into(),
            smtp_username: "u".into(),
            smtp_password: "p".into(),
            mailbox_password: "Secret12".into(),
            mailbox_password_confirm: "Secret12".into(),
            return_to: "/email/create".into(),
        };
        // selected_domain needs a real site file; only test password mismatch here.
        let mismatch = MailAccountCreateForm {
            mailbox_password_confirm: "other".into(),
            ..form
        };
        assert!(mismatch.to_input().unwrap_err().contains("do not match"));
    }
}
