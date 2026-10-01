//! The tray-menu popup's model (TRAY-08, spec decision #14): what the
//! service's `PickerHost1.ShowMenu` sends, turned into the rows QML draws.
//!
//! The payload is the `Tray` JSON (`wye_api::tray::TrayMenu`) plus an
//! optional `placement` (the pointer, as for the picker). The popup shows
//! the same items as the tray: headers, radio items, actions, separators and
//! nested submenus (TRAY-15: More, then Recent Links), with the fixed
//! accelerators `P` and `1`–`9` (TRAY-13, KEY-51). Choosing an item sends its
//! ID back to the service with `ActivateTrayItem`, the tray's own dispatcher.

use serde::{Deserialize, Serialize};
use wye_api::picker::Placement;
use wye_api::tray::{TrayItem, TrayItemKind, TrayMenu};

/// What `ShowMenu` carries.
#[derive(Debug, Deserialize)]
struct Payload {
    #[serde(flatten)]
    menu: TrayMenu,
    #[serde(default)]
    placement: Option<Placement>,
}

/// The popup's content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuView {
    /// The top-level rows, in order.
    pub rows: Vec<Row>,
    /// Where the pointer is; centred when `None`.
    pub placement: Option<Placement>,
}

/// One row as QML draws it.
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent flags QML reads by name from the row JSON, not a state machine"
)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Row {
    pub id: String,
    /// `action`, `header`, `radio`, `separator` or `submenu`.
    pub kind: &'static str,
    pub label: String,
    /// Icon theme name or path; empty for none.
    pub icon: String,
    /// Shortcut shown on the right (TRAY-13); empty for none.
    pub shortcut: String,
    pub enabled: bool,
    pub checked: bool,
    /// Whether the row can be chosen: an enabled action or radio item.
    pub selectable: bool,
    /// Whether the row opens a submenu: an enabled submenu with entries.
    pub opens: bool,
    pub children: Vec<Row>,
}

/// Why a payload cannot be shown.
#[derive(Debug, thiserror::Error)]
#[error("the tray menu cannot be read: {0}")]
pub struct MenuError(#[from] serde_json::Error);

impl MenuView {
    /// Read a `ShowMenu` payload.
    ///
    /// # Errors
    ///
    /// [`MenuError`] when it is not a tray menu.
    pub fn parse(json: &str) -> Result<Self, MenuError> {
        let payload: Payload = serde_json::from_str(json)?;
        Ok(Self {
            rows: payload.menu.items.iter().map(row).collect(),
            placement: payload.placement,
        })
    }

    /// The rows as the JSON QML parses.
    #[must_use]
    pub fn rows_json(&self) -> String {
        // Plain strings and booleans always encode.
        serde_json::to_string(&self.rows).unwrap_or_else(|_| "[]".to_owned())
    }

    /// The top-level row whose fixed accelerator is `text` (KEY-51), for
    /// example `p` for the Picker or `1` for the first browser.
    #[must_use]
    pub fn accelerator(&self, text: &str) -> Option<&str> {
        let mut chars = text.chars();
        let key = chars.next()?;
        if chars.next().is_some() || key.is_whitespace() {
            return None;
        }
        self.rows
            .iter()
            .filter(|row| row.selectable)
            .find(|row| is_accelerator(&row.shortcut, key))
            .map(|row| row.id.as_str())
    }

    /// Whether `id` is a row that can be chosen, at any depth.
    #[must_use]
    pub fn is_selectable(&self, id: &str) -> bool {
        find(&self.rows, id).is_some_and(|row| row.selectable)
    }
}

fn is_accelerator(shortcut: &str, key: char) -> bool {
    let mut chars = shortcut.chars();
    matches!(
        (chars.next(), chars.next()),
        (Some(single), None) if single.eq_ignore_ascii_case(&key)
    )
}

fn find<'a>(rows: &'a [Row], id: &str) -> Option<&'a Row> {
    rows.iter().find_map(|row| {
        if row.id == id {
            Some(row)
        } else {
            find(&row.children, id)
        }
    })
}

fn row(item: &TrayItem) -> Row {
    let selectable =
        item.enabled && matches!(item.kind, TrayItemKind::Action | TrayItemKind::Radio);
    let children: Vec<Row> = item.children.iter().map(row).collect();
    Row {
        id: item.id.clone(),
        kind: item.kind.as_str(),
        label: item.label.clone(),
        icon: item.icon.clone().unwrap_or_default(),
        shortcut: item.shortcut.clone().unwrap_or_default(),
        enabled: item.enabled,
        checked: item.checked,
        selectable,
        opens: item.enabled && item.kind == TrayItemKind::Submenu && !children.is_empty(),
        children,
    }
}

#[cfg(test)]
mod tests;
