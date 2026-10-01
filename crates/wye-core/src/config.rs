//! The configuration file ([12-data-model.md](../../../docs/spec/12-data-model.md)).
//!
//! One TOML file under `$XDG_CONFIG_HOME/wye/`, written by the frontends and
//! editable by hand or by a dotfile manager. Every key is optional: a missing
//! key takes the default from the spec's "Value in design" columns. Unknown
//! keys, values of the wrong type and values that cannot apply are reported
//! as warnings, never as a reason to stop opening links. Only text that is
//! not TOML at all is an error.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::keys::{Modifier, Modifiers};
use crate::rule::Rule;
use crate::target::Target;

mod load;
mod sanitize;
mod warnings;

pub use warnings::ConfigWarning;

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
    /// KEY-06: whether the service probes which modifiers are held.
    pub held_keys: HeldKeys,
    /// ADV-12: which frontend shows the picker, the tray popup and windows.
    pub frontend: Frontend,
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
            held_keys: HeldKeys::Auto,
            frontend: Frontend::Auto,
        }
    }
}

/// `advanced.frontend` (ADV-12): the frontend that shows the picker, the
/// tray-menu popup and the windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Frontend {
    /// GNOME's frontend in a GNOME session, KDE's everywhere else.
    #[default]
    Auto,
    /// `wye-ui` (Qt/Kirigami) on every desktop.
    Kde,
    /// The GNOME Shell extension while it runs, the GTK host otherwise, on
    /// every desktop.
    Gnome,
}

/// `advanced.held-keys` (KEY-06): probe held modifiers when a binding
/// depends on them, or never.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HeldKeys {
    #[default]
    Auto,
    /// Never probe; held-key bindings do not fire.
    Off,
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
    /// 500 to 5000 ms (DLG-EXP-04). Wider than the range, so a value outside
    /// it is clamped with a warning instead of failing to parse.
    pub timeout_ms: u32,
    /// 1 to 10 (DLG-EXP-04). Wider than the range, like `timeout_ms`.
    pub max_redirects: u16,
    pub notify_on_failure: bool,
}

impl ExpansionSettings {
    pub const TIMEOUT_RANGE: std::ops::RangeInclusive<u32> = 500..=5000;
    pub const REDIRECT_RANGE: std::ops::RangeInclusive<u16> = 1..=10;

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

/// A configuration file that could not be read at all.
#[derive(Debug, thiserror::Error)]
#[error("invalid configuration: {0}")]
pub struct ConfigError(#[from] toml::de::Error);

/// A parsed configuration and every problem found in it.
#[derive(Debug, Clone)]
pub struct Loaded {
    /// The configuration as read. Saving it with [`Config::to_toml`] keeps
    /// every value that was read, including values that parse but cannot
    /// apply (they are only reported; [`Pipeline::new`](crate::Pipeline::new)
    /// routes with the [`Config::sanitized`] copy). It does **not** keep
    /// unknown keys, values that could not be read (they come back as their
    /// default), list entries that could not be read, or comments and
    /// formatting. Check [`Loaded::is_lossless`] before writing the file.
    pub config: Config,
    /// Every problem found. A list entry (rule, shown browser) is named by
    /// its place in the file, counted from 0 in key paths and from 1 in
    /// messages, even when unreadable entries before it were dropped.
    pub warnings: Vec<ConfigWarning>,
}

impl Loaded {
    /// Whether saving [`Loaded::config`] would write back every value the
    /// file set. False when a warning shows that a value was dropped: an
    /// unknown key, an unreadable value or entry, or a file that could not be
    /// combined into a configuration. Comments and formatting are never kept,
    /// so true does not mean the saved file equals the loaded one.
    ///
    /// A frontend must not overwrite the file when this is false.
    #[must_use]
    pub fn is_lossless(&self) -> bool {
        !self.warnings.iter().any(|warning| {
            matches!(
                warning,
                ConfigWarning::UnknownKey(_)
                    | ConfigWarning::InvalidValue { .. }
                    | ConfigWarning::InvalidEntry { .. }
                    | ConfigWarning::Unusable(_)
            )
        })
    }
}

impl Config {
    /// Parses a configuration file and reports what cannot apply.
    ///
    /// A value of the wrong type or shape is dropped with a warning and keeps
    /// its default; a rule or shown browser that cannot be read is skipped on
    /// its own. `known_services` lists the web app IDs of the catalogue, to
    /// flag mappings for services Wye does not know.
    ///
    /// # Errors
    ///
    /// Returns an error only when the text is not valid TOML.
    pub fn parse(text: &str, known_services: &[&str]) -> Result<Loaded, ConfigError> {
        let table: toml::Table = text.parse()?;
        let mut warnings = Vec::new();
        let (config, kept) = load::lenient::<Self>(table, &mut warnings);
        let (_, corrections) = config.sanitized(known_services);
        warnings.extend(corrections.into_iter().map(|warning| kept.locate(warning)));
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
}

#[cfg(test)]
mod tests;
