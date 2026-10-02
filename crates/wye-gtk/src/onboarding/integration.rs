//! ONB-04, the desktop integration step: **Launch at login** (on), locked
//! with the reason when the Nix modules manage it (GEN-01). On GNOME, while
//! Wye's Shell extension is not enabled: "Enable GNOME Shell integration"
//! with **Enable**, and what it adds; once it runs, a check mark. On other
//! desktops without a tray host: the note that the tray icon needs one.
//!
//! KDE counterpart: crates/wye-ui/qml/onboarding/OnboardingIntegration.qml.

use std::cell::Cell;
use std::rc::Rc;

use adw::prelude::*;
use gtk::glib;

use super::desktop::Desktop;
use super::flow::Step;
use super::pages::{self, Context, Intent, Send};
use super::shell::ShellState;
use crate::widgets::row::{self, LeadingIcon, Tint};

/// ONB-04: what GNOME Shell integration adds.
const SHELL_ADDS: &str =
    "Adds the tray icon, the picker at the pointer, held keys and clipboard features.";

/// The integration step.
#[derive(Debug)]
pub struct IntegrationStep {
    page: adw::NavigationPage,
    login: adw::SwitchRow,
    managed: adw::ActionRow,
    /// Set while the switch shows the view, so that is not taken as a change.
    showing: Rc<Cell<bool>>,
    shell_group: adw::PreferencesGroup,
    shell: adw::ActionRow,
    shell_glyph: LeadingIcon,
    enable: gtk::Button,
    note: adw::ActionRow,
}

impl IntegrationStep {
    pub fn new(send: &Send) -> Self {
        let (page, column) = pages::step_page(
            Step::Integration,
            "Start with your desktop",
            "Wye works best when it is already running when you click a link.",
        );
        let login = adw::SwitchRow::builder().title("Launch at login").build();
        let showing = Rc::new(Cell::new(false));
        login.connect_active_notify(glib::clone!(
            #[strong]
            showing,
            #[strong]
            send,
            move |row| {
                if !showing.get() {
                    send(Intent::LaunchAtLogin(row.is_active()));
                }
            }
        ));
        // Why the switch is locked, in a row of its own so the reason is not
        // dimmed with the switch.
        let managed = adw::ActionRow::new();
        managed.add_prefix(
            &gtk::Image::builder()
                .icon_name("changes-prevent-symbolic")
                .accessible_role(gtk::AccessibleRole::Presentation)
                .build(),
        );
        managed.add_css_class("property");
        let login_group = adw::PreferencesGroup::new();
        login_group.add(&login);
        login_group.add(&managed);
        column.append(&login_group);

        let shell = adw::ActionRow::builder().subtitle(SHELL_ADDS).build();
        let shell_glyph = row::add_leading_icon(&shell);
        let enable = gtk::Button::builder()
            .label("Enable")
            .valign(gtk::Align::Center)
            .build();
        let enabling = Rc::clone(send);
        enable.connect_clicked(move |_| enabling(Intent::EnableShell));
        shell.add_suffix(&enable);
        let note = adw::ActionRow::new();
        note.add_prefix(
            &gtk::Image::builder()
                .icon_name("dialog-information-symbolic")
                .accessible_role(gtk::AccessibleRole::Presentation)
                .build(),
        );
        let shell_group = adw::PreferencesGroup::new();
        shell_group.add(&shell);
        shell_group.add(&note);
        column.append(&shell_group);
        Self {
            page,
            login,
            managed,
            showing,
            shell_group,
            shell,
            shell_glyph,
            enable,
            note,
        }
    }

    pub const fn page(&self) -> &adw::NavigationPage {
        &self.page
    }

    pub fn show(&self, context: &Context<'_>) {
        let view = context.view;
        self.showing.set(true);
        self.login.set_active(view.launch_at_login);
        self.showing.set(false);
        // GEN-01: the Nix modules decide login start; the switch shows what
        // they set and saves nothing.
        self.login
            .set_sensitive(view.writable && !view.login_managed);
        self.managed.set_visible(view.login_managed);
        self.managed.set_title(if view.launch_at_login {
            "Your Nix configuration starts Wye at login. Change it there."
        } else {
            "Your Nix configuration does not start Wye at login. Change it there."
        });
        self.show_desktop(context);
    }

    fn show_desktop(&self, context: &Context<'_>) {
        let gnome = context.desktop == Desktop::Gnome;
        let shell = if gnome {
            context.shell
        } else {
            ShellState::Unknown
        };
        let (title, subtitle) = match shell {
            ShellState::Unknown => ("", SHELL_ADDS.to_owned()),
            ShellState::Missing => (
                "GNOME Shell integration",
                format!("Install Wye’s Shell extension to use it. {SHELL_ADDS}"),
            ),
            ShellState::Disabled => ("Enable GNOME Shell integration", SHELL_ADDS.to_owned()),
            ShellState::Enabled => ("GNOME Shell integration is on", SHELL_ADDS.to_owned()),
            ShellState::Broken => (
                "GNOME Shell integration is not running",
                format!(
                    "GNOME Shell could not run Wye’s Shell extension; it may not support this GNOME version. {SHELL_ADDS}"
                ),
            ),
        };
        self.shell.set_visible(shell != ShellState::Unknown);
        self.shell.set_title(title);
        self.shell.set_subtitle(&subtitle);
        match shell {
            ShellState::Enabled => self
                .shell_glyph
                .set("object-select-symbolic", Tint::Success),
            ShellState::Missing | ShellState::Broken => self
                .shell_glyph
                .set("dialog-warning-symbolic", Tint::Warning),
            ShellState::Disabled | ShellState::Unknown => {
                self.shell_glyph
                    .set("application-x-addon-symbolic", Tint::Accent);
            }
        }
        self.enable.set_visible(shell == ShellState::Disabled);
        // ONB-04: other desktops get their note; on GNOME the Shell row says
        // it, and Plasma has a tray.
        let note = if gnome {
            None
        } else {
            context.view.desktop_note
        };
        self.note.set_visible(note.is_some());
        self.note.set_title(note.unwrap_or_default());
        self.shell_group
            .set_visible(self.shell.is_visible() || self.note.is_visible());
    }
}
