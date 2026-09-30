#![allow(
    clippy::unused_async_trait_impl,
    reason = "the fake service answers at once; the trait's methods are async"
)]

use std::sync::Mutex;

use super::*;

/// A service in memory: one revision, calls recorded.
#[derive(Default)]
struct Fake {
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    revision: u64,
    inventory: u64,
    /// Answer the first `update_config` with `Conflict` (someone else wrote).
    conflict_once: bool,
    read_only: bool,
    calls: Vec<String>,
}

impl Fake {
    fn with(revision: u64, inventory: u64) -> Self {
        Self {
            state: Mutex::new(State {
                revision,
                inventory,
                ..State::default()
            }),
        }
    }

    fn calls(&self) -> Vec<String> {
        self.state.lock().expect("lock").calls.clone()
    }

    fn record(&self, call: &str) {
        self.state.lock().expect("lock").calls.push(call.to_owned());
    }
}

impl Api for Fake {
    async fn update_config(&self, patch: &str, base: u64) -> Result<u64, Error> {
        let mut state = self.state.lock().expect("lock");
        state.calls.push(format!("update {patch} @{base}"));
        if state.read_only {
            return Err(Error::ReadOnly("home-manager".into()));
        }
        if state.conflict_once {
            state.conflict_once = false;
            state.revision += 1;
            return Err(Error::Conflict("stale".into()));
        }
        if base != 0 && base != state.revision {
            return Err(Error::Conflict("stale".into()));
        }
        state.revision += 1;
        Ok(state.revision)
    }

    async fn get_config(&self) -> Result<(String, u64), Error> {
        self.record("get-config");
        Ok(("{}".to_owned(), self.state.lock().expect("lock").revision))
    }

    async fn status(&self) -> Result<String, Error> {
        self.record("status");
        Ok("{}".to_owned())
    }

    async fn config_revision(&self) -> Result<u64, Error> {
        Ok(self.state.lock().expect("lock").revision)
    }

    async fn inventory_revision(&self) -> Result<u64, Error> {
        Ok(self.state.lock().expect("lock").inventory)
    }

    async fn get_targets(&self) -> Result<String, Error> {
        self.record("get-targets");
        Ok("{}".to_owned())
    }

    async fn get_services(&self) -> Result<String, Error> {
        self.record("get-services");
        Ok("{}".to_owned())
    }

    async fn rescan(&self) -> Result<(), Error> {
        self.record("rescan");
        self.state.lock().expect("lock").inventory += 1;
        Ok(())
    }

    async fn make_default(&self) -> Result<(), Error> {
        self.record("make-default");
        Ok(())
    }

    async fn stop_being_default(&self) -> Result<(), Error> {
        self.record("stop-being-default");
        Err(Error::NotFound("no previous browser".into()))
    }

    async fn quit(&self) -> Result<(), Error> {
        self.record("quit");
        Ok(())
    }
}

fn block_on<T>(future: impl Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("runtime")
        .block_on(future)
}

#[test]
fn a_patch_is_saved_on_the_revision_the_window_knows() {
    // SET-06
    let api = Fake::with(3, 1);
    let (_, revision) = block_on(save(&api, r#"{"a":1}"#, 3)).expect("saved");
    assert_eq!(revision, 4);
    assert_eq!(api.calls(), [r#"update {"a":1} @3"#, "get-config"]);
}

/// Save `{"a":1}` on revision 3 against a fake that misbehaves as `change`
/// says, and what the fake saw.
fn save_where(change: impl FnOnce(&mut State)) -> (Result<(String, u64), Error>, Vec<String>) {
    let api = Fake::with(3, 1);
    change(&mut api.state.lock().expect("lock"));
    let result = block_on(save(&api, r#"{"a":1}"#, 3));
    (result, api.calls())
}

#[test]
fn a_conflict_reloads_and_applies_the_patch_again() {
    // the brief: on Conflict reload and re-apply
    let (result, calls) = save_where(|state| state.conflict_once = true);
    assert_eq!(result.expect("saved").1, 5);
    assert_eq!(
        calls,
        [
            r#"update {"a":1} @3"#,
            "get-config",
            r#"update {"a":1} @4"#,
            "get-config",
        ]
    );
}

#[test]
fn a_read_only_file_is_reported_not_retried() {
    // Status `writable: false`, home-manager
    let (result, calls) = save_where(|state| state.read_only = true);
    let error = result.expect_err("refused");
    assert!(matches!(error, Error::ReadOnly(_)), "{error:?}");
    assert_eq!(calls.len(), 1);
    assert!(describe(&error).contains("read-only"));
}

#[test]
fn polling_reads_only_what_moved() {
    let api = Fake::with(3, 7);
    let quiet = block_on(poll(
        &api,
        Known {
            config: 3,
            inventory: 7,
        },
    ))
    .expect("poll");
    assert!(quiet.config.is_none() && quiet.inventory.is_none());
    assert_eq!(api.calls(), ["status"]);
    let all = block_on(poll(&api, Known::default())).expect("poll");
    assert_eq!(all.config.as_ref().map(|(_, r)| *r), Some(3));
    assert_eq!(all.inventory.as_ref().map(|i| i.revision), Some(7));
}

#[test]
fn a_delta_knows_the_revisions_it_brought() {
    let delta = Delta {
        status: "{}".into(),
        config: Some(("{}".into(), 9)),
        inventory: None,
    };
    assert_eq!(
        delta.known(Known {
            config: 1,
            inventory: 4
        }),
        Known {
            config: 9,
            inventory: 4
        }
    );
}

#[test]
fn rescan_reads_the_new_inventory() {
    // BRW-06
    let api = Fake::with(3, 7);
    let delta = block_on(run(
        &api,
        Action::Rescan,
        Known {
            config: 3,
            inventory: 7,
        },
    ))
    .expect("run");
    assert_eq!(delta.and_then(|d| d.inventory).map(|i| i.revision), Some(8));
}

#[test]
fn make_default_then_reads_the_status() {
    // GEN-05
    let api = Fake::with(3, 7);
    block_on(run(
        &api,
        Action::MakeDefault,
        Known {
            config: 3,
            inventory: 7,
        },
    ))
    .expect("run");
    assert_eq!(api.calls(), ["make-default", "status"]);
}

#[test]
fn restoring_with_nothing_remembered_is_an_error_the_window_can_show() {
    // DEF-05: NotFound when no previous browser is remembered
    let api = Fake::with(3, 7);
    let error =
        block_on(run(&api, Action::StopBeingDefault, Known::default())).expect_err("refused");
    assert_eq!(describe(&error), "no previous browser");
}

#[test]
fn quitting_reads_nothing_afterwards() {
    // KEY-50 Ctrl+Q: the service is gone, there is no status to read
    let api = Fake::with(3, 7);
    assert!(
        block_on(run(&api, Action::Quit, Known::default()))
            .expect("quit")
            .is_none()
    );
    assert_eq!(api.calls(), ["quit"]);
}

#[test]
fn actions_parse_from_their_names() {
    assert_eq!(Action::parse("rescan"), Some(Action::Rescan));
    assert_eq!(Action::parse("nope"), None);
}

#[test]
fn a_delta_applies_to_a_snapshot() {
    let delta = Delta {
        status: r#"{"config":{"writable":true}}"#.into(),
        config: Some((r#"{"general":{}}"#.into(), 2)),
        inventory: Some(Inventory {
            targets: r#"{"targets":[]}"#.into(),
            services: r#"{"services":[]}"#.into(),
            revision: 5,
        }),
    };
    let next = delta.apply(&Snapshot::default()).expect("applies");
    assert!(next.writable());
    assert_eq!(next.revision, 2);
}
