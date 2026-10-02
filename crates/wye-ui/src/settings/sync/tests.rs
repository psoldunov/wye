#![allow(
    clippy::unused_async_trait_impl,
    reason = "the fake service answers at once; the trait's methods are async"
)]

use std::sync::Mutex;

use super::*;
use crate::settings::save::ConfigApi;

/// A service in memory: one revision, calls recorded.
#[derive(Default)]
struct Fake {
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    revision: u64,
    inventory: u64,
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

impl ConfigApi for Fake {
    async fn update_config(&self, patch: &str, base: u64) -> Result<u64, Error> {
        let mut state = self.state.lock().expect("lock");
        state.calls.push(format!("update {patch} @{base}"));
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
}

impl Api for Fake {
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
fn polling_reads_only_what_moved() {
    let api = Fake::with(3, 7);
    let quiet = block_on(poll(
        &api,
        Known {
            config: 3,
            inventory: 7,
            inventory_config: 3,
        },
    ))
    .expect("poll");
    assert!(quiet.config.is_none() && quiet.inventory.is_none());
    assert_eq!(api.calls(), ["status"]);
    let all = block_on(poll(&api, Known::default())).expect("poll");
    assert_eq!(all.config.as_ref().map(|(_, r)| *r), Some(3));
    assert_eq!(all.inventory.as_ref().map(|i| i.revision), Some(7));
    assert_eq!(all.inventory.as_ref().map(|i| i.config_revision), Some(3));
}

#[test]
fn a_config_move_alone_reads_the_inventory_again() {
    // TGT-06, APP-10: the targets name the configuration's custom apps, which
    // the inventory revision does not count
    let api = Fake::with(4, 7);
    let delta = block_on(poll(
        &api,
        Known {
            config: 3,
            inventory: 7,
            inventory_config: 3,
        },
    ))
    .expect("poll");
    assert_eq!(delta.config.as_ref().map(|(_, r)| *r), Some(4));
    let inventory = delta.inventory.expect("targets and services read");
    assert_eq!((inventory.revision, inventory.config_revision), (7, 4));
    assert_eq!(
        api.calls(),
        ["status", "get-config", "get-targets", "get-services"]
    );
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
            inventory: 4,
            inventory_config: 1,
        }),
        Known {
            config: 9,
            inventory: 4,
            inventory_config: 1,
        }
    );
    let with_inventory = Delta {
        status: "{}".into(),
        config: None,
        inventory: Some(Inventory {
            targets: "{}".into(),
            services: "{}".into(),
            revision: 5,
            config_revision: 9,
        }),
    };
    assert_eq!(
        with_inventory.known(Known::default()),
        Known {
            config: 0,
            inventory: 5,
            inventory_config: 9,
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
            inventory_config: 3,
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
            inventory_config: 3,
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
    assert!(
        matches!(&error, Error::NotFound(reason) if reason == "no previous browser"),
        "{error:?}"
    );
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
            config_revision: 2,
        }),
    };
    let next = delta.apply(&Snapshot::default()).expect("applies");
    assert!(next.writable());
    assert_eq!(next.revision, 2);
}
