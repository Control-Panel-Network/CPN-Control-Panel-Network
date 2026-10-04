//! Redaction for third-party installer output before it reaches events or disk logs.

const OLS_WEBADMIN_CREDENTIAL_MARKER: &str = "WebAdmin user/password is ";

pub(crate) fn redact_sensitive_line(line: &str) -> String {
    let Some(marker_start) = line.find(OLS_WEBADMIN_CREDENTIAL_MARKER) else {
        return line.to_string();
    };
    let value_start = marker_start + OLS_WEBADMIN_CREDENTIAL_MARKER.len();
    format!("{}[REDACTED]", &line[..value_start])
}

#[cfg(test)]
mod tests {
    use super::redact_sensitive_line;

    #[test]
    fn redacts_vendor_generated_webadmin_credentials() {
        let generated = ["temporary", "credential"].join("-");
        let line = format!("WebAdmin user/password is admin/{generated}");
        let redacted = redact_sensitive_line(&line);
        assert_eq!(redacted, "WebAdmin user/password is [REDACTED]");
        assert!(!redacted.contains(&generated));
    }

    #[test]
    fn preserves_unrelated_package_output() {
        let line = "Setting up openlitespeed (1.9.3-1+resolute) ...";
        assert_eq!(redact_sensitive_line(line), line);
    }
}
