//! ONB-03, the browsers step: the **Primary browser** popup, set to the
//! Picker at first with the browser Wye replaced listed first, and the
//! checklist of detected browsers and profiles for the picker (BLK-15), the
//! first six browsers pre-checked. Hotkeys are left to the shown browsers
//! sheet.
//!
//! KDE counterpart: crates/wye-ui/qml/onboarding/OnboardingBrowsers.qml.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use adw::prelude::*;
use gtk::glib;
use serde_json::{Value, json};
use wye_api::targets::TargetInventory;

use super::choices::{ListRow, PrimaryChoice};
use super::flow::Step;
use super::pages::{self, Intent, Send};
use super::view::View;
use crate::widgets::checklist::{Checklist, ChecklistItem};
use crate::widgets::icon;

/// The icon size in the popup.
const ICON_SIZE: i32 = 16;

/// The browsers step.
#[derive(Debug)]
pub struct BrowsersStep {
    page: adw::NavigationPage,
    primary: adw::ComboRow,
    names: gtk::StringList,
    /// What the popup lists, in its order.
    choices: Rc<RefCell<Vec<PrimaryChoice>>>,
    /// Set while the popup is filled, so that is not taken as a choice.
    filling: Rc<Cell<bool>>,
    checklist: Checklist,
}

impl BrowsersStep {
    pub fn new(send: &Send) -> Self {
        let (page, column) = pages::step_page(
            Step::Browsers,
            "Choose your browsers",
            "The primary browser opens links no rule handles. The picker offers the browsers you check.",
        );
        let choices: Rc<RefCell<Vec<PrimaryChoice>>> = Rc::default();
        let names = gtk::StringList::new(&[]);
        let primary = adw::ComboRow::builder()
            .title("Primary browser")
            .subtitle("Choose the Picker to be asked each time.")
            .model(&names)
            .factory(&choice_factory(&choices, &names))
            .build();
        let filling = Rc::new(Cell::new(false));
        primary.connect_selected_notify(glib::clone!(
            #[strong]
            choices,
            #[strong]
            filling,
            #[strong]
            send,
            move |row| {
                if filling.get() {
                    return;
                }
                let chosen = usize::try_from(row.selected())
                    .ok()
                    .and_then(|index| choices.borrow().get(index).cloned());
                if let Some(choice) = chosen.filter(|choice| !choice.checked) {
                    send(Intent::SetPrimary(choice.target));
                }
            }
        ));
        let primary_group = adw::PreferencesGroup::new();
        primary_group.add(&primary);
        column.append(&primary_group);

        let checklist = Checklist::new();
        let toggled = Rc::clone(send);
        checklist.connect_toggled(move |key, checked| {
            toggled(Intent::Toggle(key.to_owned(), checked));
        });
        let moved = Rc::clone(send);
        checklist.connect_moved(move |from, to| moved(Intent::Move(from, to)));
        let browsers = adw::PreferencesGroup::builder()
            .title("Browsers in the picker")
            .build();
        browsers.add(checklist.widget());
        column.append(&browsers);
        column.append(&pages::note(
            "Set a hotkey for each browser later, in Settings.",
        ));
        Self {
            page,
            primary,
            names,
            choices,
            filling,
            checklist,
        }
    }

    pub const fn page(&self) -> &adw::NavigationPage {
        &self.page
    }

    pub fn show(&self, view: &View, targets: &TargetInventory) {
        self.filling.set(true);
        let names: Vec<&str> = view.primary.iter().map(|c| c.name.as_str()).collect();
        let current: Vec<String> = (0..self.names.n_items())
            .filter_map(|index| self.names.string(index).map(Into::into))
            .collect();
        self.choices.replace(view.primary.clone());
        if current != names {
            self.names.splice(0, self.names.n_items(), &names);
        }
        let selected = view
            .primary
            .iter()
            .position(|choice| choice.checked)
            .and_then(|index| u32::try_from(index).ok())
            .unwrap_or(0);
        self.primary.set_selected(selected);
        self.primary.set_sensitive(view.writable);
        self.filling.set(false);

        let items: Vec<ChecklistItem> = view
            .checklist
            .iter()
            .map(|row| item(row, targets))
            .collect();
        self.checklist.set_items(&items);
        self.checklist.widget().set_sensitive(view.writable);
    }
}

/// A checklist row for `row`, with its profile badge (TGT-03).
fn item(row: &ListRow, targets: &TargetInventory) -> ChecklistItem {
    let badge = targets
        .targets
        .iter()
        .find(|info| info.target == row.target)
        .and_then(|info| serde_json::to_value(info.badge.as_ref()?).ok());
    ChecklistItem {
        key: row.key.clone(),
        title: row.name.clone(),
        subtitle: String::new(),
        icon: row.icon.clone(),
        badge,
        checked: row.checked,
        extra: None,
    }
}

/// The popup's rows and its button: the choice's icon and name.
///
/// A row finds its choice by its item's place in `names`, not by
/// `ListItem::position`: the button's item reports position 0 whatever is
/// selected, so it showed the first choice (the replaced browser) even with
/// the Picker chosen (ONB-03).
fn choice_factory(
    choices: &Rc<RefCell<Vec<PrimaryChoice>>>,
    names: &gtk::StringList,
) -> gtk::SignalListItemFactory {
    let factory = gtk::SignalListItemFactory::new();
    factory.connect_setup(|_, item| {
        let Some(item) = item.downcast_ref::<gtk::ListItem>() else {
            return;
        };
        let line = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        line.append(&icon::image("", ICON_SIZE));
        line.append(&gtk::Label::builder().xalign(0.0).build());
        item.set_child(Some(&line));
    });
    let choices = Rc::clone(choices);
    let names = names.downgrade();
    factory.connect_bind(move |_, item| {
        let Some(item) = item.downcast_ref::<gtk::ListItem>() else {
            return;
        };
        let Some(line) = item.child() else {
            return;
        };
        let choice = item
            .item()
            .zip(names.upgrade())
            .and_then(|(object, names)| {
                (0..names.n_items()).find(|&index| names.item(index).as_ref() == Some(&object))
            })
            .and_then(|index| usize::try_from(index).ok())
            .and_then(|index| choices.borrow().get(index).cloned());
        let Some(choice) = choice else {
            return;
        };
        let Some(line) = line.downcast_ref::<gtk::Box>() else {
            return;
        };
        if let Some(old) = line.first_child() {
            line.remove(&old);
        }
        line.prepend(&icon::image(&choice.icon, ICON_SIZE));
        if let Some(label) = line.last_child().and_downcast::<gtk::Label>() {
            label.set_label(&choice.name);
        }
    });
    factory
}

/// `shown` after the checked row at `from` was dropped on the one at `to`
/// (BLK-15). `checked` are the checklist's checked targets, in its order;
/// each keeps its entry (and hotkey), and entries the checklist does not
/// show (an app that is gone) stay at the end.
#[must_use]
pub fn moved(shown: &[Value], checked: &[Value], from: usize, to: usize) -> Vec<Value> {
    if from >= checked.len() || to >= checked.len() {
        return shown.to_vec();
    }
    let mut order: Vec<&Value> = checked.iter().collect();
    let target = order.remove(from);
    order.insert(to, target);
    let entry = |target: &Value| {
        shown
            .iter()
            .find(|entry| entry.get("target") == Some(target))
            .cloned()
            .unwrap_or_else(|| json!({"target": target}))
    };
    let rest = shown.iter().filter(|entry| {
        entry
            .get("target")
            .is_none_or(|target| !checked.contains(target))
    });
    order.into_iter().map(entry).chain(rest.cloned()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dragged_browser_moves_and_keeps_its_hotkey() {
        // ONB-03, BLK-15
        let firefox = json!({"app": "firefox.desktop"});
        let chrome = json!({"app": "google-chrome.desktop"});
        let zen = json!({"app": "zen.desktop"});
        let gone = json!({"app": "gone.desktop"});
        let shown = vec![
            json!({"target": firefox, "hotkey": "f"}),
            json!({"target": gone}),
            json!({"target": chrome}),
        ];
        let checked = [firefox.clone(), chrome.clone(), zen.clone()];
        assert_eq!(
            moved(&shown, &checked, 0, 2),
            [
                json!({"target": chrome}),
                json!({"target": zen}),
                json!({"target": firefox, "hotkey": "f"}),
                json!({"target": gone}),
            ]
        );
    }

    #[test]
    fn a_move_outside_the_list_changes_nothing() {
        let shown = vec![json!({"target": {"app": "a.desktop"}})];
        assert_eq!(moved(&shown, &[], 0, 1), shown);
    }
}
