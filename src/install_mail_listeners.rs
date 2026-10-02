//! Postfix master.cf submission / SMTPS listener helpers (System Repair + Email heal).

use std::path::Path;
use std::process::{Command as StdCommand, Stdio};

/// True when `master.cf` has an active (uncommented) line containing `needle`.
/// Vendor templates often ship `#submission inet ...` comments; those must not count.
pub fn master_cf_has_active(raw: &str, needle: &str) -> bool {
    raw.lines().any(|line| {
        let t = line.trim_start();
        !t.is_empty() && !t.starts_with('#') && t.contains(needle)
    })
}

/// True when submission (587) is already enabled for local clients.
pub fn master_cf_has_submission(raw: &str) -> bool {
    master_cf_has_active(raw, "127.0.0.1:587")
        || master_cf_has_active(raw, "127.0.0.1:submission")
        || raw.lines().any(|line| {
            let t = line.trim_start();
            !t.starts_with('#')
                && (t.starts_with("submission ")
                    || t.starts_with("submission\t")
                    || t.starts_with("submission inet"))
        })
}

/// True when SMTPS / submissions (465) is already enabled for local clients.
pub fn master_cf_has_smtps(raw: &str) -> bool {
    master_cf_has_active(raw, "127.0.0.1:465")
        || master_cf_has_active(raw, "127.0.0.1:submissions")
        || master_cf_has_active(raw, "127.0.0.1:smtps")
        || raw.lines().any(|line| {
            let t = line.trim_start();
            !t.starts_with('#')
                && (t.starts_with("submissions ")
                    || t.starts_with("submissions\t")
                    || t.starts_with("submissions inet")
                    || t.starts_with("smtps ")
                    || t.starts_with("smtps\t")
                    || t.starts_with("smtps inet"))
        })
}

pub fn ensure_postfix_tls_material() {
    let cert = Path::new("/etc/pki/tls/certs/cpn-postfix.pem");
    let key = Path::new("/etc/pki/tls/private/cpn-postfix.key");
    if !cert.is_file() || !key.is_file() {
        let _ = std::fs::create_dir_all("/etc/pki/tls/certs");
        let _ = std::fs::create_dir_all("/etc/pki/tls/private");
        let _ = StdCommand::new("openssl")
            .args([
                "req",
                "-x509",
                "-nodes",
                "-newkey",
                "rsa:2048",
                "-days",
                "825",
                "-subj",
                "/CN=cpn-postfix-local",
                "-keyout",
                key.to_str()
                    .unwrap_or("/etc/pki/tls/private/cpn-postfix.key"),
                "-out",
                cert.to_str()
                    .unwrap_or("/etc/pki/tls/certs/cpn-postfix.pem"),
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(cert, std::fs::Permissions::from_mode(0o644));
            let _ = std::fs::set_permissions(key, std::fs::Permissions::from_mode(0o640));
            let _ = StdCommand::new("chgrp")
                .args(["postfix", key.to_str().unwrap_or("")])
                .status();
        }
    }
    if cert.is_file() && key.is_file() {
        let _ = StdCommand::new("postconf")
            .args(["-e", &format!("smtpd_tls_cert_file={}", cert.display())])
            .status();
        let _ = StdCommand::new("postconf")
            .args(["-e", &format!("smtpd_tls_key_file={}", key.display())])
            .status();
    }
}

/// Drop a previously appended CPN listener block so it can be rewritten correctly.
///
/// Rust `\ ` string continuations historically stripped leading spaces on `-o`
/// lines, which Postfix rejects as `bad field count`. Re-heal must replace that.
pub fn strip_cpn_mail_listener_block(raw: &str) -> String {
    const MARKER: &str = "# CPN local mail listeners";
    if let Some(idx) = raw.find(MARKER) {
        return format!("{}\n", raw[..idx].trim_end());
    }
    const LEGACY: &str = "# CPN local submission";
    if let Some(idx) = raw.find(LEGACY) {
        return format!("{}\n", raw[..idx].trim_end());
    }
    raw.to_string()
}

/// True when a prior CPN listener block used unindented `-o` lines (Postfix bad field count).
fn cpn_block_has_bad_indent(raw: &str) -> bool {
    let idx = raw
        .find("# CPN local mail listeners")
        .or_else(|| raw.find("# CPN local submission"));
    let Some(idx) = idx else {
        return false;
    };
    raw[idx..].lines().any(|line| {
        let trimmed = line.trim_start();
        trimmed.starts_with("-o ") && line == trimmed
    })
}

/// Append CPN loopback submission (587) and SMTPS (465) when vendor comments left them off.
pub fn append_cpn_mail_listeners(raw: &str) -> Option<String> {
    let has_587 = master_cf_has_submission(raw);
    let has_465 = master_cf_has_smtps(raw);
    let broken = cpn_block_has_bad_indent(raw);
    // Idempotent: both listeners present and CPN block (if any) is well-formed.
    if has_587 && has_465 && !broken {
        return None;
    }

    let base = strip_cpn_mail_listener_block(raw);
    let need_587 = !master_cf_has_submission(&base);
    let need_465 = !master_cf_has_smtps(&base);
    if !need_587 && !need_465 {
        // Stripped a broken/duplicate CPN block; vendor (or other) listeners remain.
        return if base == raw { None } else { Some(base) };
    }
    let mut extra = String::from("\n# CPN local mail listeners (System Repair / Email heal)\n");
    if need_587 {
        // Continuation lines must keep leading spaces (do not split with `\`).
        extra.push_str(
            "127.0.0.1:587 inet n - n - - smtpd\n  -o syslog_name=postfix/submission\n  -o smtpd_tls_security_level=may\n  -o smtpd_sasl_auth_enable=yes\n  -o smtpd_relay_restrictions=permit_sasl_authenticated,reject\n",
        );
    }
    if need_465 {
        extra.push_str(
            "127.0.0.1:465 inet n - n - - smtpd\n  -o syslog_name=postfix/smtps\n  -o smtpd_tls_wrappermode=yes\n  -o smtpd_sasl_auth_enable=yes\n  -o smtpd_relay_restrictions=permit_sasl_authenticated,reject\n",
        );
    }
    Some(format!("{base}{extra}"))
}

#[cfg(test)]
mod tests {
    use super::{append_cpn_mail_listeners, master_cf_has_smtps, master_cf_has_submission};

    #[test]
    fn commented_vendor_submission_does_not_count() {
        let raw = "\
#127.0.0.1:submission inet n -   n       -       -       smtpd
#submission inet n       -       n       -       -       smtpd
#127.0.0.1:submissions inet n  -       n       -       -       smtpd
#submissions     inet  n       -       n       -       -       smtpd
";
        assert!(!master_cf_has_submission(raw));
        assert!(!master_cf_has_smtps(raw));
        let updated = append_cpn_mail_listeners(raw).expect("should append");
        assert!(updated.contains("127.0.0.1:587"));
        assert!(updated.contains("127.0.0.1:465"));
        assert!(master_cf_has_submission(&updated));
        assert!(master_cf_has_smtps(&updated));
        assert!(append_cpn_mail_listeners(&updated).is_none());
    }

    #[test]
    fn appended_listeners_keep_master_cf_continuation_indent() {
        let raw = "#submission inet n - n - - smtpd\n";
        let updated = append_cpn_mail_listeners(raw).expect("append");
        assert!(
            updated.contains("\n  -o syslog_name=postfix/submission\n"),
            "continuation lines must start with spaces; got:\n{updated}"
        );
        assert!(updated.contains("\n  -o syslog_name=postfix/smtps\n"));
    }

    #[test]
    fn broken_unindented_o_lines_are_rewritten() {
        let raw = "\
# CPN local mail listeners (System Repair / Email heal)
127.0.0.1:587 inet n - n - - smtpd
-o syslog_name=postfix/submission
127.0.0.1:465 inet n - n - - smtpd
-o syslog_name=postfix/smtps
";
        assert!(master_cf_has_submission(raw));
        assert!(master_cf_has_smtps(raw));
        let updated = append_cpn_mail_listeners(raw).expect("rewrite broken indent");
        assert!(updated.contains("\n  -o syslog_name=postfix/submission\n"));
        assert!(
            !updated
                .lines()
                .any(|l| l == "-o syslog_name=postfix/submission")
        );
        assert!(append_cpn_mail_listeners(&updated).is_none());
    }
}
