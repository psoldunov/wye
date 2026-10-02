//! Changing the default browser (DEF-02, DEF-05): the file work behind
//! `MakeDefault` and `StopBeingDefault`. Blocking.

use wye_api::Error;
use wye_core::DesktopId;
use wye_desktop::default_browser::release_html;
use wye_desktop::kdeglobals::{self, Applied};
use wye_desktop::{
    DefaultBrowserError, WYE_DESKTOP_ID, XdgDirs, current_default, find_entry, forwards_links,
    listed_default, set_default,
};

/// What [`make_default`] changed, for the state file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Made {
    /// The browser Wye replaced in `mimeapps.list`, when it may be restored.
    pub previous: Option<DesktopId>,
    /// What happened to Plasma's own setting.
    pub kdeglobals: Kdeglobals,
}

/// What [`make_default`] did to `kdeglobals`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) enum Kdeglobals {
    /// Not KDE, already Wye, or it could not be changed.
    #[default]
    Untouched,
    /// Wye set it; this was the value before (none when the key was unset).
    Replaced(Option<String>),
}

/// Wye's desktop ID.
///
/// # Errors
///
/// `Failed` when the built-in ID is invalid (a bug).
pub(crate) fn wye_id() -> Result<DesktopId, Error> {
    DesktopId::new(WYE_DESKTOP_ID)
        .map_err(|error| Error::failed(format!("Wye's desktop ID is invalid: {error}")))
}

/// DEF-02: Wye handles `http` and `https` (and HTML files when
/// `include_html`, DEF-07) in `mimeapps.list`, and in Plasma's own setting
/// on KDE. A `kdeglobals` that cannot be changed is logged; links from KDE
/// apps then follow `mimeapps.list` only.
///
/// # Errors
///
/// `Failed` when Wye's desktop entry is not installed or a file cannot be
/// written, `ReadOnly` when `mimeapps.list` is managed elsewhere.
pub(crate) fn make_default(xdg: &XdgDirs, include_html: bool) -> Result<Made, Error> {
    let wye = wye_id()?;
    if find_entry(xdg, &wye).is_none() {
        return Err(Error::failed(format!(
            "Wye's desktop entry ({WYE_DESKTOP_ID}) is not installed, so links sent to Wye \
             would fail; install Wye first"
        )));
    }
    let previous = current_default(xdg).filter(|id| rememberable(xdg, id, &wye));
    set_default(xdg, &wye, include_html).map_err(explain)?;
    // DEF-07: with the switch off, HTML files Wye still claims from an
    // earlier "on" go to the browser it replaced.
    if !include_html && let Err(error) = release_html(xdg, &wye, previous.as_ref()) {
        tracing::warn!(%error, "cannot hand Wye's HTML types back");
    }
    let kdeglobals = match kdeglobals::set_browser(xdg, &wye) {
        Ok(Applied::Set { previous }) => Kdeglobals::Replaced(previous),
        Ok(Applied::NotKde | Applied::AlreadySet) => Kdeglobals::Untouched,
        Err(error) => {
            tracing::warn!(%error, "cannot set Plasma's default browser");
            Kdeglobals::Untouched
        }
    };
    Ok(Made {
        previous,
        kdeglobals,
    })
}

/// DEF-05: gives links back to `previous` and restores Plasma's value.
/// Returns the browser that has them now; `None` when Wye was not the
/// default, so nothing changed.
///
/// # Errors
///
/// `NotFound` when no previous browser is remembered or it is gone,
/// `Failed` when it would send links back to Wye or a file cannot be
/// written, `ReadOnly` when `mimeapps.list` is managed elsewhere.
pub(crate) fn stop_being_default(
    xdg: &XdgDirs,
    previous: Option<&DesktopId>,
    previous_kdeglobals: Option<&str>,
    include_html: bool,
) -> Result<Option<DesktopId>, Error> {
    let wye = wye_id()?;
    if listed_default(xdg).as_ref() != Some(&wye) {
        return Ok(None);
    }
    let previous = previous.ok_or_else(|| {
        Error::NotFound(
            "Wye does not remember which browser was the default before it; choose one in \
             your desktop's settings"
                .to_owned(),
        )
    })?;
    let entry = find_entry(xdg, previous).ok_or_else(|| {
        Error::NotFound(format!(
            "{previous}, the default browser before Wye, is no longer installed; choose \
             another in your desktop's settings"
        ))
    })?;
    if forwards_links(&entry) {
        return Err(Error::failed(format!(
            "{previous}, the default browser before Wye, sends links back to the default \
             browser; choose another in your desktop's settings"
        )));
    }
    set_default(xdg, previous, include_html).map_err(explain)?;
    // DEF-07: Wye no longer claims HTML files, whatever the setting says now.
    if let Err(error) = wye_desktop::default_browser::remove_html_association(xdg, &wye) {
        tracing::warn!(%error, "cannot remove Wye's HTML association");
    }
    if let Err(error) = kdeglobals::restore_browser(xdg, &wye, previous_kdeglobals) {
        tracing::warn!(%error, "cannot restore Plasma's default browser");
    }
    Ok(Some(previous.clone()))
}

/// DEF-07: "Also open local HTML files" changed to `include_html`. While Wye
/// is the default browser in `mimeapps.list` (whatever Plasma's own setting
/// names, which says nothing about HTML files), the HTML types follow at
/// once: Wye registers for them, or hands them to `previous` (the browser
/// it replaced) when that is still a browser to give links back to, else
/// drops its own keys. Returns false, changing nothing, while Wye is not
/// the default.
///
/// # Errors
///
/// `Failed` when a file cannot be written, `ReadOnly` when `mimeapps.list`
/// is managed elsewhere.
pub(crate) fn follow_html(
    xdg: &XdgDirs,
    include_html: bool,
    previous: Option<&DesktopId>,
) -> Result<bool, Error> {
    let wye = wye_id()?;
    if listed_default(xdg).as_ref() != Some(&wye) {
        return Ok(false);
    }
    if include_html {
        set_default(xdg, &wye, true).map_err(explain)?;
    } else {
        let to = previous.filter(|id| find_entry(xdg, id).is_some() && rememberable(xdg, id, &wye));
        release_html(xdg, &wye, to).map_err(explain)?;
    }
    Ok(true)
}

/// True when `id` may be remembered as the browser to give links back to:
/// not Wye, and not an app that forwards links to the default browser,
/// which would be Wye again (DEF-06).
fn rememberable(xdg: &XdgDirs, id: &DesktopId, wye: &DesktopId) -> bool {
    id != wye && find_entry(xdg, id).is_none_or(|entry| !forwards_links(&entry))
}

/// The D-Bus error for a file that could not be changed.
fn explain(error: DefaultBrowserError) -> Error {
    match error {
        DefaultBrowserError::Managed { path } => Error::ReadOnly(format!(
            "{} is managed by another tool (for example home-manager's xdg.mimeApps), so \
             Wye cannot change it. Set {WYE_DESKTOP_ID} as the handler for \
             x-scheme-handler/http and x-scheme-handler/https there.",
            path.display()
        )),
        error @ DefaultBrowserError::Io { .. } => Error::failed(error.to_string()),
    }
}
