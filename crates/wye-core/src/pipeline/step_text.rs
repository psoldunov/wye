//! The text of each [`Step`]: one line in the rule tester's "Steps"
//! (DLG-TST-02) and in `wye test`.
//!
//! The steps that change the link and the steps that decide the target are
//! written by one function each, so neither grows into a single long `match`.

use std::fmt;

use super::{ScriptScope, Step};

impl fmt::Display for Step {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = self.link_text().or_else(|| self.routing_text());
        f.write_str(&text.unwrap_or_default())
    }
}

impl Step {
    /// The steps that expand, clean or transform the link (PIPE-03 to
    /// PIPE-05, PIPE-14).
    fn link_text(&self) -> Option<String> {
        Some(match self {
            Self::Unwrapped { wrapper, url } => format!("Expanded ({wrapper} redirect): {url}"),
            Self::ShortLinkNotExpanded => {
                "Short link not expanded: network expansion is not available yet".to_owned()
            }
            Self::ShortLinkExpanded { url } => format!("Expanded (short link): {url}"),
            Self::ShortLinkFailed { reason } => {
                format!("Short link not fully expanded: {reason}")
            }
            Self::TrackingRemoved { params, url } => {
                format!("Cleaned (removed {}): {url}", params.join(", "))
            }
            Self::HttpsForced { url } => format!("Forced HTTPS: {url}"),
            Self::ScriptNotRun(ScriptScope::Global) => {
                "Global transform script not run: scripts are not supported yet".to_owned()
            }
            Self::ScriptNotRun(ScriptScope::Rule) => {
                "Rule transform script not run: scripts are not supported yet".to_owned()
            }
            Self::Transformed { scope, url } => {
                format!("Transformed ({} script): {url}", scope.label())
            }
            Self::ScriptUnchanged(scope) => {
                format!("Transformed ({} script): unchanged", scope.label())
            }
            Self::ScriptFailed { scope, message } => format!(
                "The {} transform script failed, link unchanged: {message}",
                scope.label()
            ),
            _ => return None,
        })
    }

    /// The steps that decide where the link opens (PIPE-06 to PIPE-13).
    fn routing_text(&self) -> Option<String> {
        Some(match self {
            Self::AlternativeKey { target } => format!("Alternative-browser key: {target}"),
            Self::RuleMatched {
                index,
                name,
                position,
                target,
            } => format!(
                "Matched rule {} {name:?} ({}): {target}",
                index + 1,
                position.label()
            ),
            Self::MappingMatched { service, target } => {
                format!("Matched web app mapping {service:?}: {target}")
            }
            Self::MappingTargetMissing { service, target } => {
                format!("Web app mapping {service:?} skipped: {target} is not installed")
            }
            Self::Fallback { target } => format!("No match, primary browser: {target}"),
            Self::DefaultIsPrimary { target } => {
                format!("Default means the primary browser: {target}")
            }
            Self::TargetMissing { target } => {
                format!("{target} is not available; asking with the picker")
            }
            Self::ForcedPicker { by_extension: true } => {
                "Picker forced for links from the browser extension".to_owned()
            }
            Self::ForcedPicker {
                by_extension: false,
            } => "Picker requested".to_owned(),
            Self::LockedScreen { target } => {
                format!("Screen locked, picker skipped: {target}")
            }
            Self::HeldUntilUnlock => {
                "Screen locked and the alternative browser is the picker: held until unlock"
                    .to_owned()
            }
            Self::PickerChoice { target } => format!("Picker choice: {target}"),
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use url::Url;

    use super::*;
    use crate::rule::RunPosition;
    use crate::target::{DesktopId, Target};

    fn firefox() -> Target {
        Target::App(DesktopId::new("firefox.desktop").unwrap())
    }

    /// Every variant of [`Step`], so a new variant without text is caught
    /// here: `link_text` and `routing_text` together must cover them all.
    fn every_step() -> Vec<Step> {
        let url = Url::parse("https://example.com/").unwrap();
        let target = firefox();
        vec![
            Step::Unwrapped {
                wrapper: "google".into(),
                url: url.clone(),
            },
            Step::ShortLinkNotExpanded,
            Step::ShortLinkExpanded { url: url.clone() },
            Step::ShortLinkFailed {
                reason: "too many redirects".into(),
            },
            Step::TrackingRemoved {
                params: vec!["utm_source".into(), "fbclid".into()],
                url: url.clone(),
            },
            Step::HttpsForced { url: url.clone() },
            Step::ScriptNotRun(ScriptScope::Global),
            Step::ScriptNotRun(ScriptScope::Rule),
            Step::Transformed {
                scope: ScriptScope::Global,
                url,
            },
            Step::ScriptUnchanged(ScriptScope::Rule),
            Step::ScriptFailed {
                scope: ScriptScope::Global,
                message: "boom".into(),
            },
            Step::AlternativeKey {
                target: target.clone(),
            },
            Step::RuleMatched {
                index: 0,
                name: "R".into(),
                position: RunPosition::After,
                target: target.clone(),
            },
            Step::MappingMatched {
                service: "Spotify".into(),
                target: target.clone(),
            },
            Step::MappingTargetMissing {
                service: "Spotify".into(),
                target: target.clone(),
            },
            Step::Fallback {
                target: target.clone(),
            },
            Step::DefaultIsPrimary {
                target: target.clone(),
            },
            Step::TargetMissing {
                target: target.clone(),
            },
            Step::ForcedPicker { by_extension: true },
            Step::ForcedPicker {
                by_extension: false,
            },
            Step::LockedScreen {
                target: target.clone(),
            },
            Step::HeldUntilUnlock,
            Step::PickerChoice { target },
        ]
    }

    #[test]
    fn every_step_has_exactly_one_text() {
        for step in every_step() {
            let in_link = step.link_text().is_some();
            let in_routing = step.routing_text().is_some();
            assert!(
                in_link != in_routing,
                "{step:?} must be in exactly one list"
            );
            assert!(!step.to_string().is_empty(), "{step:?}");
        }
    }

    #[test]
    fn the_texts_name_what_happened() {
        let texts: Vec<String> = every_step().iter().map(ToString::to_string).collect();
        assert_eq!(texts[0], "Expanded (google redirect): https://example.com/");
        assert_eq!(
            texts[4],
            "Cleaned (removed utm_source, fbclid): https://example.com/"
        );
        assert_eq!(
            texts[12],
            "Matched rule 1 \"R\" (after built-in rules): firefox.desktop"
        );
        assert_eq!(
            texts[18],
            "Picker forced for links from the browser extension"
        );
        assert_eq!(texts[19], "Picker requested");
        assert_eq!(texts[22], "Picker choice: firefox.desktop");
    }
}
