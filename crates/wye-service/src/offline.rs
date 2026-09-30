//! Routing a link without the service: what `wye open` uses when no service
//! answers (IN-01, IN-07), so the fallback routes as the service would.
//!
//! The same hooks as the service's link path: short-link expansion over
//! HTTP (PIPE-03) and the transform scripts next to `config.toml` (PIPE-05,
//! PIPE-14). What needs the session service stays out: held modifiers, the
//! lock state, the picker, and script-failure notifications (SCR-22, logged
//! instead).

use std::path::Path;
use std::sync::Arc;

use wye_core::{Config, Hooks};

use crate::api::expansion;
use crate::api::scripts::{self, Scripts};
use crate::platform::http::{Resolver, UreqClient};

/// The hooks for one link routed without the service.
pub struct OfflineHooks {
    short_links: Resolver,
    scripts: Scripts,
}

impl std::fmt::Debug for OfflineHooks {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("OfflineHooks")
            .field("short_links", &self.short_links)
            .finish_non_exhaustive()
    }
}

impl OfflineHooks {
    /// The hooks for a link routed with `config`, read from `config_file`;
    /// the short-link deadline starts now (DLG-EXP-04).
    #[must_use]
    pub fn new(config_file: &Path, config: &Config) -> Self {
        Self {
            short_links: Resolver::new(Arc::new(UreqClient::new()), expansion::timeout(config)),
            scripts: scripts::offline_transformer(config_file),
        }
    }

    /// The hooks to hand to `Pipeline::resolve_with` and `finish`.
    #[must_use]
    pub fn hooks(&self) -> Hooks<'_> {
        Hooks::none()
            .with_short_links(&self.short_links)
            .with_transformer(&self.scripts)
    }
}
