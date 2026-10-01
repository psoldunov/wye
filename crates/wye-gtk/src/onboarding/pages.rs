//! The first-run steps (ONB-01 to ONB-05), one `AdwNavigationPage` each: a
//! centred heading and lead paragraph, then the step's rows. The pages only
//! show a [`View`] and report what the user did as an [`Intent`]; the
//! controller (`super`) decides what that means.
//!
//! KDE counterpart: crates/wye-ui/qml/onboarding/Onboarding*.qml.

use std::rc::Rc;

use adw::prelude::*;
use serde_json::Value;
use wye_api::targets::TargetInventory;

use super::browsers::BrowsersStep;
use super::desktop::Desktop;
use super::flow::Step;
use super::integration::IntegrationStep;
use super::shell::ShellState;
use super::view::View;
use crate::widgets::icon;

/// The project page, which describes how to install the extension (GEN-04
/// points there too).
pub const INSTALL_URL: &str = "https://github.com/psoldunov/wye";

/// The widest the steps' column grows.
const COLUMN_WIDTH: i32 = 480;

/// What the user did on a step.
#[derive(Debug, Clone, PartialEq)]
pub enum Intent {
    /// **Make Default** (ONB-02).
    MakeDefault,
    /// A primary browser was chosen (ONB-03).
    SetPrimary(Value),
    /// A browser was checked or unchecked for the picker: its target as
    /// JSON text (ONB-03).
    Toggle(String, bool),
    /// A checked browser was dragged from one place to another (ONB-03).
    Move(usize, usize),
    /// **Launch at login** (ONB-04).
    LaunchAtLogin(bool),
    /// **Enable** GNOME Shell integration (ONB-04).
    EnableShell,
    /// A link to open (ONB-05).
    OpenLink(&'static str),
}

/// What the pages hand their intents to.
pub type Send = Rc<dyn Fn(Intent)>;

/// What a step shows besides the [`View`].
#[derive(Debug, Clone, Copy)]
pub struct Context<'a> {
    pub view: &'a View,
    pub targets: &'a TargetInventory,
    /// The icon of the app that handles links now, if it has one (ONB-02).
    pub current_icon: Option<&'a str>,
    pub desktop: Desktop,
    pub shell: ShellState,
}

/// Every step's page and the parts that change.
#[derive(Debug)]
pub struct Steps {
    welcome: adw::NavigationPage,
    default: DefaultStep,
    browsers: BrowsersStep,
    integration: IntegrationStep,
    extension: adw::NavigationPage,
}

impl Steps {
    pub fn new(send: &Send) -> Self {
        Self {
            welcome: welcome(),
            default: DefaultStep::new(send),
            browsers: BrowsersStep::new(send),
            integration: IntegrationStep::new(send),
            extension: extension(send),
        }
    }

    /// The page of `step`.
    pub fn page(&self, step: Step) -> &adw::NavigationPage {
        match step {
            Step::Welcome => &self.welcome,
            Step::DefaultBrowser => &self.default.page,
            Step::Browsers => self.browsers.page(),
            Step::Integration => self.integration.page(),
            Step::Extension => &self.extension,
        }
    }

    /// The pages from the first step up to `last`, as the navigation view
    /// holds them while `last` shows.
    pub fn through(&self, last: Step) -> Vec<adw::NavigationPage> {
        Step::ALL
            .into_iter()
            .take(last.index() + 1)
            .map(|step| self.page(step).clone())
            .collect()
    }

    /// The step whose page is `page`.
    pub fn step_of(&self, page: &adw::NavigationPage) -> Option<Step> {
        Step::ALL.into_iter().find(|step| self.page(*step) == page)
    }

    /// Show `context` on every step.
    pub fn show(&self, context: &Context<'_>) {
        self.default.show(context.view, context.current_icon);
        self.browsers.show(context.view, context.targets);
        self.integration.show(context);
    }
}

/// A step's page: a centred heading and lead in a scrolling column; the
/// returned box takes the step's own content under them.
pub fn step_page(step: Step, heading: &str, lead: &str) -> (adw::NavigationPage, gtk::Box) {
    let column = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(18)
        .margin_top(6)
        .margin_bottom(24)
        .margin_start(12)
        .margin_end(12)
        .build();
    let title = gtk::Label::builder()
        .label(heading)
        .wrap(true)
        .justify(gtk::Justification::Center)
        .build();
    title.add_css_class("title-1");
    title.set_accessible_role(gtk::AccessibleRole::Heading);
    let intro = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(10)
        .build();
    intro.append(&title);
    if !lead.is_empty() {
        let paragraph = gtk::Label::builder()
            .label(lead)
            .wrap(true)
            .justify(gtk::Justification::Center)
            .build();
        paragraph.add_css_class("body");
        intro.append(&paragraph);
    }
    column.append(&intro);
    let page = scrolling_page(step, heading, &column);
    (page, column)
}

fn scrolling_page(step: Step, title: &str, column: &gtk::Box) -> adw::NavigationPage {
    let clamp = adw::Clamp::builder()
        .maximum_size(COLUMN_WIDTH)
        .child(column)
        .build();
    let scroller = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&clamp)
        .vexpand(true)
        .build();
    adw::NavigationPage::builder()
        .tag(tag(step))
        .title(title)
        .child(&scroller)
        .build()
}

/// The navigation tag of `step`.
fn tag(step: Step) -> &'static str {
    match step {
        Step::Welcome => "welcome",
        Step::DefaultBrowser => "default-browser",
        Step::Browsers => "browsers",
        Step::Integration => "integration",
        Step::Extension => "extension",
    }
}

/// A dimmed note under a step's rows.
pub fn note(text: &str) -> gtk::Label {
    let label = gtk::Label::builder()
        .label(text)
        .wrap(true)
        .xalign(0.0)
        .margin_start(6)
        .margin_end(6)
        .build();
    label.add_css_class("dimmed");
    label.add_css_class("caption");
    label
}

/// ONB-01: Wye's icon, the welcome and the two-sentence explanation.
fn welcome() -> adw::NavigationPage {
    let hero = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(12)
        .valign(gtk::Align::Center)
        .vexpand(true)
        .margin_bottom(24)
        .build();
    let icon = gtk::Image::builder()
        .icon_name("dev.soldunov.wye")
        .pixel_size(128)
        .margin_bottom(12)
        .accessible_role(gtk::AccessibleRole::Presentation)
        .build();
    icon.add_css_class("icon-dropshadow");
    hero.append(&icon);
    let title = gtk::Label::builder()
        .label("Welcome to Wye")
        .wrap(true)
        .justify(gtk::Justification::Center)
        .build();
    title.add_css_class("title-1");
    title.set_accessible_role(gtk::AccessibleRole::Heading);
    hero.append(&title);
    let body = gtk::Label::builder()
        .label("Wye opens every link in the browser you want. It becomes your default browser and forwards each link to the right place.")
        .wrap(true)
        .max_width_chars(44)
        .justify(gtk::Justification::Center)
        .build();
    body.add_css_class("body");
    hero.append(&body);
    let column = gtk::Box::new(gtk::Orientation::Vertical, 0);
    column.set_vexpand(true);
    column.append(&hero);
    scrolling_page(Step::Welcome, "Welcome to Wye", &column)
}

/// The size of the app icon in the default-browser row.
const APP_ICON_SIZE: i32 = 32;

/// ONB-02: "Make Wye your default browser", the current default named with
/// its icon, and **Make Default**, which turns into a check mark and "Wye is
/// your default browser" on success.
#[derive(Debug)]
struct DefaultStep {
    page: adw::NavigationPage,
    row: adw::ActionRow,
    /// Holds the app's icon, replaced on every show.
    app: gtk::Box,
    check: gtk::Image,
    button: gtk::Button,
    note: gtk::Label,
}

impl DefaultStep {
    fn new(send: &Send) -> Self {
        let (page, column) = step_page(
            Step::DefaultBrowser,
            "Make Wye your default browser",
            "When Wye is your default browser, every link you click in another app goes through it. Wye then sends the link to the right place.",
        );
        let row = adw::ActionRow::new();
        let app = gtk::Box::builder().valign(gtk::Align::Center).build();
        row.add_prefix(&app);
        let check = gtk::Image::builder()
            .icon_name("object-select-symbolic")
            .valign(gtk::Align::Center)
            .accessible_role(gtk::AccessibleRole::Presentation)
            .build();
        check.add_css_class("success");
        row.add_suffix(&check);
        let button = gtk::Button::builder()
            .label("Make Default")
            .valign(gtk::Align::Center)
            .build();
        button.add_css_class("suggested-action");
        let send = Rc::clone(send);
        button.connect_clicked(move |_| send(Intent::MakeDefault));
        row.add_suffix(&button);
        let list = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .build();
        list.add_css_class("boxed-list");
        list.append(&row);
        column.append(&list);
        let note = note(
            "You can skip this. Wye then only sees links from the clipboard and the browser extension.",
        );
        column.append(&note);
        Self {
            page,
            row,
            app,
            check,
            button,
            note,
        }
    }

    fn show(&self, view: &View, current_icon: Option<&str>) {
        let title = if view.is_default {
            "Wye is your default browser".to_owned()
        } else {
            view.current_default.as_ref().map_or_else(
                || "No default browser is set".to_owned(),
                |name| format!("Currently: {name}"),
            )
        };
        self.row.set_title(&gtk::glib::markup_escape_text(&title));
        let source = if view.is_default {
            "dev.soldunov.wye"
        } else {
            current_icon.unwrap_or("web-browser-symbolic")
        };
        while let Some(old) = self.app.first_child() {
            self.app.remove(&old);
        }
        self.app.append(&icon::image(source, APP_ICON_SIZE));
        self.check.set_visible(view.is_default);
        self.button.set_visible(!view.is_default);
        self.note.set_visible(!view.is_default);
    }
}

/// ONB-05: why the extension exists and where to get it; **Done** (the
/// footer's) closes the window and opens nothing else.
fn extension(send: &Send) -> adw::NavigationPage {
    let (page, column) = step_page(
        Step::Extension,
        "Browser extension",
        "A link you click inside a browser is opened by that browser and never reaches Wye. The optional Wye browser extension sends those links to Wye.",
    );
    let list = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .build();
    list.add_css_class("boxed-list");
    for (title, subtitle) in [
        (
            "Install for Chromium-based browsers",
            "Chrome, Chromium, Brave, Vivaldi, Edge",
        ),
        (
            "Install for Firefox-based browsers",
            "Firefox, Zen, LibreWolf, Floorp",
        ),
    ] {
        let row = adw::ActionRow::builder()
            .title(title)
            .subtitle(subtitle)
            .activatable(true)
            .build();
        row.add_prefix(
            &gtk::Image::builder()
                .icon_name("application-x-addon-symbolic")
                .build(),
        );
        row.add_suffix(
            &gtk::Image::builder()
                .icon_name("adw-external-link-symbolic")
                .accessible_role(gtk::AccessibleRole::Presentation)
                .build(),
        );
        let send = Rc::clone(send);
        row.connect_activated(move |_| send(Intent::OpenLink(INSTALL_URL)));
        list.append(&row);
    }
    column.append(&list);
    page
}
