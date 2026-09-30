#![allow(
    clippy::unused_async_trait_impl,
    reason = "the fake service answers at once; the trait's methods are async"
)]

use std::sync::Mutex;

use serde_json::json;

use super::*;

/// A service in memory: the configuration, its revision, the calls seen.
struct Fake {
    state: Mutex<State>,
}

struct State {
    config: Value,
    revision: u64,
    read_only: bool,
    calls: Vec<String>,
}

impl Fake {
    fn with(config: Value, revision: u64) -> Self {
        Self {
            state: Mutex::new(State {
                config,
                revision,
                read_only: false,
                calls: Vec::new(),
            }),
        }
    }

    /// Someone else writes `patch` (a hand edit, the CLI, another window).
    fn external(&self, patch: &Value) {
        let mut state = self.state.lock().expect("lock");
        state.config = wye_core::merge_patch::apply(&state.config, patch);
        state.revision += 1;
    }

    fn calls(&self) -> Vec<String> {
        self.state.lock().expect("lock").calls.clone()
    }

    fn config(&self) -> Value {
        self.state.lock().expect("lock").config.clone()
    }
}

impl ConfigApi for Fake {
    async fn update_config(&self, patch: &str, base: u64) -> Result<u64, Error> {
        let mut state = self.state.lock().expect("lock");
        state.calls.push(format!("update @{base}"));
        if state.read_only {
            return Err(Error::ReadOnly("home-manager".into()));
        }
        if base != 0 && base != state.revision {
            return Err(Error::Conflict("stale".into()));
        }
        let patch: Value = serde_json::from_str(patch).expect("a patch");
        state.config = wye_core::merge_patch::apply(&state.config, &patch);
        state.revision += 1;
        Ok(state.revision)
    }

    async fn get_config(&self) -> Result<(String, u64), Error> {
        let mut state = self.state.lock().expect("lock");
        state.calls.push("get-config".to_owned());
        Ok((state.config.to_string(), state.revision))
    }
}

fn block_on<T>(future: impl Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("runtime")
        .block_on(future)
}

fn rules(names: &[&str]) -> Value {
    json!({"rules": names.iter().map(|name| json!({"name": name})).collect::<Vec<_>>()})
}

fn change(patch: Value, base_revision: u64, base_config: Value) -> Change {
    Change {
        patch,
        base_revision,
        base_config,
    }
}

#[test]
fn set_06_a_patch_is_saved_on_the_revision_the_window_read() {
    let api = Fake::with(json!({}), 3);
    let (_, revision) =
        block_on(save(&api, &change(json!({"a": 1}), 3, json!({})))).expect("saved");
    assert_eq!(revision, 4);
    assert_eq!(api.calls(), ["update @3", "get-config"]);
}

#[test]
fn set_06_a_patch_without_arrays_is_sent_again_after_a_conflict() {
    let api = Fake::with(json!({}), 3);
    api.external(&json!({"b": 2}));
    let (config, revision) =
        block_on(save(&api, &change(json!({"a": 1}), 3, json!({})))).expect("saved");
    assert_eq!(revision, 5);
    assert_eq!(
        api.calls(),
        ["update @3", "get-config", "update @4", "get-config"]
    );
    let config: Value = serde_json::from_str(&config).expect("json");
    assert_eq!(config, json!({"a": 1, "b": 2}), "both changes kept");
}

#[test]
fn set_06_a_list_nobody_else_touched_is_sent_again() {
    // RUL-07: the rule list is the same as when the toggle was built; another
    // key changed.
    let base = rules(&["a", "b"]);
    let api = Fake::with(base.clone(), 3);
    api.external(&json!({"general": {"tray-icon": false}}));
    let patch = json!({"rules": [{"name": "a", "enabled": false}, {"name": "b"}]});
    block_on(save(&api, &change(patch.clone(), 3, base))).expect("saved");
    assert_eq!(api.config()["rules"], patch["rules"]);
    assert_eq!(api.config()["general"]["tray-icon"], json!(false));
}

#[test]
fn set_06_a_list_changed_elsewhere_is_not_overwritten() {
    // A rule added by hand while the window showed the old list must survive
    // a toggle built from that list.
    let base = rules(&["a", "b"]);
    let api = Fake::with(base.clone(), 3);
    api.external(&rules(&["a", "b", "by hand"]));
    let patch = json!({"rules": [{"name": "a", "enabled": false}, {"name": "b"}]});
    let error = block_on(save(&api, &change(patch, 3, base))).expect_err("refused");
    assert!(
        matches!(&error, Error::Conflict(reason) if reason == CHANGED_ELSEWHERE),
        "{error:?}"
    );
    assert_eq!(api.config(), rules(&["a", "b", "by hand"]));
    assert_eq!(api.calls(), ["update @3", "get-config"]);
}

#[test]
fn set_06_a_list_without_the_config_it_came_from_is_refused_on_a_conflict() {
    let api = Fake::with(rules(&["a"]), 3);
    api.external(&json!({"b": 2}));
    let error = block_on(save(&api, &change(rules(&[]), 3, Value::Null))).expect_err("refused");
    assert!(matches!(error, Error::Conflict(_)), "{error:?}");
}

#[test]
fn a_read_only_file_is_reported_not_retried() {
    let api = Fake::with(json!({}), 3);
    api.state.lock().expect("lock").read_only = true;
    let error = block_on(save(&api, &change(json!({"a": 1}), 3, json!({})))).expect_err("refused");
    assert!(matches!(error, Error::ReadOnly(_)), "{error:?}");
    assert_eq!(api.calls(), ["update @3"]);
}

#[test]
fn an_unchecked_change_skips_the_revision_check() {
    let api = Fake::with(json!({}), 3);
    api.external(&json!({"b": 2}));
    block_on(save(
        &api,
        &Change::unchecked(json!({"advanced": {"history": true}})),
    ))
    .expect("saved");
    assert_eq!(api.calls(), ["update @0", "get-config"]);
}

#[test]
fn array_paths_escape_keys_and_stop_at_arrays() {
    let paths = array_paths(&json!({
        "rules": [{"x": []}],
        "picker": {"keys": {"open": ["Return"], "a/b": []}, "size": 1}
    }));
    assert_eq!(
        paths,
        ["/picker/keys/a~1b", "/picker/keys/open", "/rules"],
        "sorted as serde_json's map keeps them"
    );
}
