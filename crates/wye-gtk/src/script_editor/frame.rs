//! The editor window's frame (SCR-01): an `AdwApplicationWindow` of about
//! 680 × 600, resizable, titled "Transform Script — Global" or
//! "Transform Script — <rule name>".
//!
//! - Header bar: Undo and Redo at the start; the script's name as the title
//!   with "Global" or the rule below it (and "Edited" while there are
//!   unsaved changes); at the end the **Reference** toggle (SCR-06),
//!   **Save** (the suggested action, SCR-07) and the main menu (Revert to
//!   Saved, Run Test).
//! - Banners under it: the file changed on disk, with **Reload** (SCR-08);
//!   a failed save or load, with **Dismiss**.
//! - The content: the code card over the Test group, with the Reference as
//!   the split view's sidebar at the end; below 760 px it overlays the code.
//!   While the script loads a spinner stands in, and a script that cannot
//!   be read shows why with **Try Again**.
//!
//! Shortcuts: Ctrl+S saves, F1 toggles the Reference, Ctrl+Return runs the
//! test, Escape closes the overlaid Reference and otherwise the window, as
//! Ctrl+W does (SET-07).
//!
//! KDE counterpart: crates/wye-ui/qml/script/ScriptEditorWindow.qml.

use adw::prelude::*;
use gtk::{gio, glib};

use super::code::Code;
use super::reference;
use super::test_group::TestGroup;
use crate::widgets::empty_state;

/// SCR-01: the default size. Taller than KDE's 520 px, for libadwaita's
/// taller rows.
const DEFAULT_SIZE: (i32, i32) = (680, 600);
/// The smallest size that still shows the code and the test.
const MINIMUM_SIZE: (i32, i32) = (440, 420);
/// Below this width (in sp) the Reference overlays the code.
const SIDE_BY_SIDE: f64 = 760.0;
/// The Reference's width.
/// Wide enough for the examples' longest line (SCR-06).
const SIDEBAR_WIDTH: (f64, f64) = (380.0, 420.0);
const SIDEBAR_FRACTION: f64 = 0.4;

/// The pages of the window's stack.
pub mod page {
    pub const LOADING: &str = "loading";
    pub const EDITOR: &str = "editor";
    pub const FAILED: &str = "failed";
}

/// The window's actions, as `win.<name>`.
pub mod action {
    pub const SAVE: &str = "save";
    pub const REVERT: &str = "revert";
    pub const RUN_TEST: &str = "run-test";
    pub const REFERENCE: &str = "reference";
}

/// The window's widgets; the controller fills them.
#[derive(Debug, Clone)]
pub struct Frame {
    pub window: adw::ApplicationWindow,
    pub heading: adw::WindowTitle,
    pub undo: gtk::Button,
    pub redo: gtk::Button,
    pub save: gtk::Button,
    pub external: adw::Banner,
    pub error: adw::Banner,
    pub stack: gtk::Stack,
    pub failed: adw::StatusPage,
    pub retry: gtk::Button,
    pub split: adw::OverlaySplitView,
    pub code: Code,
    pub test: TestGroup,
}

/// The header bar and its controls.
struct Header {
    bar: adw::HeaderBar,
    heading: adw::WindowTitle,
    undo: gtk::Button,
    redo: gtk::Button,
    save: gtk::Button,
    reference: gtk::ToggleButton,
    menu: gtk::MenuButton,
}

impl Header {
    fn new() -> Self {
        let heading = adw::WindowTitle::new("Transform Script", "");
        let undo = icon_button("edit-undo-symbolic", "Undo (Ctrl+Z)", "Undo");
        let redo = icon_button("edit-redo-symbolic", "Redo (Ctrl+Shift+Z)", "Redo");
        let save = gtk::Button::builder()
            .label("_Save")
            .use_underline(true)
            .action_name(format!("win.{}", action::SAVE))
            .tooltip_text("Save (Ctrl+S)")
            .build();
        save.add_css_class("suggested-action");
        let reference = gtk::ToggleButton::builder()
            .child(
                &adw::ButtonContent::builder()
                    .icon_name("sidebar-show-right-symbolic")
                    .label("Reference")
                    .build(),
            )
            .tooltip_text("The script API and examples (F1)")
            .build();
        let menu = gtk::MenuButton::builder()
            .icon_name("open-menu-symbolic")
            .menu_model(&main_menu())
            .tooltip_text("Main Menu")
            .primary(true)
            .build();
        let bar = adw::HeaderBar::builder().title_widget(&heading).build();
        bar.pack_start(&undo);
        bar.pack_start(&redo);
        bar.pack_end(&menu);
        bar.pack_end(&save);
        bar.pack_end(&reference);
        Self {
            bar,
            heading,
            undo,
            redo,
            save,
            reference,
            menu,
        }
    }

    /// The controls that act on the script, shown only with the editor.
    fn controls(&self) -> [gtk::Widget; 5] {
        [
            self.undo.clone().upcast(),
            self.redo.clone().upcast(),
            self.save.clone().upcast(),
            self.reference.clone().upcast(),
            self.menu.clone().upcast(),
        ]
    }
}

/// The code over the Test group, with the Reference as the sidebar. The
/// code takes the height the compact Test group leaves (as on KDE).
fn split_view(code: &Code, test: &TestGroup) -> adw::OverlaySplitView {
    let editor = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(12)
        .margin_top(12)
        .margin_bottom(12)
        .margin_start(12)
        .margin_end(12)
        .build();
    editor.append(&code.widget);
    editor.append(&test.widget);
    adw::OverlaySplitView::builder()
        .content(&editor)
        .sidebar(&reference::pane())
        .sidebar_position(gtk::PackType::End)
        .min_sidebar_width(SIDEBAR_WIDTH.0)
        .max_sidebar_width(SIDEBAR_WIDTH.1)
        .sidebar_width_fraction(SIDEBAR_FRACTION)
        .show_sidebar(false)
        .build()
}

/// The loading spinner, the editor and the failure page; the script's
/// controls show only with the editor.
fn pages(
    editor: &impl IsA<gtk::Widget>,
    controls: [gtk::Widget; 5],
) -> (gtk::Stack, adw::StatusPage, gtk::Button) {
    let spinner = adw::Spinner::builder()
        .width_request(32)
        .height_request(32)
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Center)
        .build();
    let failed = empty_state::empty_state("dialog-error-symbolic", "Cannot Open the Script", "");
    failed.remove_css_class("compact");
    let retry = empty_state::with_action(&failed, "Try Again");
    let stack = gtk::Stack::builder()
        .transition_type(gtk::StackTransitionType::Crossfade)
        .vexpand(true)
        .build();
    stack.add_named(&spinner, Some(page::LOADING));
    stack.add_named(editor, Some(page::EDITOR));
    stack.add_named(&failed, Some(page::FAILED));
    stack.connect_visible_child_name_notify(move |stack| {
        let editing = stack.visible_child_name().as_deref() == Some(page::EDITOR);
        for control in &controls {
            control.set_visible(editing);
        }
    });
    (stack, failed, retry)
}

impl Frame {
    /// Build the window for `app`, hidden.
    pub fn new(app: &adw::Application) -> Self {
        let header = Header::new();
        let external = adw::Banner::builder()
            .button_label("_Reload")
            .use_markup(false)
            .build();
        let error = adw::Banner::builder()
            .button_label("_Dismiss")
            .use_markup(false)
            .build();
        error.connect_button_clicked(|banner| banner.set_revealed(false));
        let code = Code::new();
        let test = TestGroup::new();
        let split = split_view(&code, &test);
        header
            .reference
            .bind_property("active", &split, "show-sidebar")
            .bidirectional()
            .sync_create()
            .build();
        let (stack, failed, retry) = pages(&split, header.controls());
        let view = adw::ToolbarView::builder().content(&stack).build();
        view.add_top_bar(&header.bar);
        view.add_top_bar(&external);
        view.add_top_bar(&error);
        let window = adw::ApplicationWindow::builder()
            .application(app)
            .title("Transform Script")
            .default_width(DEFAULT_SIZE.0)
            .default_height(DEFAULT_SIZE.1)
            .width_request(MINIMUM_SIZE.0)
            .height_request(MINIMUM_SIZE.1)
            .hide_on_close(true)
            .content(&view)
            .build();
        window.add_css_class("wye-script-editor");
        let narrow = adw::Breakpoint::new(adw::BreakpointCondition::new_length(
            adw::BreakpointConditionLengthType::MaxWidth,
            SIDE_BY_SIDE,
            adw::LengthUnit::Sp,
        ));
        narrow.add_setter(&split, "collapsed", Some(&true.to_value()));
        window.add_breakpoint(narrow);
        let frame = Self {
            window,
            heading: header.heading,
            undo: header.undo,
            redo: header.redo,
            save: header.save,
            external,
            error,
            stack,
            failed,
            retry,
            split,
            code,
            test,
        };
        frame.bind_undo();
        frame.add_shortcuts();
        frame
    }

    /// Add `win.<name>` running `then`.
    pub fn add_action(&self, name: &str, then: impl Fn() + 'static) -> gio::SimpleAction {
        let action = gio::SimpleAction::new(name, None);
        action.connect_activate(move |_, _| then());
        self.window.add_action(&action);
        action
    }

    /// Undo and Redo follow the buffer, and give the focus back to the code.
    fn bind_undo(&self) {
        let buffer = &self.code.buffer;
        buffer
            .bind_property("can-undo", &self.undo, "sensitive")
            .sync_create()
            .build();
        buffer
            .bind_property("can-redo", &self.redo, "sensitive")
            .sync_create()
            .build();
        let view = self.code.view.clone();
        self.undo.connect_clicked(glib::clone!(
            #[weak]
            view,
            move |_| {
                view.buffer().undo();
                view.grab_focus();
            }
        ));
        self.redo.connect_clicked(glib::clone!(
            #[weak]
            view,
            move |_| {
                view.buffer().redo();
                view.grab_focus();
            }
        ));
    }

    /// Escape, Ctrl+W, Ctrl+S, F1 and Ctrl+Return. In the capture phase, so
    /// the code area does not take Ctrl+Return as a newline; Escape is left to
    /// a dialog that is open.
    fn add_shortcuts(&self) {
        let controller = gtk::ShortcutController::new();
        controller.set_propagation_phase(gtk::PropagationPhase::Capture);
        let escape = gtk::CallbackAction::new(glib::clone!(
            #[weak(rename_to = split)]
            self.split,
            #[upgrade_or]
            glib::Propagation::Proceed,
            move |widget, _| {
                let Some(window) = widget.downcast_ref::<adw::ApplicationWindow>() else {
                    return glib::Propagation::Proceed;
                };
                if window.visible_dialog().is_some() {
                    return glib::Propagation::Proceed;
                }
                if split.is_collapsed() && split.shows_sidebar() {
                    split.set_show_sidebar(false);
                } else {
                    window.close();
                }
                glib::Propagation::Stop
            }
        ));
        let close = gtk::CallbackAction::new(|widget, _| {
            if let Some(window) = widget.downcast_ref::<gtk::Window>() {
                window.close();
            }
            glib::Propagation::Stop
        });
        let shortcuts: [(&str, gtk::ShortcutAction); 5] = [
            ("Escape", escape.upcast()),
            ("<Control>w", close.upcast()),
            ("<Control>s", named(action::SAVE)),
            ("F1", named(action::REFERENCE)),
            ("<Control>Return|<Control>KP_Enter", named(action::RUN_TEST)),
        ];
        for (trigger, action) in shortcuts {
            controller.add_shortcut(gtk::Shortcut::new(
                gtk::ShortcutTrigger::parse_string(trigger),
                Some(action),
            ));
        }
        self.window.add_controller(controller);
    }
}

fn named(name: &str) -> gtk::ShortcutAction {
    gtk::NamedAction::new(&format!("win.{name}")).upcast()
}

fn icon_button(icon: &str, tooltip: &str, label: &str) -> gtk::Button {
    let button = gtk::Button::builder()
        .icon_name(icon)
        .tooltip_text(tooltip)
        .build();
    button.update_property(&[gtk::accessible::Property::Label(label)]);
    button
}

/// The main menu: Revert to Saved and Run Test, then the Reference.
fn main_menu() -> gio::Menu {
    let menu = gio::Menu::new();
    let edit = gio::Menu::new();
    edit.append(
        Some("_Revert to Saved"),
        Some(&format!("win.{}", action::REVERT)),
    );
    edit.append(
        Some("Run _Test"),
        Some(&format!("win.{}", action::RUN_TEST)),
    );
    menu.append_section(None, &edit);
    let help = gio::Menu::new();
    help.append(
        Some("Script _Reference"),
        Some(&format!("win.{}", action::REFERENCE)),
    );
    menu.append_section(None, &help);
    menu
}
