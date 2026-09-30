//! Rules: `TestLink`, `ExportRules`, `ImportRules` (IN-08, DLG-TST, RUL-02).

mod request;
mod trace;
pub(crate) mod transfer;

use wye_api::{Error, json};

use super::{Caller, Dict, Result};
use crate::context::{ServiceContext, blocking};
use crate::platform::Platform;

/// State this topic keeps: none; rules live in the configuration.
#[derive(Debug, Default)]
pub struct State;

impl State {
    pub(crate) fn new(_platform: &Platform) -> Self {
        Self
    }
}

/// `dev.soldunov.wye1.TestLink` (IN-08): JSON [`wye_api::trace::LinkTrace`].
/// Routes with the configuration in use; nothing is opened or recorded.
pub async fn test_link(
    ctx: &ServiceContext,
    _caller: &Caller,
    url: &str,
    context: &Dict,
) -> Result<String> {
    let request = request::from_context(url, context)?;
    let skip_network = request::skip_network(context)?;
    let snapshot = super::config::snapshot(ctx).await?;
    let config = super::config::current(ctx).await?;
    let locale = ctx.environment()?.xdg.locale.clone();
    // PIPE-03 as for a real link, unless the tester asked not to.
    // Scripts run as for a real link but never notify (SCR-22).
    let hooks = super::link::hooks::LinkHooks::trial(ctx, &config.config);
    let hooks = if skip_network {
        hooks.without_network()
    } else {
        hooks
    };
    let traced = blocking(move || {
        let catalog = super::inventory::targets::catalog(
            &snapshot.inventory,
            &locale,
            &config.config,
            config.pipeline.services(),
        );
        trace::trace(&snapshot, &catalog, &request, hooks.hooks())
    })
    .await?;
    json::encode(&traced)
}

/// `dev.soldunov.wye1.ExportRules` (RUL-02): TOML with rules and scripts.
pub async fn export_rules(ctx: &ServiceContext) -> Result<String> {
    let current = super::config::current(ctx).await?;
    let scripts = transfer::scripts_dir(&current.environment.config);
    let rules = current.config.rules.clone();
    blocking(move || transfer::export(&rules, &scripts)).await?
}

/// `dev.soldunov.wye1.ImportRules` (RUL-02): appends; returns how many.
pub async fn import_rules(ctx: &ServiceContext, text: &str) -> Result<u32> {
    let imported = transfer::read(text)?;
    for skipped in &imported.skipped {
        tracing::info!(position = skipped.position, name = ?skipped.name, "rule not imported");
    }
    let current = super::config::current(ctx).await?;
    // Refuse before writing any script file.
    super::config::check_writable(&current)?;
    let scripts = transfer::scripts_dir(&current.environment.config);
    let existing = current.config.rules.clone();
    let added = blocking(move || transfer::place(imported.rules, &existing, &scripts)).await??;
    let count = u32::try_from(added.len()).unwrap_or(u32::MAX);
    let rules: Vec<_> = current.config.rules.iter().cloned().chain(added).collect();
    let rules = serde_json::to_value(&rules)
        .map_err(|error| Error::failed(format!("cannot encode the rules: {error}")))?;
    let patch = serde_json::json!({ "rules": rules });
    super::config::apply_patch(ctx, &patch, current.revision).await?;
    Ok(count)
}
