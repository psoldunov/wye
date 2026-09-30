//! The target menu (TGT-02, TGT-05, TGT-07): the popup every "Open in" row
//! opens, as data. Sections, headers, order, checkmark and icons are decided
//! here; the interface only draws them.

use serde::Serialize;

use crate::target::{DesktopId, Target};

mod catalog;

pub use catalog::{
    AppEntry, Badge, CustomEntry, HandlerEntry, ProfileEntry, ShownTarget, TargetCaps,
    TargetCatalog, TargetInfo, shown_targets,
};

/// The Picker item's label (TGT-02 b).
pub const PICKER_LABEL: &str = "Picker";
/// The header of the private-window section (TGT-02 e).
pub const PRIVATE_HEADER: &str = "Private Browsing";
/// The last item, which opens the app chooser (TGT-02 g, TGT-06).
pub const OTHER_LABEL: &str = "Other…";

/// Where the menu is shown, which decides which sections it has.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Surface {
    /// Primary and alternative browser (BRW-01, BRW-02).
    Browsers,
    /// A web app mapping (APP-04).
    Apps,
    /// The rule editor's "Open in" (RUL-12).
    Rule,
    /// The picker's "Open In" submenu (PICK-08): no Default, no Picker, no
    /// service app.
    Picker,
}

/// Which part of the menu a section is (TGT-02 a to g).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SectionKind {
    Default,
    Picker,
    OwnApp,
    Apps,
    Private,
    Profiles,
    Other,
}

/// What choosing an item does.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum MenuChoice {
    /// Set this target.
    Target(Target),
    /// Open the app chooser (TGT-06).
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MenuItem {
    pub choice: MenuChoice,
    pub label: String,
    /// An icon-theme name or an absolute path. The Picker uses the picker
    /// glyph, which the interface draws.
    pub icon: Option<String>,
    /// The profile badge for profile items (TGT-03).
    pub badge: Option<Badge>,
    /// The current value (TGT-03).
    pub checked: bool,
    /// The app is not installed (APP-10). Only the current value is ever
    /// shown this way.
    pub missing: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MenuSection {
    pub kind: SectionKind,
    /// A dimmed header such as "Private Browsing", when the section has one.
    pub header: Option<String>,
    pub items: Vec<MenuItem>,
}

/// What to build.
#[derive(Debug, Clone, Copy)]
pub struct MenuSpec<'a> {
    pub surface: Surface,
    /// The current value, which gets the checkmark.
    pub current: Option<&'a Target>,
    /// The primary browser, for "Default (<primary>)" (TGT-02 a).
    pub primary: &'a Target,
    /// The desktop apps of the service being mapped (TGT-02 c); only used on
    /// [`Surface::Apps`].
    pub own_apps: &'a [DesktopId],
    /// Targets to leave out, such as the picker's tiles.
    pub exclude: &'a [Target],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TargetMenu {
    pub sections: Vec<MenuSection>,
}

impl TargetMenu {
    /// Builds the menu in the order of TGT-02: (a) Default, (b) Picker,
    /// (c) the service's own app, (d) every web-link handler and added app,
    /// (e) private windows, (f) one section per browser with profiles, (g)
    /// Other…. Empty sections are left out.
    #[must_use]
    pub fn build(spec: &MenuSpec<'_>, catalog: &TargetCatalog) -> Self {
        let builder = Builder { spec, catalog };
        let mut sections = Vec::new();
        if matches!(spec.surface, Surface::Apps | Surface::Rule) {
            sections.push(builder.default_section());
        }
        if spec.surface != Surface::Picker {
            sections.push(builder.picker_section());
        }
        let own = builder.own_app_section();
        let own_targets: Vec<Target> = own
            .iter()
            .flat_map(|s| s.items.iter())
            .filter_map(|item| match &item.choice {
                MenuChoice::Target(target) => Some(target.clone()),
                MenuChoice::Other => None,
            })
            .collect();
        sections.extend(own);
        sections.push(builder.apps_section(&own_targets));
        sections.push(builder.private_section());
        sections.extend(builder.profile_sections());
        sections.push(Self::other_section());
        sections.retain(|section| !section.items.is_empty());
        Self { sections }
    }

    fn other_section() -> MenuSection {
        MenuSection {
            kind: SectionKind::Other,
            header: None,
            items: vec![MenuItem {
                choice: MenuChoice::Other,
                label: OTHER_LABEL.to_owned(),
                icon: None,
                badge: None,
                checked: false,
                missing: false,
            }],
        }
    }

    /// Every item, top to bottom.
    pub fn items(&self) -> impl Iterator<Item = &MenuItem> {
        self.sections.iter().flat_map(|s| s.items.iter())
    }

    /// The item for `target`, when the menu has one.
    #[must_use]
    pub fn find(&self, target: &Target) -> Option<&MenuItem> {
        self.items()
            .find(|item| item.choice == MenuChoice::Target(target.clone()))
    }

    /// The checked item, if any.
    #[must_use]
    pub fn checked(&self) -> Option<&MenuItem> {
        self.items().find(|item| item.checked)
    }
}

struct Builder<'a> {
    spec: &'a MenuSpec<'a>,
    catalog: &'a TargetCatalog,
}

impl Builder<'_> {
    fn is_current(&self, target: &Target) -> bool {
        self.spec.current == Some(target)
    }

    fn excluded(&self, target: &Target) -> bool {
        self.spec.exclude.contains(target)
    }

    fn item(&self, info: &TargetInfo, label: String) -> MenuItem {
        MenuItem {
            choice: MenuChoice::Target(info.target.clone()),
            label,
            icon: info.icon.clone(),
            badge: info.badge.clone(),
            checked: self.is_current(&info.target),
            missing: false,
        }
    }

    // (a)
    fn default_section(&self) -> MenuSection {
        let primary = self.spec.primary;
        let (name, icon) = match self.catalog.describe(primary) {
            Some(info) => (info.name, info.icon),
            None if *primary == Target::Picker => (PICKER_LABEL.to_owned(), None),
            None => (primary.to_string(), None),
        };
        MenuSection {
            kind: SectionKind::Default,
            header: None,
            items: vec![MenuItem {
                choice: MenuChoice::Target(Target::Default),
                label: format!("Default ({name})"),
                icon,
                badge: None,
                checked: self.is_current(&Target::Default),
                missing: false,
            }],
        }
    }

    // (b), offered on every menu that can hold it (TGT-07)
    fn picker_section(&self) -> MenuSection {
        MenuSection {
            kind: SectionKind::Picker,
            header: None,
            items: vec![MenuItem {
                choice: MenuChoice::Target(Target::Picker),
                label: PICKER_LABEL.to_owned(),
                icon: None,
                badge: None,
                checked: self.is_current(&Target::Picker),
                missing: false,
            }],
        }
    }

    // (c): only on the Apps page and only when installed
    fn own_app_section(&self) -> Option<MenuSection> {
        if self.spec.surface != Surface::Apps {
            return None;
        }
        let items: Vec<MenuItem> = self
            .spec
            .own_apps
            .iter()
            .filter_map(|id| self.catalog.describe(&Target::App(id.clone())))
            .map(|info| self.item(&info, info.name.clone()))
            .collect();
        (!items.is_empty()).then_some(MenuSection {
            kind: SectionKind::OwnApp,
            header: None,
            items,
        })
    }

    // (d), alphabetical (TGT-05)
    fn apps_section(&self, already_listed: &[Target]) -> MenuSection {
        let mut items: Vec<MenuItem> = self
            .catalog
            .app_targets()
            .iter()
            .filter(|t| !self.excluded(t) && !already_listed.contains(t))
            .filter_map(|t| self.catalog.describe(t))
            .map(|info| self.item(&info, info.name.clone()))
            .collect();
        items.extend(self.missing_current(already_listed));
        MenuSection {
            kind: SectionKind::Apps,
            header: None,
            items,
        }
    }

    /// The current value when its app is gone, so it stays visible and
    /// checked (APP-10, TGT-01).
    fn missing_current(&self, already_listed: &[Target]) -> Option<MenuItem> {
        let current = self.spec.current?;
        let gone = current.is_concrete()
            && self.catalog.describe(current).is_none()
            && !already_listed.contains(current)
            && !self.excluded(current);
        gone.then(|| MenuItem {
            choice: MenuChoice::Target(current.clone()),
            label: current.to_string(),
            icon: None,
            badge: None,
            checked: true,
            missing: true,
        })
    }

    // (e)
    fn private_section(&self) -> MenuSection {
        let items = self
            .catalog
            .sorted_handlers()
            .into_iter()
            .filter(|h| h.private)
            .map(|h| Target::Private(h.app.clone()))
            .filter(|t| !self.excluded(t))
            .filter_map(|t| self.catalog.describe(&t))
            .map(|info| self.item(&info, info.name.clone()))
            .collect();
        MenuSection {
            kind: SectionKind::Private,
            header: Some(PRIVATE_HEADER.to_owned()),
            items,
        }
    }

    // (f): the profile names, under "Profiles: <Browser>"
    fn profile_sections(&self) -> Vec<MenuSection> {
        self.catalog
            .sorted_handlers()
            .into_iter()
            .map(|handler| {
                let items = handler
                    .profiles
                    .iter()
                    .map(|p| Target::Profile {
                        app: handler.app.clone(),
                        id: p.id.clone(),
                    })
                    .filter(|t| !self.excluded(t))
                    .filter_map(|t| self.catalog.describe(&t))
                    .map(|info| self.item(&info, info.name.clone()))
                    .collect();
                MenuSection {
                    kind: SectionKind::Profiles,
                    header: Some(format!("Profiles: {}", handler.name)),
                    items,
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests;
