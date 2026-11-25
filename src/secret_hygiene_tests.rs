use super::*;
use std::collections::HashSet;

/// Known-bad values are assembled at run time so this file never holds a usable secret.
fn known_bad_samples() -> Vec<String> {
    vec![
        String::new(),
        "short".into(),
        ["pass", "word"].concat().repeat(5),
        ["change", "me"].concat().repeat(6),
        ["Secret", "Secret", "Secret", "Secret", "Secret"].concat(),
        ["admin", "123"].concat().repeat(6),
        "a".repeat(40),
        "ab".repeat(24),
        "0123".repeat(10),
        "x1y2".repeat(10),
        ["Cpn", "Lab", "2026", "!"].concat().repeat(4),
        ["By", "YouCanChange", "Me"].concat() + &"z9".repeat(10),
        ["mra", "18c3f2a9b1d"].concat(),
    ]
}

#[test]
fn weak_and_default_values_are_rejected() {
    for sample in known_bad_samples() {
        assert!(
            weak_reason(&sample, MIN_SECRET_LEN).is_some(),
            "sample of length {} must be weak",
            sample.len()
        );
    }
}

#[test]
fn generated_secrets_are_strong_unique_and_never_default() {
    let mut seen = HashSet::new();
    for _ in 0..400 {
        for secret in [
            strong_alnum(32),
            strong_alnum(48),
            strong_hex(32),
            strong_hex(16),
        ] {
            assert!(weak_reason(&secret, MIN_SECRET_LEN).is_none());
            assert!(secret.len() >= MIN_SECRET_LEN);
            let lower = secret.to_ascii_lowercase();
            for marker in KNOWN_BAD_MARKERS.iter().filter(|m| m.len() >= 6) {
                assert!(!lower.contains(marker));
            }
            assert!(seen.insert(secret), "generator repeated a value");
        }
    }
}

#[test]
fn generators_raise_short_requests_to_the_minimum() {
    assert_eq!(strong_alnum(4).len(), MIN_SECRET_LEN);
    assert_eq!(strong_hex(2).len(), MIN_SECRET_LEN);
}

#[test]
fn key_bytes_check_length_and_diversity() {
    assert_eq!(weak_key_bytes(&[], 32), Some(WeakReason::Empty));
    assert_eq!(weak_key_bytes(&[7u8; 32], 32), Some(WeakReason::Repeating));
    assert_eq!(weak_key_bytes(&[1u8; 16], 32), Some(WeakReason::TooShort));
    let varied: Vec<u8> = (0..32u8)
        .map(|i| i.wrapping_mul(37).wrapping_add(11))
        .collect();
    assert_eq!(weak_key_bytes(&varied, 32), None);
}

#[test]
fn entropy_estimate_orders_inputs() {
    assert_eq!(entropy_bits_per_symbol(b""), 0.0);
    assert!(entropy_bits_per_symbol(b"aaaaaaaa") < 0.1);
    assert!(entropy_bits_per_symbol(strong_hex(32).as_bytes()) > 3.0);
}

#[test]
fn write_private_replaces_content_atomically_with_mode_600() {
    let dir = std::env::temp_dir().join(format!("cpn-secret-hygiene-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let path = dir.join("nested").join("value.secret");
    write_private(&path, b"first").unwrap();
    write_private(&path, b"second").unwrap();
    assert_eq!(fs::read(&path).unwrap(), b"second");
    assert!(!path.with_extension("tmp-secret").exists());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    let _ = fs::remove_dir_all(&dir);
}
