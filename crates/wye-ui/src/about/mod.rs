//! The About window's data on the UI side (17-dialogs.md, DLG-ABT-01 and
//! DLG-ABT-02), kept out of the bridge so it is tested without Qt.
//! `bridge/about.rs` converts types and calls it.
//!
//! - [`info`]: the project facts `FormCard.AboutPage` shows.
//! - [`fixture`]: the self-test's stand-in for the service.
//! - [`link`]: opening a link through the service, for this window and the first-run window.

pub mod fixture;
pub mod info;
pub mod link;
