//! The cxx-qt bridges: every `QObject` QML can create, and the C++ shim.
//!
//! One file per surface, so each UI unit edits only its own. `build.rs`
//! passes every `*.rs` in this directory except this one to cxx-qt-build,
//! so a new bridge needs only its file and one `mod` line here, with the
//! same `allow` as the others.
//!
//! The `allow` on each `mod` is the only place unsafe code is permitted in
//! the crate (`unsafe_code = "deny"` in `Cargo.toml`): the bridge macros
//! expand to `unsafe` blocks and `unsafe fn` declarations at the spans of
//! the declarations themselves, and cxx-qt rejects attributes on the bridge
//! module (only `doc` and `cxx_qt::bridge` are accepted), so the `mod` line
//! is the narrowest scope that covers them. The generated code also returns
//! boxed sized types, which `clippy::pedantic` flags.
//!
//! Plain Rust logic belongs in the crate's other modules, where it can be
//! tested without Qt; a bridge file only converts types and calls it.

// Infrastructure.
#[allow(
    unsafe_code,
    clippy::unnecessary_box_returns,
    reason = "cxx-qt bridge: the generated FFI code is unsafe and boxes sized types"
)]
pub mod app;
#[allow(
    unsafe_code,
    clippy::unnecessary_box_returns,
    reason = "cxx-qt bridge: the generated FFI code is unsafe and boxes sized types"
)]
pub mod shim;
#[allow(
    unsafe_code,
    clippy::unnecessary_box_returns,
    dead_code,
    reason = "cxx-qt bridge: the generated FFI code is unsafe and boxes sized types; the shim emits activationTokenReady from C++ by name, so Rust never calls its generated emitter"
)]
pub mod window_effects;

// One per surface or sheet.
#[allow(
    unsafe_code,
    clippy::unnecessary_box_returns,
    reason = "cxx-qt bridge: the generated FFI code is unsafe and boxes sized types"
)]
pub mod about;
#[allow(
    unsafe_code,
    clippy::unnecessary_box_returns,
    reason = "cxx-qt bridge: the generated FFI code is unsafe and boxes sized types"
)]
pub mod app_chooser;
#[allow(
    unsafe_code,
    clippy::unnecessary_box_returns,
    reason = "cxx-qt bridge: the generated FFI code is unsafe and boxes sized types"
)]
pub mod history;
#[allow(
    unsafe_code,
    clippy::unnecessary_box_returns,
    reason = "cxx-qt bridge: the generated FFI code is unsafe and boxes sized types"
)]
pub mod onboarding;
#[allow(
    unsafe_code,
    clippy::unnecessary_box_returns,
    reason = "cxx-qt bridge: the generated FFI code is unsafe and boxes sized types"
)]
pub mod picker;
#[allow(
    unsafe_code,
    clippy::unnecessary_box_returns,
    reason = "cxx-qt bridge: the generated FFI code is unsafe and boxes sized types"
)]
pub mod rules;
#[allow(
    unsafe_code,
    clippy::unnecessary_box_returns,
    reason = "cxx-qt bridge: the generated FFI code is unsafe and boxes sized types"
)]
pub mod script_editor;
#[allow(
    unsafe_code,
    clippy::unnecessary_box_returns,
    reason = "cxx-qt bridge: the generated FFI code is unsafe and boxes sized types"
)]
pub mod settings;
#[allow(
    unsafe_code,
    clippy::unnecessary_box_returns,
    reason = "cxx-qt bridge: the generated FFI code is unsafe and boxes sized types"
)]
pub mod tester;
#[allow(
    unsafe_code,
    clippy::unnecessary_box_returns,
    reason = "cxx-qt bridge: the generated FFI code is unsafe and boxes sized types"
)]
pub mod tray_menu;
