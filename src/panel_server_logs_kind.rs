//! The host logs shown under Server > Logs and their fixed allowlists.
//!
//! The request never supplies a path: only these files and systemd units are ever read.

/// Page sizes offered in the UI (default is the first entry that equals [`DEFAULT_PER_PAGE`]).
pub const PAGE_SIZES: [usize; 6] = [5, 10, 25, 50, 100, 200];
pub const DEFAULT_PER_PAGE: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostLogKind {
    Panel,
    Access,
    Error,
    Email,
    Ftp,
    ModSec,
}

impl HostLogKind {
    pub const ALL: [HostLogKind; 6] = [
        HostLogKind::Panel,
        HostLogKind::Access,
        HostLogKind::Error,
        HostLogKind::Email,
        HostLogKind::Ftp,
        HostLogKind::ModSec,
    ];

    pub fn from_slug(slug: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.slug() == slug)
    }

    pub fn slug(self) -> &'static str {
        match self {
            Self::Panel => "panel",
            Self::Access => "access",
            Self::Error => "error",
            Self::Email => "email",
            Self::Ftp => "ftp",
            Self::ModSec => "modsec",
        }
    }

    /// Viewer route for this log.
    pub fn href(self) -> &'static str {
        match self {
            Self::Panel => "/server/logs/panel",
            Self::Access => "/server/logs/access",
            Self::Error => "/server/logs/error",
            Self::Email => "/server/logs/email",
            Self::Ftp => "/server/logs/ftp",
            Self::ModSec => "/server/logs/modsec",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::Panel => "Main Log",
            Self::Access => "Access Logs",
            Self::Error => "Error Logs",
            Self::Email => "Email Log",
            Self::Ftp => "FTP Logs",
            Self::ModSec => "ModSec Audit",
        }
    }

    pub fn subtitle(self) -> &'static str {
        match self {
            Self::Panel => "CPN panel log",
            Self::Access => "Web server and per-site access",
            Self::Error => "Web server and per-site errors",
            Self::Email => "Postfix and Dovecot mail log",
            Self::Ftp => "FTP daemon and jailed SFTP activity",
            Self::ModSec => "WAF audit log",
        }
    }

    /// True for the logs that also exist per website (a non-admin may see their own sites).
    pub fn per_site(self) -> bool {
        matches!(self, Self::Access | Self::Error)
    }

    /// Allowlisted log files, first existing and readable wins.
    pub fn files(self) -> &'static [&'static str] {
        match self {
            Self::Panel => &["/var/log/cpn/panel.log", "/var/log/cpn-installer.log"],
            Self::Access => &[
                "/usr/local/lsws/logs/access.log",
                "/var/log/httpd/access_log",
                "/var/log/nginx/access.log",
                "/var/log/apache2/access.log",
            ],
            Self::Error => &[
                "/usr/local/lsws/logs/error.log",
                "/var/log/httpd/error_log",
                "/var/log/nginx/error.log",
                "/var/log/apache2/error.log",
                "/var/log/cpn/error.log",
            ],
            Self::Email => &[
                "/var/log/maillog",
                "/var/log/mail.log",
                "/var/log/postfix.log",
            ],
            Self::Ftp => &[
                "/var/log/pure-ftpd/transfer.log",
                "/var/log/pureftpd.log",
                "/var/log/vsftpd.log",
                "/var/log/xferlog",
                "/var/log/proftpd/proftpd.log",
            ],
            Self::ModSec => &[
                "/usr/local/lsws/logs/auditmodsec.log",
                "/usr/local/lsws/logs/modsec_audit.log",
                "/var/log/httpd/modsec_audit.log",
                "/var/log/apache2/modsec_audit.log",
                "/var/log/nginx/modsec_audit.log",
                "/var/log/modsecurity/modsec_audit.log",
                "/var/log/modsec_audit.log",
            ],
        }
    }

    /// systemd units whose journal is the fallback when no file exists.
    pub fn units(self) -> &'static [&'static str] {
        match self {
            Self::Panel => &["cpn-installer.service"],
            // Access lines never come from a service journal; per-site logs live under each home.
            Self::Access | Self::ModSec => &[],
            Self::Error => &["lshttpd.service", "lsws.service", "httpd.service"],
            Self::Email => &["postfix.service", "dovecot.service"],
            Self::Ftp => &["pure-ftpd.service", "vsftpd.service", "proftpd.service"],
        }
    }

    /// Sentence shown when no log source exists at all.
    pub fn empty_hint(self) -> &'static str {
        match self {
            Self::Panel => {
                "No panel journal is readable yet. On the host run: journalctl -u cpn-installer.service -n 100"
            }
            Self::Access => {
                "No access log found yet. Server-wide and per-site access logs appear here once the web server has served a request."
            }
            Self::Error => {
                "No error log found yet. Server-wide and per-site error logs appear here once the web server has logged something."
            }
            Self::Email => {
                "No mail log found. Install the Email host stack (Postfix and Dovecot) to start logging mail."
            }
            Self::Ftp => {
                "No FTP daemon log and no jailed SFTP accounts were found. Create a website and reset its SFTP access (Websites > Manage), or install an FTP server, and activity is listed here."
            }
            Self::ModSec => {
                "ModSecurity is not installed on this host (no OpenLiteSpeed, Apache or Nginx ModSecurity module was found), so there is no audit log. It is listed here once ModSecurity is enabled and a rule has recorded a transaction."
            }
        }
    }

    /// Sentence shown when a source exists but holds no lines yet.
    pub fn quiet_hint(self) -> &'static str {
        match self {
            Self::Panel => "The panel log is empty.",
            Self::Access => {
                "The access logs are empty so far. Lines appear as the web server serves requests."
            }
            Self::Error => "The error logs are empty so far, which means no errors were recorded.",
            Self::Email => "The mail log is empty so far.",
            Self::Ftp => {
                "No FTP or SFTP activity has been recorded yet. Logins and transfers appear here."
            }
            Self::ModSec => {
                "ModSecurity is installed but its audit log has no transactions yet. Entries appear when a rule matches a request."
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs_round_trip_and_reject_unknown() {
        for kind in HostLogKind::ALL {
            assert_eq!(HostLogKind::from_slug(kind.slug()), Some(kind));
            assert_eq!(kind.href(), format!("/server/logs/{}", kind.slug()));
        }
        assert_eq!(HostLogKind::from_slug("../etc/passwd"), None);
        assert_eq!(HostLogKind::from_slug(""), None);
    }

    #[test]
    fn allowlisted_paths_are_absolute_without_traversal() {
        for kind in HostLogKind::ALL {
            for path in kind.files() {
                assert!(path.starts_with("/var/log/") || path.starts_with("/usr/local/lsws/logs/"));
                assert!(!path.contains(".."));
            }
        }
    }

    #[test]
    fn default_page_size_is_five_and_offered() {
        assert_eq!(DEFAULT_PER_PAGE, 5);
        assert!(PAGE_SIZES.contains(&10));
        assert!(PAGE_SIZES.contains(&DEFAULT_PER_PAGE));
    }

    #[test]
    fn only_access_and_error_are_per_site() {
        let sites: Vec<_> = HostLogKind::ALL
            .into_iter()
            .filter(|k| k.per_site())
            .collect();
        assert_eq!(sites, vec![HostLogKind::Access, HostLogKind::Error]);
    }
}
