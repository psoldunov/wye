//! History: `GetHistory`, `ClearHistory`, `DeleteHistoryEntry`,
//! `ReopenHistoryEntry`, the `HistoryRevision` property, and [`record`],
//! which the link path calls for every opened link (DLG-HIS-01 to
//! DLG-HIS-04, PIPE-16, ADV-09, TRAY-15).
//!
//! Stored as JSON in `$XDG_STATE_HOME/wye/history.json`, 100 entries
//! (decision 10). While history is off nothing is recorded (ADV-09).

mod store;
mod view;

pub(crate) use view::target_name;

use std::collections::HashMap;
use std::sync::{Arc, PoisonError, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

use wye_api::actions::Reopen;
use wye_api::{Error, context, json};
use wye_core::history::{History, HistoryEntry};
use zbus::zvariant::{OwnedValue, Value};

use super::{Caller, Dict, Result};
use crate::api::link::Environment;
use crate::bus::Property;
use crate::context::{ServiceContext, blocking};
use crate::platform::Platform;

/// The history in memory and the files it belongs to.
#[derive(Debug)]
struct Stored {
    environment: Arc<Environment>,
    history: History,
    revision: u64,
}

/// What this topic keeps between calls.
#[derive(Debug, Default)]
pub struct State {
    current: RwLock<Option<Arc<Stored>>>,
    /// One change at a time, so no entry is lost between load and save.
    writing: tokio::sync::Mutex<()>,
}

impl State {
    pub(crate) fn new(_platform: &Platform) -> Self {
        Self::default()
    }

    fn get(&self) -> Option<Arc<Stored>> {
        self.current
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn set(&self, stored: Arc<Stored>) {
        *self.current.write().unwrap_or_else(PoisonError::into_inner) = Some(stored);
    }
}

async fn current(ctx: &ServiceContext) -> Result<Arc<Stored>> {
    let environment = ctx.environment()?;
    if let Some(stored) = ctx.history().get()
        && Arc::ptr_eq(&stored.environment, &environment)
    {
        return Ok(stored);
    }
    let path = store::path(&environment);
    let history = blocking(move || store::load(&path)).await?;
    let stored = Arc::new(Stored {
        environment,
        history,
        revision: 1,
    });
    ctx.history().set(stored.clone());
    Ok(stored)
}

/// Change the history with `change`, save it and announce it.
async fn change(ctx: &ServiceContext, change: impl FnOnce(&History) -> History) -> Result<()> {
    let writing = ctx.history().writing.lock().await;
    let before = current(ctx).await?;
    let history = change(&before.history);
    if history == before.history {
        return Ok(());
    }
    let path = store::path(&before.environment);
    let saved = history.clone();
    blocking(move || store::save(&path, &saved)).await??;
    ctx.history().set(Arc::new(Stored {
        environment: before.environment.clone(),
        history,
        revision: before.revision + 1,
    }));
    drop(writing);
    super::config::effects::changed(ctx, Property::HistoryRevision);
    // The tray's "Recent Links" follow the history (TRAY-15).
    super::config::effects::changed(ctx, Property::Tray);
    Ok(())
}

/// PIPE-16: record one opened link, built by the link path when it planned
/// the launch (with `time` 0, stamped now). The source app is remembered for
/// the app chooser either way (DLG-APP-02); the link is stored only while
/// history is on (ADV-09). Failures are logged: history never fails a link.
pub(crate) async fn record_entry(ctx: &ServiceContext, entry: HistoryEntry) {
    if let Some(source) = entry
        .source
        .as_deref()
        .filter(|source| source.ends_with(".desktop"))
    {
        super::inventory::remember_source(ctx, source);
    }
    let enabled = match super::config::current(ctx).await {
        Ok(current) => current.config.advanced.history,
        Err(error) => {
            tracing::warn!(%error, "cannot read whether history is on");
            return;
        }
    };
    if !enabled {
        return;
    }
    let entry = if entry.time == 0 {
        HistoryEntry {
            time: now(),
            ..entry
        }
    } else {
        entry
    };
    if let Err(error) = change(ctx, |history| history.record(entry)).await {
        tracing::warn!(%error, "cannot record the link in the history");
    }
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| {
            i64::try_from(since.as_secs()).unwrap_or(i64::MAX)
        })
}

/// The `HistoryRevision` property; 0 before the history could be read.
pub async fn revision(ctx: &ServiceContext) -> u64 {
    current(ctx).await.map_or(0, |stored| stored.revision)
}

/// `dev.soldunov.wye1.GetHistory`: JSON [`wye_api::history::History`].
pub async fn get_history(ctx: &ServiceContext) -> Result<String> {
    let stored = current(ctx).await?;
    let config = super::config::current(ctx).await?;
    let scan = super::inventory::current(ctx).await?;
    let locale = &scan.environment.xdg.locale;
    let catalog = super::inventory::targets::catalog(
        &scan.inventory,
        locale,
        &config.config,
        config.pipeline.services(),
    );
    json::encode(&wye_api::history::History {
        enabled: config.config.advanced.history,
        entries: stored
            .history
            .entries()
            .iter()
            .map(|entry| view::entry(entry, &catalog, &scan.inventory, locale))
            .collect(),
    })
}

/// `dev.soldunov.wye1.ClearHistory` (DLG-HIS-01, ADV-09).
pub async fn clear_history(ctx: &ServiceContext) -> Result<()> {
    change(ctx, History::cleared).await
}

/// `dev.soldunov.wye1.DeleteHistoryEntry` (DLG-HIS-03).
pub async fn delete_history_entry(ctx: &ServiceContext, id: u64) -> Result<()> {
    found(ctx, id).await?;
    change(ctx, |history| history.remove(id)).await
}

/// `dev.soldunov.wye1.ReopenHistoryEntry` ([`wye_api::actions::Reopen`],
/// DLG-HIS-03, TRAY-15): the link again, in the picker or in the target it
/// opened in. It enters the pipeline like a link from the CLI, so the
/// reopened link is recorded again.
pub async fn reopen_history_entry(ctx: &ServiceContext, id: u64, how: &str) -> Result<()> {
    let how: Reopen = how
        .parse()
        .map_err(|error| Error::invalid_args(format!("{error}")))?;
    let entry = found(ctx, id).await?;
    let force = match how {
        Reopen::Picker => context::Force::Picker,
        Reopen::SameTarget => context::Force::None,
    };
    if how == Reopen::SameTarget && entry.target.is_concrete() {
        return reopen_in(ctx, &entry).await;
    }
    let link = link_context(context::Entry::Cli, force)?;
    super::link::open_link(ctx, &Caller::default(), &entry.url, &link).await
}

/// Open `entry`'s link in the target it opened in before.
async fn reopen_in(ctx: &ServiceContext, entry: &HistoryEntry) -> Result<()> {
    let snapshot = super::config::snapshot(ctx).await?;
    let (target, url) = (entry.target.clone(), entry.url.clone());
    let plan = blocking(move || super::link::plan_for(&snapshot, &target, &url)).await?;
    super::link::open_plan(ctx, plan, &super::link::Activation::default()).await
}

async fn found(ctx: &ServiceContext, id: u64) -> Result<HistoryEntry> {
    current(ctx)
        .await?
        .history
        .get(id)
        .cloned()
        .ok_or_else(|| Error::NotFound(format!("there is no history entry {id}")))
}

/// An `OpenLink` context with `entry` and `force`.
fn link_context(entry: context::Entry, force: context::Force) -> Result<Dict> {
    let value = |text: &str| {
        OwnedValue::try_from(Value::from(text))
            .map_err(|error| Error::failed(format!("cannot build the link context: {error}")))
    };
    Ok(HashMap::from([
        (context::ENTRY.to_owned(), value(entry.as_str())?),
        (context::FORCE.to_owned(), value(force.as_str())?),
    ]))
}

#[cfg(test)]
mod tests {
    use wye_core::Target;
    use wye_core::history::Reason;
    use wye_core::pipeline::EntryPoint;

    use super::*;
    use crate::platform::fake::FakePlatform;

    /// A service context over a temporary home with `config` as its file.
    fn context(home: &std::path::Path, config: &str) -> ServiceContext {
        let ctx = ServiceContext::new(FakePlatform::new().platform());
        let root = home.to_owned();
        let environment = Environment::from_lookup(move |name| {
            (name == "HOME").then(|| std::ffi::OsString::from(root.clone()))
        })
        .expect("home");
        let file = environment.config.clone();
        std::fs::create_dir_all(file.parent().expect("parent")).expect("dir");
        std::fs::write(&file, config).expect("written");
        ctx.set_environment(environment);
        ctx
    }

    fn opened() -> HistoryEntry {
        HistoryEntry {
            id: 0,
            time: 0,
            original: "https://example.com/".to_owned(),
            url: "https://example.com/".to_owned(),
            entry: EntryPoint::Handler,
            source: Some("org.kde.konsole.desktop".to_owned()),
            target: Target::Picker,
            reason: Reason::Picker,
            expanded: false,
            cleaned: false,
            transformed: false,
        }
    }

    #[tokio::test]
    async fn adv09_nothing_is_recorded_while_history_is_off() {
        let home = tempfile::tempdir().expect("temp dir");
        let ctx = context(home.path(), "");
        record_entry(&ctx, opened()).await;
        assert!(current(&ctx).await.expect("history").history.is_empty());
        let recent = ctx.inventory().recent_sources();
        assert_eq!(recent, ["org.kde.konsole.desktop"], "DLG-APP-02 either way");
    }

    #[tokio::test]
    async fn pipe16_a_link_is_recorded_and_saved_while_history_is_on() {
        let home = tempfile::tempdir().expect("temp dir");
        let ctx = context(home.path(), "[advanced]\nhistory = true\n");
        record_entry(&ctx, opened()).await;
        let stored = current(&ctx).await.expect("history");
        assert_eq!(stored.history.len(), 1);
        assert!(stored.history.entries()[0].time > 0, "stamped");
        let path = store::path(&ctx.environment().expect("environment"));
        assert_eq!(store::load(&path), stored.history, "saved atomically");
    }
}
