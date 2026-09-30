//! `wye default`: show, take and give back the default web browser (DEF-02,
//! DEF-05).

use std::process::ExitCode;

use anyhow::bail;
use wye_core::DesktopId;
use wye_desktop::{
    DefaultBrowserError, WYE_DESKTOP_ID, XdgDirs, current_default, find_entry, forwards_links,
    set_default,
};

use super::{Console, Context, wye_id};
use crate::cli::DefaultAction;
use crate::state::State;

pub fn run(
    context: &Context,
    console: &mut Console<'_>,
    action: DefaultAction,
) -> anyhow::Result<ExitCode> {
    match action {
        DefaultAction::Status => status(context, console),
        DefaultAction::Set => set(context, console),
        DefaultAction::Unset => unset(context, console),
    }?;
    Ok(ExitCode::SUCCESS)
}

fn status(context: &Context, console: &mut Console<'_>) -> anyhow::Result<()> {
    let wye = wye_id()?;
    match current_default(&context.xdg) {
        Some(id) if id == wye => writeln!(console.out, "Wye is your default browser")?,
        Some(id) => writeln!(console.out, "{id} is your default browser")?,
        None => writeln!(console.out, "No default browser is set")?,
    }
    Ok(())
}

/// DEF-02: makes Wye the default, remembering the browser it replaces
/// (DEF-05).
fn set(context: &Context, console: &mut Console<'_>) -> anyhow::Result<()> {
    let wye = wye_id()?;
    if find_entry(&context.xdg, &wye).is_none() {
        bail!(
            "Wye's desktop entry ({WYE_DESKTOP_ID}) is not installed, so links sent to Wye \
             would fail; install Wye first"
        );
    }
    let include_html = context.config(console.err, true).general.open_local_html;
    let previous = current_default(&context.xdg).filter(|id| rememberable(&context.xdg, id, &wye));
    if let Some(previous) = previous {
        // Remember it before changing anything, so `unset` can always undo.
        let state = State {
            previous_default_browser: Some(previous),
        };
        state.save(&context.paths.state)?;
    }
    set_default(&context.xdg, &wye, include_html).map_err(explain)?;
    writeln!(console.out, "Wye is now your default browser")?;
    Ok(())
}

/// True when `id` may be remembered as the browser to give the default
/// back to: not Wye, and not an app that forwards links to the default
/// browser, which would be Wye again (DEF-06).
fn rememberable(xdg: &XdgDirs, id: &DesktopId, wye: &DesktopId) -> bool {
    id != wye && find_entry(xdg, id).is_none_or(|entry| !forwards_links(&entry))
}

/// DEF-05: gives the default back to the browser Wye replaced.
///
/// When Wye is no longer the default (the user picked another browser
/// since), nothing is changed, the remembered browser is kept, and the
/// command succeeds (exit 0) after saying so: the default is already not
/// Wye, which is what `unset` asks for.
fn unset(context: &Context, console: &mut Console<'_>) -> anyhow::Result<()> {
    let wye = wye_id()?;
    let current = current_default(&context.xdg);
    if current.as_ref() != Some(&wye) {
        match current {
            Some(id) => writeln!(
                console.out,
                "Wye is not your default browser ({id} is); nothing changed"
            )?,
            None => writeln!(
                console.out,
                "Wye is not your default browser; nothing changed"
            )?,
        }
        return Ok(());
    }
    let state = State::load(&context.paths.state)?;
    let Some(previous) = state.previous_default_browser.clone() else {
        bail!(
            "Wye does not remember which browser was the default before it; \
             choose one in your desktop's settings"
        );
    };
    let Some(entry) = find_entry(&context.xdg, &previous) else {
        bail!(
            "{previous}, the default browser before Wye, is no longer installed; \
             choose another in your desktop's settings"
        );
    };
    if forwards_links(&entry) {
        bail!(
            "{previous}, the default browser before Wye, sends links back to the default \
             browser; choose another in your desktop's settings"
        );
    }
    let include_html = context.config(console.err, true).general.open_local_html;
    set_default(&context.xdg, &previous, include_html).map_err(explain)?;
    State::default().save(&context.paths.state)?;
    writeln!(
        console.out,
        "{} ({previous}) is your default browser again",
        entry.name
    )?;
    Ok(())
}

/// Adds the fix to a managed-file error: the tool that owns the file has to
/// name Wye as the handler.
fn explain(error: DefaultBrowserError) -> anyhow::Error {
    match error {
        DefaultBrowserError::Managed { path } => anyhow::anyhow!(
            "{} is managed by another tool (for example home-manager's xdg.mimeApps), \
             so Wye cannot change it. Set {WYE_DESKTOP_ID} as the handler for \
             x-scheme-handler/http and x-scheme-handler/https there.",
            path.display()
        ),
        error @ DefaultBrowserError::Io { .. } => {
            anyhow::Error::new(error).context("cannot change the default browser")
        }
    }
}

#[cfg(test)]
mod tests {
    use wye_core::DesktopId;

    use super::*;

    #[test]
    fn managed_error_names_the_handler() {
        let error = explain(DefaultBrowserError::Managed {
            path: "/home/u/.config/mimeapps.list".into(),
        });
        let message = error.to_string();
        assert!(message.contains("/home/u/.config/mimeapps.list"));
        assert!(message.contains(WYE_DESKTOP_ID));
        assert!(message.contains("x-scheme-handler/https"));
    }

    #[test]
    fn wye_id_is_valid() {
        assert_eq!(wye_id().unwrap(), DesktopId::new(WYE_DESKTOP_ID).unwrap());
    }
}
