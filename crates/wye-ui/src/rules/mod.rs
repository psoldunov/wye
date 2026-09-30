//! The Rules page, the rule editor and the rule tester without Qt
//! (08-rules.md, 17-dialogs.md "Rule tester"). The bridges
//! `bridge/rules.rs` and `bridge/tester.rs` convert types and call the
//! service; the page saves through `SettingsBackend.applyPatch`.

pub mod draft;
pub mod files;
pub mod list;
pub mod ops;
pub mod tester;
