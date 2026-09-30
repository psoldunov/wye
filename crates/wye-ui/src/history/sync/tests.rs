use std::sync::Mutex;

use super::*;

/// A service in memory: revisions, canned replies, calls recorded.
struct Fake {
    calls: Mutex<Vec<String>>,
    history_revision: u64,
    inventory_revision: u64,
    read_only: bool,
}

impl Fake {
    fn at(history_revision: u64, inventory_revision: u64) -> Self {
        Self {
            calls: Mutex::default(),
            history_revision,
            inventory_revision,
            read_only: false,
        }
    }

    fn calls(&self) -> Vec<String> {
        self.calls.lock().expect("lock").clone()
    }

    fn record(&self, call: impl Into<String>) {
        self.calls.lock().expect("lock").push(call.into());
    }
}

#[allow(
    clippy::unused_async_trait_impl,
    reason = "the fake service answers at once; the trait's methods are async"
)]
impl Api for Fake {
    async fn history_revision(&self) -> Result<u64, Error> {
        Ok(self.history_revision)
    }

    async fn inventory_revision(&self) -> Result<u64, Error> {
        Ok(self.inventory_revision)
    }

    async fn get_history(&self) -> Result<String, Error> {
        self.record("get-history");
        Ok(r#"{"enabled": true, "entries": [{"id": 4}]}"#.to_owned())
    }

    async fn get_targets(&self) -> Result<String, Error> {
        self.record("get-targets");
        Ok(r#"{"targets": []}"#.to_owned())
    }

    async fn clear_history(&self) -> Result<(), Error> {
        self.record("clear");
        Ok(())
    }

    async fn delete_history_entry(&self, id: u64) -> Result<(), Error> {
        self.record(format!("delete {id}"));
        Ok(())
    }

    async fn reopen_history_entry(&self, id: u64, how: &str) -> Result<(), Error> {
        self.record(format!("reopen {id} {how}"));
        Ok(())
    }

    async fn get_config(&self) -> Result<(String, u64), Error> {
        Ok(("{}".to_owned(), 9))
    }

    async fn update_config(&self, patch: &str, base: u64) -> Result<u64, Error> {
        self.record(format!("update {patch} @{base}"));
        if self.read_only {
            return Err(Error::ReadOnly("home-manager".into()));
        }
        Ok(base + 1)
    }

    async fn show_window(&self, window: &str, argument: &str) -> Result<(), Error> {
        self.record(format!("show {window} {argument}"));
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
fn nothing_is_read_while_both_revisions_are_known() {
    // DLG-HIS-01: the poll costs two property reads
    let fake = Fake::at(3, 2);
    let known = Known {
        history: 3,
        inventory: 2,
    };
    assert_eq!(block_on(poll(&fake, known)).expect("poll"), None);
    assert!(fake.calls().is_empty());
}

#[test]
fn a_new_history_revision_reads_the_history_only() {
    let fake = Fake::at(4, 2);
    let known = Known {
        history: 3,
        inventory: 2,
    };
    let update = block_on(poll(&fake, known)).expect("poll").expect("update");
    assert_eq!(fake.calls(), ["get-history"]);
    let next = update.apply(&Snapshot::default());
    assert_eq!(next.history.entries.len(), 1);
    assert_eq!(next.known.history, 4);
    assert_eq!(next.known.inventory, 0);
}

#[test]
fn the_first_poll_reads_everything() {
    let fake = Fake::at(1, 1);
    let update = block_on(poll(&fake, Known::default()))
        .expect("poll")
        .expect("update");
    assert_eq!(fake.calls(), ["get-history", "get-targets"]);
    let next = update.apply(&Snapshot::default());
    assert_eq!(
        next.known,
        Known {
            history: 1,
            inventory: 1
        }
    );
}

#[test]
fn an_update_keeps_what_it_does_not_name() {
    let base = Snapshot {
        history: History {
            enabled: true,
            entries: Vec::new(),
        },
        known: Known {
            history: 1,
            inventory: 5,
        },
        ..Snapshot::default()
    };
    let update = Update {
        history: None,
        targets: Some((TargetInventory::default(), 6)),
    };
    let next = update.apply(&base);
    assert!(next.history.enabled);
    assert_eq!(next.known.inventory, 6);
    assert_eq!(next.known.history, 1);
}

#[test]
fn the_actions_call_the_service_and_reload() {
    // DLG-HIS-01, DLG-HIS-03
    let known = Known {
        history: 3,
        inventory: 2,
    };
    for (action, first) in [
        (Action::Clear, "clear"),
        (Action::Delete(7), "delete 7"),
        (Action::Reopen(7, Reopen::Picker), "reopen 7 picker"),
        (
            Action::Reopen(7, Reopen::SameTarget),
            "reopen 7 same-target",
        ),
    ] {
        let fake = Fake::at(4, 2);
        let update = block_on(run(&fake, action, known)).expect("run");
        assert_eq!(fake.calls(), [first, "get-history"], "{first}");
        assert!(update.is_some());
    }
}

#[test]
fn turning_history_on_patches_the_configuration_and_reads_everything() {
    // DLG-HIS-04, ADV-09
    let fake = Fake::at(1, 1);
    let known = Known {
        history: 1,
        inventory: 1,
    };
    block_on(run(&fake, Action::TurnOn, known)).expect("run");
    assert_eq!(
        fake.calls(),
        [
            r#"update {"advanced":{"history":true}} @9"#,
            "get-history",
            "get-targets"
        ]
    );
}

#[test]
fn a_read_only_configuration_refuses_turning_history_on() {
    let mut fake = Fake::at(1, 1);
    fake.read_only = true;
    let result = block_on(run(&fake, Action::TurnOn, Known::default()));
    assert!(matches!(result, Err(Error::ReadOnly(_))), "{result:?}");
}

#[test]
fn create_rule_opens_the_rule_editor_with_the_prefill() {
    // DLG-HIS-03
    let fake = Fake::at(1, 1);
    let prefill = r#"{"domain":"github.com","sourceApp":null}"#.to_owned();
    let update = block_on(run(
        &fake,
        Action::CreateRule {
            prefill: prefill.clone(),
        },
        Known::default(),
    ))
    .expect("run");
    assert_eq!(update, None);
    assert_eq!(fake.calls(), [format!("show rule-editor {prefill}")]);
}
