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

/// XOR-obfuscated `CPN_BUILD_SHA=` so the parser and stamper do not compile a decoy
/// marker prefix into `.rodata`. The only contiguous `CPN_BUILD_SHA=` bytes in a
/// release binary must be the real marker (static above or the EOF stamp); otherwise
/// older panels that only inspect the first occurrence read an empty SHA.
const MARKER_NEEDLE_KEY: u8 = 0x5A;
const MARKER_NEEDLE_OBF: [u8; 14] = [
    b'C' ^ MARKER_NEEDLE_KEY,
    b'P' ^ MARKER_NEEDLE_KEY,
    b'N' ^ MARKER_NEEDLE_KEY,
    b'_' ^ MARKER_NEEDLE_KEY,
    b'B' ^ MARKER_NEEDLE_KEY,
    b'U' ^ MARKER_NEEDLE_KEY,
    b'I' ^ MARKER_NEEDLE_KEY,
    b'L' ^ MARKER_NEEDLE_KEY,
    b'D' ^ MARKER_NEEDLE_KEY,
    b'_' ^ MARKER_NEEDLE_KEY,
    b'S' ^ MARKER_NEEDLE_KEY,
    b'H' ^ MARKER_NEEDLE_KEY,
    b'A' ^ MARKER_NEEDLE_KEY,
    b'=' ^ MARKER_NEEDLE_KEY,
];

/// Runtime-built `CPN_BUILD_SHA=` prefix (never a contiguous literal in the binary).
pub fn marker_needle() -> Vec<u8> {
    let key = std::hint::black_box(MARKER_NEEDLE_KEY);
    MARKER_NEEDLE_OBF.iter().map(|byte| byte ^ key).collect()
}

fn find_from(haystack: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if needle.is_empty() || from >= haystack.len() {
        return None;
    }
    haystack[from..]
        .windows(needle.len())
        .position(|window| window == needle)
        .map(|rel| from + rel)
}

/// Scan every `CPN_BUILD_SHA=` occurrence and return the best SHA candidate.
///
/// A release ELF can contain more than one occurrence of the prefix (for example
/// a format-string piece or a needle literal from older source), and `.rodata`
/// layout decides which one comes first. Older code stopped at the first match,
/// so a decoy prefix followed by non-hex bytes made the real marker unreadable
/// ("Stamped CPN_BUILD_SHA marker was not readable back from the binary").
///
/// Ranking: NUL-terminated 40-hex (the compiled static or the EOF stamp) wins,
/// then any NUL-terminated SHA, then a bare 40-hex run, then a shorter run.
fn parse_marker_from_bytes(bytes: &[u8]) -> Option<String> {
    let needle = marker_needle();
    let mut best: Option<(u8, String)> = None;
    let mut cursor = 0usize;
    while let Some(pos) = find_from(bytes, &needle, cursor) {
        let start = pos + needle.len();
        cursor = start;
        let mut sha = String::new();
        let mut idx = start;
        while idx < bytes.len() && sha.len() < 40 && bytes[idx].is_ascii_hexdigit() {
            sha.push(bytes[idx] as char);
            idx += 1;
        }
        if !looks_like_git_sha(&sha) {
            continue;
        }
        let next = bytes.get(idx).copied();
        // A 40-hex run that continues with more hex digits is not a SHA marker.
        if sha.len() == 40 && next.map(|b| b.is_ascii_hexdigit()).unwrap_or(false) {
            continue;
        }
        let nul_terminated = matches!(next, Some(0) | None);
        let rank = match (nul_terminated, sha.len() == 40) {
            (true, true) => 3,
            (true, false) => 2,
            (false, true) => 1,
            (false, false) => 0,
        };
        let better = match &best {
            Some((best_rank, _)) => rank > *best_rank,
            None => true,
        };
        if better {
            best = Some((rank, sha));
            if rank == 3 {
                break;
            }
        }
    }
    best.map(|(_, sha)| sha)
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
            bytes.extend_from_slice(&marker_needle());
            bytes.extend_from_slice(expected.as_bytes());
            bytes.push(0);
            std::fs::write(path, bytes)
                .map_err(|error| format!("Could not stamp CPN_BUILD_SHA marker: {error}"))?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755));
            }
            match sha_embedded_in_binary(path) {
                Some(got) if sha_equal(&got, expected) => Ok(()),
                Some(got) => Err(format!(
                    "Stamped CPN_BUILD_SHA marker for {} but {} reads back as {}. Retry the commit upgrade; if it repeats, report the panel log.",
                    short_sha(expected),
                    path.display(),
                    short_sha(&got)
                )),
                None => Err(format!(
                    "Stamped CPN_BUILD_SHA marker for {} but it was not readable back from {}. Retry the commit upgrade; if it repeats, report the panel log.",
                    short_sha(expected),
                    path.display()
                )),
            }
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
    fn needle_is_built_at_runtime_and_matches_marker_prefix() {
        assert_eq!(marker_needle(), b"CPN_BUILD_SHA=".to_vec());
    }

    #[test]
    fn skips_decoy_prefix_before_real_marker() {
        // Layout observed on a lab tip build: the inject_keep_module format string
        // piece (`... = b"CPN_BUILD_SHA=` + fmt boundary bytes) sits in .rodata
        // before the real static marker.
        let mut bytes: Vec<u8> = Vec::new();
        bytes.extend_from_slice(b"KEEP: &[u8] = b\"CPN_BUILD_SHA=");
        bytes.extend_from_slice(&[0xC0, 0x05]);
        bytes.extend_from_slice(b"\\0\";\n\0!phpMyAdmin OLS listener ready at ");
        bytes.extend_from_slice(b"nc/mpmc/list.rs\0");
        bytes.extend_from_slice(
            b"CPN_BUILD_SHA=cc78b0e3a434dc426193ca90dd636aa4c44d6dc0\0/rustc/b9",
        );
        let got = parse_marker_from_bytes(&bytes).expect("real marker after decoy");
        assert_eq!(got, "cc78b0e3a434dc426193ca90dd636aa4c44d6dc0");
    }

    #[test]
    fn reads_eof_stamp_when_compiled_marker_is_empty() {
        // Release binary compiled without a SHA (`CPN_BUILD_SHA=\0`) plus a later
        // EOF stamp must resolve to the stamped SHA.
        let mut bytes: Vec<u8> = Vec::new();
        bytes.extend_from_slice(b"mp/diy_float.rs\0CPN_BUILD_SHA=\0\x01v\xC0\0\x0EFailed to run");
        bytes.extend_from_slice(&[0u8; 64]);
        bytes.extend_from_slice(b"CPN_BUILD_SHA=c27a4a2aba629238593ae13907a8b5e66bb1f58c\0");
        let got = parse_marker_from_bytes(&bytes).expect("eof stamp");
        assert_eq!(got, "c27a4a2aba629238593ae13907a8b5e66bb1f58c");
    }

    #[test]
    fn prefers_nul_terminated_full_sha_over_hex_like_noise() {
        // A decoy prefix adjacent to a hex lookup table must not win over the
        // NUL-terminated 40-hex marker that follows it.
        let mut bytes: Vec<u8> = Vec::new();
        bytes.extend_from_slice(b"CPN_BUILD_SHA=0123456789abcdefXYZ");
        bytes.extend_from_slice(b"\0pad\0");
        bytes.extend_from_slice(b"CPN_BUILD_SHA=c27a4a2aba629238593ae13907a8b5e66bb1f58c\0");
        let got = parse_marker_from_bytes(&bytes).expect("marker");
        assert_eq!(got, "c27a4a2aba629238593ae13907a8b5e66bb1f58c");
    }

    #[test]
    fn ignores_hex_runs_longer_than_a_sha() {
        let bytes = b"CPN_BUILD_SHA=c27a4a2aba629238593ae13907a8b5e66bb1f58c0123\0";
        assert!(parse_marker_from_bytes(bytes).is_none());
    }

    #[test]
    fn returns_none_without_any_valid_marker() {
        assert!(parse_marker_from_bytes(b"CPN_BUILD_SHA=\0 and CPN_BUILD_SHA=zz").is_none());
        assert!(parse_marker_from_bytes(b"nothing here").is_none());
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
