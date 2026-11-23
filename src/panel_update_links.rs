//! GitHub links attached to update notifications (commit, compare, release).
//!
//! Links always point at the configured update source repo (official repo or an
//! operator fork from Update source settings). Every URL is built from a
//! validated `owner/repo` and a validated hex SHA or tag, so the result is safe
//! to put into an `href` (the panel UI still escapes it and opens it with
//! `target=_blank rel="noopener noreferrer"`).

use crate::panel_notifications::NotificationLink;
use crate::releases_source::normalize_repo;

const GITHUB_BASE: &str = "https://github.com";

/// True for a 7 to 40 character hex git object name.
pub fn is_hex_sha(raw: &str) -> bool {
    let s = raw.trim();
    (7..=40).contains(&s.len()) && s.chars().all(|c| c.is_ascii_hexdigit())
}

/// Extract a SHA from a manifest tag such as `stable@42aa3bc` or a bare SHA.
pub fn sha_from_tag(tag: &str) -> Option<String> {
    let tail = tag.trim().rsplit('@').next().unwrap_or("").trim();
    if is_hex_sha(tail) {
        Some(tail.to_ascii_lowercase())
    } else {
        None
    }
}

fn clean_repo(repo: &str) -> Option<String> {
    let trimmed = repo.trim();
    if trimmed.is_empty() {
        return None;
    }
    let repo = normalize_repo(trimmed).ok()?;
    // `owner/..` style segments are valid characters but never a real repo.
    if repo.split('/').any(|seg| seg.chars().all(|c| c == '.')) {
        return None;
    }
    Some(repo)
}

/// `https://github.com/<owner>/<repo>/commit/<sha>`
pub fn commit_url(repo: &str, sha: &str) -> Option<String> {
    let repo = clean_repo(repo)?;
    if !is_hex_sha(sha) {
        return None;
    }
    Some(format!(
        "{GITHUB_BASE}/{repo}/commit/{}",
        sha.trim().to_ascii_lowercase()
    ))
}

/// `https://github.com/<owner>/<repo>/compare/<previous>...<new>`
pub fn compare_url(repo: &str, previous_sha: &str, new_sha: &str) -> Option<String> {
    let repo = clean_repo(repo)?;
    if !is_hex_sha(previous_sha) || !is_hex_sha(new_sha) {
        return None;
    }
    let prev = previous_sha.trim().to_ascii_lowercase();
    let new = new_sha.trim().to_ascii_lowercase();
    if new.starts_with(&prev) || prev.starts_with(&new) {
        return None;
    }
    Some(format!("{GITHUB_BASE}/{repo}/compare/{prev}...{new}"))
}

/// `https://github.com/<owner>/<repo>/releases/tag/<tag>`
pub fn release_url(repo: &str, tag: &str) -> Option<String> {
    let repo = clean_repo(repo)?;
    let tag = tag.trim();
    let ok = !tag.is_empty()
        && tag.len() <= 100
        && tag
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | '+'));
    if !ok {
        return None;
    }
    Some(format!("{GITHUB_BASE}/{repo}/releases/tag/{tag}"))
}

/// Links for a commit / tip upgrade: the commit (minimum) plus compare when the
/// previous commit is known and differs.
pub fn commit_links(
    repo: &str,
    previous_ref: Option<&str>,
    new_sha: &str,
) -> Vec<NotificationLink> {
    let mut links = Vec::new();
    let short: String = new_sha.trim().chars().take(7).collect();
    if let Some(url) = commit_url(repo, new_sha) {
        links.push(NotificationLink {
            label: format!("View commit {short}"),
            url,
        });
    }
    if let Some(prev) = previous_ref.and_then(sha_from_tag)
        && let Some(url) = compare_url(repo, &prev, new_sha)
    {
        links.push(NotificationLink {
            label: "View changes".into(),
            url,
        });
    }
    links
}

/// Link for a release-package upgrade.
pub fn release_links(repo: &str, tag: &str) -> Vec<NotificationLink> {
    release_url(repo, tag)
        .map(|url| {
            vec![NotificationLink {
                label: format!("View release {}", tag.trim()),
                url,
            }]
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    const OFFICIAL: &str = "Control-Panel-Network/CPN-Control-Panel-Network";
    const SHA: &str = "19f7a9a1b2c3d4e5f60718293a4b5c6d7e8f9012";

    #[test]
    fn commit_url_uses_configured_repo() {
        assert_eq!(
            commit_url(OFFICIAL, SHA).unwrap(),
            format!("https://github.com/{OFFICIAL}/commit/{SHA}")
        );
        assert_eq!(
            commit_url("Acme/Fork", "19F7A9A").unwrap(),
            "https://github.com/Acme/Fork/commit/19f7a9a"
        );
    }

    #[test]
    fn rejects_unsafe_input() {
        assert!(commit_url(OFFICIAL, "zzzzzzz").is_none());
        assert!(commit_url(OFFICIAL, "abc").is_none());
        assert!(commit_url(OFFICIAL, "19f7a9a\"onmouseover=x").is_none());
        assert!(commit_url("../evil", SHA).is_none());
        assert!(commit_url("", SHA).is_none());
        assert!(release_url(OFFICIAL, "v1.4.12\"><script>").is_none());
        assert!(release_url(OFFICIAL, "").is_none());
    }

    #[test]
    fn compare_needs_distinct_commits() {
        assert_eq!(
            compare_url(OFFICIAL, "42aa3bc", "19f7a9a").unwrap(),
            format!("https://github.com/{OFFICIAL}/compare/42aa3bc...19f7a9a")
        );
        assert!(compare_url(OFFICIAL, "19f7a9a", SHA).is_none());
        assert!(compare_url(OFFICIAL, "nothex!", SHA).is_none());
    }

    #[test]
    fn tag_parsing_and_link_sets() {
        assert_eq!(sha_from_tag("stable@42AA3BC").as_deref(), Some("42aa3bc"));
        assert_eq!(sha_from_tag("dev@9a11b4e").as_deref(), Some("9a11b4e"));
        assert!(sha_from_tag("v1.4.12").is_none());
        assert!(sha_from_tag("").is_none());

        let both = commit_links(OFFICIAL, Some("stable@42aa3bc"), SHA);
        assert_eq!(both.len(), 2);
        assert_eq!(both[0].label, "View commit 19f7a9a");
        assert!(both[1].url.contains("/compare/42aa3bc...19f7a9a1"));

        let only_commit = commit_links(OFFICIAL, Some("v1.4.11"), SHA);
        assert_eq!(only_commit.len(), 1);
        assert!(commit_links(OFFICIAL, None, "nope").is_empty());

        let rel = release_links(OFFICIAL, "v1.4.12");
        assert_eq!(rel.len(), 1);
        assert!(rel[0].url.ends_with("/releases/tag/v1.4.12"));
    }

    #[test]
    fn no_em_dashes_in_labels() {
        for link in commit_links(OFFICIAL, Some("stable@42aa3bc"), SHA) {
            assert!(!link.label.contains('\u{2014}'));
        }
    }
}
