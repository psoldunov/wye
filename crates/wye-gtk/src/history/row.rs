//! One row of the History window (DLG-HIS-02, DLG-HIS-03): the target's icon
//! with its profile badge; the link with its host emphasised and the rest
//! dimmed and cut in the middle, the time at the end of that line; under it
//! the source app and the target, then why it went there and what changed
//! it, as small pills. The original link is in the tooltip. Double-click or Enter opens the link in
//! the picker (the list's `row-activated`); the "⋯" button, a right click,
//! Shift+F10 and the Menu key open the row's menu.
//!
//! KDE counterpart: crates/wye-ui/qml/history/HistoryRow.qml and
//! HistoryBadge.qml.

use std::rc::Rc;

use adw::prelude::*;
use gtk::{gdk, gio, glib};
use serde_json::Value;

use super::view::{ReasonKind, Row};
use crate::widgets::icon;

/// The target icon's size: the list's rows are two lines high.
const ICON_SIZE: i32 = 32;

/// The host never takes more than this many characters before the rest of
/// the link gets room.
const HOST_MAX_CHARS: i32 = 32;

/// What a row asks the window to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RowAction {
    /// **Open in Picker** (DLG-HIS-03).
    Picker(u64),
    /// **Open in \<target\> Again**.
    SameTarget(u64),
    /// **Copy Link**, **Copy Original Link**.
    Copy(String),
    /// **Create Rule…**.
    CreateRule(u64),
    /// **Delete Entry**.
    Delete(u64),
}

/// What the window hands every row.
pub type Act = Rc<dyn Fn(RowAction)>;

/// A built row and its menu button (the self-test opens it).
#[derive(Debug, Clone)]
pub struct HistoryRow {
    pub row: gtk::ListBoxRow,
    pub menu_button: gtk::MenuButton,
}

/// The row for `data`, opened at `time` (already formatted), with the
/// target's profile `badge`.
pub fn build(data: &Row, time: &str, badge: Option<&Value>, act: &Act) -> HistoryRow {
    let details = details(data);
    let content = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    content.add_css_class("wye-history-row");
    let picture = icon::target_icon(&data.icon, badge, ICON_SIZE);
    picture.set_valign(gtk::Align::Center);
    content.append(&picture);

    let text = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(4)
        .hexpand(true)
        .valign(gtk::Align::Center)
        .build();
    text.append(&link_line(data, time));
    text.append(&details_line(data, &details));
    content.append(&text);

    let menu_button = menu_button(data);
    content.append(&menu_button);

    let row = gtk::ListBoxRow::builder()
        .child(&content)
        .activatable(true)
        .tooltip_text(tooltip(data))
        .build();
    row.update_property(&[gtk::accessible::Property::Label(&format!(
        "{}{}, {details}, {time}, {}",
        data.host, data.rest, data.reason
    ))]);
    row.insert_action_group("row", Some(&actions(data, act)));
    add_context_menu(&row, &menu_button);
    HistoryRow { row, menu_button }
}

/// "from Slack · Firefox".
fn details(data: &Row) -> String {
    let source = data
        .source_name
        .as_deref()
        .filter(|name| !name.is_empty())
        .map(|name| format!("from {name}"));
    source
        .into_iter()
        .chain([data.target_name.clone()])
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" · ")
}

/// DLG-HIS-02: the original link when the link was changed, the whole link
/// otherwise (the row cuts it).
fn tooltip(data: &Row) -> String {
    if data.changed {
        format!("Original link: {}", data.original_url)
    } else {
        data.final_url.clone()
    }
}

/// The host, emphasised, the rest of the link, dimmed and cut in the
/// middle, and the time at the end.
fn link_line(data: &Row, time: &str) -> gtk::Box {
    let line = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    let host = gtk::Label::builder()
        .label(&data.host)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .max_width_chars(HOST_MAX_CHARS)
        .xalign(0.0)
        .build();
    host.add_css_class("wye-history-host");
    line.append(&host);
    if data.rest.is_empty() {
        host.set_hexpand(true);
    } else {
        let rest = gtk::Label::builder()
            .label(&data.rest)
            .ellipsize(gtk::pango::EllipsizeMode::Middle)
            .hexpand(true)
            .xalign(0.0)
            .build();
        rest.add_css_class("dimmed");
        line.append(&rest);
    }
    let time = gtk::Label::builder()
        .label(time)
        .margin_start(12)
        .valign(gtk::Align::Baseline)
        .build();
    for class in ["caption", "numeric", "dimmed"] {
        time.add_css_class(class);
    }
    line.append(&time);
    line
}

/// The details, then the reason and what changed the link as pills. The
/// details (where the link came from and the target that opened it) stay
/// whole; the pills give way when the row is narrow.
fn details_line(data: &Row, details: &str) -> gtk::Box {
    let line = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let label = gtk::Label::builder().label(details).xalign(0.0).build();
    label.add_css_class("caption");
    label.add_css_class("dimmed");
    line.append(&label);
    if !data.reason.is_empty() {
        line.append(&pill(&data.reason, reason_class(data.reason_kind)));
    }
    for badge in &data.badges {
        line.append(&pill(badge, "wye-history-changed"));
    }
    line
}

/// The colour of a reason pill: rules in the accent, web apps in green, the
/// alternative key in amber, the rest quiet (DLG-HIS-02).
const fn reason_class(kind: ReasonKind) -> &'static str {
    match kind {
        ReasonKind::Rule => "wye-history-rule",
        ReasonKind::Mapping => "wye-history-mapping",
        ReasonKind::Alternative => "wye-history-alternative",
        ReasonKind::Picker | ReasonKind::Fallback | ReasonKind::Other => "wye-history-quiet",
    }
}

fn pill(text: &str, class: &str) -> gtk::Label {
    let label = gtk::Label::builder()
        .label(text)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .valign(gtk::Align::Center)
        .build();
    label.add_css_class("wye-history-pill");
    label.add_css_class(class);
    label
}

/// DLG-HIS-03: the row's menu.
fn menu(data: &Row) -> gio::Menu {
    let open = gio::Menu::new();
    open.append(Some("Open in Picker"), Some("row.picker"));
    if data.same_target {
        let label = format!("Open in {} Again", data.target_name);
        open.append(Some(&label), Some("row.same-target"));
    }
    let copy = gio::Menu::new();
    copy.append(Some("Copy Link"), Some("row.copy"));
    if data.changed {
        copy.append(Some("Copy Original Link"), Some("row.copy-original"));
    }
    copy.append(Some("Create Rule…"), Some("row.create-rule"));
    let delete = gio::Menu::new();
    delete.append(Some("Delete Entry"), Some("row.delete"));
    let menu = gio::Menu::new();
    menu.append_section(None, &open);
    menu.append_section(None, &copy);
    menu.append_section(None, &delete);
    menu
}

fn menu_button(data: &Row) -> gtk::MenuButton {
    let button = gtk::MenuButton::builder()
        .icon_name("view-more-symbolic")
        .menu_model(&menu(data))
        .valign(gtk::Align::Center)
        .tooltip_text("Actions")
        .build();
    button.add_css_class("flat");
    button.add_css_class("circular");
    button.update_property(&[gtk::accessible::Property::Label(&format!(
        "Actions for {}",
        data.host
    ))]);
    button
}

/// The `row.*` actions behind the menu.
fn actions(data: &Row, act: &Act) -> gio::SimpleActionGroup {
    let id = data.id;
    let entries: [(&str, RowAction); 6] = [
        ("picker", RowAction::Picker(id)),
        ("same-target", RowAction::SameTarget(id)),
        ("copy", RowAction::Copy(data.final_url.clone())),
        ("copy-original", RowAction::Copy(data.original_url.clone())),
        ("create-rule", RowAction::CreateRule(id)),
        ("delete", RowAction::Delete(id)),
    ];
    let group = gio::SimpleActionGroup::new();
    for (name, action) in entries {
        let simple = gio::SimpleAction::new(name, None);
        let act = Rc::clone(act);
        simple.connect_activate(move |_, _| act(action.clone()));
        group.add_action(&simple);
    }
    group
}

/// A right click opens the row's menu where it was clicked; Shift+F10 and
/// the Menu key open it from the "⋯" button.
fn add_context_menu(row: &gtk::ListBoxRow, menu_button: &gtk::MenuButton) {
    let click = gtk::GestureClick::builder()
        .button(gdk::BUTTON_SECONDARY)
        .build();
    click.connect_pressed(glib::clone!(
        #[weak]
        row,
        #[weak]
        menu_button,
        move |gesture, _, x, y| {
            gesture.set_state(gtk::EventSequenceState::Claimed);
            popup_at(&row, &menu_button, x, y);
        }
    ));
    row.add_controller(click);
    let keys = gtk::ShortcutController::new();
    let open = gtk::CallbackAction::new(glib::clone!(
        #[weak]
        menu_button,
        #[upgrade_or]
        glib::Propagation::Proceed,
        move |_, _| {
            menu_button.popup();
            glib::Propagation::Stop
        }
    ));
    keys.add_shortcut(gtk::Shortcut::new(
        gtk::ShortcutTrigger::parse_string("<Shift>F10|Menu"),
        Some(open),
    ));
    row.add_controller(keys);
}

/// A menu at (`x`, `y`) of `row`. It is a child of the row only while open:
/// a popover left on a row would outlive the list's rebuilds.
fn popup_at(row: &gtk::ListBoxRow, menu_button: &gtk::MenuButton, x: f64, y: f64) {
    let Some(model) = menu_button.menu_model() else {
        return;
    };
    let popover = gtk::PopoverMenu::from_model(Some(&model));
    popover.set_has_arrow(false);
    popover.set_halign(gtk::Align::Start);
    popover.set_parent(row);
    #[allow(
        clippy::cast_possible_truncation,
        reason = "pointer positions inside a row are far below i32::MAX"
    )]
    popover.set_pointing_to(Some(&gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
    popover.connect_closed(|popover| {
        // After the item's action ran: GTK activates it as the menu closes.
        // An action can rebuild the list first (DLG-HIS-03: Delete Entry, and
        // any update the service announces meanwhile) and drop this row. Keep
        // the row until the popover has left it: a row finalized with its
        // popover still attached leaves the popover a dangling parent, and
        // unparenting it then crashes GTK.
        let (popover, row) = (popover.clone(), popover.parent());
        glib::idle_add_local_once(move || {
            popover.unparent();
            drop(row);
        });
    });
    popover.popup();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row() -> Row {
        Row {
            id: 7,
            time: 0,
            host: "github.com".to_owned(),
            rest: "/example/repo".to_owned(),
            final_url: "https://github.com/example/repo".to_owned(),
            original_url: "https://github.com/example/repo?utm_source=x".to_owned(),
            changed: true,
            source: Some("com.slack.Slack.desktop".to_owned()),
            source_name: Some("Slack".to_owned()),
            target_name: "Firefox".to_owned(),
            icon: "firefox".to_owned(),
            same_target: true,
            reason: "primary browser".to_owned(),
            reason_kind: ReasonKind::Fallback,
            badges: vec!["cleaned"],
        }
    }

    #[test]
    fn the_details_name_the_source_and_target() {
        // DLG-HIS-02
        assert_eq!(details(&row()), "from Slack · Firefox");
        let without_source = Row {
            source_name: None,
            ..row()
        };
        assert_eq!(details(&without_source), "Firefox");
    }

    #[test]
    fn the_tooltip_shows_the_original_link_when_it_changed() {
        assert_eq!(
            tooltip(&row()),
            "Original link: https://github.com/example/repo?utm_source=x"
        );
        let same = Row {
            changed: false,
            ..row()
        };
        assert_eq!(tooltip(&same), "https://github.com/example/repo");
    }

    #[test]
    fn the_menu_offers_the_same_target_and_the_original_only_when_they_apply() {
        // DLG-HIS-03
        let labels = |menu: &gio::Menu| -> Vec<String> {
            (0..menu.n_items())
                .filter_map(|section| menu.item_link(section, gio::MENU_LINK_SECTION))
                .flat_map(|section| {
                    (0..section.n_items())
                        .filter_map(|item| {
                            section
                                .item_attribute_value(item, gio::MENU_ATTRIBUTE_LABEL, None)
                                .and_then(|value| value.get::<String>())
                        })
                        .collect::<Vec<_>>()
                })
                .collect()
        };
        assert_eq!(
            labels(&menu(&row())),
            [
                "Open in Picker",
                "Open in Firefox Again",
                "Copy Link",
                "Copy Original Link",
                "Create Rule…",
                "Delete Entry"
            ]
        );
        let picker = Row {
            same_target: false,
            changed: false,
            ..row()
        };
        assert_eq!(
            labels(&menu(&picker)),
            [
                "Open in Picker",
                "Copy Link",
                "Create Rule…",
                "Delete Entry"
            ]
        );
    }
}
