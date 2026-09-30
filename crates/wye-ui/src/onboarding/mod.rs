//! The first-run window's logic on the UI side (18-onboarding.md, ONB-01 to
//! ONB-06), kept out of the bridge so it is tested without Qt.
//! `bridge/onboarding.rs` converts types and calls it.
//!
//! - [`flow`]: the steps, and what moving between them asks for.
//! - [`choices`]: the browsers step's menus and checklist, and the patches.
//! - [`desktop`]: what the integration step says on each desktop.
//! - [`view`]: the JSON QML reads.
//! - [`sync`]: reading state and carrying out choices, over D-Bus (tested
//!   with a fake).
//!
//! The self-test feeds the window `settings::fixture::Fixture`, the same
//! stand-in for the service the Settings window uses.

pub mod choices;
pub mod desktop;
pub mod flow;
pub mod sync;
pub mod view;

#[cfg(test)]
mod fixtures;
