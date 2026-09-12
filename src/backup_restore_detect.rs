//! Detect backup archive formats from filenames and member path lists.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackupFormat {
    Cpn,
    WordPress,
    Cpanel,
    CyberPanel,
    Unknown,
}

impl BackupFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cpn => "cpn",
            Self::WordPress => "wordpress",
            Self::Cpanel => "cpanel",
            Self::CyberPanel => "cyberpanel",
            Self::Unknown => "unknown",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Cpn => "CPN archive",
            Self::WordPress => "WordPress backup",
            Self::Cpanel => "cPanel backup",
            Self::CyberPanel => "CyberPanel backup (source format)",
            Self::Unknown => "Unknown / unsupported",
        }
    }

    pub fn parse(raw: &str) -> Result<Self, String> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "" | "auto" | "detect" => Ok(Self::Unknown),
            "cpn" | "native" => Ok(Self::Cpn),
            "wordpress" | "wp" => Ok(Self::WordPress),
            "cpanel" | "cpmove" => Ok(Self::Cpanel),
            "cyberpanel" | "cp" => Ok(Self::CyberPanel),
            other => Err(format!(
                "Unknown format `{other}`. Use: auto, cpn, wordpress, cpanel, cyberpanel"
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectedBackup {
    pub format: BackupFormat,
    pub confidence: &'static str,
    pub notes: Vec<String>,
    pub has_sql: bool,
    pub has_wpress: bool,
}

fn norm_member(raw: &str) -> String {
    raw.trim()
        .trim_start_matches("./")
        .replace('\\', "/")
        .to_ascii_lowercase()
}

fn basename_lower(path: &str) -> String {
    path.rsplit('/').next().unwrap_or(path).to_ascii_lowercase()
}

/// Score archive members (and optional filename) into a format guess.
pub fn detect_from_members(filename: &str, members: &[String]) -> DetectedBackup {
    let name = basename_lower(filename);
    let paths: Vec<String> = members.iter().map(|m| norm_member(m)).collect();
    let mut notes = Vec::new();
    let has_sql = paths.iter().any(|p| {
        p.ends_with(".sql")
            || p.ends_with(".sql.gz")
            || p.ends_with("-db.gz")
            || p.contains("/mysql/")
            || p == "databases.sql"
    });
    let has_wpress = name.ends_with(".wpress") || paths.iter().any(|p| p.ends_with(".wpress"));

    let has = |needle: &str| paths.iter().any(|p| p == needle || p.ends_with(needle));
    let contains = |needle: &str| paths.iter().any(|p| p.contains(needle));

    // CyberPanel classic: meta.xml + public_html (source format only; CPN is not CyberPanel).
    let cyber = has("meta.xml")
        || (name.starts_with("backup-") && contains("meta.xml"))
        || (has("meta.xml") && contains("public_html/"));

    // cPanel full / cpmove layout.
    let cpanel = name.starts_with("cpmove-")
        || contains("homedir/")
        || (contains("mysql/") && contains("userdata/"))
        || (contains("homedir/public_html/") || has("homedir/public_html"));

    // WordPress plugin / plain layouts.
    let wp_updraft = contains("updraft")
        || paths.iter().any(|p| {
            p.contains("-plugins.zip") || p.contains("-themes.zip") || p.contains("-db.gz")
        });
    let wp_duplicator = contains("dup-installer/")
        || paths
            .iter()
            .any(|p| p.contains("dup-database") || p.ends_with("database.sql"));
    let wp_plain = (has("wp-config.php") || contains("/wp-config.php"))
        && (contains("wp-content/") || contains("/wp-content/"));
    let wordpress = has_wpress || wp_updraft || wp_duplicator || wp_plain;

    // Native CPN selective archive markers.
    let cpn = contains("panel-config/")
        || has("databases.sql")
        || (contains("public_html/")
            && (contains("plugins/")
                || contains("backups-copy/")
                || name.starts_with("site-")
                || name.starts_with("panel-")
                || name.starts_with("subdomain-")));

    if has_wpress {
        notes.push(
            "All-in-One WP Migration (.wpress) detected. Export zip+SQL from the plugin, or use a zip that contains wp-content and a .sql dump."
                .into(),
        );
    }
    if cyber {
        notes.push(
            "Import CyberPanel backup: files map to the chosen site docroot; databases import when MariaDB is available. Email (vmail) is best-effort only."
                .into(),
        );
    }
    if cpanel {
        notes.push(
            "cPanel layout: homedir/public_html maps to the site docroot; mysql dumps import when present. Email accounts are best-effort."
                .into(),
        );
    }

    // Prefer stronger structural markers over filename heuristics.
    if cyber && !cpanel {
        return DetectedBackup {
            format: BackupFormat::CyberPanel,
            confidence: "high",
            notes,
            has_sql,
            has_wpress,
        };
    }
    if cpanel {
        return DetectedBackup {
            format: BackupFormat::Cpanel,
            confidence: "high",
            notes,
            has_sql,
            has_wpress,
        };
    }
    if wordpress && !cpn {
        return DetectedBackup {
            format: BackupFormat::WordPress,
            confidence: if wp_plain || wp_duplicator || wp_updraft {
                "high"
            } else {
                "medium"
            },
            notes,
            has_sql,
            has_wpress,
        };
    }
    if cpn {
        return DetectedBackup {
            format: BackupFormat::Cpn,
            confidence: "high",
            notes,
            has_sql,
            has_wpress,
        };
    }
    if wordpress {
        return DetectedBackup {
            format: BackupFormat::WordPress,
            confidence: "medium",
            notes,
            has_sql,
            has_wpress,
        };
    }

    notes.push(
        "Could not confidently detect format. Pick a format manually or use a supported archive."
            .into(),
    );
    DetectedBackup {
        format: BackupFormat::Unknown,
        confidence: "low",
        notes,
        has_sql,
        has_wpress,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_cpanel_cpmove() {
        let members = vec![
            "./homedir/public_html/index.php".into(),
            "./mysql/user_wp.sql".into(),
            "./userdata/example.com".into(),
        ];
        let d = detect_from_members("cpmove-user.tar.gz", &members);
        assert_eq!(d.format, BackupFormat::Cpanel);
    }

    #[test]
    fn detects_cyberpanel_meta() {
        let members = vec![
            "./meta.xml".into(),
            "./public_html/index.html".into(),
            "./site_db.sql".into(),
        ];
        let d = detect_from_members("backup-example.com-01.tar.gz", &members);
        assert_eq!(d.format, BackupFormat::CyberPanel);
        assert!(d.has_sql);
    }

    #[test]
    fn detects_wordpress_plain() {
        let members = vec![
            "wp-config.php".into(),
            "wp-content/themes/twentytwenty/style.css".into(),
            "dump.sql".into(),
        ];
        let d = detect_from_members("site-files.zip", &members);
        assert_eq!(d.format, BackupFormat::WordPress);
    }

    #[test]
    fn detects_duplicator() {
        let members = vec![
            "dup-installer/main.installer.php".into(),
            "20240101_database.sql".into(),
            "wp-content/index.php".into(),
        ];
        let d = detect_from_members("dup-package.zip", &members);
        assert_eq!(d.format, BackupFormat::WordPress);
    }

    #[test]
    fn detects_cpn_native() {
        let members = vec![
            "./public_html/index.html".into(),
            "./databases.sql".into(),
            "./plugins/demo/plugin.json".into(),
        ];
        let d = detect_from_members("site-example.com-123.tar.gz", &members);
        assert_eq!(d.format, BackupFormat::Cpn);
    }

    #[test]
    fn labels_never_claim_cpn_is_cyberpanel() {
        assert!(BackupFormat::CyberPanel.label().contains("source format"));
        assert!(
            !BackupFormat::Cpn
                .label()
                .to_ascii_lowercase()
                .contains("cyberpanel")
        );
    }
}
