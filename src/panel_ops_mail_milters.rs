//! Postfix milter composition for OpenDKIM (+ optional rspamd Auth-Results).

/// OpenDKIM Postfix milter target (`inet:host:port`).
pub const OPENDKIM_POSTFIX_MILTER: &str = "inet:127.0.0.1:8891";
/// Rspamd proxy milter (SPF/DKIM/DMARC Authentication-Results for Tachyon).
pub const RSPAMD_POSTFIX_MILTER: &str = "inet:127.0.0.1:11332";

/// Build smtpd_milters value: OpenDKIM first, then rspamd when requested.
pub fn compose_smtpd_milters(opendkim: &str, include_rspamd: bool) -> String {
    if include_rspamd {
        format!("{opendkim},{RSPAMD_POSTFIX_MILTER}")
    } else {
        opendkim.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smtpd_milters_append_rspamd_when_requested() {
        assert_eq!(
            compose_smtpd_milters(OPENDKIM_POSTFIX_MILTER, false),
            OPENDKIM_POSTFIX_MILTER
        );
        assert_eq!(
            compose_smtpd_milters(OPENDKIM_POSTFIX_MILTER, true),
            format!("{OPENDKIM_POSTFIX_MILTER},{RSPAMD_POSTFIX_MILTER}")
        );
    }
}
