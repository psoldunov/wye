//! Copy-time rewrites (EXT-02, EXT-03, EXT-05, EXT-12, EXT-13, EXT-15):
//! each clipboard change goes through core `decide`, and the result is
//! written back once. Wye's own writes are remembered so they are not
//! rewritten again.

use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use tokio::sync::broadcast::error::RecvError;
use url::Url;
use wye_core::clean::TrackingRules;
use wye_core::clipboard::{
    ClipboardOffer, OwnWrites, Rewrite, RewriteOptions, decide, songlink_api_url, songlink_page,
};

use crate::context::{ServiceContext, blocking};
use crate::platform::{HttpClient, HttpMethod, HttpRequest};

/// How long Songlink may take before the clipboard is left alone (EXT-15).
const SONGLINK_TIMEOUT: Duration = Duration::from_secs(5);

/// What the rewrites remember.
#[derive(Debug, Default)]
pub(crate) struct Rewrites {
    own: Mutex<OwnWrites>,
}

impl Rewrites {
    fn own(&self) -> OwnWrites {
        self.own
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn remember(&self, text: &str) {
        let mut own = self.own.lock().unwrap_or_else(PoisonError::into_inner);
        *own = own.remember(text);
    }
}

/// Watch the clipboard for as long as the service runs; nothing when the
/// session cannot watch it (capability `clipboardWatch` is then `null`).
pub(crate) async fn watch(ctx: ServiceContext) {
    let Some(mut changes) = ctx.platform().clipboard.watch() else {
        tracing::info!("the clipboard cannot be watched; copy-time rewrites are off");
        return;
    };
    let tracking = Arc::new(TrackingRules::shipped());
    loop {
        let text = match changes.recv().await {
            Ok(text) => text,
            Err(RecvError::Lagged(missed)) => {
                tracing::debug!(missed, "missed clipboard changes");
                continue;
            }
            Err(RecvError::Closed) => return,
        };
        if let Err(error) = changed(&ctx, &tracking, text).await {
            tracing::warn!(%error, "cannot rewrite the clipboard");
        }
    }
}

/// One clipboard change: decide and, when something changes, write once.
async fn changed(
    ctx: &ServiceContext,
    tracking: &TrackingRules,
    text: String,
) -> super::Result<()> {
    let config = super::super::config::current(ctx).await?;
    let options = RewriteOptions::from_extras(&config.config.extras);
    if !options.any() {
        return Ok(());
    }
    let offer = ClipboardOffer {
        text: Some(text),
        // The providers pass on plain text only, never secrets (EXT-12).
        rich: false,
        password_manager_hint: None,
    };
    let rewrites = &ctx.clipboard().rewrites;
    let replacement = match decide(&offer, &rewrites.own(), options, tracking) {
        Rewrite::Unchanged => return Ok(()),
        Rewrite::Write(text) => Some(text),
        Rewrite::Songlink { query, fallback } => {
            let http = ctx.platform().http.clone();
            blocking(move || songlink(http.as_ref(), &query))
                .await?
                .or(fallback)
        }
    };
    let Some(replacement) = replacement else {
        return Ok(());
    };
    rewrites.remember(&replacement);
    ctx.platform()
        .clipboard
        .write(&replacement)
        .await
        .map_err(Into::into)
}

/// EXT-15: the song.link page for `link`, or `None` on any failure (the
/// clipboard then stays as it is).
pub(crate) fn songlink(http: &dyn HttpClient, link: &Url) -> Option<String> {
    let request = HttpRequest {
        method: HttpMethod::Get,
        url: songlink_api_url(link).to_string(),
        timeout: SONGLINK_TIMEOUT,
        read_body: true,
    };
    let response = http
        .send(&request)
        .inspect_err(|error| tracing::info!(%error, "Songlink did not answer"))
        .ok()?;
    if response.status != 200 {
        tracing::info!(status = response.status, "Songlink refused the link");
        return None;
    }
    songlink_page(response.body.as_deref()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::HttpResponse;
    use crate::platform::fake::FakeHttp;

    const TRACK: &str = "https://open.spotify.com/track/abc";

    fn api() -> String {
        songlink_api_url(&Url::parse(TRACK).expect("valid")).to_string()
    }

    #[test]
    fn ext15_the_page_replaces_the_link() {
        let http = FakeHttp::default();
        http.respond(
            &api(),
            HttpResponse {
                status: 200,
                location: None,
                body: Some(r#"{"pageUrl": "https://song.link/s/abc"}"#.to_owned()),
            },
        );
        let page = songlink(&http, &Url::parse(TRACK).expect("valid"));
        assert_eq!(page.as_deref(), Some("https://song.link/s/abc"));
        assert!(http.requests()[0].read_body);
    }

    #[test]
    fn ext15_any_failure_leaves_the_clipboard_alone() {
        let http = FakeHttp::default();
        assert_eq!(songlink(&http, &Url::parse(TRACK).expect("valid")), None);
        http.respond(
            &api(),
            HttpResponse {
                status: 429,
                location: None,
                body: Some(r#"{"pageUrl": "https://song.link/s/abc"}"#.to_owned()),
            },
        );
        assert_eq!(songlink(&http, &Url::parse(TRACK).expect("valid")), None);
    }
}
