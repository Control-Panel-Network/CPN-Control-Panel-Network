//! Hub route facade (re-exports feature route modules).

pub use crate::panel_dashboard_activity_routes::*;
pub use crate::panel_hub_routes_account::*;
pub use crate::panel_hub_routes_account_security::*;
pub use crate::panel_hub_routes_backups::*;
pub use crate::panel_hub_routes_cloudflare::*;
pub use crate::panel_hub_routes_db::*;
pub use crate::panel_hub_routes_dns::*;
pub use crate::panel_hub_routes_email::*;
pub use crate::panel_hub_routes_email_actions::*;
pub use crate::panel_hub_routes_email_stubs::*;
pub use crate::panel_hub_routes_error_messages::*;
pub use crate::panel_hub_routes_files::*;
pub use crate::panel_hub_routes_firewall::*;
pub use crate::panel_hub_routes_passkeys::*;
pub use crate::panel_hub_routes_php::*;
pub use crate::panel_hub_routes_profile::*;
pub use crate::panel_hub_routes_security::*;
pub use crate::panel_hub_routes_server::*;
pub use crate::panel_hub_routes_settings_logs::*;
pub use crate::panel_hub_routes_sidebar_acl::*;
pub use crate::panel_hub_routes_site_files::*;
pub use crate::panel_hub_routes_ssl_actions::*;

use crate::panel_hub_pages_hosting::{databases_ftp_hub_main, email_hub_main};

pub fn email_hub_html() -> String {
    email_hub_main()
}

pub fn databases_hub_html() -> String {
    databases_ftp_hub_main()
}
