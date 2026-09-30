//! Wye's session service: the `dev.soldunov.wye` D-Bus service that `wye
//! service` runs.
//!
//! One process per session owns the bus name, keeps the configuration,
//! history and inventory, routes every link, brokers the picker with the UI
//! host and serves the tray. The contract is `docs/dbus-api.md`; its types
//! and names are in `wye-api`.
//!
//! - [`run`]: connect, serve, take the name, wait for shutdown.
//! - [`bus`]: the interface impls; they only delegate to [`api`].
//! - [`api`]: what each member does, one module per topic.
//! - [`context`]: the shared state handed to every handler.
//! - [`platform`]: the desktop session behind traits, with no-op and fake
//!   implementations.
//! - [`watch`]: file watchers.
//!
//! Requirement IDs such as `DEF-04` refer to the specification in
//! `docs/spec/`.

pub(crate) mod api;
pub mod bus;
pub mod context;
pub mod offline;
pub mod platform;
pub mod run;
pub mod watch;

pub use context::ServiceContext;
pub use run::{AlreadyRunning, ServiceOptions, is_already_running};
