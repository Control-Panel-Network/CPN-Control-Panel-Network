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
    let cert = Path::new("/var/lib/cpn/ssl/postfix/cert.pem");
    let key = Path::new("/var/lib/cpn/ssl/postfix/key.pem");
    if !cert.is_file() || !key.is_file() {
        let _ = std::fs::create_dir_all("/var/lib/cpn/ssl/postfix");
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
                key.to_str().unwrap_or("/var/lib/cpn/ssl/postfix/key.pem"),
                "-out",
                cert.to_str().unwrap_or("/var/lib/cpn/ssl/postfix/cert.pem"),
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(key, std::fs::Permissions::from_mode(0o600));
        }
    }
    if cert.is_file() && key.is_file() {
        let _ = StdCommand::new("postconf")
            .args([
                "-e",
                &format!("smtpd_tls_cert_file={}", cert.display()),
            ])
            .status();
        let _ = StdCommand::new("postconf")
            .args(["-e", &format!("smtpd_tls_key_file={}", key.display())])
            .status();
    }
}

/// Append CPN loopback submission (587) and SMTPS (465) when vendor comments left them off.
pub fn append_cpn_mail_listeners(raw: &str) -> Option<String> {
    let need_587 = !master_cf_has_submission(raw);
    let need_465 = !master_cf_has_smtps(raw);
    if !need_587 && !need_465 {
        return None;
    }
    let mut extra = String::from("\n# CPN local mail listeners (System Repair / Email heal)\n");
    if need_587 {
        extra.push_str(
            "127.0.0.1:587 inet n - n - - smtpd\n\
  -o syslog_name=postfix/submission\n\
  -o smtpd_tls_security_level=may\n\
  -o smtpd_sasl_auth_enable=yes\n\
  -o smtpd_relay_restrictions=permit_sasl_authenticated,reject\n",
        );
    }
    if need_465 {
        extra.push_str(
            "127.0.0.1:465 inet n - n - - smtpd\n\
  -o syslog_name=postfix/smtps\n\
  -o smtpd_tls_wrappermode=yes\n\
  -o smtpd_sasl_auth_enable=yes\n\
  -o smtpd_relay_restrictions=permit_sasl_authenticated,reject\n",
        );
    }
    Some(format!("{raw}{extra}"))
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
    fn active_submission_skips_duplicate() {
        let raw = "127.0.0.1:587 inet n - n - - smtpd\n  -o syslog_name=postfix/submission\n";
        assert!(master_cf_has_submission(raw));
        let with_both = append_cpn_mail_listeners(raw).expect("still need 465");
        assert!(with_both.contains("127.0.0.1:465"));
        assert!(append_cpn_mail_listeners(&with_both).is_none());
    }
}
