//! Compile-time panel build metadata (git SHA).

use std::path::Path;

/// Marker bytes compiled into the panel ELF so commit-upgrade can verify the SHA.
/// `#[used]` keeps the prefix visible to `strings` under thin LTO.
/// `unsafe(no_mangle)` is required on current rustc (bare `#[no_mangle]` is denied).
#[used]
#[unsafe(no_mangle)]
pub static CPN_BUILD_SHA_MARKER: &[u8] =
    concat!("CPN_BUILD_SHA=", env!("CPN_GIT_SHA"), "\0").as_bytes();

pub fn build_sha_marker() -> &'static str {
    concat!("CPN_BUILD_SHA=", env!("CPN_GIT_SHA"))
}

pub fn embedded_git_sha() -> &'static str {
    let _keep = std::hint::black_box(CPN_BUILD_SHA_MARKER);
    let from_env = option_env!("CPN_GIT_SHA").unwrap_or("").trim();
    if looks_like_git_sha(from_env) {
        return from_env;
    }
    option_env!("CPN_BUILD_SHA").unwrap_or("").trim()
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
    parse_marker_from_bytes(&bytes)
}

fn parse_marker_from_bytes(bytes: &[u8]) -> Option<String> {
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

/// After a source cargo build of `expected_sha`, stamp the marker when LTO dropped it
/// or the extracted tree predates the keep-static. Does not rewrite a mismatched SHA.
pub fn stamp_sha_marker_if_missing(path: &Path, expected_sha: &str) -> Result<(), String> {
    let expected = expected_sha.trim();
    if !looks_like_git_sha(expected) {
        return Err("Cannot stamp CPN_BUILD_SHA: expected value is not a git SHA".into());
    }
    match sha_embedded_in_binary(path) {
        Some(found) if sha_equal(&found, expected) => Ok(()),
        Some(found) => Err(format!(
            "Commit build SHA {} does not match requested {}. Rebuild required.",
            short_sha(&found),
            short_sha(expected)
        )),
        None => {
            let mut bytes = std::fs::read(path)
                .map_err(|error| format!("Could not read built installer: {error}"))?;
            bytes.push(0);
            bytes.extend_from_slice(b"CPN_BUILD_SHA=");
            bytes.extend_from_slice(expected.as_bytes());
            bytes.push(0);
            std::fs::write(path, bytes)
                .map_err(|error| format!("Could not stamp CPN_BUILD_SHA marker: {error}"))?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755));
            }
            if sha_embedded_in_binary(path)
                .filter(|got| sha_equal(got, expected))
                .is_none()
            {
                return Err(
                    "Stamped CPN_BUILD_SHA marker was not readable back from the binary".into(),
                );
            }
            Ok(())
        }
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
        assert!(CPN_BUILD_SHA_MARKER.starts_with(b"CPN_BUILD_SHA="));
        let _ = std::hint::black_box(CPN_BUILD_SHA_MARKER);
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

    #[test]
    fn stamps_missing_marker_without_clobbering_mismatch() {
        let dir = std::env::temp_dir();
        let path = dir.join("cpn-sha-stamp-probe.bin");
        std::fs::write(&path, b"elf-placeholder").expect("write");
        stamp_sha_marker_if_missing(&path, "c27a4a2aba629238593ae13907a8b5e66bb1f58c")
            .expect("stamp");
        let got = sha_embedded_in_binary(&path).expect("sha");
        assert!(sha_equal(&got, "c27a4a2aba629238593ae13907a8b5e66bb1f58c"));
        let err = stamp_sha_marker_if_missing(&path, "bbbbbbb000000000000000000000000000000000")
            .expect_err("mismatch");
        assert!(err.contains("does not match"));
        let _ = std::fs::remove_file(&path);
    }
}
