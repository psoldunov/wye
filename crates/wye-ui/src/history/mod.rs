//! The history window's logic on the UI side (17-dialogs.md, DLG-HIS-01 to
//! DLG-HIS-04), kept out of the bridge so it is tested without Qt.
//! `bridge/history.rs` converts types and calls it.
//!
//! - [`filter`]: the search box.
//! - [`view`]: the rows QML draws.
//! - [`sync`]: reloading when the history changes, and the row actions,
//!   over D-Bus (tested with a fake).
//! - [`fixture`]: the self-test's stand-in for the service.

pub mod filter;
pub mod fixture;
pub mod sync;
pub mod view;
