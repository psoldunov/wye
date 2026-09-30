//! Wye's D-Bus contract, shared by the service and every client.
//!
//! The contract itself is documented in `docs/dbus-api.md`. This crate holds
//! what both sides compile against:
//!
//! - [`names`]: bus names, object paths and interface names.
//! - [`Error`]: the D-Bus errors the service returns.
//! - [`context`] and [`actions`]: the keys and string values that travel in
//!   `a{sv}` dictionaries and `s` arguments.
//! - The JSON payload types, one module per topic ([`tray`], [`status`],
//!   [`targets`], [`apps`], [`services`], [`expansion`], [`picker`],
//!   [`trace`], [`history`], [`shortcuts`], [`scripts`]). Structured data
//!   travels as JSON in an `s` value with camelCase keys (decision 9).
//! - [`proxy`]: zbus proxies for every interface.
//!
//! Requirement IDs such as `DEF-04` refer to the specification in `docs/spec/`.

#[macro_use]
mod wire;

pub mod actions;
pub mod apps;
pub mod common;
pub mod context;
pub mod error;
pub mod expansion;
pub mod history;
pub mod json;
pub mod names;
pub mod picker;
pub mod proxy;
pub mod scripts;
pub mod services;
pub mod shortcuts;
pub mod status;
pub mod targets;
pub mod trace;
pub mod tray;

pub use common::{AppRef, Badge, Packaging, TargetCapabilities, TargetSpec};
pub use error::Error;
pub use wire::UnknownValue;
