//! What the first-run window's QML reads: one object with the step the
//! user is on and the data every step shows (ONB-01 to ONB-06).

use serde::Serialize;
use serde_json::Value;
use wye_api::AppRef;

use super::choices::{self, ListRow, PrimaryChoice};
use super::desktop::Desktop;
use super::flow::{Flow, Step};
use crate::settings::snapshot::Snapshot;

/// The whole window's state.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[allow(
    clippy::struct_excessive_bools,
    reason = "one flag per thing a button or a checkmark in the window depends on"
)]
pub struct View {
    pub step: Step,
    /// The progress dot that is lit (ONB-06).
    pub step_index: usize,
    /// How many dots there are.
    pub step_count: usize,
    /// Back is on every step after the first (ONB-06).
    pub can_go_back: bool,
    /// The last step's button is **Done** (ONB-05).
    pub is_last: bool,
    /// Wye handles `https` links (ONB-02).
    pub is_default: bool,
    /// The app that handles them otherwise, for "Currently: Firefox".
    pub current_default: Option<String>,
    /// The user chose to keep another default (ONB-02, ONB-11).
    pub kept_current: bool,
    /// The primary browser menu (ONB-03).
    pub primary: Vec<PrimaryChoice>,
    /// The name of the chosen primary browser.
    pub primary_name: String,
    /// The browsers and profiles for the picker (ONB-03).
    pub checklist: Vec<ListRow>,
    /// The **Launch at login** switch (ONB-04).
    pub launch_at_login: bool,
    /// Login start is managed outside Wye (`Status.loginManaged`, the Nix
    /// modules): the switch is disabled, shows `launch_at_login` as the
    /// modules set it, and saves nothing (GEN-01, ONB-04).
    pub login_managed: bool,
    /// The desktop's note under the switch, if it needs one (ONB-04).
    pub desktop_note: Option<&'static str>,
    /// The configuration can be changed; false for a read-only file.
    pub writable: bool,
}

impl View {
    /// The window for `flow` and what the service said.
    #[must_use]
    pub fn build(flow: Flow, snapshot: &Snapshot, desktop: Desktop) -> Self {
        let foreign = choices::foreign_app_ids(&snapshot.services.services);
        let registration = &snapshot.status.default_browser;
        // The browser Wye replaced: still the current default, or, once Wye
        // took over, the one it remembers.
        let replaced: Option<&AppRef> = registration
            .previous
            .as_ref()
            .or(registration.current.as_ref());
        let primary = choices::primary_choices(
            &snapshot.targets,
            &foreign,
            replaced,
            snapshot.config.pointer("/browsers/primary"),
        );
        let shown = choices::effective_shown(&snapshot.config, &snapshot.targets, &foreign);
        let step = flow.step();
        Self {
            step,
            step_index: step.index(),
            step_count: Step::ALL.len(),
            can_go_back: flow.can_go_back(),
            is_last: flow.is_last(),
            is_default: registration.is_default,
            current_default: registration.current.as_ref().map(|app| app.name.clone()),
            kept_current: registration.kept_current,
            primary_name: primary
                .iter()
                .find(|choice| choice.checked)
                .map(|choice| choice.name.clone())
                .unwrap_or_default(),
            primary,
            checklist: choices::checklist(&snapshot.targets, &foreign, &shown),
            launch_at_login: if snapshot.status.login_managed {
                snapshot.status.login_managed_on
            } else {
                launch_at_login(&snapshot.config)
            },
            login_managed: snapshot.status.login_managed,
            desktop_note: desktop.note(),
            writable: snapshot.writable(),
        }
    }
}

/// `general.launch-at-login`, on unless the configuration says otherwise
/// (GEN-01).
fn launch_at_login(config: &Value) -> bool {
    config
        .pointer("/general/launch-at-login")
        .and_then(Value::as_bool)
        .unwrap_or(true)
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use wye_api::status::{DefaultBrowserStatus, Status};

    use super::*;

    fn app(id: &str, name: &str) -> AppRef {
        AppRef {
            id: id.to_owned(),
            name: name.to_owned(),
            icon: None,
        }
    }

    fn snapshot(status: Status, config: Value) -> Snapshot {
        Snapshot {
            config,
            status,
            ..Snapshot::default()
        }
    }

    #[test]
    fn the_welcome_step_has_no_back_and_five_dots() {
        // ONB-01, ONB-06
        let view = View::build(Flow::new(), &Snapshot::default(), Desktop::Kde);
        assert_eq!(view.step, Step::Welcome);
        assert!(!view.can_go_back);
        assert_eq!(view.step_count, 5);
        assert_eq!(view.step_index, 0);
    }

    #[test]
    fn the_default_step_names_the_current_default() {
        // ONB-02
        let status = Status {
            default_browser: DefaultBrowserStatus {
                current: Some(app("firefox.desktop", "Firefox")),
                ..DefaultBrowserStatus::default()
            },
            ..Status::default()
        };
        let view = View::build(Flow::new(), &snapshot(status, json!({})), Desktop::Kde);
        assert!(!view.is_default);
        assert_eq!(view.current_default.as_deref(), Some("Firefox"));
    }

    #[test]
    fn launch_at_login_is_on_unless_the_configuration_says_otherwise() {
        // ONB-04, GEN-01
        let on = View::build(Flow::new(), &Snapshot::default(), Desktop::Kde);
        assert!(on.launch_at_login);
        let off = snapshot(
            Status::default(),
            json!({"general": {"launch-at-login": false}}),
        );
        assert!(!View::build(Flow::new(), &off, Desktop::Kde).launch_at_login);
    }

    #[test]
    fn onb_04_a_managed_login_start_shows_what_nix_sets_not_the_file() {
        let managed = snapshot(
            Status {
                login_managed: true,
                ..Status::default()
            },
            json!({"general": {"launch-at-login": false}}),
        );
        let view = View::build(Flow::new(), &managed, Desktop::Kde);
        assert!(view.login_managed);
        assert!(!view.launch_at_login, "Nix does not start it");

        let started = snapshot(
            Status {
                login_managed: true,
                login_managed_on: true,
                ..Status::default()
            },
            json!({"general": {"launch-at-login": false}}),
        );
        let view = View::build(Flow::new(), &started, Desktop::Kde);
        assert!(
            view.launch_at_login,
            "Nix starts it, whatever the file says"
        );
    }

    #[test]
    fn the_picker_is_the_chosen_primary_until_the_user_chooses_another() {
        // ONB-03
        let view = View::build(Flow::new(), &Snapshot::default(), Desktop::Kde);
        assert_eq!(view.primary_name, "Picker");
    }

    #[test]
    fn a_read_only_file_is_reported_and_the_desktop_note_follows_the_desktop() {
        let mut status = Status::default();
        status.config.writable = false;
        let view = View::build(Flow::new(), &snapshot(status, json!({})), Desktop::Gnome);
        assert!(!view.writable);
        assert!(view.desktop_note.is_some());
    }
}
