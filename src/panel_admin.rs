//! Panel admin identity helpers (bootstrap account).

use crate::account::load_bootstrap;

fn names_equal(a: &str, b: &str) -> bool {
    a.trim().eq_ignore_ascii_case(b.trim())
}

/// True when `username` matches the panel bootstrap (primary admin) account.
pub fn is_panel_admin(username: &str) -> bool {
    match load_bootstrap() {
        Some(boot) => names_equal(&boot.username, username),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;

    #[test]
    fn admin_false_without_bootstrap() {
        with_test_data_dir(|| {
            assert!(!is_panel_admin("admin"));
        });
    }
}
