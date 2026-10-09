//! The tray menu as data (TRAY-02, TRAY-10 to TRAY-15, TRAY-18, TRAY-20).
//!
//! One model feeds every tray surface: the `StatusNotifierItem` maps it to a
//! `DBusMenu` and the `wye-ui` popup draws it. Item IDs are stable, so a
//! host can tell which item was activated, and the model carries
//! the labels, enabled and checked state and shortcut of each item.
//!
//! ```text
//! [Make Wye Default Browser]      only while Wye is not the default (TRAY-18)
//! Open URL from Clipboard         disabled without a URL on the clipboard
//! ──
//! Primary Browser                 header
//! (•) Picker                P     radio
//! ( ) Firefox               1
//! ──
//! Settings…              Ctrl+,
//! More                          ›  History… · Recent Links › · Test Rules… …
//! ──
//! Quit Wye               Ctrl+Q
//! ```

use serde::Serialize;

use crate::config::{Config, TrayIcon};
use crate::keybinding::KeyBinding;
use crate::keys::{Modifier, Modifiers};
use crate::link_text::host_and_path;
use crate::target::Target;
use crate::target_menu::{ShownTarget, TargetCatalog, shown_targets};

/// The item IDs a host reports back when an item is activated.
pub mod ids {
    pub const MAKE_DEFAULT: &str = "make-default";
    pub const OPEN_CLIPBOARD: &str = "open-clipboard";
    pub const PRIMARY_HEADER: &str = "primary-header";
    pub const PRIMARY_PICKER: &str = "primary:picker";
    pub const SETTINGS: &str = "settings";
    pub const MORE: &str = "more";
    pub const HISTORY: &str = "history";
    pub const RECENT: &str = "recent";
    pub const TEST_RULES: &str = "test-rules";
    pub const RESCAN: &str = "rescan";
    pub const SET_UP: &str = "set-up";
    pub const HELP: &str = "help";
    pub const ABOUT: &str = "about";
    pub const QUIT: &str = "quit";

    /// The radio item of the shown target at `position` (from 0).
    #[must_use]
    pub fn primary(position: usize) -> String {
        format!("primary:{position}")
    }

    /// The item for a recent history entry.
    #[must_use]
    pub fn recent(entry_id: &str) -> String {
        format!("recent:{entry_id}")
    }

    pub(super) const SEPARATOR_PREFIX: &str = "separator:";
}

/// How many recent links the submenu lists (TRAY-15).
pub const RECENT_LIMIT: usize = 10;

/// TRAY-20: holding either of these while choosing a primary-browser item
/// opens its target instead of making it the primary browser.
pub const OPEN_MODIFIERS: [Modifier; 2] = [Modifier::Ctrl, Modifier::Shift];
/// The longest label of a recent link before it is cut in the middle.
pub const RECENT_LABEL_CHARS: usize = 48;

/// The item copy, from the menu table of 01-tray-menu.md.
pub mod labels {
    pub const MAKE_DEFAULT: &str = "Make Wye Default Browser";
    pub const OPEN_CLIPBOARD: &str = "Open URL from Clipboard";
    pub const PRIMARY_HEADER: &str = "Primary Browser";
    pub const PICKER: &str = "Picker";
    pub const SETTINGS: &str = "Settings…";
    pub const MORE: &str = "More";
    pub const HISTORY: &str = "History…";
    pub const RECENT: &str = "Recent Links";
    pub const TEST_RULES: &str = "Test Rules…";
    pub const RESCAN: &str = "Rescan Browsers";
    pub const SET_UP: &str = "Set Up Wye…";
    pub const HELP: &str = "Help";
    pub const ABOUT: &str = "About Wye";
    pub const QUIT: &str = "Quit Wye";
}

/// The icon of the tray item (TRAY-02).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TrayIconSpec {
    /// Wye's own icon.
    App,
    /// The picker glyph: a bulleted list.
    Picker,
    /// An icon-theme name or an absolute path, for the primary browser.
    Named(String),
}

/// An item's icon (TRAY-14: full-colour app icons, drawn large).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ItemIcon {
    Picker,
    Named(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ItemKind {
    Action,
    /// Non-interactive and dimmed.
    Header,
    Radio,
    Separator,
    Submenu,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TrayItem {
    pub id: String,
    pub kind: ItemKind,
    pub label: String,
    pub enabled: bool,
    /// The selected radio item.
    pub checked: bool,
    pub icon: Option<ItemIcon>,
    /// The accelerator to show (TRAY-13, KEY-50, KEY-51).
    pub shortcut: Option<KeyBinding>,
    /// For the radio items of the primary-browser group: what choosing the
    /// item sets as the primary browser (TRAY-11).
    pub target: Option<Target>,
    /// The items of a submenu.
    pub children: Vec<Self>,
}

impl TrayItem {
    fn new(id: impl Into<String>, kind: ItemKind, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            kind,
            label: label.into(),
            enabled: true,
            checked: false,
            icon: None,
            shortcut: None,
            target: None,
            children: Vec::new(),
        }
    }

    fn action(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self::new(id, ItemKind::Action, label)
    }

    fn separator() -> Self {
        Self::new(ids::SEPARATOR_PREFIX, ItemKind::Separator, "")
    }

    fn with_shortcut(self, modifiers: &[Modifier], key: &str) -> Self {
        Self {
            shortcut: KeyBinding::new(Modifiers::from_slice(modifiers), key).ok(),
            ..self
        }
    }

    fn disabled(self) -> Self {
        Self {
            enabled: false,
            ..self
        }
    }
}

/// What the menu needs to know besides the configuration.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TrayStatus {
    /// TRAY-10: the clipboard holds a URL. The service refreshes it each
    /// time the menu opens.
    pub clipboard_has_url: bool,
    /// TRAY-18: Wye is the default browser, or the user kept the app that is
    /// (ONB-11). False shows the warning and "Make Wye Default Browser".
    pub wye_is_default: bool,
    /// The newest history entries, newest first. Only used while history is
    /// on (TRAY-15).
    pub recent: Vec<RecentLink>,
}

/// A history entry for the "Recent Links" submenu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecentLink {
    /// The history entry's ID, which names the item.
    pub id: String,
    pub url: String,
}

/// What choosing an item of the "Primary Browser" group does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrimaryChoice {
    /// TRAY-11: the target becomes the primary browser.
    SetPrimary(Target),
    /// TRAY-20: the target opens without a link; the primary browser stays.
    Open(Target),
    /// TRAY-20: the Picker has nothing to open, so a click with
    /// [`OPEN_MODIFIERS`] changes nothing.
    Nothing,
}

impl PrimaryChoice {
    /// What choosing the item for `target` does while `held` are held.
    #[must_use]
    pub fn new(target: Target, held: Modifiers) -> Self {
        let opens = OPEN_MODIFIERS
            .iter()
            .any(|modifier| held.contains(*modifier));
        if !opens {
            Self::SetPrimary(target)
        } else if target.is_concrete() {
            Self::Open(target)
        } else {
            Self::Nothing
        }
    }
}

/// The tray icon and its menu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TrayMenu {
    pub icon: TrayIconSpec,
    /// TRAY-18: the icon carries a warning overlay.
    pub warning: bool,
    /// TRAY-04: false hides the icon. Wye keeps running.
    pub visible: bool,
    pub items: Vec<TrayItem>,
}

impl TrayMenu {
    /// Builds the menu for the current configuration (TRAY-02, TRAY-10 to
    /// TRAY-15, TRAY-18).
    #[must_use]
    pub fn build(config: &Config, catalog: &TargetCatalog, status: &TrayStatus) -> Self {
        let shown = shown_targets(config, catalog);
        let mut items = Vec::new();
        if !status.wye_is_default {
            items.push(TrayItem::action(ids::MAKE_DEFAULT, labels::MAKE_DEFAULT));
            items.push(TrayItem::separator());
        }
        let mut clipboard = TrayItem::action(ids::OPEN_CLIPBOARD, labels::OPEN_CLIPBOARD);
        clipboard.enabled = status.clipboard_has_url;
        items.push(clipboard);
        items.push(TrayItem::separator());
        items.extend(primary_group(&config.browsers.primary, &shown));
        items.push(TrayItem::separator());
        items.push(
            TrayItem::action(ids::SETTINGS, labels::SETTINGS).with_shortcut(&[Modifier::Ctrl], ","),
        );
        items.push(more_submenu(config.advanced.history, &status.recent));
        items.push(TrayItem::separator());
        items.push(TrayItem::action(ids::QUIT, labels::QUIT).with_shortcut(&[Modifier::Ctrl], "q"));
        Self {
            icon: icon(config, catalog),
            warning: !status.wye_is_default,
            visible: config.general.show_tray_icon,
            items: number_separators(items, &mut 0),
        }
    }

    /// The item with this ID, searching submenus too.
    #[must_use]
    pub fn find(&self, id: &str) -> Option<&TrayItem> {
        find_in(&self.items, id)
    }

    /// What choosing the primary-browser item `id` does while `held` are
    /// held (TRAY-11, TRAY-20); `None` when `id` names no such item.
    #[must_use]
    pub fn primary_choice(&self, id: &str, held: Modifiers) -> Option<PrimaryChoice> {
        let target = self.find(id)?.target.clone()?;
        Some(PrimaryChoice::new(target, held))
    }
}

fn find_in<'a>(items: &'a [TrayItem], id: &str) -> Option<&'a TrayItem> {
    items.iter().find_map(|item| {
        if item.id == id {
            Some(item)
        } else {
            find_in(&item.children, id)
        }
    })
}

/// TRAY-02: Wye's icon, the picker glyph, or the primary browser's icon.
fn icon(config: &Config, catalog: &TargetCatalog) -> TrayIconSpec {
    match config.general.tray_icon {
        TrayIcon::Wye => TrayIconSpec::App,
        TrayIcon::PrimaryBrowser => match &config.browsers.primary {
            Target::Picker | Target::Default => TrayIconSpec::Picker,
            primary => catalog
                .describe(primary)
                .and_then(|info| info.icon)
                .map_or(TrayIconSpec::App, TrayIconSpec::Named),
        },
    }
}

/// TRAY-11 to TRAY-14: the header and the radio group of the Picker and every
/// shown target.
fn primary_group(primary: &Target, shown: &[ShownTarget]) -> Vec<TrayItem> {
    let mut items = vec![
        TrayItem::new(
            ids::PRIMARY_HEADER,
            ItemKind::Header,
            labels::PRIMARY_HEADER,
        )
        .disabled(),
    ];
    items.push(TrayItem {
        checked: *primary == Target::Picker,
        icon: Some(ItemIcon::Picker),
        target: Some(Target::Picker),
        ..TrayItem::new(ids::PRIMARY_PICKER, ItemKind::Radio, labels::PICKER)
            .with_shortcut(&[], "p")
    });
    items.extend(shown.iter().enumerate().map(|(position, shown)| {
        let info = &shown.info;
        // TRAY-13: digits 1 to 9 by position, none beyond the ninth.
        let digit = (position < 9).then(|| (position + 1).to_string());
        let item = TrayItem {
            checked: info.target == *primary,
            icon: info.icon.clone().map(ItemIcon::Named),
            target: Some(info.target.clone()),
            // TRAY-12: a profile is just its name.
            ..TrayItem::new(ids::primary(position), ItemKind::Radio, info.name.clone())
        };
        match digit {
            Some(digit) => item.with_shortcut(&[], &digit),
            None => item,
        }
    }));
    items
}

/// TRAY-15.
fn more_submenu(history_on: bool, recent: &[RecentLink]) -> TrayItem {
    let mut children = vec![TrayItem::action(ids::HISTORY, labels::HISTORY)];
    if history_on {
        children.push(recent_submenu(recent));
    }
    children.extend([
        TrayItem::separator(),
        TrayItem::action(ids::TEST_RULES, labels::TEST_RULES),
        TrayItem::action(ids::RESCAN, labels::RESCAN),
        TrayItem::action(ids::SET_UP, labels::SET_UP),
        TrayItem::separator(),
        TrayItem::action(ids::HELP, labels::HELP),
        TrayItem::action(ids::ABOUT, labels::ABOUT),
    ]);
    TrayItem {
        children,
        ..TrayItem::new(ids::MORE, ItemKind::Submenu, labels::MORE)
    }
}

fn recent_submenu(recent: &[RecentLink]) -> TrayItem {
    let children: Vec<TrayItem> = recent
        .iter()
        .take(RECENT_LIMIT)
        .map(|link| {
            TrayItem::action(
                ids::recent(&link.id),
                host_and_path(&link.url, RECENT_LABEL_CHARS),
            )
        })
        .collect();
    TrayItem {
        enabled: !children.is_empty(),
        children,
        ..TrayItem::new(ids::RECENT, ItemKind::Submenu, labels::RECENT)
    }
}

/// Gives every separator its own ID (`separator:0`, `separator:1`, …) in menu
/// order, so a host that needs unique IDs has them.
fn number_separators(items: Vec<TrayItem>, next: &mut usize) -> Vec<TrayItem> {
    items
        .into_iter()
        .map(|item| {
            let id = if item.kind == ItemKind::Separator {
                let id = format!("{}{next}", ids::SEPARATOR_PREFIX);
                *next += 1;
                id
            } else {
                item.id
            };
            TrayItem {
                id,
                children: number_separators(item.children, next),
                ..item
            }
        })
        .collect()
}

#[cfg(test)]
mod tests;
