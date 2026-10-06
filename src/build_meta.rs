//! Compile-time panel build metadata (git SHA).

use std::path::Path;

/// Marker bytes compiled into the panel ELF so tip-upgrade can verify the SHA.
pub fn build_sha_marker() -> &'static str {
    concat!("CPN_BUILD_SHA=", env!("CPN_GIT_SHA"))
}

pub fn embedded_git_sha() -> &'static str {
    let _keep = build_sha_marker();
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

/// Read `CPN_BUILD_SHA=` from a built `cpn-installer` ELF (or any file).
pub fn sha_embedded_in_binary(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    let needle = b"CPN_BUILD_SHA=";
    let pos = bytes.windows(needle.len()).position(|w| w == needle)?;
    let start = pos + needle.len();
    let rest = bytes.get(start..)?;
    let mut sha = String::new();
    for byte in rest.iter().copied() {
        if byte.is_ascii_hexdigit() {
            sha.push(byte as char);
            if sha.len() >= 40 {
                break;
            }
        } else {
            break;
        }
    }
    if looks_like_git_sha(&sha) {
        Some(sha)
    } else {
        None
    }
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

    #[test]
    fn marker_uses_cpn_build_sha_prefix() {
        assert!(build_sha_marker().starts_with("CPN_BUILD_SHA="));
    }

    #[test]
    fn reads_marker_from_bytes() {
        let dir = std::env::temp_dir();
        let path = dir.join("cpn-sha-marker-probe.bin");
        std::fs::write(
            &path,
            b"pad CPN_BUILD_SHA=c27a4a2aba629238593ae13907a8b5e66bb1f58c\0more",
        )
        .expect("write");
        let got = sha_embedded_in_binary(&path).expect("sha");
        assert!(sha_equal(&got, "c27a4a2aba629238593ae13907a8b5e66bb1f58c"));
        let _ = std::fs::remove_file(&path);
    }
}
