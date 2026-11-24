use super::*;
use crate::secret_hygiene::{KNOWN_BAD_MARKERS, weak_reason};

fn weak_sample() -> String {
    ["Change", "Me"].concat()
}

fn pma_config(literal: &str) -> String {
    format!(
        "<?php\n$i = 0;\n$cfg['blowfish_secret'] = '{literal}';\n$cfg['PmaAbsoluteUri'] = '/pma/';\n"
    )
}

fn assert_strong(value: &str) {
    assert!(
        weak_reason(value, MIN_SECRET_LEN).is_none(),
        "value of length {} must be strong",
        value.len()
    );
}

#[test]
fn weak_blowfish_literal_is_replaced_and_strong_one_is_kept() {
    let weak = pma_config(&weak_sample());
    let healed = rewrite_blowfish(&weak).expect("weak literal must be rewritten");
    let values = blowfish_literals(&healed);
    assert_eq!(values.len(), 1);
    assert_strong(values[0].as_deref().unwrap());
    assert!(healed.contains("PmaAbsoluteUri"));
    assert!(
        rewrite_blowfish(&healed).is_none(),
        "strong value must be left alone"
    );
}

#[test]
fn empty_blowfish_literal_is_replaced() {
    let healed = rewrite_blowfish(&pma_config("")).unwrap();
    assert_strong(blowfish_literals(&healed)[0].as_deref().unwrap());
}

#[test]
fn missing_blowfish_is_appended_before_closing_tag() {
    let plain = "<?php\n$i = 0;\n";
    let added = rewrite_blowfish(plain).unwrap();
    assert_strong(blowfish_literals(&added)[0].as_deref().unwrap());

    let closed = "<?php\n$i = 0;\n?>\n";
    let added = rewrite_blowfish(closed).unwrap();
    assert!(added.trim_end().ends_with("?>"));
    assert!(added.find("blowfish_secret").unwrap() < added.rfind("?>").unwrap());
}

#[test]
fn php_expressions_and_comments_are_not_touched() {
    let expr = "<?php\n$cfg['blowfish_secret'] = sodium_hex2bin(getenv('X'));\n";
    assert!(rewrite_blowfish(expr).is_none());
    let commented =
        "<?php\n// $cfg['blowfish_secret'] = '';\n$cfg['blowfish_secret'] = getenv('X');\n";
    assert!(rewrite_blowfish(commented).is_none());
}

#[test]
fn both_blowfish_spellings_are_healed() {
    let weak = weak_sample();
    let raw =
        format!("<?php\n$cfg['BlowfishSecret'] = '{weak}';\n$cfg['blowfish_secret'] = '{weak}';\n");
    let healed = rewrite_blowfish(&raw).unwrap();
    for value in blowfish_literals(&healed) {
        assert_strong(value.as_deref().unwrap());
    }
}

#[test]
fn fresh_phpmyadmin_config_has_a_random_non_default_blowfish() {
    let dir = std::env::temp_dir().join(format!("cpn-pma-fresh-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let mut seen = Vec::new();
    for round in 0..8 {
        let path = dir.join(format!("config-{round}.inc.php"));
        crate::apps_phpmyadmin_sso::ensure_config_includes_signon(&path, "CPNPmaSignon").unwrap();
        let raw = fs::read_to_string(&path).unwrap();
        for value in blowfish_literals(&raw) {
            let value = value.expect("fresh config uses a literal");
            assert_strong(&value);
            seen.push(value);
        }
        assert!(rewrite_blowfish(&raw).is_none());
    }
    seen.sort();
    seen.dedup();
    assert!(seen.len() >= 8, "every fresh config needs its own secret");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn legacy_clock_derived_access_password_is_detected_and_rewritten() {
    let legacy = ["mra", "18c3f2a9b1d4e7"].concat();
    assert!(is_legacy_time_derived_password(&legacy));
    assert!(!is_legacy_time_derived_password(&strong_alnum(32)));
    let body = format!(
        "<?php\nreturn [\n    'access_password' => '{legacy}',\n    'domain' => 'x',\n];\n"
    );
    let (updated, fresh) = rewrite_legacy_access_password(&body).unwrap();
    assert_strong(&fresh);
    assert!(!updated.contains(&legacy));
    assert!(updated.contains(&fresh));
    assert!(rewrite_legacy_access_password(&updated).is_none());
}

#[test]
fn webmail_bridge_token_is_random_kept_when_strong_and_rotated_when_weak() {
    let dir = std::env::temp_dir().join(format!("cpn-bridge-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let path = dir.join("admin-bridge.json");
    let url = "http://127.0.0.1:2086";
    assert!(crate::tachyon_admin_bridge_install::write_config_if_needed(&path, url).unwrap());
    let first = json_string(&path, "token").unwrap();
    assert_strong(&first);
    assert!(first.len() >= 64);
    assert!(!crate::tachyon_admin_bridge_install::write_config_if_needed(&path, url).unwrap());
    assert_eq!(json_string(&path, "token").unwrap(), first);

    let weak = weak_sample().repeat(6);
    let body = format!("{{\"version\":1,\"url\":\"{url}\",\"token\":\"{weak}\"}}");
    fs::write(&path, body).unwrap();
    assert!(crate::tachyon_admin_bridge_install::write_config_if_needed(&path, url).unwrap());
    let rotated = json_string(&path, "token").unwrap();
    assert_strong(&rotated);
    assert_ne!(rotated, weak);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn reports_never_contain_secret_values() {
    let value = strong_hex(32);
    let path = std::env::temp_dir().join("cpn-secret-report-unused");
    let report = SecretReport::text("sample", &value, MIN_SECRET_LEN, &path, 0o600);
    assert_eq!(report.len, value.len());
    assert!(report.weak.is_none());
    let table = render_table(std::slice::from_ref(&report));
    let json = serde_json::to_string(&report).unwrap();
    assert!(!table.contains(&value));
    assert!(!json.contains(&value));
    let weak = SecretReport::text("weak", &weak_sample(), MIN_SECRET_LEN, &path, 0o600);
    assert!(weak.weak.is_some());
}

#[test]
fn marker_list_has_no_empty_entries() {
    assert!(KNOWN_BAD_MARKERS.iter().all(|m| m.len() >= 3));
}
