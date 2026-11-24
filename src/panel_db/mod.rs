//! MariaDB store for the panel state that used to live only under `/var/lib/cpn`.
//!
//! Databases: `cpn_panel` (namespaced documents, encrypted at the application
//! layer where they hold secrets) and `cpn_mail` (virtual mailbox auth). The
//! only state that stays on disk is the bootstrap set listed in
//! [`manifest::BOOTSTRAP_FILES`], because the panel needs it to reach MariaDB.
//!
//! Migration is copy-only and idempotent; see [`state::ensure`].

pub mod backup;
pub mod client;
pub mod config;
pub mod crypto;
pub mod docs_store;
pub mod import;
pub mod mail;
pub mod mail_ops;
pub mod manifest;
pub mod migrate;
pub mod provision;
pub mod sqlfmt;
pub mod state;
