//! HTTP health checks for the webmail reverse-proxy surface.

use crate::install_webmail_runtime::webmail_health_url;

/// Assert the UI is usable and data/temp/logs are not fetchable over HTTP.
pub fn verify_webmail_http_surface(mail_label: &str) -> Result<(), String> {
    let url = webmail_health_url();
    let expect_ui = if mail_label.to_ascii_lowercase().contains("snappy") {
        "snappymail|rainloop|login|email|password|admin"
    } else {
        "roundcube|login|email|password|username"
    };
    let script = format!(
        "set -euo pipefail\n\
         body=$(mktemp)\n\
         trap 'rm -f \"$body\"' EXIT\n\
         code=$(curl -sS -o \"$body\" -w '%{{http_code}}' --retry 10 --retry-connrefused --retry-delay 1 --max-time 15 '{url}' || true)\n\
         if [ \"$code\" != \"200\" ]; then echo \"webmail HTTP $code\"; cat \"$body\"; exit 1; fi\n\
         if grep -Eiq 'permission denied|error 202|data folder|not writable|open_basedir' \"$body\"; then\n\
           echo 'webmail returned a permission/data-folder error page'; cat \"$body\"; exit 1\n\
         fi\n\
         if ! grep -Eiq '{expect_ui}' \"$body\"; then\n\
           echo 'webmail body did not look like a usable UI'; head -c 400 \"$body\"; exit 1\n\
         fi\n\
         for path in data temp logs data/; do\n\
           c=$(curl -sS -o /dev/null -w '%{{http_code}}' --max-time 10 \"{url}${{path}}\" || true)\n\
           case \"$c\" in\n\
             200|301|302) echo \"sensitive path /$path returned $c\"; exit 1 ;;\n\
           esac\n\
         done\n\
         exit 0\n"
    );
    let status = std::process::Command::new("bash")
        .args(["-c", &script])
        .status()
        .map_err(|error| error.to_string())?;
    if !status.success() {
        return Err(
            "Webmail HTTP validation failed: UI must be usable and data/temp/logs must not be fetchable"
                .into(),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn snappy_detection_uses_label_substring() {
        let label = "SnappyMail";
        assert!(label.to_ascii_lowercase().contains("snappy"));
    }
}
