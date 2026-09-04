//! Hub route facade (re-exports feature route modules).

pub use crate::panel_hub_routes_backups::*;
pub use crate::panel_hub_routes_db::*;
pub use crate::panel_hub_routes_email::*;
pub use crate::panel_hub_routes_server::*;

use crate::panel_hub_pages_hosting::{databases_ftp_hub_main, email_hub_main};

pub fn email_hub_html() -> String {
    email_hub_main()
}

pub fn databases_hub_html() -> String {
    databases_ftp_hub_main()
}
