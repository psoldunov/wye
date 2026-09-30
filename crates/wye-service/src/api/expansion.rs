//! Network short-link expansion (PIPE-03, ADV-01, DLG-EXP-02 to
//! DLG-EXP-04): the resolver the pipeline's hook calls, and the
//! notification when a link could not be expanded.
//!
//! The core owns the redirect loop and what may be contacted
//! (`ExpansionCatalogue::expand_short_link`); this module answers one hop at
//! a time over [`crate::platform::HttpClient`]: `HEAD`, falling back to a
//! body-less `GET` when the server refuses `HEAD`, all within one deadline
//! for the whole chain (DLG-EXP-04 "Timeout"). A link that runs out of time
//! continues unexpanded (PIPE-03).

use std::time::Duration;

use wye_core::Config;
use wye_core::pipeline::Step;

use crate::context::ServiceContext;
use crate::platform::Notification;
pub use crate::platform::http::Resolver;

/// The resolver for one link routed with `config`: its deadline is
/// `advanced.expansion.timeout-ms` from now (DLG-EXP-04).
#[must_use]
pub fn resolver(ctx: &ServiceContext, config: &Config) -> Resolver {
    Resolver::new(ctx.platform().http.clone(), timeout(config))
}

/// The whole-chain deadline of short-link expansion (DLG-EXP-04), within
/// the allowed range.
#[must_use]
pub(crate) fn timeout(config: &Config) -> Duration {
    let timeout = config.advanced.expansion.timeout_ms.clamp(
        *wye_core::config::ExpansionSettings::TIMEOUT_RANGE.start(),
        *wye_core::config::ExpansionSettings::TIMEOUT_RANGE.end(),
    );
    Duration::from_millis(u64::from(timeout))
}

/// DLG-EXP-04: when `advanced.expansion.notify-on-failure` (`enabled`) is on and a short
/// link could not be expanded fully, say so once for this link. The link
/// itself continues (PIPE-03).
pub async fn notify_failures(ctx: &ServiceContext, enabled: bool, steps: &[Step]) {
    if !enabled {
        return;
    }
    let Some(reason) = steps.iter().find_map(|step| match step {
        Step::ShortLinkFailed { reason } => Some(reason.clone()),
        _ => None,
    }) else {
        return;
    };
    let notification = Notification {
        summary: "Wye could not expand a short link".to_owned(),
        body: format!("The link opens as far as it got: {reason}."),
        ..Notification::default()
    };
    if let Err(error) = ctx.platform().notifier.notify(&notification).await {
        tracing::warn!(%error, "cannot show the expansion notification");
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::platform::fake::FakeHttp;

    #[test]
    fn pipe03_a_timed_out_short_link_opens_unexpanded() {
        let resolver = Resolver::new(Arc::new(FakeHttp::default()), Duration::ZERO);
        let pipeline = wye_core::Pipeline::with_shipped_data(Config::default());
        let request =
            wye_core::LinkRequest::new("https://bit.ly/x", wye_core::pipeline::EntryPoint::Cli);
        let apps = wye_desktop::Inventory::from_apps(Vec::new(), Vec::new());
        let resolution = pipeline
            .resolve_with(&request, &apps, resolver.hooks())
            .expect("resolved");
        assert_eq!(resolution.url.as_str(), "https://bit.ly/x");
        assert!(
            resolution
                .steps
                .iter()
                .any(|step| matches!(step, Step::ShortLinkFailed { .. })),
            "{:?}",
            resolution.steps
        );
    }
}
