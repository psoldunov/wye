//! Links an app must not receive (DEF-08, DEF-09).
//!
//! A rule or web app mapping may send a link to a desktop app. Two kinds of
//! link would then loop or fail: a link the app itself just handed to Wye,
//! and a sign-in page, which only a browser can complete. [`Guard`] spots
//! both so the pipeline passes over that rule or mapping.

use url::Url;

use super::{EntryPoint, LinkRequest};
use crate::catalogue::ServiceDefinition;
use crate::sign_in;
use crate::source::SourceApp;
use crate::target::{Availability, Target};

/// Why a rule or web app mapping was passed over.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkipReason {
    /// DEF-08: the link came from the target app itself.
    BackToSource,
    /// DEF-09: a sign-in page, which only a browser can complete.
    SignInPage,
}

/// What the pipeline knows about one link that decides whether an app may
/// receive it.
pub(super) struct Guard<'a> {
    /// The app that handed Wye the link; `None` when the link was an
    /// explicit request or the app is unknown.
    source: Option<&'a SourceApp>,
    url: &'a Url,
    apps: &'a dyn Availability,
}

impl<'a> Guard<'a> {
    /// Only a link that arrived as the default browser's handler (IN-01)
    /// came from an app. The extension, the clipboard and the CLI are
    /// explicit requests, so DEF-08 leaves them alone; DEF-09 applies to
    /// every entry point.
    pub(super) fn new(request: &'a LinkRequest, url: &'a Url, apps: &'a dyn Availability) -> Self {
        let source = (request.entry == EntryPoint::Handler && !request.source.is_unknown())
            .then_some(&request.source);
        Self { source, url, apps }
    }

    /// Why a rule must not send the link to `target`, or `None` when it
    /// may. Rules know only the generic sign-in words (DEF-09).
    pub(super) fn skip(&self, target: &Target) -> Option<SkipReason> {
        self.skip_with(target, sign_in::is_sign_in_page(self.url))
    }

    /// Why the mapping of `service` must not send the link to `target`, or
    /// `None` when it may. A mapping also knows the service's own sign-in
    /// routes (DEF-09).
    pub(super) fn skip_mapping(
        &self,
        service: &ServiceDefinition,
        target: &Target,
    ) -> Option<SkipReason> {
        self.skip_with(target, service.is_sign_in(self.url))
    }

    /// Only an app that is not a browser is ever skipped: the picker,
    /// Default, browsers, their profiles and their private windows always
    /// take the link.
    fn skip_with(&self, target: &Target, sign_in: bool) -> Option<SkipReason> {
        if !matches!(target, Target::App(_) | Target::Custom(_)) || self.apps.is_browser(target) {
            return None;
        }
        if self.source.is_some_and(|source| source.is_app(target)) {
            Some(SkipReason::BackToSource)
        } else if sign_in {
            Some(SkipReason::SignInPage)
        } else {
            None
        }
    }
}
