//! Everything the pipeline calls out to for one link, built in one place:
//! network expansion of short links (PIPE-03) and transform scripts
//! (PIPE-05, PIPE-14).

use wye_core::{Config, Hooks};

use crate::api::expansion::{self, Resolver};
use crate::api::scripts::{self, Scripts};
use crate::context::ServiceContext;

/// The hooks' owners for one link; [`LinkHooks::hooks`] lends them to the
/// pipeline. Built before the blocking pipeline call, moved into it.
#[derive(Debug)]
pub(crate) struct LinkHooks {
    short_links: Option<Resolver>,
    scripts: Option<Scripts>,
}

impl LinkHooks {
    /// The hooks for a link routed with `config`; the short-link deadline
    /// starts now (DLG-EXP-04). Script failures notify once (SCR-22).
    pub(crate) fn new(ctx: &ServiceContext, config: &Config) -> Self {
        Self {
            short_links: Some(expansion::resolver(ctx, config)),
            scripts: scripts::transformer(ctx),
        }
    }

    /// The hooks for `TestLink` (IN-08): as [`LinkHooks::new`], but a
    /// failing script is only traced, never notified.
    pub(crate) fn trial(ctx: &ServiceContext, config: &Config) -> Self {
        Self {
            scripts: scripts::trial_transformer(ctx),
            ..Self::new(ctx, config)
        }
    }

    /// Only the scripts, for finishing a link after the picker (PIPE-14);
    /// expansion ran before the picker was shown.
    pub(crate) fn scripts_only(ctx: &ServiceContext) -> Self {
        Self {
            short_links: None,
            scripts: scripts::transformer(ctx),
        }
    }

    /// Without network access (`TestLink` with `skip-network`, IN-08); the
    /// other hooks stay.
    pub(crate) fn without_network(mut self) -> Self {
        self.short_links = None;
        self
    }

    /// The hooks to hand to `Pipeline::resolve_with` and `finish`.
    pub(crate) fn hooks(&self) -> Hooks<'_> {
        let hooks = match &self.short_links {
            Some(resolver) => Hooks::none().with_short_links(resolver),
            None => Hooks::none(),
        };
        match &self.scripts {
            Some(scripts) => hooks.with_transformer(scripts),
            None => hooks,
        }
    }
}
