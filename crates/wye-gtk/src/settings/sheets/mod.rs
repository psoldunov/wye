//! Sheets of the Settings window that are not one page's own form: the app
//! chooser every target menu and app list opens (DLG-APP), and the shown
//! browsers sheet (SHOWN-01 to SHOWN-08). Each is a modal `AdwDialog` over
//! the window (BLK-11), built when it is opened and dropped when it closes.

pub mod app_chooser;
pub mod shown_browsers;

use adw::prelude::*;

/// Most sheets that stack over one another (a chooser over a sheet).
const MAX_STACKED: usize = 4;

/// Close every sheet and popup open over `window`, so a self-test case
/// starts from the window alone (the kit gallery does the same).
pub fn close_open(window: &adw::ApplicationWindow) {
    close_popovers(window.upcast_ref());
    for _ in 0..MAX_STACKED {
        let Some(dialog) = window.visible_dialog() else {
            return;
        };
        dialog.force_close();
    }
}

/// Pop down every open popover under `widget` (a target menu, a hotkey
/// grid).
pub fn close_popovers(widget: &gtk::Widget) {
    if let Some(popover) = widget.downcast_ref::<gtk::Popover>()
        && popover.is_visible()
    {
        popover.popdown();
    }
    let mut child = widget.first_child();
    while let Some(next) = child {
        close_popovers(&next);
        child = next.next_sibling();
    }
}
