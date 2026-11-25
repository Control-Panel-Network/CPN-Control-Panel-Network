//! Guard: production code and helper scripts must not carry static secrets.
//!
//! Every secret is created from the OS CSPRNG at install time. This test scans the
//! shipped sources (test modules excluded) for the shapes a static secret takes, so a
//! regression fails CI instead of reaching a release build.

use std::fs;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn collect(dir: &Path, exts: &[&str], out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, exts, out);
        } else if path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| exts.contains(&e))
        {
            out.push(path);
        }
    }
}

fn is_test_file(path: &Path) -> bool {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    name == "tests.rs"
        || name.ends_with("_tests.rs")
        || path.components().any(|c| c.as_os_str() == "tests")
}

/// Production part of a Rust source: everything before the first `#[cfg(test)]`.
fn production_part(raw: &str) -> &str {
    raw.find("#[cfg(test)]").map_or(raw, |idx| &raw[..idx])
}

fn first_string_literal(rest: &str) -> Option<&str> {
    let start = rest.find('"')? + 1;
    let end = start + rest[start..].find('"')?;
    Some(&rest[start..end])
}

const SECRET_WORDS: &[&str] = &[
    "SECRET",
    "PASSWORD",
    "PASSWD",
    "TOKEN",
    "API_KEY",
    "APIKEY",
    "PRIVATE_KEY",
    "BLOWFISH",
    "SIGNING",
];
const BENIGN_WORDS: &[&str] = &[
    "FILE", "PATH", "COOKIE", "PREFIX", "NAME", "URL", "LEN", "HEADER", "ENV", "DIR", "LABEL",
    "KEY_ID", "ROUTE", "FIELD", "PARAM", "STORAGE", "SESSION", "SUFFIX", "MARKER", "TTL", "MAX",
    "MIN", "LABELS",
];

/// `const SOMETHING_SECRET: &str = "literal";` with a non-trivial literal.
fn static_secret_constant(line: &str) -> bool {
    let trimmed = line.trim_start();
    let decl = trimmed
        .strip_prefix("pub const ")
        .or_else(|| trimmed.strip_prefix("pub(crate) const "))
        .or_else(|| trimmed.strip_prefix("const "))
        .or_else(|| trimmed.strip_prefix("static "));
    let Some(decl) = decl else {
        return false;
    };
    let Some((name, rest)) = decl.split_once(':') else {
        return false;
    };
    if !rest.contains("&str") {
        return false;
    }
    let name = name.trim().to_ascii_uppercase();
    let secretish = SECRET_WORDS.iter().any(|w| name.contains(w));
    let benign = BENIGN_WORDS.iter().any(|w| name.contains(w));
    let literal = rest
        .split_once('=')
        .and_then(|(_, v)| first_string_literal(v));
    // Filesystem paths (for example a secret file location) are not secret values.
    secretish && !benign && literal.is_some_and(|l| l.len() >= 6 && !l.starts_with('/'))
}

/// `$cfg['blowfish_secret'] = 'literal'` written by a format string with a fixed value.
fn static_blowfish_literal(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    let Some(idx) = lower.find("blowfish") else {
        return false;
    };
    let after = &line[idx..];
    let Some(eq) = after.find("= '") else {
        return false;
    };
    let value = &after[eq + 3..];
    !value.starts_with('{') && !value.starts_with('\'') && !value.starts_with('"')
}

#[test]
fn production_rust_sources_have_no_static_secret_constants() {
    let mut files = Vec::new();
    collect(&root().join("src"), &["rs"], &mut files);
    assert!(files.len() > 100, "source scan found too few files");
    let mut violations = Vec::new();
    for file in files.iter().filter(|f| !is_test_file(f)) {
        let raw = fs::read_to_string(file).unwrap_or_default();
        for (n, line) in production_part(&raw).lines().enumerate() {
            if static_secret_constant(line) || static_blowfish_literal(line) {
                violations.push(format!(
                    "{}:{}",
                    file.strip_prefix(root()).unwrap().display(),
                    n + 1
                ));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "static secret-looking constants in production code (values not shown): {violations:?}"
    );
}

#[test]
fn production_code_never_derives_secrets_from_the_clock() {
    let mut files = Vec::new();
    collect(&root().join("src"), &["rs"], &mut files);
    let mut violations = Vec::new();
    for file in files.iter().filter(|f| !is_test_file(f)) {
        let raw = fs::read_to_string(file).unwrap_or_default();
        let prod = production_part(&raw);
        let lower = prod.to_ascii_lowercase();
        // The legacy shape was `format!("mra{nanos:x}")`.
        if lower.contains("format!(\"mra{") {
            violations.push(file.strip_prefix(root()).unwrap().display().to_string());
        }
    }
    assert!(
        violations.is_empty(),
        "clock-derived secret generators: {violations:?}"
    );
}

#[test]
fn helper_scripts_read_passwords_from_the_environment() {
    let mut files = Vec::new();
    for dir in ["scripts", "Test", "tests", "docker", "packaging"] {
        collect(&root().join(dir), &["py", "sh", "ps1"], &mut files);
    }
    let mut violations = Vec::new();
    for file in &files {
        let raw = fs::read_to_string(file).unwrap_or_default();
        for (n, line) in raw.lines().enumerate() {
            let t = line.trim();
            let lower = t.to_ascii_lowercase();
            let assigns = ["password", "passwd", "secret", "token", "api_key"]
                .iter()
                .any(|k| {
                    lower.starts_with(&format!("{k} =")) || lower.starts_with(&format!("{k}="))
                });
            if !assigns
                || lower.contains("environ")
                || lower.contains("getenv")
                || lower.contains("$env:")
            {
                continue;
            }
            // A quoted non-empty literal on the right-hand side.
            let rhs = t.split_once('=').map(|(_, v)| v.trim()).unwrap_or("");
            let quoted =
                (rhs.starts_with('"') && rhs.len() > 2) || (rhs.starts_with('\'') && rhs.len() > 2);
            let is_shell_var = rhs.starts_with("\"$") || rhs.starts_with("\"${");
            if quoted && !is_shell_var {
                violations.push(format!(
                    "{}:{}",
                    file.strip_prefix(root()).unwrap().display(),
                    n + 1
                ));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "hard-coded passwords in helper scripts (values not shown): {violations:?}"
    );
}
