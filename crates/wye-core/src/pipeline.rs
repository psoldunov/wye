//! The link pipeline ([11-url-pipeline.md](../../../docs/spec/11-url-pipeline.md),
//! "Processing order").
//!
//! [`Pipeline::resolve`] runs PIPE-02 to PIPE-12 and returns where the link
//! goes, with a trace of every stage that changed the link or decided the
//! target (the rule tester's "Steps", DLG-TST-02). Showing the picker
//! (PIPE-13), launching (PIPE-15) and recording history (PIPE-16) belong to
//! the caller; [`Pipeline::launch_url`] prepares the link for the chosen
//! target.
//!
//! [`Pipeline::resolve_with`] plugs in the hooks that need more than data:
//! network expansion of short links (PIPE-03) and the global transform
//! script (PIPE-05). [`Pipeline::finish`] runs once the picker has answered
//! and applies the matched rule's script (PIPE-14).

use serde::{Deserialize, Serialize};
use url::Url;

use crate::catalogue::ServiceCatalogue;
use crate::clean::{TrackingRules, force_https};
use crate::config::Config;
use crate::expand::ExpansionCatalogue;
use crate::hooks::Hooks;
use crate::keys::Modifiers;
use crate::normalize::MatchUrl;
use crate::rule::{CompiledRule, MatchInput, Rule, RunPosition};
use crate::source::SourceApp;
use crate::target::{Availability, Target};

/// How the link reached Wye (IN-01 to IN-08). The names match the script
/// API's `context.entryPoint`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EntryPoint {
    /// Opened in another app while Wye is the default browser.
    Handler,
    /// Tray item or global shortcut reading the clipboard.
    Clipboard,
    /// The browser extension, over native messaging.
    Extension,
    /// `wye open` and friends.
    Cli,
}

/// A caller's override of the normal decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Force {
    #[default]
    None,
    /// `wye open --pick` (PIPE-11).
    Picker,
    /// IN-04 and `wye open --alternative`: as if the alternative-browser key
    /// were held (PIPE-06).
    Alternative,
}

/// Everything known about an incoming link (PIPE-01).
#[derive(Debug, Clone)]
pub struct LinkRequest {
    pub url: String,
    pub entry: EntryPoint,
    pub source: SourceApp,
    pub held: Modifiers,
    pub screen_locked: bool,
    pub force: Force,
}

impl LinkRequest {
    /// A request with nothing but the link and its entry point.
    #[must_use]
    pub fn new(url: impl Into<String>, entry: EntryPoint) -> Self {
        Self {
            url: url.into(),
            entry,
            source: SourceApp::default(),
            held: Modifiers::NONE,
            screen_locked: false,
            force: Force::None,
        }
    }
}

/// A link Wye refuses (PIPE-02).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Rejected {
    #[error("Wye can't open this link: {0}")]
    Malformed(String),
    #[error("Wye can't open this kind of link ({0}:)")]
    UnsupportedScheme(String),
}

/// Options that change how the target opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OpenOptions {
    /// RUL-22 / LAUNCH-04.
    pub background: bool,
    /// RUL-23 / LAUNCH-05.
    pub new_window: bool,
}

/// What decided the target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    AlternativeKey,
    Rule {
        /// Position in `Config::rules` as loaded, which is the file position
        /// only when no unreadable rule was dropped before it.
        index: usize,
        name: String,
        position: RunPosition,
    },
    Mapping {
        service: String,
    },
    Fallback,
}

/// Why a script did not run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptScope {
    Global,
    Rule,
}

impl ScriptScope {
    /// "global" or "rule", for messages.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Global => "global",
            Self::Rule => "rule",
        }
    }
}

/// One stage that changed the link or decided the target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    Unwrapped {
        wrapper: String,
        url: Url,
    },
    ShortLinkNotExpanded,
    /// One hop of a short link's redirect chain (DLG-EXP-03).
    ShortLinkExpanded {
        url: Url,
    },
    /// The chain ended early; the link continues as far as it got (PIPE-03).
    ShortLinkFailed {
        reason: String,
    },
    TrackingRemoved {
        params: Vec<String>,
        url: Url,
    },
    HttpsForced {
        url: Url,
    },
    ScriptNotRun(ScriptScope),
    /// A transform script changed the link (PIPE-05, PIPE-14).
    Transformed {
        scope: ScriptScope,
        url: Url,
    },
    /// A transform script ran and left the link as it was.
    ScriptUnchanged(ScriptScope),
    /// A transform script failed; the link continues unchanged (SCR-22).
    ScriptFailed {
        scope: ScriptScope,
        message: String,
    },
    AlternativeKey {
        target: Target,
    },
    RuleMatched {
        /// Position in `Config::rules` as loaded; see [`Decision::Rule`].
        index: usize,
        name: String,
        position: RunPosition,
        target: Target,
    },
    MappingMatched {
        service: String,
        target: Target,
    },
    MappingTargetMissing {
        service: String,
        target: Target,
    },
    /// A rule whose target is an app that must not get this link (DEF-08,
    /// DEF-09); the next matching rule is tried.
    RuleSkipped {
        /// Position in `Config::rules` as loaded; see [`Decision::Rule`].
        index: usize,
        name: String,
        position: RunPosition,
        target: Target,
        reason: SkipReason,
    },
    /// A web app mapping whose app must not get this link (DEF-08, DEF-09).
    MappingSkipped {
        service: String,
        target: Target,
        reason: SkipReason,
    },
    Fallback {
        target: Target,
    },
    DefaultIsPrimary {
        target: Target,
    },
    TargetMissing {
        target: Target,
    },
    ForcedPicker {
        by_extension: bool,
    },
    LockedScreen {
        target: Target,
    },
    HeldUntilUnlock,
    /// The user's answer in the picker (PIPE-13).
    PickerChoice {
        target: Target,
    },
}

/// Where a link goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolution {
    pub original: String,
    /// The link after expansion and cleaning.
    pub url: Url,
    /// The picker, or a concrete app that is available.
    pub target: Target,
    pub options: OpenOptions,
    pub decision: Decision,
    pub steps: Vec<Step>,
    /// PKS-07: the screen is locked and the picker must wait for the unlock.
    pub hold_until_unlock: bool,
}

/// A configuration compiled for routing, plus the shipped data it needs.
#[derive(Debug, Clone)]
pub struct Pipeline {
    config: Config,
    rules: Vec<(usize, CompiledRule)>,
    services: ServiceCatalogue,
    expansion: ExpansionCatalogue,
    tracking: TrackingRules,
}

impl Pipeline {
    /// Builds a pipeline over the [`Config::sanitized`] copy of `config`, so
    /// values that cannot apply (a primary browser set to Default, a mapping
    /// for an unknown service, an out-of-range timeout) are corrected here
    /// and routing never ends on [`Target::Default`]. Rules that fail
    /// validation are left out. The configuration loader has already
    /// reported both.
    #[must_use]
    #[allow(
        clippy::needless_pass_by_value,
        reason = "callers hand the configuration over; taking a reference would break them"
    )]
    pub fn new(
        config: Config,
        services: ServiceCatalogue,
        expansion: ExpansionCatalogue,
        tracking: TrackingRules,
    ) -> Self {
        let known: Vec<&str> = services
            .services()
            .iter()
            .map(|service| service.id.as_str())
            .collect();
        let (config, _) = config.sanitized(&known);
        let rules = config
            .rules
            .iter()
            .enumerate()
            .filter_map(|(i, rule)| rule.compile().ok().map(|compiled| (i, compiled)))
            .collect();
        Self {
            config,
            rules,
            services,
            expansion,
            tracking,
        }
    }

    /// A pipeline over the data that ships with Wye.
    #[must_use]
    pub fn with_shipped_data(config: Config) -> Self {
        Self::new(
            config,
            ServiceCatalogue::shipped(),
            ExpansionCatalogue::shipped(),
            TrackingRules::shipped(),
        )
    }

    /// The corrected configuration the pipeline routes with.
    #[must_use]
    pub fn config(&self) -> &Config {
        &self.config
    }

    #[must_use]
    pub fn services(&self) -> &ServiceCatalogue {
        &self.services
    }

    /// Runs PIPE-02 to PIPE-12 without hooks: short links are recognised but
    /// not followed, and scripts are reported as not run.
    ///
    /// # Errors
    ///
    /// Returns [`Rejected`] for links Wye does not handle (PIPE-02).
    pub fn resolve(
        &self,
        request: &LinkRequest,
        apps: &dyn Availability,
    ) -> Result<Resolution, Rejected> {
        self.resolve_with(request, apps, Hooks::none())
    }

    /// Runs PIPE-02 to PIPE-12 with `hooks`: network expansion of short links
    /// (PIPE-03) and the global transform script (PIPE-05). The rule script
    /// (PIPE-14) waits for [`Pipeline::finish`].
    ///
    /// # Errors
    ///
    /// Returns [`Rejected`] for links Wye does not handle (PIPE-02).
    pub fn resolve_with(
        &self,
        request: &LinkRequest,
        apps: &dyn Availability,
        hooks: Hooks<'_>,
    ) -> Result<Resolution, Rejected> {
        let mut steps = Vec::new();
        let mut url = self.validate(&request.url)?;
        // Expansion and cleaning only mean something for web links; the
        // global transform (PIPE-05) also sees local HTML files (DEF-07).
        if is_web(&url) {
            url = self.expand(url, hooks, &mut steps);
            url = self.clean(url, &mut steps);
        }
        url = self.transform_global(url, request, hooks, &mut steps);

        let (target, decision, options) = self.decide(&url, request, apps, hooks, &mut steps);
        let mut resolution = Resolution {
            original: request.url.clone(),
            url,
            target,
            options,
            decision,
            steps,
            hold_until_unlock: false,
        };
        self.force_picker(&mut resolution, request);
        self.skip_picker_when_locked(&mut resolution, request, apps);
        Ok(resolution)
    }

    /// The link to hand to `target`: translated for a service's own desktop
    /// app (LAUNCH-02), unchanged otherwise.
    #[must_use]
    pub fn launch_url(&self, url: &Url, target: &Target) -> String {
        match target {
            Target::App(app) => self
                .services
                .matching(url)
                .find(|service| service.is_own_app(app))
                .map_or_else(|| url.to_string(), |service| service.hand_over.apply(url)),
            _ => url.to_string(),
        }
    }

    // PIPE-02
    fn validate(&self, link: &str) -> Result<Url, Rejected> {
        let url = Url::parse(link.trim()).map_err(|e| Rejected::Malformed(e.to_string()))?;
        match url.scheme() {
            "http" | "https" if url.host().is_some() => Ok(url),
            "http" | "https" => Err(Rejected::Malformed("the link has no host".into())),
            "file" if self.config.general.open_local_html && is_html_file(&url) => Ok(url),
            other => Err(Rejected::UnsupportedScheme(other.to_owned())),
        }
    }

    // PIPE-03: wrappers locally, then a short link over the network, then
    // wrappers again because a short link may land on one.
    fn expand(&self, url: Url, hooks: Hooks<'_>, steps: &mut Vec<Step>) -> Url {
        if !self.config.advanced.expand_urls {
            return url;
        }
        let settings = &self.config.advanced.expansion;
        let url = self.unwrap_wrappers(url, steps);
        if !self.expansion.is_short_link(&url, settings) {
            return url;
        }
        let Some(resolver) = hooks.short_links() else {
            steps.push(Step::ShortLinkNotExpanded);
            return url;
        };
        let expanded = self.expansion.expand_short_link(&url, settings, resolver);
        steps.extend(
            expanded
                .hops
                .iter()
                .map(|hop| Step::ShortLinkExpanded { url: hop.clone() }),
        );
        if let Some(reason) = expanded.stop.problem() {
            steps.push(Step::ShortLinkFailed { reason });
        }
        let url = expanded.last().cloned().unwrap_or(url);
        self.unwrap_wrappers(url, steps)
    }

    fn unwrap_wrappers(&self, url: Url, steps: &mut Vec<Step>) -> Url {
        let unwrapped = self.expansion.unwrap(&url, &self.config.advanced.expansion);
        let url = unwrapped.last().map_or(url, |last| last.url.clone());
        steps.extend(unwrapped.into_iter().map(|u| Step::Unwrapped {
            wrapper: u.wrapper,
            url: u.url,
        }));
        url
    }

    // PIPE-04
    fn clean(&self, mut url: Url, steps: &mut Vec<Step>) -> Url {
        if self.config.extras.strip_tracking_on_open {
            let params = self.tracking.strip(&mut url);
            if !params.is_empty() {
                steps.push(Step::TrackingRemoved {
                    params,
                    url: url.clone(),
                });
            }
        }
        if self.config.extras.force_https && force_https(&mut url) {
            steps.push(Step::HttpsForced { url: url.clone() });
        }
        url
    }

    // PIPE-06 to PIPE-10
    fn decide(
        &self,
        url: &Url,
        request: &LinkRequest,
        apps: &dyn Availability,
        hooks: Hooks<'_>,
        steps: &mut Vec<Step>,
    ) -> (Target, Decision, OpenOptions) {
        let browsers = &self.config.browsers;
        if request.force == Force::Alternative || browsers.alternative_key.matches(request.held) {
            steps.push(Step::AlternativeKey {
                target: browsers.alternative.clone(),
            });
            let target = self.settle(&browsers.alternative, apps, steps);
            return (target, Decision::AlternativeKey, OpenOptions::default());
        }

        // The alternative key, the fallback, the forced picker and the
        // picker's answer are the user's own choice and are never guarded.
        let guard = Guard::new(request, url, apps);
        let match_url = MatchUrl::new(url);
        let input = MatchInput {
            url: &match_url,
            source: &request.source,
            held: request.held,
        };
        if let Some(hit) = self.first_rule(RunPosition::Before, input, &guard, apps, hooks, steps) {
            return hit;
        }
        if let Some(hit) = self.mapping(url, &guard, apps, steps) {
            return hit;
        }
        if let Some(hit) = self.first_rule(RunPosition::After, input, &guard, apps, hooks, steps) {
            return hit;
        }
        steps.push(Step::Fallback {
            target: browsers.primary.clone(),
        });
        let target = self.settle(&browsers.primary, apps, steps);
        (target, Decision::Fallback, OpenOptions::default())
    }

    // PIPE-07 and PIPE-09: first match wins (RUL-03), except that a rule
    // aimed at an app that must not get the link is passed over (DEF-08,
    // DEF-09).
    fn first_rule(
        &self,
        position: RunPosition,
        input: MatchInput<'_>,
        guard: &Guard<'_>,
        apps: &dyn Availability,
        hooks: Hooks<'_>,
        steps: &mut Vec<Step>,
    ) -> Option<(Target, Decision, OpenOptions)> {
        let matching = self
            .rules
            .iter()
            .filter(|(_, compiled)| compiled.rule.run == position)
            .filter(|(_, compiled)| compiled.matches(input));
        for (index, compiled) in matching {
            let rule = &compiled.rule;
            if let Some(reason) = guard.skip(&rule.target) {
                steps.push(Step::RuleSkipped {
                    index: *index,
                    name: rule.name.clone(),
                    position,
                    target: rule.target.clone(),
                    reason,
                });
                continue;
            }
            return Some(self.rule_hit(*index, rule, position, hooks, apps, steps));
        }
        None
    }

    fn rule_hit(
        &self,
        index: usize,
        rule: &Rule,
        position: RunPosition,
        hooks: Hooks<'_>,
        apps: &dyn Availability,
        steps: &mut Vec<Step>,
    ) -> (Target, Decision, OpenOptions) {
        steps.push(Step::RuleMatched {
            index,
            name: rule.name.clone(),
            position,
            target: rule.target.clone(),
        });
        // With a transformer, the rule's script runs in `finish` (PIPE-14).
        if rule.transform && hooks.transformer().is_none() {
            steps.push(Step::ScriptNotRun(ScriptScope::Rule));
        }
        let target = self.settle(&rule.target, apps, steps);
        let decision = Decision::Rule {
            index,
            name: rule.name.clone(),
            position,
        };
        let options = OpenOptions {
            background: rule.open_in_background,
            new_window: rule.force_new_window,
        };
        (target, decision, options)
    }

    // PIPE-08: a mapping left at Default, or whose app is gone (APP-10), does
    // not match; nor does one whose app must not get the link (DEF-08,
    // DEF-09).
    fn mapping(
        &self,
        url: &Url,
        guard: &Guard<'_>,
        apps: &dyn Availability,
        steps: &mut Vec<Step>,
    ) -> Option<(Target, Decision, OpenOptions)> {
        for service in self.services.matching(url) {
            let Some(target) = self.config.apps.get(&service.id) else {
                continue;
            };
            if *target == Target::Default {
                continue;
            }
            if target.is_concrete() && !apps.is_available(target) {
                steps.push(Step::MappingTargetMissing {
                    service: service.name.clone(),
                    target: target.clone(),
                });
                continue;
            }
            if let Some(reason) = guard.skip_mapping(service, target) {
                steps.push(Step::MappingSkipped {
                    service: service.name.clone(),
                    target: target.clone(),
                    reason,
                });
                continue;
            }
            steps.push(Step::MappingMatched {
                service: service.name.clone(),
                target: target.clone(),
            });
            let decision = Decision::Mapping {
                service: service.id.clone(),
            };
            return Some((target.clone(), decision, OpenOptions::default()));
        }
        None
    }

    /// Turns Default into the primary browser (PIPE-10) and an unavailable
    /// app into the picker (12-data-model.md, "Target").
    fn settle(&self, target: &Target, apps: &dyn Availability, steps: &mut Vec<Step>) -> Target {
        let target = if *target == Target::Default {
            let primary = self.config.browsers.primary.clone();
            steps.push(Step::DefaultIsPrimary {
                target: primary.clone(),
            });
            primary
        } else {
            target.clone()
        };
        if target.is_concrete() && !apps.is_available(&target) {
            steps.push(Step::TargetMissing { target });
            return Target::Picker;
        }
        target
    }

    /// PIPE-11. The alternative-browser key is the user's escape hatch
    /// (PIPE-06; the BRW-02 help text in 19-help-texts.md) and works for
    /// links from the browser extension too, so it beats the extension's forced picker
    /// (ADV-10). An explicit `--pick` still wins.
    fn force_picker(&self, resolution: &mut Resolution, request: &LinkRequest) {
        let by_extension = request.entry == EntryPoint::Extension
            && resolution.decision != Decision::AlternativeKey
            && self.config.advanced.force_picker_from_extension
            && !self.config.advanced.bypass_key.matches(request.held);
        if (by_extension || request.force == Force::Picker) && resolution.target != Target::Picker {
            resolution.steps.push(Step::ForcedPicker { by_extension });
            resolution.target = Target::Picker;
        }
    }

    // PIPE-12 and PKS-07
    fn skip_picker_when_locked(
        &self,
        resolution: &mut Resolution,
        request: &LinkRequest,
        apps: &dyn Availability,
    ) {
        if resolution.target != Target::Picker
            || !request.screen_locked
            || !self.config.picker.skip_when_locked
        {
            return;
        }
        let alternative = self.config.browsers.alternative.clone();
        resolution.steps.push(Step::LockedScreen {
            target: alternative.clone(),
        });
        let target = self.settle(&alternative, apps, &mut resolution.steps);
        if target == Target::Picker {
            resolution.steps.push(Step::HeldUntilUnlock);
            resolution.hold_until_unlock = true;
        }
        resolution.target = target;
    }
}

pub(crate) fn is_web(url: &Url) -> bool {
    matches!(url.scheme(), "http" | "https")
}

fn is_html_file(url: &Url) -> bool {
    let path = url.path().to_ascii_lowercase();
    [".html", ".htm", ".xhtml", ".xht"]
        .iter()
        .any(|ext| path.ends_with(ext))
}

mod finish;
mod guard;
mod step_text;

use guard::Guard;

pub use finish::{Chosen, Finished};
pub use guard::SkipReason;

#[cfg(test)]
mod guard_tests;
#[cfg(test)]
mod hook_tests;
#[cfg(test)]
mod tests;
