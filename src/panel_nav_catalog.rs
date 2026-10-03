//! Static sidebar catalog: flatter sections and child routes for CPN Panel.
//!
//! Information architecture favors section-level icon rows (Main / Server /
//! Security / Settings) over deep nest-only hubs. Routes stay unchanged.

#[derive(Clone, Copy)]
pub(crate) struct NavChild {
    pub label: &'static str,
    pub href: &'static str,
}

#[derive(Clone, Copy)]
pub(crate) enum NavEntry {
    Link {
        id: &'static str,
        href: &'static str,
        label: &'static str,
    },
    Group {
        id: &'static str,
        href: &'static str,
        label: &'static str,
        children: &'static [NavChild],
    },
}

const WEBSITES_CHILDREN: &[NavChild] = &[
    NavChild {
        label: "List Websites",
        href: "/websites/list",
    },
    NavChild {
        label: "Create Website",
        href: "/websites/create",
    },
    NavChild {
        label: "List Sub-domains",
        href: "/subdomains",
    },
    NavChild {
        label: "Create Sub-domain",
        href: "/subdomains/create",
    },
];

const WORDPRESS_CHILDREN: &[NavChild] = &[
    NavChild {
        label: "WordPress Sites",
        href: "/wordpress/list",
    },
    NavChild {
        label: "Install WordPress",
        href: "/wordpress/install",
    },
    NavChild {
        label: "WordPress Sub-sites",
        href: "/wordpress/subsites",
    },
    NavChild {
        label: "Install WordPress Sub-site",
        href: "/wordpress/subsites/install",
    },
];

const EMAIL_CHILDREN: &[NavChild] = &[
    NavChild {
        label: "Email Accounts",
        href: "/email/accounts",
    },
    NavChild {
        label: "Create Email",
        href: "/email/create",
    },
    NavChild {
        label: "Forwarding",
        href: "/email/forwarding",
    },
    NavChild {
        label: "Catch-All",
        href: "/email/catchall",
    },
    NavChild {
        label: "Pattern Forwarding",
        href: "/email/pattern-forwarding",
    },
    NavChild {
        label: "Email Limits",
        href: "/email/limits",
    },
    NavChild {
        label: "Change Password",
        href: "/email/password",
    },
    NavChild {
        label: "DKIM Manager",
        href: "/email/dkim",
    },
    NavChild {
        label: "Webmail",
        href: "/email/webmail",
    },
    NavChild {
        label: "MTA-STS",
        href: "/email/mta-sts",
    },
    NavChild {
        label: "BIMI",
        href: "/email/bimi",
    },
    NavChild {
        label: "Email Delivery",
        href: "/email/delivery",
    },
    NavChild {
        label: "Email Debugger",
        href: "/email/debugger",
    },
    NavChild {
        label: "Mail Queue",
        href: "/email/queue",
    },
    NavChild {
        label: "SpamAssassin",
        href: "/email/spamassassin",
    },
    NavChild {
        label: "Rspamd",
        href: "/email/rspamd",
    },
    NavChild {
        label: "MailScanner",
        href: "/email/mailscanner",
    },
    NavChild {
        label: "Email Marketing",
        href: "/email/marketing",
    },
    NavChild {
        label: "Plus-Addressing",
        href: "/email/plus-addressing",
    },
];

const DATABASES_CHILDREN: &[NavChild] = &[
    NavChild {
        label: "All Databases",
        href: "/databases/all",
    },
    NavChild {
        label: "Create Database",
        href: "/databases/create",
    },
    NavChild {
        label: "Change password",
        href: "/databases/password",
    },
    NavChild {
        label: "Delete Database",
        href: "/databases/delete",
    },
    NavChild {
        label: "phpMyAdmin",
        href: "/databases/phpmyadmin",
    },
];

const FTP_CHILDREN: &[NavChild] = &[
    NavChild {
        label: "FTP Accounts",
        href: "/ftp/accounts",
    },
    NavChild {
        label: "Create SFTP Account",
        href: "/ftp/create",
    },
    NavChild {
        label: "Delete SFTP Account",
        href: "/ftp/delete",
    },
    NavChild {
        label: "Reset SFTP",
        href: "/ftp/reset",
    },
];

const DNS_CHILDREN: &[NavChild] = &[
    NavChild {
        label: "DNS Zones",
        href: "/server/dns/zones",
    },
    NavChild {
        label: "Cloudflare DNS",
        href: "/dns/cloudflare",
    },
    NavChild {
        label: "Nameservers",
        href: "/server/dns/nameservers",
    },
    NavChild {
        label: "Default Nameservers",
        href: "/server/dns/defaults",
    },
];

const BACKUPS_CHILDREN: &[NavChild] = &[
    NavChild {
        label: "Create Backup",
        href: "/backups/create",
    },
    NavChild {
        label: "Restore Backup",
        href: "/backups/restore",
    },
    NavChild {
        label: "Schedule Backup",
        href: "/backups/schedule",
    },
    NavChild {
        label: "Destinations",
        href: "/backups/destinations",
    },
];

const SSL_CHILDREN: &[NavChild] = &[
    NavChild {
        label: "Manage SSL",
        href: "/security/ssl/manage",
    },
    NavChild {
        label: "Hostname SSL",
        href: "/security/ssl/hostname",
    },
    NavChild {
        label: "Mail SSL",
        href: "/security/ssl/mail",
    },
];

const USERS_CHILDREN: &[NavChild] = &[
    NavChild {
        label: "View Profile",
        href: "/account/users/profile",
    },
    NavChild {
        label: "Create New User",
        href: "/account/users/create",
    },
    NavChild {
        label: "List Users",
        href: "/account/users/list",
    },
    NavChild {
        label: "Create ACL",
        href: "/account/acl/create",
    },
    NavChild {
        label: "Modify ACL",
        href: "/account/acl/modify",
    },
];

const PHP_CHILDREN: &[NavChild] = &[
    NavChild {
        label: "PHP Extensions",
        href: "/server/php/extensions",
    },
    NavChild {
        label: "PHP Configurations",
        href: "/server/php/configs",
    },
    NavChild {
        label: "PHP Tuning",
        href: "/server/php/tuning",
    },
];

/// Server > Logs: overview (panel activity) plus one viewer per host log. Log retention stays
/// under Settings because it is a panel preference, not a log viewer.
const LOGS_CHILDREN: &[NavChild] = &[
    NavChild {
        label: "Overview",
        href: "/server/logs",
    },
    NavChild {
        label: "Main Log",
        href: "/server/logs/panel",
    },
    NavChild {
        label: "Access Logs",
        href: "/server/logs/access",
    },
    NavChild {
        label: "Error Logs",
        href: "/server/logs/error",
    },
    NavChild {
        label: "Email Log",
        href: "/server/logs/email",
    },
    NavChild {
        label: "FTP Logs",
        href: "/server/logs/ftp",
    },
    NavChild {
        label: "ModSec Audit",
        href: "/server/logs/modsec",
    },
];

const LITESPEED_CHILDREN: &[NavChild] = &[
    NavChild {
        label: "Open OLS",
        href: "/server/openlitespeed",
    },
    NavChild {
        label: "Open OLSE",
        href: "/server/litespeed-enterprise",
    },
    NavChild {
        label: "LiteSpeed plans",
        href: "/server/litespeed/plans",
    },
];

const SECURITY_CHILDREN: &[NavChild] = &[
    NavChild {
        label: "Firewall",
        href: "/security/firewall",
    },
    NavChild {
        label: "Secure SSH",
        href: "/security/ssh",
    },
    NavChild {
        label: "Fail2ban",
        href: "/security/fail2ban",
    },
    NavChild {
        label: "Malware scan",
        href: "/security/malware-scan",
    },
];

const SETTINGS_CHILDREN: &[NavChild] = &[
    NavChild {
        label: "Version Management",
        href: "/settings/version",
    },
    NavChild {
        label: "System Repair",
        href: "/server/system-repair",
    },
    NavChild {
        label: "Design",
        href: "/settings/design",
    },
    NavChild {
        label: "Setup Wizard",
        href: "/settings/setup",
    },
    NavChild {
        label: "Connect",
        href: "/settings/connect",
    },
    NavChild {
        label: "Site messages",
        href: "/settings/site-messages",
    },
    NavChild {
        label: "Error messages",
        href: "/settings/error-messages",
    },
    NavChild {
        label: "Log retention",
        href: "/settings/logs",
    },
    NavChild {
        label: "Change Port",
        href: "/settings/port",
    },
];

const PLUGINS_CHILDREN: &[NavChild] = &[
    NavChild {
        label: "Installed",
        href: "/plugins?view=installed",
    },
    NavChild {
        label: "Store",
        href: "/plugins?view=store",
    },
];

const DOCKER_CHILDREN: &[NavChild] = &[
    NavChild {
        label: "Active Containers",
        href: "/docker/list",
    },
    NavChild {
        label: "Create Container",
        href: "/docker/create",
    },
    NavChild {
        label: "Images",
        href: "/docker/images",
    },
];

/// Main / hosting: frequent site and account tools at section level.
pub(crate) const MAIN: &[NavEntry] = &[
    NavEntry::Link {
        id: "dashboard",
        href: "/dashboard",
        label: "Dashboard",
    },
    NavEntry::Group {
        id: "users",
        href: "/account/users",
        label: "Users",
        children: USERS_CHILDREN,
    },
    NavEntry::Group {
        id: "websites",
        href: "/websites",
        label: "Websites",
        children: WEBSITES_CHILDREN,
    },
    NavEntry::Group {
        id: "wordpress",
        href: "/wordpress",
        label: "WordPress",
        children: WORDPRESS_CHILDREN,
    },
    NavEntry::Link {
        id: "packages",
        href: "/packages",
        label: "Packages",
    },
    NavEntry::Group {
        id: "email",
        href: "/email",
        label: "Email",
        children: EMAIL_CHILDREN,
    },
    NavEntry::Group {
        id: "databases",
        href: "/databases",
        label: "Databases",
        children: DATABASES_CHILDREN,
    },
    NavEntry::Group {
        id: "ftp",
        href: "/ftp",
        label: "FTP",
        children: FTP_CHILDREN,
    },
    NavEntry::Group {
        id: "dns",
        href: "/server/dns/zones",
        label: "DNS",
        children: DNS_CHILDREN,
    },
    NavEntry::Group {
        id: "backups",
        href: "/backups",
        label: "Backups",
        children: BACKUPS_CHILDREN,
    },
    NavEntry::Group {
        id: "ssl",
        href: "/security/ssl",
        label: "SSL",
        children: SSL_CHILDREN,
    },
    NavEntry::Group {
        id: "plugins",
        href: "/plugins",
        label: "Plugins",
        children: PLUGINS_CHILDREN,
    },
    NavEntry::Group {
        id: "docker",
        href: "/docker",
        label: "Docker",
        children: DOCKER_CHILDREN,
    },
];

/// Server tools promoted out of a single nested Server tree.
pub(crate) const SERVER: &[NavEntry] = &[
    NavEntry::Link {
        id: "server",
        href: "/server",
        label: "Server",
    },
    NavEntry::Link {
        id: "mariadb",
        href: "/databases/manager",
        label: "MariaDB Manager",
    },
    NavEntry::Link {
        id: "root-files",
        href: "/server/files",
        label: "Root File Manager",
    },
    NavEntry::Group {
        id: "php",
        href: "/server/php",
        label: "PHP",
        children: PHP_CHILDREN,
    },
    NavEntry::Link {
        id: "services",
        href: "/server/services",
        label: "Manage Services",
    },
    NavEntry::Link {
        id: "system-repair",
        href: "/server/system-repair",
        label: "System Repair",
    },
    NavEntry::Link {
        id: "processes",
        href: "/server/processes",
        label: "Top Processes",
    },
    NavEntry::Group {
        id: "logs",
        href: "/server/logs",
        label: "Logs",
        children: LOGS_CHILDREN,
    },
    NavEntry::Link {
        id: "package-manager",
        href: "/server/packages",
        label: "Package Manager",
    },
    NavEntry::Group {
        id: "litespeed",
        href: "/server/litespeed",
        label: "LiteSpeed",
        children: LITESPEED_CHILDREN,
    },
];

/// Security remains a shallow group (ACL id `security` preserved).
pub(crate) const SECURITY: &[NavEntry] = &[NavEntry::Group {
    id: "security",
    href: "/security",
    label: "Security",
    children: SECURITY_CHILDREN,
}];

/// Settings hub with frequent panel preferences.
pub(crate) const SETTINGS: &[NavEntry] = &[NavEntry::Group {
    id: "settings",
    href: "/settings",
    label: "Settings",
    children: SETTINGS_CHILDREN,
}];

/// All catalog sections in sidebar order (for ACL + path maps).
pub(crate) fn all_sections() -> [&'static [NavEntry]; 4] {
    [MAIN, SERVER, SECURITY, SETTINGS]
}
