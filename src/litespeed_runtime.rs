//! Cross-distribution OpenLiteSpeed runtime account helpers.

use crate::install_recipes::{CommandSpec, command};
use std::path::Path;

const OWNERSHIP_SCRIPT: &str = "\
owner=nobody; \
group=$(id -gn \"$owner\" 2>/dev/null) || { echo 'OpenLiteSpeed runtime user nobody is missing.' >&2; exit 1; }; \
test -n \"$group\" || { echo 'Could not resolve the primary group for OpenLiteSpeed runtime user nobody.' >&2; exit 1; }; \
chown -R \"$owner:$group\" /var/www/cpn";

pub(crate) fn ownership_command() -> CommandSpec {
    command(
        "bash",
        vec!["-c", OWNERSHIP_SCRIPT],
        "Adjusting permissions for OpenLiteSpeed",
        "installing",
        81,
    )
}

fn unit_file_exists(unit: &str) -> bool {
    [
        "/etc/systemd/system",
        "/usr/lib/systemd/system",
        "/lib/systemd/system",
    ]
    .iter()
    .any(|directory| {
        Path::new(directory)
            .join(format!("{unit}.service"))
            .exists()
    })
}

fn select_systemd_unit(lshttpd_present: bool, lsws_present: bool) -> Option<&'static str> {
    if lshttpd_present {
        Some("lshttpd")
    } else if lsws_present {
        Some("lsws")
    } else {
        None
    }
}

pub(crate) fn detect_systemd_unit() -> Result<&'static str, String> {
    select_systemd_unit(unit_file_exists("lshttpd"), unit_file_exists("lsws"))
        .ok_or_else(|| "OpenLiteSpeed vendor systemd unit not found (lshttpd/lsws)".into())
}

#[cfg(test)]
mod tests {
    use super::{ownership_command, select_systemd_unit};

    #[test]
    fn ownership_uses_the_runtime_users_primary_group() {
        let command = ownership_command();
        assert_eq!(command.program, "bash");
        assert!(command.args.join(" ").contains("id -gn"));
        assert!(!command.args.join(" ").contains("nobody:nobody"));
    }

    #[test]
    fn canonical_lshttpd_unit_wins_over_its_lsws_alias() {
        assert_eq!(select_systemd_unit(true, true), Some("lshttpd"));
        assert_eq!(select_systemd_unit(false, true), Some("lsws"));
        assert_eq!(select_systemd_unit(false, false), None);
    }
}
