//! Static sidebar catalog: groups and child routes for CPN Panel.

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
        href: "/websites",
    },
    NavChild {
        label: "Create Website",
        href: "/websites",
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
        label: "DKIM Manager",
        href: "/email/dkim",
    },
    NavChild {
        label: "Webmail",
        href: "/email/webmail",
    },
    NavChild {
        label: "Email Delivery",
        href: "/email/delivery",
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
        label: "phpMyAdmin",
        href: "/databases/phpmyadmin",
    },
    NavChild {
        label: "MariaDB Manager",
        href: "/databases/manager",
    },
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
        label: "Modify User",
        href: "/account/users/modify",
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

const SERVER_CHILDREN: &[NavChild] = &[
    NavChild {
        label: "Services Status",
        href: "/server/services",
    },
    NavChild {
        label: "PHP Extensions",
        href: "/server/php/extensions",
    },
    NavChild {
        label: "Top Processes",
        href: "/server/processes",
    },
    NavChild {
        label: "Root File Manager",
        href: "/server/files",
    },
    NavChild {
        label: "DNS Zones",
        href: "/server/dns/zones",
    },
    NavChild {
        label: "Cloudflare DNS",
        href: "/dns/cloudflare",
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
        label: "Manage SSL",
        href: "/security/ssl",
    },
    NavChild {
        label: "Hostname SSL",
        href: "/security/ssl/hostname",
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
        label: "Change Port",
        href: "/settings/port",
    },
];

const PLUGINS_CHILDREN: &[NavChild] = &[
    NavChild {
        label: "Installed",
        href: "/plugins",
    },
    NavChild {
        label: "Plugin Store",
        href: "/plugins?view=store",
    },
];

pub(crate) const HOSTING: &[NavEntry] = &[
    NavEntry::Link {
        id: "dashboard",
        href: "/dashboard",
        label: "Dashboard",
    },
    NavEntry::Group {
        id: "websites",
        href: "/websites",
        label: "Websites",
        children: WEBSITES_CHILDREN,
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
        label: "Databases & FTP",
        children: DATABASES_CHILDREN,
    },
    NavEntry::Group {
        id: "backups",
        href: "/backups",
        label: "Backups",
        children: BACKUPS_CHILDREN,
    },
    NavEntry::Link {
        id: "apps",
        href: "/apps",
        label: "Apps",
    },
    NavEntry::Group {
        id: "plugins",
        href: "/plugins",
        label: "Plugins",
        children: PLUGINS_CHILDREN,
    },
];

pub(crate) const ACCOUNT: &[NavEntry] = &[
    NavEntry::Group {
        id: "users",
        href: "/account/users",
        label: "Users & Plans",
        children: USERS_CHILDREN,
    },
    NavEntry::Link {
        id: "packages",
        href: "/packages",
        label: "Packages",
    },
];

pub(crate) const ADMINISTRATION: &[NavEntry] = &[
    NavEntry::Group {
        id: "server",
        href: "/server",
        label: "Server",
        children: SERVER_CHILDREN,
    },
    NavEntry::Group {
        id: "security",
        href: "/security",
        label: "Security",
        children: SECURITY_CHILDREN,
    },
    NavEntry::Group {
        id: "settings",
        href: "/settings",
        label: "Settings",
        children: SETTINGS_CHILDREN,
    },
];
