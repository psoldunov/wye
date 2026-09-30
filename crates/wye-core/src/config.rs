//! The configuration file ([12-data-model.md](../../../docs/spec/12-data-model.md)).
//!
//! One TOML file under `$XDG_CONFIG_HOME/wye/`, written by the frontends and
//! editable by hand or by a dotfile manager. Every key is optional: a missing
//! key takes the default from the spec's "Value in design" columns. Unknown
//! keys and values that cannot apply are reported as warnings, never as a
//! reason to stop opening links.

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::keys::{Modifier, Modifiers};
use crate::rule::{Rule, RuleError};
use crate::target::Target;

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct Config {
    pub general: General,
    pub browsers: Browsers,
    /// Web app mappings: service ID → target (APP-04). Only non-Default
    /// mappings need to be stored.
    pub apps: BTreeMap<String, Target>,
    pub rules: Vec<Rule>,
    pub picker: PickerSettings,
    pub extras: Extras,
    pub advanced: Advanced,
    pub shortcuts: Shortcuts,
}

/// General page (04-general.md).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct General {
    pub launch_at_login: bool,
    pub tray_icon: TrayIcon,
    pub show_tray_icon: bool,
    /// DEF-07: also handle local HTML files.
    pub open_local_html: bool,
}

impl Default for General {
    fn default() -> Self {
        Self {
            launch_at_login: true,
            tray_icon: TrayIcon::PrimaryBrowser,
            show_tray_icon: true,
            open_local_html: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TrayIcon {
    #[default]
    PrimaryBrowser,
    Wye,
}

/// Browsers page (05-browsers.md).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct Browsers {
    pub primary: Target,
    pub alternative: Target,
    pub alternative_key: Modifiers,
    /// Targets shown in the picker and the tray menu, in order.
    pub shown: Vec<ShownEntry>,
}

impl Default for Browsers {
    fn default() -> Self {
        Self {
            primary: Target::Picker,
            alternative: Target::Picker,
            alternative_key: Modifiers::from_slice(&[Modifier::Shift]),
            shown: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ShownEntry {
    pub target: Target,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hotkey: Option<String>,
}

/// Picker page (07-picker-settings.md) and picker keys (15-keyboard.md).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", default)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "one switch per row on the page"
)]
pub struct PickerSettings {
    pub icon_size: IconSize,
    pub show_names: bool,
    pub show_url: bool,
    pub show_profile_badge: bool,
    pub skip_when_locked: bool,
    pub hotkeys: HotkeyScheme,
    pub keys: PickerKeys,
}

impl Default for PickerSettings {
    fn default() -> Self {
        Self {
            icon_size: IconSize::Large,
            show_names: true,
            show_url: false,
            show_profile_badge: true,
            skip_when_locked: false,
            hotkeys: HotkeyScheme::PerTarget,
            keys: PickerKeys::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum IconSize {
    Small,
    Medium,
    #[default]
    Large,
}

/// KEY-10.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HotkeyScheme {
    #[default]
    PerTarget,
    Numbers,
    Letters,
    Off,
}

/// Picker action keys (KEY-22), stored with XKB key names (KEY-03).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct PickerKeys {
    pub open: Vec<String>,
    pub cancel: Vec<String>,
    pub next: Vec<String>,
    pub previous: Vec<String>,
    pub first: Vec<String>,
    pub last: Vec<String>,
    pub copy_link: Vec<String>,
    pub more: Vec<String>,
    pub create_rule: Vec<String>,
    pub private_modifier: Modifiers,
    pub background_modifier: Modifiers,
    pub new_window_modifier: Modifiers,
}

impl Default for PickerKeys {
    fn default() -> Self {
        let keys = |names: &[&str]| names.iter().map(|&n| n.to_owned()).collect();
        Self {
            open: keys(&["Return", "KP_Enter", "space"]),
            cancel: keys(&["Escape"]),
            next: keys(&["Right", "Tab"]),
            previous: keys(&["Left", "Shift+Tab"]),
            first: keys(&["Home"]),
            last: keys(&["End"]),
            copy_link: keys(&["Ctrl+c"]),
            more: keys(&["Menu"]),
            create_rule: keys(&["Ctrl+r"]),
            private_modifier: Modifiers::from_slice(&[Modifier::Shift]),
            background_modifier: Modifiers::from_slice(&[Modifier::Ctrl]),
            new_window_modifier: Modifiers::from_slice(&[Modifier::Alt]),
        }
    }
}

impl PickerKeys {
    /// Every bound action key, for hotkey conflict checks (KEY-12).
    fn action_keys(&self) -> impl Iterator<Item = &str> {
        [
            &self.open,
            &self.cancel,
            &self.next,
            &self.previous,
            &self.first,
            &self.last,
            &self.copy_link,
            &self.more,
            &self.create_rule,
        ]
        .into_iter()
        .flatten()
        .map(String::as_str)
    }
}

/// Extras page (09-extras.md).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", default)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "one switch per row on the page"
)]
pub struct Extras {
    pub strip_tracking_on_open: bool,
    pub strip_tracking_on_copy: bool,
    pub strip_mailto_on_copy: bool,
    pub force_https: bool,
    pub songlink_on_copy: bool,
}

impl Default for Extras {
    fn default() -> Self {
        Self {
            strip_tracking_on_open: true,
            strip_tracking_on_copy: false,
            strip_mailto_on_copy: false,
            force_https: false,
            songlink_on_copy: false,
        }
    }
}

/// Advanced page (10-advanced.md).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", default)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "one switch per row on the page"
)]
pub struct Advanced {
    pub expand_urls: bool,
    pub expansion: ExpansionSettings,
    /// ADV-03: run the global transform script (`transform.js`).
    pub transform: bool,
    /// ADV-09: keep the last 100 opened links.
    pub history: bool,
    pub force_picker_from_extension: bool,
    pub bypass_key: Modifiers,
}

impl Default for Advanced {
    fn default() -> Self {
        Self {
            expand_urls: true,
            expansion: ExpansionSettings::default(),
            transform: false,
            history: false,
            force_picker_from_extension: true,
            bypass_key: Modifiers::from_slice(&[Modifier::Alt]),
        }
    }
}

/// URL expansion sheet (DLG-EXP). Shipped wrappers and short-link domains are
/// on unless listed in `disabled`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct ExpansionSettings {
    /// Wrapper IDs and short-link domains turned off.
    pub disabled: Vec<String>,
    /// Short-link domains the user added (DLG-EXP-02).
    pub custom_short_links: Vec<String>,
    /// 500 to 5000 ms (DLG-EXP-04).
    pub timeout_ms: u16,
    /// 1 to 10 (DLG-EXP-04).
    pub max_redirects: u8,
    pub notify_on_failure: bool,
}

impl ExpansionSettings {
    pub const TIMEOUT_RANGE: std::ops::RangeInclusive<u16> = 500..=5000;
    pub const REDIRECT_RANGE: std::ops::RangeInclusive<u8> = 1..=10;

    #[must_use]
    pub fn is_enabled(&self, id: &str) -> bool {
        !self.disabled.iter().any(|d| d.eq_ignore_ascii_case(id))
    }
}

impl Default for ExpansionSettings {
    fn default() -> Self {
        Self {
            disabled: Vec::new(),
            custom_short_links: Vec::new(),
            timeout_ms: 1500,
            max_redirects: 5,
            notify_on_failure: false,
        }
    }
}

/// Global shortcuts (ADV-05 to ADV-07), unset by default.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct Shortcuts {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub toggle_menu: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clipboard_primary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clipboard_alternative: Option<String>,
}

/// A problem with the configuration that Wye works around.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigWarning {
    UnknownKey(String),
    /// `browsers.primary` or `browsers.alternative` is `default`, which only
    /// means something in mappings and rules. Treated as the picker.
    DefaultNotAllowed(&'static str),
    /// A shown entry that is the picker or Default; dropped.
    ShownNotConcrete(usize),
    DuplicateHotkey(String),
    HotkeyTakenByAction(String),
    UnknownService(String),
    InvalidRule {
        index: usize,
        name: String,
        errors: Vec<RuleError>,
    },
    DuplicateRuleId(String),
    OutOfRange {
        key: &'static str,
        value: String,
        used: String,
    },
}

impl fmt::Display for ConfigWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownKey(path) => write!(f, "unknown key `{path}` is ignored"),
            Self::DefaultNotAllowed(key) => write!(
                f,
                "`{key}` cannot be `default` (it only applies to web app mappings and rules); using the picker"
            ),
            Self::ShownNotConcrete(i) => write!(
                f,
                "shown browser {} is the picker or Default, which the picker cannot show; it is skipped",
                i + 1
            ),
            Self::DuplicateHotkey(key) => {
                write!(
                    f,
                    "picker hotkey {key:?} is assigned more than once; only the first counts"
                )
            }
            Self::HotkeyTakenByAction(key) => {
                write!(
                    f,
                    "picker hotkey {key:?} is already a picker action key; it is ignored"
                )
            }
            Self::UnknownService(id) => {
                write!(
                    f,
                    "`apps.{id}` does not name a known web app; it is ignored"
                )
            }
            Self::InvalidRule {
                index,
                name,
                errors,
            } => {
                let errors: Vec<_> = errors.iter().map(ToString::to_string).collect();
                write!(
                    f,
                    "rule {} ({name:?}) is skipped: {}",
                    index + 1,
                    errors.join("; ")
                )
            }
            Self::DuplicateRuleId(id) => write!(f, "rule ID {id:?} is used more than once"),
            Self::OutOfRange { key, value, used } => {
                write!(f, "`{key}` = {value} is out of range; using {used}")
            }
        }
    }
}

/// A configuration file that could not be read at all.
#[derive(Debug, thiserror::Error)]
#[error("invalid configuration: {0}")]
pub struct ConfigError(#[from] toml::de::Error);

/// A parsed configuration, already corrected for the problems listed in
/// `warnings`.
#[derive(Debug, Clone)]
pub struct Loaded {
    pub config: Config,
    pub warnings: Vec<ConfigWarning>,
}

impl Config {
    /// Parses a configuration file and corrects what cannot apply.
    ///
    /// `known_services` lists the web app IDs of the catalogue, to flag
    /// mappings for services Wye does not know.
    ///
    /// # Errors
    ///
    /// Returns an error when the text is not valid TOML or a value has the
    /// wrong type.
    pub fn parse(text: &str, known_services: &[&str]) -> Result<Loaded, ConfigError> {
        let mut warnings = Vec::new();
        let deserializer = toml::Deserializer::parse(text)?;
        let config: Self = serde_ignored::deserialize(deserializer, |path| {
            warnings.push(ConfigWarning::UnknownKey(path.to_string()));
        })?;
        let (config, more) = config.sanitized(known_services);
        warnings.extend(more);
        Ok(Loaded { config, warnings })
    }

    /// Serialises the configuration for saving.
    ///
    /// # Errors
    ///
    /// Returns an error only if a value cannot be represented in TOML.
    pub fn to_toml(&self) -> Result<String, toml::ser::Error> {
        toml::to_string(self)
    }

    /// Returns a copy with every value that cannot apply replaced, plus a
    /// warning for each replacement.
    #[must_use]
    pub fn sanitized(&self, known_services: &[&str]) -> (Self, Vec<ConfigWarning>) {
        let mut warnings = Vec::new();
        let browsers = self.browsers.sanitized(&self.picker.keys, &mut warnings);
        let apps = self
            .apps
            .iter()
            .filter(|(id, target)| {
                let known = known_services.contains(&id.as_str());
                if !known {
                    warnings.push(ConfigWarning::UnknownService((*id).clone()));
                }
                known && **target != Target::Default
            })
            .map(|(id, target)| (id.clone(), target.clone()))
            .collect();
        check_rules(&self.rules, &mut warnings);
        let advanced = Advanced {
            expansion: self.advanced.expansion.sanitized(&mut warnings),
            ..self.advanced.clone()
        };
        let config = Self {
            browsers,
            apps,
            advanced,
            ..self.clone()
        };
        (config, warnings)
    }
}

impl Browsers {
    fn sanitized(&self, keys: &PickerKeys, warnings: &mut Vec<ConfigWarning>) -> Self {
        let concrete_or_picker = |target: &Target, key: &'static str, warnings: &mut Vec<_>| {
            if *target == Target::Default {
                warnings.push(ConfigWarning::DefaultNotAllowed(key));
                Target::Picker
            } else {
                target.clone()
            }
        };
        let primary = concrete_or_picker(&self.primary, "browsers.primary", warnings);
        let alternative = concrete_or_picker(&self.alternative, "browsers.alternative", warnings);

        let action_keys: Vec<String> = keys.action_keys().map(str::to_lowercase).collect();
        let mut seen_hotkeys: Vec<String> = Vec::new();
        let shown = self
            .shown
            .iter()
            .enumerate()
            .filter_map(|(i, entry)| {
                if !entry.target.is_concrete() {
                    warnings.push(ConfigWarning::ShownNotConcrete(i));
                    return None;
                }
                let hotkey = entry.hotkey.as_ref().and_then(|key| {
                    let lower = key.to_lowercase();
                    if action_keys.contains(&lower) {
                        warnings.push(ConfigWarning::HotkeyTakenByAction(key.clone()));
                        None
                    } else if seen_hotkeys.contains(&lower) {
                        warnings.push(ConfigWarning::DuplicateHotkey(key.clone()));
                        None
                    } else {
                        seen_hotkeys.push(lower);
                        Some(key.clone())
                    }
                });
                Some(ShownEntry {
                    target: entry.target.clone(),
                    hotkey,
                })
            })
            .collect();
        Self {
            primary,
            alternative,
            alternative_key: self.alternative_key,
            shown,
        }
    }
}

impl ExpansionSettings {
    fn sanitized(&self, warnings: &mut Vec<ConfigWarning>) -> Self {
        let timeout_ms = clamp_setting(
            "advanced.expansion.timeout-ms",
            self.timeout_ms,
            &Self::TIMEOUT_RANGE,
            warnings,
        );
        let max_redirects = clamp_setting(
            "advanced.expansion.max-redirects",
            self.max_redirects,
            &Self::REDIRECT_RANGE,
            warnings,
        );
        Self {
            timeout_ms,
            max_redirects,
            ..self.clone()
        }
    }
}

fn clamp_setting<T: Copy + Ord + fmt::Display>(
    key: &'static str,
    value: T,
    range: &std::ops::RangeInclusive<T>,
    warnings: &mut Vec<ConfigWarning>,
) -> T {
    let used = value.clamp(*range.start(), *range.end());
    if used != value {
        warnings.push(ConfigWarning::OutOfRange {
            key,
            value: value.to_string(),
            used: used.to_string(),
        });
    }
    used
}

fn check_rules(rules: &[Rule], warnings: &mut Vec<ConfigWarning>) {
    let mut ids: Vec<&str> = Vec::new();
    for (index, rule) in rules.iter().enumerate() {
        if let Err(errors) = rule.compile() {
            warnings.push(ConfigWarning::InvalidRule {
                index,
                name: rule.name.clone(),
                errors,
            });
        }
        if let Some(id) = rule.id.as_deref() {
            if ids.contains(&id) {
                warnings.push(ConfigWarning::DuplicateRuleId(id.to_owned()));
            }
            ids.push(id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::matcher::MatcherKind;
    use crate::rule::RunPosition;
    use crate::source::SourceAppSpec;
    use crate::target::DesktopId;

    const SERVICES: &[&str] = &["google-meet", "discord"];

    /// The illustrative configuration in 12-data-model.md.
    const SPEC_EXAMPLE: &str = r#"
[browsers]
primary = { picker = true }
alternative = { app = "firefox.desktop" }
alternative-key = ["Shift"]

[[browsers.shown]]
target = { app = "app.zen_browser.zen.desktop" }
hotkey = "a"

[[browsers.shown]]
target = { profile = { app = "google-chrome.desktop", id = "Profile 1" } }
hotkey = "c"

[apps]
google-meet = { profile = { app = "google-chrome.desktop", id = "Profile 1" } }

[[rules]]
name = "GitHub in Firefox"
target = { app = "firefox.desktop" }
url-matchers = [{ kind = "domain", pattern = "github.com" }]
source-apps = ["com.slack.Slack.desktop"]
run = "before"

[picker]
icon-size = "large"
hotkeys = "per-target"

[picker.keys]
open = ["Return", "KP_Enter", "space"]
cancel = ["Escape"]
private-modifier = ["Shift"]
"#;

    fn id(s: &str) -> DesktopId {
        DesktopId::new(s).unwrap()
    }

    #[test]
    fn parses_the_spec_example() {
        let loaded = Config::parse(SPEC_EXAMPLE, SERVICES).unwrap();
        assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
        let config = loaded.config;
        assert_eq!(
            config.browsers.alternative,
            Target::App(id("firefox.desktop"))
        );
        assert_eq!(config.browsers.shown.len(), 2);
        assert_eq!(config.browsers.shown[1].hotkey.as_deref(), Some("c"));
        assert!(matches!(config.apps["google-meet"], Target::Profile { .. }));
        let rule = &config.rules[0];
        assert_eq!(rule.url_matchers[0].kind, MatcherKind::Domain);
        assert_eq!(
            rule.source_apps,
            [SourceAppSpec::Desktop(id("com.slack.Slack.desktop"))]
        );
        assert_eq!(rule.run, RunPosition::Before);
        assert!(rule.enabled);
        // Keys not in the file keep their defaults.
        assert_eq!(config.picker.keys.next, ["Right", "Tab"]);
        assert!(config.extras.strip_tracking_on_open);
    }

    #[test]
    fn empty_file_is_all_defaults() {
        let loaded = Config::parse("", SERVICES).unwrap();
        assert!(loaded.warnings.is_empty());
        let config = loaded.config;
        assert_eq!(config, Config::default());
        assert_eq!(config.browsers.primary, Target::Picker);
        assert_eq!(config.browsers.alternative_key.to_string(), "Shift");
        assert_eq!(config.advanced.bypass_key.to_string(), "Alt");
        assert!(config.advanced.expand_urls);
        assert!(config.advanced.force_picker_from_extension);
        assert!(!config.advanced.history);
        assert!(config.general.launch_at_login);
        assert_eq!(config.picker.icon_size, IconSize::Large);
    }

    #[test]
    fn round_trips_through_toml() {
        let config = Config::parse(SPEC_EXAMPLE, SERVICES).unwrap().config;
        let text = config.to_toml().unwrap();
        let again = Config::parse(&text, SERVICES).unwrap();
        assert!(again.warnings.is_empty(), "{:?}\n{text}", again.warnings);
        assert_eq!(again.config, config);
    }

    #[test]
    fn reports_unknown_keys() {
        let loaded =
            Config::parse("[extras]\nforce-http = true\n[nonsense]\na = 1\n", SERVICES).unwrap();
        let unknown: Vec<_> = loaded.warnings.iter().map(ToString::to_string).collect();
        assert!(
            unknown.iter().any(|w| w.contains("extras.force-http")),
            "{unknown:?}"
        );
        assert!(
            unknown.iter().any(|w| w.contains("nonsense")),
            "{unknown:?}"
        );
    }

    #[test]
    fn type_errors_are_errors() {
        assert!(Config::parse("[extras]\nforce-https = \"yes\"\n", SERVICES).is_err());
        assert!(Config::parse("[browsers]\nalternative-key = [\"Hyper\"]\n", SERVICES).is_err());
    }

    #[test]
    fn corrects_values_that_cannot_apply() {
        let text = r#"
[browsers]
primary = { default = true }

[[browsers.shown]]
target = { picker = true }

[[browsers.shown]]
target = { app = "a.desktop" }
hotkey = "x"

[[browsers.shown]]
target = { app = "b.desktop" }
hotkey = "X"

[[browsers.shown]]
target = { app = "c.desktop" }
hotkey = "Escape"

[apps]
discord = { default = true }
myspace = { app = "a.desktop" }

[[rules]]
name = ""

[advanced.expansion]
timeout-ms = 60000
max-redirects = 0
"#;
        let loaded = Config::parse(text, SERVICES).unwrap();
        let config = &loaded.config;
        assert_eq!(config.browsers.primary, Target::Picker);
        assert_eq!(config.browsers.shown.len(), 3);
        assert_eq!(config.browsers.shown[0].hotkey.as_deref(), Some("x"));
        assert_eq!(config.browsers.shown[1].hotkey, None);
        assert_eq!(config.browsers.shown[2].hotkey, None);
        assert!(config.apps.is_empty());
        assert_eq!(config.advanced.expansion.timeout_ms, 5000);
        assert_eq!(config.advanced.expansion.max_redirects, 1);
        // The invalid rule stays in the file; the pipeline skips it.
        assert_eq!(config.rules.len(), 1);
        let kinds: Vec<_> = loaded.warnings.iter().map(std::mem::discriminant).collect();
        assert_eq!(kinds.len(), 8, "{:#?}", loaded.warnings);
    }
}
