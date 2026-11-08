//! Path-to-action mapping, denylist for noisy read-like POSTs, labels, category inference.

use crate::panel_audit_types::{AuditCategory, AuditSeverity};

/// Explicit denylist: authenticated mutating methods that must NOT create audit rows.
pub fn is_noisy_read_like(path: &str) -> bool {
    let path = normalize_path(path);
    matches!(
        path,
        "/settings/markdown-preview"
            | "/api/panel/markdown-preview"
            | "/api/health"
            | "/api/healthz"
            | "/login/services-status"
            | "/api/login/services-status"
            | "/dashboard/activity/poll"
            | "/api/dashboard/metrics"
            | "/api/dashboard/activity"
            | "/server/system-repair/status"
            | "/api/system-repair/status"
            | "/api/panel/site-preview"
            | "/websites/list/search"
            | "/plugins/store/search"
            | "/docker/list/search"
            | "/account/users/search"
    ) || path.ends_with("/search")
        || path.ends_with("/preview")
        || path.contains("/poll")
        || (path.ends_with("/status") && path.starts_with("/api/"))
}

fn normalize_path(path: &str) -> &str {
    path.split('?')
        .next()
        .unwrap_or(path)
        .trim_end_matches('/')
}

pub fn action_for_path(path: &str) -> Option<&'static str> {
    let path = normalize_path(path);
    match path {
        "/apps/install" => Some("host.install"),
        "/apps/reinstall" => Some("host.reinstall"),
        "/apps/uninstall" => Some("host.uninstall"),
        "/apps/start" => Some("host.start"),
        "/apps/stop" => Some("host.stop"),
        "/apps/activate" => Some("host.attach"),
        "/apps/deactivate" => Some("host.detach"),
        "/plugins/install" => Some("plugin.install"),
        "/plugins/install-cpn" => Some("plugin.install-cpn"),
        "/plugins/install-host" => Some("plugin.install-host"),
        "/plugins/activate-host" => Some("plugin.activate-host"),
        "/plugins/deactivate-host" => Some("plugin.deactivate-host"),
        "/plugins/uninstall-host" => Some("plugin.uninstall-host"),
        "/plugins/uninstall-cpn" => Some("plugin.uninstall-cpn"),
        "/plugins/uninstall" => Some("plugin.uninstall"),
        "/plugins/enable" => Some("plugin.activate"),
        "/plugins/enable-cpn" => Some("plugin.activate-cpn"),
        "/plugins/disable" => Some("plugin.deactivate"),
        "/plugins/disable-cpn" => Some("plugin.deactivate-cpn"),
        "/plugins/settings" => Some("plugin.settings"),
        "/api/panel/themes/install" => Some("theme.install"),
        "/api/panel/themes/uninstall" => Some("theme.uninstall"),
        "/api/panel/themes/apply" => Some("theme.apply"),
        "/api/panel/themes/update-all" => Some("theme.update-all"),
        "/api/panel/themes/redeem" => Some("theme.redeem"),
        "/login" => Some("auth.login"),
        "/login/2fa" => Some("auth.mfa.verify"),
        "/logout" | "/api/logout" => Some("auth.logout"),
        "/forgot-password" => Some("auth.password.forgot"),
        "/reset-password" => Some("auth.password.reset"),
        "/api/account/setup" => Some("auth.account.setup"),
        "/account/api-access/create" => Some("token.issue"),
        "/account/api-access/revoke" => Some("token.revoke"),
        "/account/users/create" => Some("account.create"),
        "/account/users/delete" => Some("account.delete"),
        "/account/users/rename" => Some("account.rename"),
        "/account/users/deactivate" => Some("account.deactivate"),
        "/account/users/enable" => Some("account.enable"),
        "/account/users/modify" => Some("account.modify"),
        "/account/users/password" => Some("identity.password.change"),
        "/account/users/security/totp/enable" => Some("identity.totp.enroll"),
        "/account/users/security/totp/disable" => Some("identity.totp.disable"),
        "/account/users/security/passkey/register" => Some("identity.passkey.enroll"),
        "/account/users/security/passkey/delete" => Some("identity.passkey.delete"),
        "/account/users/security/passkey/rename" => Some("identity.passkey.rename"),
        "/account/users/reseller" => Some("account.reseller"),
        "/security/firewall/enable" => Some("firewall.enable"),
        "/security/firewall/start" => Some("firewall.start"),
        "/security/firewall/stop" => Some("firewall.stop"),
        "/security/firewall/reload" => Some("firewall.reload"),
        "/security/firewall/rules/add" => Some("firewall.rule.add"),
        "/security/firewall/rules/delete" => Some("firewall.rule.delete"),
        "/security/firewall/rules/import" => Some("firewall.rule.import"),
        "/security/firewall/banned/add" => Some("firewall.ban.add"),
        "/security/firewall/banned/unban" => Some("firewall.ban.unban"),
        "/security/firewall/banned/delete" => Some("firewall.ban.delete"),
        "/security/firewall/banned/import" => Some("firewall.ban.import"),
        "/security/firewall/trusted/add" => Some("firewall.trusted.add"),
        "/security/firewall/trusted/delete" => Some("firewall.trusted.delete"),
        "/security/fail2ban/unban" => Some("security.fail2ban.unban"),
        "/dns/zones/create" => Some("dns.zone.create"),
        "/dns/zones/delete" => Some("dns.zone.delete"),
        "/dns/records/add" => Some("dns.record.add"),
        "/dns/records/delete" => Some("dns.record.delete"),
        "/dns/records/edit" => Some("dns.record.edit"),
        "/dns/nameservers/save" => Some("dns.nameservers.save"),
        "/dns/cloudflare/connect" => Some("dns.cloudflare.connect"),
        "/dns/cloudflare/disconnect" => Some("dns.cloudflare.disconnect"),
        "/dns/cloudflare/token" => Some("dns.cloudflare.token"),
        "/settings/logs" => Some("settings.log_retention"),
        "/settings/audit" => Some("settings.audit"),
        "/settings/port" => Some("settings.port"),
        "/settings/site-messages" => Some("settings.site_messages"),
        "/settings/error-messages" => Some("settings.error_messages"),
        "/settings/design" => Some("settings.design"),
        "/server/system-repair/heal" => Some("repair.heal"),
        "/api/system-repair/heal" => Some("repair.heal"),
        "/settings/migrate" => Some("migrate.apply"),
        "/settings/version/upgrade" => Some("updates.upgrade"),
        "/api/panel/upgrade" => Some("updates.upgrade"),
        "/websites/create" => Some("website.create"),
        "/websites/delete" => Some("website.delete"),
        "/websites/suspend" => Some("website.suspend"),
        "/websites/resume" => Some("website.resume"),
        "/packages/create" => Some("hosting_package.create"),
        "/packages/delete" => Some("hosting_package.delete"),
        "/packages/edit" => Some("hosting_package.edit"),
        "/packages/assign" => Some("hosting_package.assign"),
        "/email/create" => Some("email.mailbox.create"),
        "/email/delete" => Some("email.mailbox.delete"),
        "/email/password" => Some("email.password"),
        "/email/dkim" => Some("email.dkim"),
        "/backups/run" => Some("backup.run"),
        "/backups/restore" => Some("backup.restore"),
        "/backups/schedule" => Some("backup.schedule"),
        "/docker/create" => Some("docker.create"),
        "/docker/start" => Some("docker.start"),
        "/docker/stop" => Some("docker.stop"),
        "/docker/remove" => Some("docker.remove"),
        "/server/files/upload" => Some("files.upload"),
        "/server/files/delete" => Some("files.delete"),
        "/server/files/mkdir" => Some("files.mkdir"),
        _ => None,
    }
}

pub fn category_for_action(action: &str) -> AuditCategory {
    let a = action.to_ascii_lowercase();
    if a.starts_with("auth.") { AuditCategory::Auth }
    else if a.starts_with("identity.") { AuditCategory::Identity }
    else if a.starts_with("account.") { AuditCategory::Accounts }
    else if a.starts_with("token.") { AuditCategory::ApiTokens }
    else if a.starts_with("plugin.") || a.starts_with("host.") { AuditCategory::Packages }
    else if a.starts_with("hosting_package.") { AuditCategory::HostingPackages }
    else if a.starts_with("website.") || a.starts_with("wordpress.") { AuditCategory::Websites }
    else if a.starts_with("db.") || a.starts_with("ftp.") || a.starts_with("sftp.") { AuditCategory::Database }
    else if a.starts_with("firewall.") || a.starts_with("security.") || a.starts_with("ssl.") { AuditCategory::Firewall }
    else if a.starts_with("dns.") { AuditCategory::Dns }
    else if a.starts_with("email.") { AuditCategory::Email }
    else if a.starts_with("backup.") { AuditCategory::Backup }
    else if a.starts_with("docker.") || a.starts_with("php.") { AuditCategory::Docker }
    else if a.starts_with("files.") { AuditCategory::Files }
    else if a.starts_with("settings.") { AuditCategory::Settings }
    else if a.starts_with("repair.") { AuditCategory::Repair }
    else if a.starts_with("migrate.") { AuditCategory::Migrate }
    else if a.starts_with("updates.") || a.starts_with("upgrade.") { AuditCategory::Updates }
    else if a.starts_with("cli.") { AuditCategory::Cli }
    else if a.starts_with("theme.") { AuditCategory::Themes }
    else if a.starts_with("panel.notifications") { AuditCategory::Notifications }
    else if a.starts_with("ui.") { AuditCategory::Ui }
    else { AuditCategory::Other }
}

pub fn category_for_path(path: &str) -> AuditCategory {
    if let Some(action) = action_for_path(path) {
        return category_for_action(action);
    }
    let path = normalize_path(path);
    if path.starts_with("/login") || path.starts_with("/logout") || path.starts_with("/forgot") {
        AuditCategory::Auth
    } else if path.starts_with("/account") {
        AuditCategory::Accounts
    } else if path.starts_with("/security") || path.starts_with("/firewall") {
        AuditCategory::Firewall
    } else if path.starts_with("/dns") {
        AuditCategory::Dns
    } else if path.starts_with("/email") {
        AuditCategory::Email
    } else if path.starts_with("/backups") {
        AuditCategory::Backup
    } else if path.starts_with("/docker") {
        AuditCategory::Docker
    } else if path.starts_with("/server/files") || path.contains("/files") {
        AuditCategory::Files
    } else if path.starts_with("/websites") || path.starts_with("/wordpress") {
        AuditCategory::Websites
    } else if path.starts_with("/packages") {
        AuditCategory::HostingPackages
    } else if path.starts_with("/plugins") || path.starts_with("/apps") {
        AuditCategory::Packages
    } else if path.starts_with("/settings") {
        AuditCategory::Settings
    } else {
        AuditCategory::Other
    }
}

pub fn default_severity(action: &str, ok: bool) -> AuditSeverity {
    if !ok {
        return match category_for_action(action) {
            AuditCategory::Auth | AuditCategory::Firewall | AuditCategory::Cli => AuditSeverity::Warning,
            _ => AuditSeverity::Notice,
        };
    }
    match action {
        "firewall.stop" | "auth.lockout" | "cli.uninstall" | "token.issue"
        | "identity.totp.disable" | "cli.mfa.clear" => AuditSeverity::Critical,
        "auth.login" | "auth.logout" | "auth.mfa.verify" => AuditSeverity::Notice,
        _ => AuditSeverity::Info,
    }
}

pub fn action_label(action: &str) -> &'static str {
    match action {
        "host.install" => "Host package: install",
        "host.reinstall" => "Host package: reinstall",
        "host.uninstall" => "Host package: uninstall",
        "host.start" => "Host package: start",
        "host.stop" => "Host package: stop",
        "host.attach" => "Host package: attach to site",
        "host.detach" => "Host package: detach from site",
        "plugin.install" => "Plugin: install",
        "plugin.install-cpn" => "Plugin: install for User Panel",
        "plugin.install-host" => "Plugin: install on host",
        "plugin.activate-host" => "Plugin: activate from host",
        "plugin.deactivate-host" => "Plugin: deactivate (host install)",
        "plugin.uninstall-host" => "Plugin: uninstall from host",
        "plugin.uninstall-cpn" => "Plugin: uninstall from User Panel",
        "plugin.uninstall" => "Plugin: uninstall",
        "plugin.activate" => "Plugin: activate",
        "plugin.activate-cpn" => "Plugin: activate (User Panel)",
        "plugin.deactivate" => "Plugin: deactivate",
        "plugin.deactivate-cpn" => "Plugin: deactivate (User Panel)",
        "plugin.settings" => "Plugin: settings",
        "theme.install" => "Theme: install",
        "theme.update" => "Theme: update",
        "theme.uninstall" => "Theme: uninstall",
        "theme.apply" => "Theme: apply",
        "theme.update-all" => "Theme: update all",
        "theme.redeem" => "Theme: redeem",
        "panel.notifications.remote" => "Panel: remote notification broadcast",
        "security.fail2ban.unban" => "Security: Fail2Ban unban",
        "auth.login" => "Auth: login",
        "auth.login.fail" => "Auth: login failed",
        "auth.mfa.verify" => "Auth: MFA verify",
        "auth.mfa.fail" => "Auth: MFA failed",
        "auth.logout" => "Auth: logout",
        "auth.lockout" => "Auth: lockout",
        "auth.password.forgot" => "Auth: forgot password",
        "auth.password.reset" => "Auth: reset password",
        "auth.account.setup" => "Auth: first account setup",
        "token.issue" => "API token: issue",
        "token.revoke" => "API token: revoke",
        "account.create" => "Account: create",
        "account.delete" => "Account: delete",
        "account.rename" => "Account: rename",
        "account.deactivate" => "Account: deactivate",
        "account.enable" => "Account: enable",
        "account.modify" => "Account: modify",
        "identity.password.change" => "Identity: password change",
        "identity.totp.enroll" => "Identity: TOTP enroll",
        "identity.totp.disable" => "Identity: TOTP disable",
        "identity.passkey.enroll" => "Identity: passkey enroll",
        "identity.passkey.delete" => "Identity: passkey delete",
        "identity.passkey.rename" => "Identity: passkey rename",
        "firewall.enable" => "Firewall: enable",
        "firewall.start" => "Firewall: start",
        "firewall.stop" => "Firewall: stop",
        "firewall.reload" => "Firewall: reload",
        "firewall.rule.add" => "Firewall: add rule",
        "firewall.rule.delete" => "Firewall: delete rule",
        "firewall.ban.add" => "Firewall: ban",
        "firewall.ban.unban" => "Firewall: unban",
        "settings.audit" => "Settings: audit policy",
        "settings.log_retention" => "Settings: log retention",
        "repair.heal" => "System repair: heal",
        "migrate.apply" => "Migrate: apply",
        "updates.upgrade" => "Updates: upgrade",
        "cli.mfa.clear" => "CLI: MFA clear",
        "cli.password" => "CLI: password reset",
        "cli.doctor.heal" => "CLI: doctor heal",
        "cli.uninstall" => "CLI: uninstall",
        "cli.account.create" => "CLI: account create",
        "cli.account.delete" => "CLI: account delete",
        "website.create" => "Website: create",
        "website.delete" => "Website: delete",
        "backup.run" => "Backup: run",
        "backup.restore" => "Backup: restore",
        _ => "Panel action",
    }
}

pub fn synthesize_action(method: &str, path: &str) -> String {
    let path = normalize_path(path);
    let method = method.to_ascii_lowercase();
    format!("http.{method}:{path}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_plugin_and_auth_paths() {
        assert_eq!(action_for_path("/apps/activate"), Some("host.attach"));
        assert_eq!(action_for_path("/login"), Some("auth.login"));
        assert_eq!(action_for_path("/security/firewall/stop"), Some("firewall.stop"));
        assert!(is_noisy_read_like("/settings/markdown-preview"));
        assert!(!is_noisy_read_like("/plugins/install-host"));
    }

    #[test]
    fn category_inference() {
        assert_eq!(category_for_action("auth.login"), AuditCategory::Auth);
        assert_eq!(category_for_action("token.issue"), AuditCategory::ApiTokens);
        assert_eq!(category_for_action("firewall.stop"), AuditCategory::Firewall);
    }
}
