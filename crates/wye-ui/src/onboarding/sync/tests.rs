use std::mem;
use std::sync::Mutex;

use super::*;

/// A service in memory: one configuration revision, calls recorded.
struct Fake {
    state: Mutex<State>,
}

#[allow(
    clippy::struct_excessive_bools,
    reason = "a fake service: one flag per behaviour a test switches on"
)]
#[derive(Default)]
struct State {
    revision: u64,
    is_default: bool,
    kept: bool,
    conflict_once: bool,
    read_only: bool,
    calls: Vec<String>,
}

impl State {
    /// One configuration write on top of revision `base`: refused when the
    /// file is managed elsewhere, stale once when a test asks for it.
    fn write(&mut self, base: u64) -> Result<u64, Error> {
        if self.read_only {
            return Err(Error::ReadOnly("home-manager".into()));
        }
        if mem::take(&mut self.conflict_once) {
            self.revision += 1;
            return Err(Error::Conflict("stale".into()));
        }
        if base != self.revision {
            return Err(Error::Conflict("stale".into()));
        }
        self.revision += 1;
        Ok(self.revision)
    }
}

impl Fake {
    fn new() -> Self {
        Self {
            state: Mutex::new(State {
                revision: 4,
                ..State::default()
            }),
        }
    }

    fn with(change: impl FnOnce(&mut State)) -> Self {
        let fake = Self::new();
        change(&mut fake.state.lock().expect("lock"));
        fake
    }

    fn calls(&self) -> Vec<String> {
        self.state.lock().expect("lock").calls.clone()
    }
}

#[allow(
    clippy::unused_async_trait_impl,
    reason = "the fake service answers at once; the trait's methods are async"
)]
impl Api for Fake {
    async fn status(&self) -> Result<String, Error> {
        let state = self.state.lock().expect("lock");
        Ok(json!({
            "defaultBrowser": {"isDefault": state.is_default, "keptCurrent": state.kept},
            "uiState": {"onboardingDone": false}
        })
        .to_string())
    }

    async fn get_config(&self) -> Result<(String, u64), Error> {
        Ok(("{}".to_owned(), self.state.lock().expect("lock").revision))
    }

    async fn update_config(&self, patch: &str, base: u64) -> Result<u64, Error> {
        let mut state = self.state.lock().expect("lock");
        state.calls.push(format!("update {patch} @{base}"));
        state.write(base)
    }

    async fn get_targets(&self) -> Result<String, Error> {
        Ok(r#"{"targets": []}"#.to_owned())
    }

    async fn get_services(&self) -> Result<String, Error> {
        Ok(r#"{"services": []}"#.to_owned())
    }

    async fn make_default(&self) -> Result<(), Error> {
        let mut state = self.state.lock().expect("lock");
        state.calls.push("make-default".to_owned());
        if state.read_only {
            return Err(Error::ReadOnly("mimeapps.list is a symlink".into()));
        }
        state.is_default = true;
        Ok(())
    }

    async fn keep_current_default(&self) -> Result<(), Error> {
        let mut state = self.state.lock().expect("lock");
        state.calls.push("keep-current".to_owned());
        state.kept = true;
        Ok(())
    }

    async fn update_ui_state(&self, patch: &str) -> Result<(), Error> {
        self.state
            .lock()
            .expect("lock")
            .calls
            .push(format!("ui-state {patch}"));
        Ok(())
    }
}

fn block_on<T>(future: impl Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("runtime")
        .block_on(future)
}

fn loaded(fake: &Fake) -> Snapshot {
    block_on(load(fake)).expect("load")
}

#[test]
fn load_reads_the_configuration_status_and_inventory() {
    let snapshot = loaded(&Fake::new());
    assert_eq!(snapshot.revision, 4);
    assert!(!snapshot.status.default_browser.is_default);
}

/// What one step's button asks of the service, and how the status reads after.
struct Choice {
    action: Action,
    call: &'static str,
    shows: fn(&Snapshot) -> bool,
}

#[test]
fn each_button_calls_the_service_and_turns_the_status_over() {
    // ONB-02, DEF-02, ONB-11: Make Default, Skip; ONB-06: Done
    let choices = [
        Choice {
            action: Action::MakeDefault,
            call: "make-default",
            shows: |next| next.status.default_browser.is_default,
        },
        Choice {
            action: Action::KeepCurrentDefault,
            call: "keep-current",
            shows: |next| next.status.default_browser.kept_current,
        },
        Choice {
            action: Action::Finish,
            call: r#"ui-state {"onboardingDone":true}"#,
            shows: |next| next.status.ui_state.onboarding_done,
        },
    ];
    for choice in choices {
        let fake = Fake::new();
        let next = block_on(run(&fake, choice.action, &loaded(&fake))).expect("run");
        assert_eq!(fake.calls(), [choice.call]);
        assert!((choice.shows)(&next), "{}", choice.call);
    }
}

#[test]
fn a_managed_file_says_read_only() {
    // ONB-02: the step shows the reason and lets the user skip; ONB-03, ONB-04
    for action in [Action::MakeDefault, Action::Patch(json!({}))] {
        let fake = Fake::with(|state| state.read_only = true);
        let result = block_on(run(&fake, action, &loaded(&fake)));
        assert!(matches!(result, Err(Error::ReadOnly(_))), "{result:?}");
    }
}

#[test]
fn a_patch_is_saved_on_the_known_revision_and_read_back() {
    // ONB-03, ONB-04
    let fake = Fake::new();
    let patch = json!({"general": {"launch-at-login": false}});
    let next = block_on(run(&fake, Action::Patch(patch), &loaded(&fake))).expect("run");
    assert_eq!(
        fake.calls(),
        [r#"update {"general":{"launch-at-login":false}} @4"#]
    );
    assert_eq!(next.revision, 5);
}

#[test]
fn a_stale_revision_applies_the_patch_again_once() {
    let fake = Fake::with(|state| state.conflict_once = true);
    let snapshot = loaded(&fake);
    let patch = json!({"browsers": {"primary": {"picker": true}}});
    let next = block_on(run(&fake, Action::Patch(patch), &snapshot)).expect("run");
    assert_eq!(fake.calls().len(), 2);
    assert!(fake.calls()[1].ends_with("@5"), "{:?}", fake.calls());
    assert_eq!(next.revision, 6);
}

#[test]
fn a_preview_shows_the_choice_before_the_service_answers() {
    // ONB-03, ONB-06
    let snapshot = Snapshot::default();
    let patch = json!({"general": {"launch-at-login": false}});
    let shown = preview(&snapshot, &Action::Patch(patch));
    assert_eq!(shown.config["general"]["launch-at-login"], false);
    assert!(
        preview(&snapshot, &Action::Finish)
            .status
            .ui_state
            .onboarding_done
    );
    assert!(
        preview(&snapshot, &Action::MakeDefault)
            .status
            .default_browser
            .is_default
    );
    assert!(
        preview(&snapshot, &Action::KeepCurrentDefault)
            .status
            .default_browser
            .kept_current
    );
    // The original is untouched.
    assert!(!snapshot.status.default_browser.is_default);
}
