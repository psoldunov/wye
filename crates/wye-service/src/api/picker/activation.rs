//! Whether the configuration can send a link to the picker, so the service
//! starts `wye-ui` ahead of time (decision 2, PICK-25).

use wye_core::{Config, Target};

/// True when any route can end on the Picker: the primary or alternative
/// browser, a web app mapping, an enabled rule, or links from the browser
/// extension forced to the picker (ADV-10).
pub(crate) fn can_end_on_picker(config: &Config) -> bool {
    let is_picker = |target: &Target| *target == Target::Picker;
    is_picker(&config.browsers.primary)
        || is_picker(&config.browsers.alternative)
        || config.apps.values().any(is_picker)
        || config
            .rules
            .iter()
            .any(|rule| rule.enabled && is_picker(&rule.target))
        || config.advanced.force_picker_from_extension
}

#[cfg(test)]
mod tests {
    use wye_core::DesktopId;

    use super::*;

    fn config(text: &str) -> Config {
        Config::parse(text, &[]).expect("valid").config
    }

    fn app(id: &str) -> Target {
        Target::App(DesktopId::new(id).expect("desktop id"))
    }

    #[test]
    fn a_picker_anywhere_on_a_route_counts() {
        let mut plain = Config::default();
        plain.browsers.primary = app("firefox.desktop");
        plain.browsers.alternative = app("firefox.desktop");
        plain.advanced.force_picker_from_extension = false;
        assert!(!can_end_on_picker(&plain));

        let mut primary = plain.clone();
        primary.browsers.primary = Target::Picker;
        assert!(can_end_on_picker(&primary));

        let mut forced = plain.clone();
        forced.advanced.force_picker_from_extension = true;
        assert!(can_end_on_picker(&forced));
    }

    #[test]
    fn only_enabled_rules_count() {
        let text = "[browsers]\nprimary = { app = \"firefox.desktop\" }\n\
                    alternative = { app = \"firefox.desktop\" }\n\
                    [advanced]\nforce-picker-from-extension = false\n\
                    [[rules]]\nname = \"r\"\nenabled = false\ntarget = { picker = true }\n";
        let mut config = config(text);
        assert!(!can_end_on_picker(&config), "{config:?}");
        if let Some(rule) = config.rules.first_mut() {
            rule.enabled = true;
        }
        assert!(can_end_on_picker(&config));
    }
}
