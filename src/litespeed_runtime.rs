//! Cross-distribution OpenLiteSpeed runtime account helpers.

use crate::install_recipes::{CommandSpec, command};

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

#[cfg(test)]
mod tests {
    use super::ownership_command;

    #[test]
    fn ownership_uses_the_runtime_users_primary_group() {
        let command = ownership_command();
        assert_eq!(command.program, "bash");
        assert!(command.args.join(" ").contains("id -gn"));
        assert!(!command.args.join(" ").contains("nobody:nobody"));
    }
}
