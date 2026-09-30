//! `wye open` (IN-01, IN-07): hand each link to the Wye service, or route
//! and launch it here when there is no service (PIPE-01 to PIPE-15).
//!
//! The service is asked first (`OpenLink`, [`bus::TIMEOUT`] including bus
//! activation) with the source app from Wye's own parent chain and the
//! launcher's activation token. When no service exists or can be started,
//! the link is routed in this process with the service's own hooks
//! (short links, scripts; [`OfflineHooks`]) and the picker stand-in, so a
//! link is never lost; held keys and the lock state are unknown here. A
//! link the service refused (PIPE-02) or failed to launch (LAUNCH-07) is
//! not retried: the service has already told the user. Neither is one the
//! service took but did not answer in time: it may still open it, and
//! opening it here as well would open it twice.
//!
//! Every message here goes to stderr best effort ([`notice`]): links
//! clicked in apps often arrive with a closed or broken stderr, and a
//! failed write must never stop a link opening.

use std::collections::HashMap;
use std::path::Path;
use std::process::ExitCode;

use anyhow::Context as _;
use wye_api::Error;
use wye_api::context as keys;
use wye_core::{
    Chosen, EntryPoint, Force, LinkRequest, OpenOptions, Pipeline, Resolution, SourceApp, Target,
};
use wye_desktop::launch::ACTIVATION_ENV;
use wye_desktop::{Inventory, LaunchRequest, build_command, source_app, spawn, stand_in};
use wye_service::offline::OfflineHooks;
use zbus::zvariant::Value;

use super::{Console, Context, INVALID};
use crate::bus::{CallError, Client};
use crate::cli::OpenArgs;
use crate::{display, notice};

/// What became of one link.
#[derive(Debug, Default, Clone, Copy)]
struct Tally {
    rejected: bool,
    failed: bool,
}

/// Opens every link. Rejected links and failed launches are reported
/// without stopping the others; the exit code is 2 when any link was
/// rejected, else 1 when any launch failed.
pub fn run(
    context: &Context,
    console: &mut Console<'_>,
    args: &OpenArgs,
) -> anyhow::Result<ExitCode> {
    // Links clicked in apps have no terminal to show warnings in.
    let debug = std::env::var_os("WYE_DEBUG").is_some();
    let client = Client::connect()
        .inspect_err(|error| {
            if debug {
                notice::write(console.err, format_args!("wye: {error}"));
            }
        })
        .ok();
    if args.urls.is_empty() {
        return Ok(activate(client.as_ref(), console));
    }
    let parent = std::os::unix::process::parent_id();
    // Cheap detection only: step 3 needs the installed apps, which the
    // service has (it gets the PID) and the in-process path reads anyway.
    let source = source_app::detect(Path::new("/proc"), parent);
    let force = args.force.force();

    let mut tally = Tally::default();
    let mut local = Vec::new();
    for url in &args.urls {
        let delegated = client.as_ref().map_or(
            Err(CallError::Unreachable("no session bus".to_owned())),
            |client| delegate(client, url, &source, parent, force),
        );
        match delegated {
            Ok(()) => {}
            Err(CallError::Refused(error)) => {
                notice::write(
                    console.err,
                    format_args!("wye: {}", crate::bus::message(&error)),
                );
                if matches!(error, Error::InvalidArgs(_)) {
                    tally.rejected = true;
                } else {
                    tally.failed = true;
                }
            }
            Err(unanswered @ CallError::NoAnswer(_)) => {
                // Never opened here as well: the service may still open it.
                notice::write(console.err, format_args!("wye: {unanswered}"));
            }
            Err(unreachable @ CallError::Unreachable(_)) => {
                if debug {
                    notice::write(
                        console.err,
                        format_args!("wye: {unreachable}; opening here"),
                    );
                }
                local.push(url.as_str());
            }
        }
    }
    if !local.is_empty() {
        tally = open_here(context, console, &local, &source, force, debug, tally)?;
    }
    Ok(if tally.rejected {
        ExitCode::from(INVALID)
    } else if tally.failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    })
}

/// No link: the app-menu entry ran `wye open` (TRAY-05). The service opens
/// Settings; without it there is nothing to do.
fn activate(client: Option<&Client>, console: &mut Console<'_>) -> ExitCode {
    if client.is_some_and(|client| client.activate().is_ok()) {
        return ExitCode::SUCCESS;
    }
    notice::write(console.err, format_args!("wye: no link given"));
    ExitCode::SUCCESS
}

/// `OpenLink` with what this process knows about the link.
fn delegate(
    client: &Client,
    url: &str,
    source: &SourceApp,
    parent: u32,
    force: Force,
) -> Result<(), CallError> {
    let context = link_context(source, parent, force, |name| std::env::var(name).ok());
    client.call(|wye| async move { wye.open_link(url, context).await })
}

/// The `OpenLink` context: source, force, entry and the launcher's
/// activation token (LAUNCH-03). Without a desktop ID the service also gets
/// `parent` to detect from, with the installed apps. Held keys are left to
/// the service.
fn link_context(
    source: &SourceApp,
    parent: u32,
    force: Force,
    env: impl Fn(&str) -> Option<String>,
) -> HashMap<&'static str, Value<'static>> {
    let [token, startup_id] = ACTIVATION_ENV;
    let force = match force {
        Force::None => keys::Force::None,
        Force::Picker => keys::Force::Picker,
        Force::Alternative => keys::Force::Alternative,
    };
    let entries = [
        (keys::ENTRY, Some(keys::Entry::Handler.as_str().to_owned())),
        (keys::FORCE, Some(force.as_str().to_owned())),
        (
            keys::SOURCE_DESKTOP_ID,
            source.desktop_id.as_ref().map(|id| id.as_str().to_owned()),
        ),
        (keys::SOURCE_EXECUTABLE, source.executable.clone()),
        (
            keys::ACTIVATION_TOKEN,
            env(token).filter(|value| !value.is_empty()),
        ),
        (
            keys::STARTUP_ID,
            env(startup_id).filter(|value| !value.is_empty()),
        ),
    ];
    let pid = source
        .desktop_id
        .is_none()
        .then(|| (keys::SOURCE_PID, Value::from(parent)));
    entries
        .into_iter()
        .filter_map(|(key, value)| value.map(|value| (key, Value::from(value))))
        .chain(pid)
        .collect()
}

/// Route and launch `urls` in this process.
fn open_here(
    context: &Context,
    console: &mut Console<'_>,
    urls: &[&str],
    source: &SourceApp,
    force: Force,
    debug: bool,
    tally: Tally,
) -> anyhow::Result<Tally> {
    let pipeline = Pipeline::with_shipped_data(context.config(console.err, debug));
    let inventory = context.inventory()?;
    let source = if source.desktop_id.is_some() {
        source.clone()
    } else {
        source_app::detect_in(
            Path::new("/proc"),
            std::os::unix::process::parent_id(),
            &inventory,
        )
    };
    let opener = Opener {
        context,
        pipeline: &pipeline,
        inventory: &inventory,
    };
    let mut tally = tally;
    for url in urls {
        // Held modifiers and the lock state need the session service; here
        // no key is held and the screen is unlocked.
        let request = LinkRequest {
            source: source.clone(),
            force,
            ..LinkRequest::new(*url, EntryPoint::Handler)
        };
        // PIPE-03, PIPE-05, PIPE-14: the same hooks as the service.
        let hooks = OfflineHooks::new(&context.paths.config, pipeline.config());
        match pipeline.resolve_with(&request, &inventory, hooks.hooks()) {
            Ok(resolution) => {
                if let Err(error) = opener.launch(console, &request, &resolution, &hooks) {
                    notice::write(console.err, format_args!("wye: {error:#}"));
                    tally.failed = true;
                }
            }
            Err(error) => {
                notice::write(console.err, format_args!("wye: {error}"));
                tally.rejected = true;
            }
        }
    }
    Ok(tally)
}

/// What launching a resolved link needs.
struct Opener<'a> {
    context: &'a Context,
    pipeline: &'a Pipeline,
    inventory: &'a Inventory,
}

impl Opener<'_> {
    /// Finishes a resolved link as the service would (PIPE-13 to PIPE-15),
    /// standing in for the picker when needed, and launches it. The
    /// stand-in is announced only after the launch, so the notice can never
    /// delay or stop it.
    fn launch(
        &self,
        console: &mut Console<'_>,
        request: &LinkRequest,
        resolution: &Resolution,
        hooks: &OfflineHooks,
    ) -> anyhow::Result<()> {
        let chosen = if stand_in::needed(resolution) {
            Some(Chosen {
                target: self.stand_in(console)?,
                options: OpenOptions::default(),
            })
        } else {
            None
        };
        let stood_in = chosen.is_some();
        let finished = self
            .pipeline
            .finish(resolution, request, chosen, hooks.hooks());
        let url = self.pipeline.launch_url(&finished.url, &finished.target);
        let command = build_command(
            self.inventory,
            &LaunchRequest {
                target: &finished.target,
                url: &url,
                background: finished.options.background,
                new_window: finished.options.new_window,
            },
        )?;
        spawn(&command).with_context(|| format!("cannot start {}", command.program))?;
        if stood_in {
            notice::write(
                console.err,
                format_args!(
                    "wye: the Wye service is not running, so no picker could be shown; opened in {}",
                    display::target_name(&finished.target, self.inventory)
                ),
            );
        }
        Ok(())
    }

    /// The picker stand-in (PIPE-13).
    fn stand_in(&self, console: &mut Console<'_>) -> anyhow::Result<Target> {
        let state = self.context.state_or_default(console.err);
        stand_in::choose(
            self.pipeline.config(),
            self.inventory,
            state.previous_default_browser.as_ref(),
        )
        .context("no picker could be shown and no web browser is installed")
    }
}

#[cfg(test)]
mod tests {
    use wye_core::DesktopId;

    use super::*;

    fn text(context: &HashMap<&str, Value<'_>>, key: &str) -> Option<String> {
        context.get(key).map(|value| match value {
            Value::Str(text) => text.as_str().to_owned(),
            other => panic!("{key} is not a string: {other:?}"),
        })
    }

    #[test]
    fn the_context_carries_source_force_and_token() {
        let source = SourceApp {
            desktop_id: DesktopId::new("org.example.Chat.desktop").ok(),
            executable: Some("chat".into()),
        };
        let env = |name: &str| (name == "XDG_ACTIVATION_TOKEN").then(|| "token-1".to_owned());
        let context = link_context(&source, 7, Force::Picker, env);
        assert_eq!(text(&context, keys::ENTRY).as_deref(), Some("handler"));
        assert_eq!(text(&context, keys::FORCE).as_deref(), Some("picker"));
        assert_eq!(
            text(&context, keys::SOURCE_DESKTOP_ID).as_deref(),
            Some("org.example.Chat.desktop")
        );
        assert_eq!(
            text(&context, keys::SOURCE_EXECUTABLE).as_deref(),
            Some("chat")
        );
        assert_eq!(
            text(&context, keys::ACTIVATION_TOKEN).as_deref(),
            Some("token-1")
        );
        assert!(!context.contains_key(keys::STARTUP_ID));
        assert!(
            !context.contains_key(keys::HELD_KNOWN),
            "the service probes held keys"
        );
        assert!(
            !context.contains_key(keys::SOURCE_PID),
            "a desktop ID needs no detection"
        );
    }

    #[test]
    fn without_a_desktop_id_the_service_detects_from_the_parent() {
        let context = link_context(&SourceApp::default(), 7, Force::None, |_| None);
        assert!(matches!(context.get(keys::SOURCE_PID), Some(Value::U32(7))));
    }
}
