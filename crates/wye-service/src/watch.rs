//! File watchers: the configuration directory (reload, SET-06), `mimeapps.list`
//! and `kdeglobals` (DEF-03), application directories and browser profile
//! files (DISC-02).
//!
//! Planned with notify 8.2, debounced (100 ms for the configuration, 500 ms
//! for the inventory). Until then nothing is watched.

use tokio::task::JoinHandle;

use crate::context::ServiceContext;

/// Start every watcher; the service aborts the handles on shutdown.
#[must_use]
pub fn spawn(_ctx: &ServiceContext) -> Vec<JoinHandle<()>> {
    Vec::new()
}
