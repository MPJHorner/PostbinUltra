//! Postbin Ultra capture engine.
//!
//! The HTTP request capture server, bounded ring-buffer store, and forward
//! proxy that the desktop app embeds. Also surfaces the persisted-config
//! shape (`settings`) and the captured-request data type. This crate is
//! lib-only; the desktop binary lives in the `postbin-ultra-desktop` crate.

pub mod capture;
pub mod request;
pub mod settings;
pub mod store;
pub mod supervisor;
pub mod update;
