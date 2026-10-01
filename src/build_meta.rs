//! Compile-time panel build metadata (git SHA).

pub fn embedded_git_sha() -> &'static str {
    option_env!("CPN_GIT_SHA").unwrap_or("").trim()
}

pub fn embedded_git_sha_short() -> String {
    short_sha(embedded_git_sha())
}

pub fn short_sha(sha: &str) -> String {
    let trimmed = sha.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    trimmed.chars().take(7).collect()
}

pub fn sha_equal(left: &str, right: &str) -> bool {
    let left = left.trim().to_ascii_lowercase();
    let right = right.trim().to_ascii_lowercase();
    if left.is_empty() || right.is_empty() {
        return false;
    }
    if left == right {
        return true;
    }
    let (short, long) = if left.len() <= right.len() {
        (left.as_str(), right.as_str())
    } else {
        (right.as_str(), left.as_str())
    };
    short.len() >= 7 && long.starts_with(short)
}

pub fn looks_like_git_sha(value: &str) -> bool {
    let trimmed = value.trim();
    let len = trimmed.len();
    (7..=40).contains(&len) && trimmed.chars().all(|ch| ch.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha_equal_accepts_short_prefix() {
        assert!(sha_equal(
            "a7dc8f9",
            "a7dc8f9638b262f7a9bef691da2e6f5ead8186bb"
        ));
        assert!(!sha_equal(
            "a7dc8f9",
            "bbbbbbb000000000000000000000000000000000"
        ));
        assert!(!sha_equal("", "a7dc8f9"));
    }

    #[test]
    fn detects_git_sha_shape() {
        assert!(looks_like_git_sha("a7dc8f9"));
        assert!(looks_like_git_sha(
            "a7dc8f9638b262f7a9bef691da2e6f5ead8186bb"
        ));
        assert!(!looks_like_git_sha("v1.0.0"));
        assert!(!looks_like_git_sha("stable"));
    }
}
