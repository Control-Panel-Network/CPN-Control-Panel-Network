//! Detect CPN default document-root placeholders so Preview can use the live site.

use std::path::Path;

/// Host suffixes that never resolve on the public internet.
const PRIVATE_SUFFIXES: &[&str] = &[
    ".local",
    ".localhost",
    ".localdomain",
    ".test",
    ".example",
    ".invalid",
    ".internal",
    ".intranet",
    ".lan",
    ".home",
    ".home.arpa",
    ".corp",
    ".private",
    ".vbox",
];

/// Distinctive strings from CPN default `index.html` (current and earlier copy).
const STUB_NEEDLES: &[&str] = &[
    "This document root was created by CPN",
    "This document root was created by <strong>CPN",
    "Replace this file with your site",
    "Your site is ready",
];

/// True when the hostname can plausibly be reached on the public internet.
pub fn is_public_internet_host(host_raw: &str) -> bool {
    let host = host_raw.trim().trim_end_matches('.').to_ascii_lowercase();
    if host.is_empty() || crate::website_preview::is_blocked_preview_host(&host) {
        return false;
    }
    if PRIVATE_SUFFIXES.iter().any(|suffix| host.ends_with(suffix)) {
        return false;
    }
    let Some(tld) = host.rsplit('.').next() else {
        return false;
    };
    tld.len() >= 2 && tld.chars().all(|ch| ch.is_ascii_alphabetic())
}

/// True when HTML is the CPN "Site ready" placeholder, not a real site.
pub fn html_looks_like_placeholder(html: &str) -> bool {
    let trimmed = html.trim();
    if trimmed.is_empty() {
        return false;
    }
    if trimmed.contains("This document root was created by CPN") {
        return true;
    }
    let heading_ready = trimmed.contains(">Site ready<")
        || trimmed.contains("<h1>Site ready</h1>")
        || trimmed.contains("<title>Site ready</title>");
    if heading_ready && trimmed.contains("document root was created") {
        return true;
    }
    STUB_NEEDLES.iter().any(|needle| trimmed.contains(needle))
}

fn read_index_html(docroot: &Path) -> Option<String> {
    for name in ["index.html", "index.htm"] {
        let path = docroot.join(name);
        if !path.is_file() {
            continue;
        }
        let bytes = std::fs::read(&path).ok()?;
        if bytes.len() > 256 * 1024 {
            return None;
        }
        return Some(String::from_utf8_lossy(&bytes).into_owned());
    }
    None
}

fn has_index_file(docroot: &Path) -> bool {
    ["index.html", "index.htm", "index.php"]
        .iter()
        .any(|name| docroot.join(name).is_file())
}

/// True when the site document root still has the default CPN placeholder index.
pub fn docroot_is_placeholder(docroot: &Path) -> bool {
    if !docroot.is_dir() {
        return false;
    }
    match read_index_html(docroot) {
        Some(html) => html_looks_like_placeholder(&html),
        None => false,
    }
}

/// True when Preview should fetch the public origin instead of local files:
/// CPN Site ready stub, missing document root, or no index.html/htm/php.
pub fn docroot_should_use_live_origin(docroot: &Path) -> bool {
    if !docroot.is_dir() {
        return true;
    }
    if !has_index_file(docroot) {
        return true;
    }
    docroot_is_placeholder(docroot)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn public_hosts_and_lab_hosts() {
        assert!(is_public_internet_host("cmstest.newstargeted.com"));
        assert!(is_public_internet_host("example.com"));
        assert!(!is_public_internet_host("lab.test"));
        assert!(!is_public_internet_host("127.0.0.1"));
        assert!(!is_public_internet_host("localhost"));
    }

    #[test]
    fn detects_short_stub_and_builtin_stub() {
        assert!(html_looks_like_placeholder(
            "<h1>Site ready</h1><p>This document root was created by CPN. Replace this file with your site.</p>"
        ));
        assert!(html_looks_like_placeholder(
            crate::site_messages::builtin_site_ready_html()
        ));
        assert!(!html_looks_like_placeholder(
            "<h1>News Targeted</h1><p>Official CheckMarx Jenkins package</p>"
        ));
    }

    #[test]
    fn docroot_placeholder_from_index() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("cpn-stub-doc-{stamp}"));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("index.html"),
            b"<h1>Site ready</h1><p>This document root was created by CPN. Replace this file with your site.</p>",
        )
        .unwrap();
        assert!(docroot_is_placeholder(&root));
        assert!(docroot_should_use_live_origin(&root));
        fs::write(root.join("index.html"), b"<h1>Real CMS</h1>").unwrap();
        assert!(!docroot_is_placeholder(&root));
        assert!(!docroot_should_use_live_origin(&root));
        fs::remove_file(root.join("index.html")).unwrap();
        assert!(docroot_should_use_live_origin(&root));
        let missing = root.join("does-not-exist");
        assert!(docroot_should_use_live_origin(&missing));
        let _ = fs::remove_dir_all(&root);
    }
}
