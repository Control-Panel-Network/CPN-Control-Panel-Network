//! Same-host migration from other hosting panels into CPN.

pub mod apply;
pub mod catalog;
pub mod detect;
pub mod scan;
mod scan_tail;
pub mod wipe;

pub use apply::{ApplyOptions, VerifyReport, apply_panel, load_report};
pub use detect::{Detection, detect_at, host_root};
pub use wipe::wipe_panel;
