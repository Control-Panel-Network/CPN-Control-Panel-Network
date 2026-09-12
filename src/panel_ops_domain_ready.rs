//! After website create: DKIM store, mail DNS (SPF/DKIM/DMARC), optional auto SSL.

use crate::panel_ops_certbot_install::ensure_certbot_on_path;
use crate::panel_ops_cloudflare::cloudflare_configured;
use crate::panel_ops_dkim_keys::ensure_dkim_for_domain;
use crate::panel_ops_mail_dns::provision_mail_dns;
use crate::panel_ops_mail_onboarding::{MailMode, load_mail_onboarding};
use crate::panel_ops_ssl_issue::issue_or_renew;
use crate::panel_ops_ssl_provider::{SslCoverageMode, SslProvider};
use crate::sites::{SiteModify, load_site, modify_site, normalize_domain};

#[derive(Debug, Clone, Default)]
pub struct DomainReadyReport {
    pub domain: String,
    pub steps: Vec<String>,
    pub warnings: Vec<String>,
}

impl DomainReadyReport {
    pub fn summary(&self) -> String {
        let mut parts = self.steps.clone();
        parts.extend(self.warnings.iter().map(|w| format!("warn: {w}")));
        if parts.is_empty() {
            format!("Domain `{}` ready (no extra steps).", self.domain)
        } else {
            format!("Domain `{}`: {}", self.domain, parts.join(" | "))
        }
    }
}

/// Best-effort provisioning after a site is created. Never rolls back the site registry.
pub fn after_site_created(domain_raw: &str) -> DomainReadyReport {
    let mut report = DomainReadyReport::default();
    let domain = match normalize_domain(domain_raw) {
        Ok(d) => d,
        Err(e) => {
            report.warnings.push(e);
            return report;
        }
    };
    report.domain = domain.clone();

    match ensure_dkim_for_domain(&domain) {
        Ok(msg) => report.steps.push(msg),
        Err(e) => report.warnings.push(format!("DKIM: {e}")),
    }

    let onboarding = load_mail_onboarding();
    if onboarding.mail_mode == MailMode::Local && !onboarding.skip_rdns {
        match provision_mail_dns(&domain) {
            Ok(msg) => report.steps.push(msg),
            Err(e) => report.warnings.push(format!("Mail DNS: {e}")),
        }
    } else if onboarding.mail_mode == MailMode::External {
        report.steps.push(
            "External mail mode: skipped local SPF/DKIM/DMARC auto-DNS (configure at your provider)."
                .into(),
        );
    } else if onboarding.skip_rdns {
        report
            .steps
            .push("Skip email/rDNS onboarding: skipped mail DNS auto-provision.".into());
    }

    let site = match load_site(&domain) {
        Ok(s) => s,
        Err(e) => {
            report.warnings.push(format!("SSL skipped: {e}"));
            return report;
        }
    };
    if site.ssl.provider.supports_auto_issue() {
        if matches!(site.ssl.coverage_mode, SslCoverageMode::Wildcard) && !cloudflare_configured()
        {
            let mut ssl = site.ssl.clone();
            ssl.coverage_mode = SslCoverageMode::San;
            if modify_site(
                &domain,
                SiteModify {
                    ssl: Some(ssl),
                    ..SiteModify::default()
                },
            )
            .is_ok()
            {
                report.steps.push(
                    "Cloudflare DNS not configured: using SAN coverage for HTTP-01 auto SSL."
                        .into(),
                );
            }
        }
        match ensure_certbot_on_path() {
            Ok(msg) => report.steps.push(msg),
            Err(e) => report.warnings.push(format!("certbot: {e}")),
        }
        match issue_or_renew(&domain) {
            Ok(msg) => report.steps.push(msg),
            Err(e) => report.warnings.push(format!(
                "Auto SSL ({}) deferred: {e}",
                site.ssl.provider.label()
            )),
        }
    } else if matches!(site.ssl.provider, SslProvider::None | SslProvider::Custom) {
        report.steps.push(format!(
            "SSL provider is {}; skipped auto-issue.",
            site.ssl.provider.label()
        ));
    }

    report
}
