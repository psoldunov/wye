//! What the discovery found, as the menus and the picker need it: apps that
//! handle web links, their private-window and profile support, and apps the
//! user added. The service fills it from the desktop entries; the core only
//! reads it, so menus are built without touching the system.

use serde::{Deserialize, Serialize};

use crate::config::{Config, ShownEntry};
use crate::target::{Availability, CustomApp, DesktopId, Target};

/// A profile badge (PICK-06, DISC-08), drawn by the interface.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Badge {
    /// The profile's avatar picture: a file path.
    Image(String),
    /// A coloured circle with an initial, for profiles without a picture.
    Initial {
        text: String,
        /// `0xRRGGBB`.
        color: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileEntry {
    /// The profile directory (Chromium) or profile path (Firefox).
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub badge: Option<Badge>,
}

/// An installed app registered for `http`/`https` (TGT-05), which is not
/// always a browser.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HandlerEntry {
    pub app: DesktopId,
    pub name: String,
    /// An icon-theme name or an absolute path.
    #[serde(default)]
    pub icon: Option<String>,
    /// True for a known browser family; false for terminals and other URL
    /// handlers.
    pub browser: bool,
    /// Wye knows how to open a private window (DISC-05).
    #[serde(default)]
    pub private: bool,
    /// `--new-window` works (LAUNCH-05).
    #[serde(default)]
    pub new_window: bool,
    #[serde(default)]
    pub profiles: Vec<ProfileEntry>,
}

/// An installed app that is a target but not a web-link handler, such as a
/// service's own desktop app (APP-05).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppEntry {
    pub app: DesktopId,
    pub name: String,
    #[serde(default)]
    pub icon: Option<String>,
}

/// An app added through "Other…" or "+" (TGT-06).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CustomEntry {
    #[serde(with = "custom_app")]
    pub app: CustomApp,
    pub name: String,
    #[serde(default)]
    pub icon: Option<String>,
}

mod custom_app {
    use serde::{Deserialize, Deserializer, Serializer};

    use crate::target::CustomApp;

    pub fn serialize<S: Serializer>(app: &CustomApp, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&app.to_string())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<CustomApp, D::Error> {
        let text = String::deserialize(deserializer)?;
        CustomApp::parse(&text).map_err(serde::de::Error::custom)
    }
}

/// Everything Wye can open a link in.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct TargetCatalog {
    pub handlers: Vec<HandlerEntry>,
    pub apps: Vec<AppEntry>,
    pub custom: Vec<CustomEntry>,
}

/// What a target can do beyond a plain open (PICK-14, PICK-30). Opening in
/// the background is best effort for every target (LAUNCH-04), so it is not
/// listed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct TargetCaps {
    pub private: bool,
    pub new_window: bool,
}

/// A target with its name, icon and abilities.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TargetInfo {
    pub target: Target,
    /// The name in menus: a profile is just its name (TRAY-12).
    pub name: String,
    /// The name where the parent browser matters: "Work (Chrome)"
    /// (SHOWN-02).
    pub long_name: String,
    pub icon: Option<String>,
    pub badge: Option<Badge>,
    pub caps: TargetCaps,
}

impl TargetCatalog {
    fn handler(&self, app: &DesktopId) -> Option<&HandlerEntry> {
        self.handlers.iter().find(|h| h.app == *app)
    }

    /// The details of a target, or `None` when it is not installed (APP-10)
    /// or is the Picker or Default.
    #[must_use]
    pub fn describe(&self, target: &Target) -> Option<TargetInfo> {
        match target {
            Target::Picker | Target::Default => None,
            Target::App(id) => self.describe_app(id, target),
            Target::Private(id) => {
                let handler = self.handler(id).filter(|h| h.private)?;
                let name = format!("{} (Private)", handler.name);
                Some(TargetInfo {
                    target: target.clone(),
                    long_name: name.clone(),
                    name,
                    icon: handler.icon.clone(),
                    badge: None,
                    caps: TargetCaps {
                        private: true,
                        new_window: handler.new_window,
                    },
                })
            }
            Target::Profile { app, id } => {
                let handler = self.handler(app)?;
                let profile = handler.profiles.iter().find(|p| p.id == *id)?;
                Some(TargetInfo {
                    target: target.clone(),
                    name: profile.name.clone(),
                    long_name: format!("{} ({})", profile.name, handler.name),
                    icon: handler.icon.clone(),
                    badge: profile.badge.clone(),
                    caps: TargetCaps {
                        private: false,
                        new_window: handler.new_window,
                    },
                })
            }
            Target::Custom(app) => {
                let entry = self.custom.iter().find(|c| c.app == *app)?;
                Some(plain(target, &entry.name, entry.icon.as_deref()))
            }
        }
    }

    fn describe_app(&self, id: &DesktopId, target: &Target) -> Option<TargetInfo> {
        if let Some(handler) = self.handler(id) {
            return Some(TargetInfo {
                caps: TargetCaps {
                    private: handler.private,
                    new_window: handler.new_window,
                },
                ..plain(target, &handler.name, handler.icon.as_deref())
            });
        }
        let app = self.apps.iter().find(|a| a.app == *id)?;
        Some(plain(target, &app.name, app.icon.as_deref()))
    }

    /// Every target that can be chosen, in the order of the target menu's
    /// sections (TGT-02 d to f): handlers and added apps by name, then
    /// private windows, then profiles.
    #[must_use]
    pub fn all(&self) -> Vec<TargetInfo> {
        let private = self
            .sorted_handlers()
            .into_iter()
            .filter(|h| h.private)
            .map(|h| Target::Private(h.app.clone()));
        let profiles = self.sorted_handlers().into_iter().flat_map(|h| {
            h.profiles.iter().map(|p| Target::Profile {
                app: h.app.clone(),
                id: p.id.clone(),
            })
        });
        self.app_targets()
            .into_iter()
            .chain(private)
            .chain(profiles)
            .filter_map(|target| self.describe(&target))
            .collect()
    }

    /// Handlers sorted alphabetically by display name (TGT-05).
    pub(super) fn sorted_handlers(&self) -> Vec<&HandlerEntry> {
        let mut handlers: Vec<&HandlerEntry> = self.handlers.iter().collect();
        handlers.sort_by_cached_key(|h| (h.name.to_lowercase(), h.app.clone()));
        handlers
    }

    /// Section (d) of the target menu: every web-link handler plus the apps
    /// the user added, alphabetical by display name (TGT-05, TGT-06).
    pub(super) fn app_targets(&self) -> Vec<Target> {
        let handlers = self
            .handlers
            .iter()
            .map(|h| (h.name.to_lowercase(), Target::App(h.app.clone())));
        let custom = self
            .custom
            .iter()
            .map(|c| (c.name.to_lowercase(), Target::Custom(c.app.clone())));
        let mut named: Vec<(String, Target)> = handlers.chain(custom).collect();
        named
            .sort_by(|(a, ta), (b, tb)| a.cmp(b).then_with(|| ta.to_string().cmp(&tb.to_string())));
        named.into_iter().map(|(_, target)| target).collect()
    }
}

fn plain(target: &Target, name: &str, icon: Option<&str>) -> TargetInfo {
    TargetInfo {
        target: target.clone(),
        name: name.to_owned(),
        long_name: name.to_owned(),
        icon: icon.map(str::to_owned),
        badge: None,
        caps: TargetCaps::default(),
    }
}

impl Availability for TargetCatalog {
    fn is_available(&self, target: &Target) -> bool {
        self.describe(target).is_some()
    }
}

/// A target shown in the picker and the tray menu (PICK-03, TRAY-11) with the
/// hotkey the user assigned (SHOWN-04).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShownTarget {
    pub info: TargetInfo,
    pub hotkey: Option<String>,
}

/// The shown targets in the user's order (SHOWN-03), skipping any whose app
/// is gone (12-data-model.md: a missing target falls back to the picker, so
/// it cannot be offered). When the user has not chosen any, the installed
/// browsers are shown, alphabetically, so the picker is never empty.
#[must_use]
pub fn shown_targets(config: &Config, catalog: &TargetCatalog) -> Vec<ShownTarget> {
    let configured: Vec<ShownTarget> = config
        .browsers
        .shown
        .iter()
        .filter_map(|entry| resolve_shown(entry, catalog))
        .collect();
    if !config.browsers.shown.is_empty() {
        return configured;
    }
    catalog
        .sorted_handlers()
        .into_iter()
        .filter(|h| h.browser)
        .filter_map(|h| catalog.describe(&Target::App(h.app.clone())))
        .map(|info| ShownTarget { info, hotkey: None })
        .collect()
}

fn resolve_shown(entry: &ShownEntry, catalog: &TargetCatalog) -> Option<ShownTarget> {
    catalog.describe(&entry.target).map(|info| ShownTarget {
        info,
        hotkey: entry.hotkey.clone(),
    })
}
