//! Hub tile catalogs for Server, Email, Databases & FTP, Backups, Users & Plans, and Security.

use crate::panel_hubs::HubTile;

pub fn security_hub_sections() -> Vec<(&'static str, Vec<HubTile<'static>>)> {
    vec![
        (
            "Security",
            vec![
                HubTile {
                    title: "Firewall",
                    subtitle: "Manage rules",
                    href: "/security/firewall",
                    live: true,
                },
                HubTile {
                    title: "Secure SSH",
                    subtitle: "Harden SSH",
                    href: "/security/ssh",
                    live: true,
                },
                HubTile {
                    title: "Fail2ban",
                    subtitle: "Brute-force jails",
                    href: "/security/fail2ban",
                    live: true,
                },
                HubTile {
                    title: "ModSecurity",
                    subtitle: "WAF config",
                    href: "/security/modsecurity",
                    live: true,
                },
                HubTile {
                    title: "ModSec Rules",
                    subtitle: "Edit / list rules",
                    href: "/security/modsec-rules",
                    live: true,
                },
                HubTile {
                    title: "Rule Packs",
                    subtitle: "OWASP CRS roots",
                    href: "/security/rule-packs",
                    live: true,
                },
                HubTile {
                    title: "Malware scan",
                    subtitle: "CPN malware status",
                    href: "/security/malware-scan",
                    live: true,
                },
            ],
        ),
        (
            "SSL certificates",
            vec![
                HubTile {
                    title: "Manage SSL",
                    subtitle: "Site certificates",
                    href: "/security/ssl",
                    live: true,
                },
                HubTile {
                    title: "Hostname SSL",
                    subtitle: "Panel hostname cert",
                    href: "/security/ssl/hostname",
                    live: true,
                },
                HubTile {
                    title: "Mail Server SSL",
                    subtitle: "Mail cert status",
                    href: "/security/ssl/mail",
                    live: true,
                },
            ],
        ),
    ]
}

pub fn users_plans_hub_sections() -> Vec<(&'static str, Vec<HubTile<'static>>)> {
    vec![
        (
            "Users",
            vec![
                HubTile {
                    title: "View Profile",
                    subtitle: "Your account details",
                    href: "/account/users/profile",
                    live: true,
                },
                HubTile {
                    title: "List Users",
                    subtitle: "All panel accounts",
                    href: "/account/users/list",
                    live: true,
                },
                HubTile {
                    title: "Create User",
                    subtitle: "Add a panel account",
                    href: "/account/users/create",
                    live: true,
                },
                HubTile {
                    title: "Modify User",
                    subtitle: "Reset password or remove",
                    href: "/account/users/modify",
                    live: true,
                },
                HubTile {
                    title: "Reseller Center",
                    subtitle: "Reseller settings",
                    href: "/account/users/reseller",
                    live: false,
                },
            ],
        ),
        (
            "Hosting Plans",
            vec![
                HubTile {
                    title: "Packages",
                    subtitle: "View hosting plans",
                    href: "/packages",
                    live: true,
                },
                HubTile {
                    title: "Create Package",
                    subtitle: "Add a plan",
                    href: "/packages/new",
                    live: true,
                },
                HubTile {
                    title: "Modify Package",
                    subtitle: "Edit a plan",
                    href: "/packages",
                    live: true,
                },
            ],
        ),
        (
            "Admin",
            vec![
                HubTile {
                    title: "Create ACL",
                    subtitle: "Site permission grants",
                    href: "/account/acl/create",
                    live: true,
                },
                HubTile {
                    title: "Modify ACL",
                    subtitle: "Edit site grants",
                    href: "/account/acl/modify",
                    live: true,
                },
                HubTile {
                    title: "API Access",
                    subtitle: "API tokens",
                    href: "/account/api-access",
                    live: false,
                },
                HubTile {
                    title: "Plugins",
                    subtitle: "Installed plugins",
                    href: "/plugins",
                    live: true,
                },
            ],
        ),
    ]
}

pub fn server_hub_sections() -> Vec<(&'static str, Vec<HubTile<'static>>)> {
    vec![
        (
            "Services",
            vec![HubTile {
                title: "Services Status",
                subtitle: "Start/stop known units",
                href: "/server/services",
                live: true,
            }],
        ),
        (
            "PHP & performance",
            vec![
                HubTile {
                    title: "PHP Extensions",
                    subtitle: "Installed PHP modules",
                    href: "/server/php/extensions",
                    live: true,
                },
                HubTile {
                    title: "PHP Configs",
                    subtitle: "php.ini path (read-only)",
                    href: "/server/php/configs",
                    live: true,
                },
                HubTile {
                    title: "PHP Tuning",
                    subtitle: "Safe read-only overview",
                    href: "/server/php/tuning",
                    live: true,
                },
                HubTile {
                    title: "Top Processes",
                    subtitle: "Live process snapshot",
                    href: "/server/processes",
                    live: true,
                },
                HubTile {
                    title: "Change Port",
                    subtitle: "Panel listen port",
                    href: "/settings/port",
                    live: true,
                },
                HubTile {
                    title: "Package Manager",
                    subtitle: "dnf/apt status and search",
                    href: "/server/packages",
                    live: true,
                },
            ],
        ),
        (
            "Containers",
            vec![
                HubTile {
                    title: "Docker Apps",
                    subtitle: "Containerized apps overview",
                    href: "/server/docker/apps",
                    live: true,
                },
                HubTile {
                    title: "Containers",
                    subtitle: "List containers",
                    href: "/server/docker/containers",
                    live: true,
                },
                HubTile {
                    title: "Docker Images",
                    subtitle: "List images",
                    href: "/server/docker/images",
                    live: true,
                },
            ],
        ),
        (
            "Files, data & DNS",
            vec![
                HubTile {
                    title: "Root File Manager",
                    subtitle: "Browse allowlisted roots",
                    href: "/server/files",
                    live: true,
                },
                HubTile {
                    title: "MariaDB Manager",
                    subtitle: "MariaDB first (MySQL alias)",
                    href: "/databases/manager",
                    live: true,
                },
                HubTile {
                    title: "DNS Zones",
                    subtitle: "Zone files under CPN data",
                    href: "/server/dns/zones",
                    live: true,
                },
                HubTile {
                    title: "Nameservers",
                    subtitle: "NS records list",
                    href: "/server/dns/nameservers",
                    live: true,
                },
                HubTile {
                    title: "Default Nameservers",
                    subtitle: "Configure defaults",
                    href: "/server/dns/defaults",
                    live: true,
                },
            ],
        ),
    ]
}

pub fn settings_hub_sections() -> Vec<(&'static str, Vec<HubTile<'static>>)> {
    vec![
        (
            "Settings",
            vec![
                HubTile {
                    title: "Version Management",
                    subtitle: "Update CPN",
                    href: "/settings/version",
                    live: true,
                },
                HubTile {
                    title: "Design",
                    subtitle: "Theme & custom CSS",
                    href: "/settings/design",
                    live: true,
                },
                HubTile {
                    title: "Setup Wizard",
                    subtitle: "Server onboarding",
                    href: "/settings/setup",
                    live: true,
                },
                HubTile {
                    title: "Connect",
                    subtitle: "Community & docs",
                    href: "/settings/connect",
                    live: true,
                },
            ],
        ),
        (
            "Panel",
            vec![HubTile {
                title: "Change Port",
                subtitle: "Panel listen port",
                href: "/settings/port",
                live: true,
            }],
        ),
    ]
}

pub fn backups_hub_tiles() -> Vec<HubTile<'static>> {
    vec![
        HubTile {
            title: "Create Backup",
            subtitle: "Back up a site",
            href: "/backups/create",
            live: true,
        },
        HubTile {
            title: "Restore Backup",
            subtitle: "Restore from a backup",
            href: "/backups/restore",
            live: true,
        },
        HubTile {
            title: "Schedule Backup",
            subtitle: "Automate backups",
            href: "/backups/schedule",
            live: true,
        },
        HubTile {
            title: "Destinations",
            subtitle: "Backup destinations",
            href: "/backups/destinations",
            live: true,
        },
        HubTile {
            title: "Google Drive",
            subtitle: "Backup to Drive",
            href: "/backups/google-drive",
            live: false,
        },
        HubTile {
            title: "Remote Backups",
            subtitle: "Transfer to another server",
            href: "/backups/remote",
            live: false,
        },
    ]
}

pub fn databases_hub_sections() -> Vec<(&'static str, Vec<HubTile<'static>>)> {
    vec![
        (
            "Databases",
            vec![
                HubTile {
                    title: "All Databases",
                    subtitle: "View databases",
                    href: "/databases/all",
                    live: true,
                },
                HubTile {
                    title: "Create Database",
                    subtitle: "Add a database",
                    href: "/databases/create",
                    live: true,
                },
                HubTile {
                    title: "Delete Database",
                    subtitle: "Remove a database",
                    href: "/databases/delete",
                    live: true,
                },
                HubTile {
                    title: "phpMyAdmin",
                    subtitle: "Open phpMyAdmin",
                    href: "/databases/phpmyadmin",
                    live: true,
                },
                HubTile {
                    title: "MariaDB Manager",
                    subtitle: "Tune and monitor MariaDB",
                    href: "/databases/manager",
                    live: true,
                },
            ],
        ),
        (
            "FTP",
            vec![
                HubTile {
                    title: "FTP Accounts",
                    subtitle: "View FTP users",
                    href: "/ftp/accounts",
                    live: true,
                },
                HubTile {
                    title: "Create FTP Account",
                    subtitle: "Add an FTP user",
                    href: "/ftp/create",
                    live: false,
                },
                HubTile {
                    title: "Delete FTP Account",
                    subtitle: "Remove an FTP user",
                    href: "/ftp/delete",
                    live: false,
                },
                HubTile {
                    title: "Reset FTP",
                    subtitle: "Reset configuration",
                    href: "/ftp/reset",
                    live: false,
                },
            ],
        ),
    ]
}

pub fn email_hub_sections() -> Vec<(&'static str, Vec<HubTile<'static>>)> {
    vec![
        (
            "Email",
            vec![
                HubTile {
                    title: "Email Accounts",
                    subtitle: "View all mailboxes",
                    href: "/email/accounts",
                    live: true,
                },
                HubTile {
                    title: "Create Email",
                    subtitle: "Add a mailbox",
                    href: "/email/create",
                    live: true,
                },
                HubTile {
                    title: "Forwarding",
                    subtitle: "Forward to other addresses",
                    href: "/email/forwarding",
                    live: true,
                },
                HubTile {
                    title: "Catch-All",
                    subtitle: "Catch unrouted mail",
                    href: "/email/catchall",
                    live: true,
                },
                HubTile {
                    title: "Pattern Forwarding",
                    subtitle: "Rule-based forwarding",
                    href: "/email/pattern-forwarding",
                    live: false,
                },
                HubTile {
                    title: "Email Limits",
                    subtitle: "Sending limits",
                    href: "/email/limits",
                    live: false,
                },
                HubTile {
                    title: "Change Password",
                    subtitle: "Reset mailbox password",
                    href: "/email/password",
                    live: false,
                },
                HubTile {
                    title: "DKIM Manager",
                    subtitle: "Email signing keys",
                    href: "/email/dkim",
                    live: true,
                },
                HubTile {
                    title: "Webmail",
                    subtitle: "Open webmail",
                    href: "/email/webmail",
                    live: true,
                },
            ],
        ),
        (
            "Deliverability & anti-spam",
            vec![
                HubTile {
                    title: "Email Delivery",
                    subtitle: "SMTP relay and domains",
                    href: "/email/delivery",
                    live: true,
                },
                HubTile {
                    title: "Email Debugger",
                    subtitle: "Diagnose mail issues",
                    href: "/email/debugger",
                    live: false,
                },
                HubTile {
                    title: "Mail Queue",
                    subtitle: "Inspect the queue",
                    href: "/email/queue",
                    live: false,
                },
                HubTile {
                    title: "SpamAssassin",
                    subtitle: "Spam filtering",
                    href: "/email/spamassassin",
                    live: false,
                },
                HubTile {
                    title: "Rspamd",
                    subtitle: "Spam filtering",
                    href: "/email/rspamd",
                    live: false,
                },
                HubTile {
                    title: "MailScanner",
                    subtitle: "Mail scanning",
                    href: "/email/mailscanner",
                    live: false,
                },
                HubTile {
                    title: "Email Marketing",
                    subtitle: "Campaigns and lists",
                    href: "/email/marketing",
                    live: false,
                },
                HubTile {
                    title: "Plus-Addressing",
                    subtitle: "user+tag addressing",
                    href: "/email/plus-addressing",
                    live: false,
                },
            ],
        ),
    ]
}
