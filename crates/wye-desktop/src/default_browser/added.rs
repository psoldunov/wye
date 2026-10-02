//! `[Added Associations]` for the HTML types (DEF-07).
//!
//! `KService` (Plasma) only honours a `[Default Applications]` entry for an
//! app associated with the type: one whose desktop entry lists it, or one
//! the user added under `[Added Associations]`. Wye's entry does not claim
//! HTML files, so opting in adds Wye there too, first in the list, and
//! stopping removes it again. Turning the switch off while Wye stays the
//! default browser also hands the HTML types' defaults back
//! ([`release_html`]).

use wye_core::DesktopId;

use super::{
    DEFAULTS_GROUP, DefaultBrowserError, HTML_TYPES, MIMEAPPS, ensure_writable, read_existing,
    remove_key, set_keys,
};
use crate::xdg::XdgDirs;
use crate::{atomic, keyfile};

const ADDED_GROUP: &str = "Added Associations";

/// `text` with `id` first in the `[Added Associations]` list of each of
/// `mimes`, once. Every other line is kept.
pub(super) fn associate(text: &str, mimes: &[&str], id: &DesktopId) -> String {
    let lists: Vec<(&str, String)> = mimes
        .iter()
        .map(|mime| {
            let rest = entries(text, mime).filter(|entry| *entry != id.as_str());
            let list = joined(std::iter::once(id.as_str()).chain(rest));
            (*mime, list)
        })
        .collect();
    let pairs: Vec<(&str, &str)> = lists
        .iter()
        .map(|(mime, list)| (*mime, list.as_str()))
        .collect();
    set_keys(text, ADDED_GROUP, &pairs)
}

/// `text` without `id` in the `[Added Associations]` lists of `mimes`; a
/// list left empty is removed.
fn dissociate(text: &str, mimes: &[&str], id: &DesktopId) -> String {
    mimes.iter().fold(text.to_owned(), |text, mime| {
        if !entries(&text, mime).any(|entry| entry == id.as_str()) {
            return text;
        }
        let rest = joined(entries(&text, mime).filter(|entry| *entry != id.as_str()));
        if rest.is_empty() {
            remove_key(&text, ADDED_GROUP, mime)
        } else {
            set_keys(&text, ADDED_GROUP, &[(mime, &rest)])
        }
    })
}

/// A desktop-entry list value: each entry followed by `;`.
fn joined<'a>(entries: impl Iterator<Item = &'a str>) -> String {
    entries.fold(String::new(), |mut list, entry| {
        list.push_str(entry);
        list.push(';');
        list
    })
}

/// The apps `[Added Associations]` lists for `mime` in `text`.
fn entries<'a>(text: &'a str, mime: &str) -> impl Iterator<Item = &'a str> {
    let list = text
        .lines()
        .scan(false, |in_group, line| {
            let trimmed = line.trim();
            if let Some(name) = keyfile::group_header(trimmed) {
                *in_group = name == ADDED_GROUP;
                return Some(None);
            }
            Some(
                trimmed
                    .split_once('=')
                    .filter(|(key, _)| *in_group && key.trim_end() == mime)
                    .map(|(_, value)| value),
            )
        })
        .flatten()
        .last()
        .unwrap_or_default();
    list.split(';')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
}

/// DEF-07: take `id` out of the HTML types' `[Added Associations]` in
/// `$XDG_CONFIG_HOME/mimeapps.list`, when it is there.
///
/// # Errors
///
/// [`DefaultBrowserError::Managed`] when the file must change but is a
/// symlink or read-only, [`DefaultBrowserError::Io`] when it cannot be read
/// or written.
pub fn remove_html_association(xdg: &XdgDirs, id: &DesktopId) -> Result<(), DefaultBrowserError> {
    let path = xdg.config_home.join(MIMEAPPS);
    let Some(text) = read_existing(&path)? else {
        return Ok(());
    };
    let updated = dissociate(&text, &HTML_TYPES, id);
    if updated == text {
        return Ok(());
    }
    ensure_writable(&path)?;
    atomic::write(&path, updated.as_bytes(), Some(&path))
        .map_err(|source| DefaultBrowserError::Io { path, source })
}

/// DEF-07: Wye stops handling HTML files. Wherever
/// `$XDG_CONFIG_HOME/mimeapps.list` or a `<desktop>-mimeapps.list` next to
/// it names `wye` first for an HTML type, `wye` leaves that list: `to` heads
/// it, the apps listed after `wye` stay as fallbacks, and a list left empty
/// is removed, so the desktop's own default applies.
/// Wye's `[Added Associations]` for the HTML types go too. HTTP links are
/// left alone.
///
/// # Errors
///
/// [`DefaultBrowserError::Managed`] when a file must change but is a symlink
/// or read-only (nothing is written then), [`DefaultBrowserError::Io`] when
/// one cannot be read or written.
pub fn release_html(
    xdg: &XdgDirs,
    wye: &DesktopId,
    to: Option<&DesktopId>,
) -> Result<(), DefaultBrowserError> {
    let paths = std::iter::once(xdg.config_home.join(MIMEAPPS)).chain(
        xdg.current_desktops.iter().map(|desktop| {
            xdg.config_home
                .join(format!("{}-{MIMEAPPS}", desktop.to_lowercase()))
        }),
    );
    let mut edits = Vec::new();
    for path in paths {
        let Some(text) = read_existing(&path)? else {
            continue;
        };
        let updated = hand_over(&text, wye, to);
        if updated != text {
            edits.push((path, updated));
        }
    }
    // Check every file before writing any, as `set_default` does.
    for (path, _) in &edits {
        ensure_writable(path)?;
    }
    for (path, text) in edits {
        atomic::write(&path, text.as_bytes(), Some(&path))
            .map_err(|source| DefaultBrowserError::Io { path, source })?;
    }
    remove_html_association(xdg, wye)
}

/// `text` with `wye` taken out of each HTML type's `[Default Applications]`
/// list that names it first. The other apps keep their order as fallbacks;
/// `to` goes first, once. A list left empty is removed.
fn hand_over(text: &str, wye: &DesktopId, to: Option<&DesktopId>) -> String {
    let groups = keyfile::parse(text);
    HTML_TYPES.iter().fold(text.to_owned(), |updated, mime| {
        let Some(list) = groups
            .iter()
            .filter(|group| group.name == DEFAULTS_GROUP)
            .filter_map(|group| group.get(mime))
            .map(keyfile::unescape_list)
            .find(|list| {
                list.first()
                    .and_then(|first| DesktopId::new(first).ok())
                    .as_ref()
                    == Some(wye)
            })
        else {
            return updated;
        };
        let rest = list.iter().map(String::as_str).filter(|entry| {
            let entry = DesktopId::new(*entry).ok();
            entry.as_ref() != Some(wye) && entry.as_ref() != to
        });
        let handlers: Vec<&str> = to.map(DesktopId::as_str).into_iter().chain(rest).collect();
        match handlers.as_slice() {
            [] => remove_key(&updated, DEFAULTS_GROUP, mime),
            // One app is written bare, as `set_default` writes it.
            [only] => set_keys(&updated, DEFAULTS_GROUP, &[(mime, only)]),
            _ => set_keys(
                &updated,
                DEFAULTS_GROUP,
                &[(mime, &joined(handlers.into_iter()))],
            ),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::Fixture;

    fn wye() -> DesktopId {
        DesktopId::new("dev.soldunov.wye.desktop").expect("id")
    }

    #[test]
    fn def_07_wye_is_added_first_once() {
        let text = "[Added Associations]\ntext/html=firefox.desktop;\n";
        let added = associate(text, &HTML_TYPES, &wye());
        assert_eq!(
            added,
            "[Added Associations]\ntext/html=dev.soldunov.wye.desktop;firefox.desktop;\n\
             application/xhtml+xml=dev.soldunov.wye.desktop;\n"
        );
        assert_eq!(associate(&added, &HTML_TYPES, &wye()), added, "once");
    }

    #[test]
    fn def_07_removing_wye_keeps_the_other_apps() {
        let text = "[Default Applications]\ntext/html=firefox.desktop\n\n\
                    [Added Associations]\ntext/html=dev.soldunov.wye.desktop;firefox.desktop;\n\
                    application/xhtml+xml=dev.soldunov.wye.desktop;\n";
        assert_eq!(
            dissociate(text, &HTML_TYPES, &wye()),
            "[Default Applications]\ntext/html=firefox.desktop\n\n\
             [Added Associations]\ntext/html=firefox.desktop;\n"
        );
    }

    fn id(value: &str) -> DesktopId {
        DesktopId::new(value).expect("id")
    }

    fn read(fx: &Fixture) -> String {
        std::fs::read_to_string(fx.path("home/.config/mimeapps.list")).expect("mimeapps.list")
    }

    const WYE_WITH_HTML: &str = "[Default Applications]\n\
        x-scheme-handler/http=dev.soldunov.wye.desktop\n\
        x-scheme-handler/https=dev.soldunov.wye.desktop\n\
        text/html=dev.soldunov.wye.desktop\n\
        application/xhtml+xml=dev.soldunov.wye.desktop\n\n\
        [Added Associations]\n\
        text/html=dev.soldunov.wye.desktop;firefox.desktop;\n\
        application/xhtml+xml=dev.soldunov.wye.desktop;\n";

    #[test]
    fn def_07_html_goes_to_the_previous_browser() {
        let fx = Fixture::new();
        fx.write("home/.config/mimeapps.list", WYE_WITH_HTML);
        release_html(&fx.xdg, &id("dev.soldunov.wye"), Some(&id("firefox"))).expect("released");
        assert_eq!(
            read(&fx),
            "[Default Applications]\n\
             x-scheme-handler/http=dev.soldunov.wye.desktop\n\
             x-scheme-handler/https=dev.soldunov.wye.desktop\n\
             text/html=firefox.desktop\n\
             application/xhtml+xml=firefox.desktop\n\n\
             [Added Associations]\n\
             text/html=firefox.desktop;\n"
        );
    }

    #[test]
    fn def_07_without_a_previous_browser_the_keys_go() {
        let fx = Fixture::new();
        fx.write("home/.config/mimeapps.list", WYE_WITH_HTML);
        release_html(&fx.xdg, &id("dev.soldunov.wye"), None).expect("released");
        let text = read(&fx);
        assert!(!text.contains("dev.soldunov.wye.desktop;"), "{text}");
        assert!(!text.contains("text/html=dev"), "{text}");
        assert!(text.contains("x-scheme-handler/https=dev.soldunov.wye.desktop"));
    }

    const WYE_FIRST: &str = "[Default Applications]\n\
        x-scheme-handler/https=dev.soldunov.wye.desktop\n\
        text/html=dev.soldunov.wye.desktop;firefox.desktop;brave.desktop;\n\
        application/xhtml+xml=dev.soldunov.wye.desktop\n\
        image/png=dev.soldunov.wye.desktop\n";

    #[test]
    fn def_07_hand_over_with_wye_only() {
        let text = "[Default Applications]\ntext/html=dev.soldunov.wye.desktop\n";
        assert_eq!(
            hand_over(text, &wye(), Some(&id("firefox"))),
            "[Default Applications]\ntext/html=firefox.desktop\n"
        );
        assert_eq!(hand_over(text, &wye(), None), "[Default Applications]\n");
    }

    #[test]
    fn def_07_hand_over_keeps_the_fallbacks_in_order() {
        assert_eq!(
            hand_over(WYE_FIRST, &wye(), Some(&id("chromium"))),
            "[Default Applications]\n\
             x-scheme-handler/https=dev.soldunov.wye.desktop\n\
             text/html=chromium.desktop;firefox.desktop;brave.desktop;\n\
             application/xhtml+xml=chromium.desktop\n\
             image/png=dev.soldunov.wye.desktop\n",
            "HTTP and other types are left alone"
        );
    }

    #[test]
    fn def_07_hand_over_to_an_app_already_listed_names_it_once() {
        assert_eq!(
            hand_over(WYE_FIRST, &wye(), Some(&id("brave"))),
            "[Default Applications]\n\
             x-scheme-handler/https=dev.soldunov.wye.desktop\n\
             text/html=brave.desktop;firefox.desktop;\n\
             application/xhtml+xml=brave.desktop\n\
             image/png=dev.soldunov.wye.desktop\n"
        );
    }

    #[test]
    fn def_07_hand_over_to_nobody_leaves_the_fallbacks() {
        assert_eq!(
            hand_over(WYE_FIRST, &wye(), None),
            "[Default Applications]\n\
             x-scheme-handler/https=dev.soldunov.wye.desktop\n\
             text/html=firefox.desktop;brave.desktop;\n\
             image/png=dev.soldunov.wye.desktop\n"
        );
    }

    #[test]
    fn def_07_hand_over_leaves_a_list_wye_does_not_head() {
        let text = "[Default Applications]\n\
                    text/html=brave.desktop;dev.soldunov.wye.desktop;\n";
        assert_eq!(hand_over(text, &wye(), Some(&id("firefox"))), text);
    }

    #[test]
    fn def_07_another_apps_html_default_stays() {
        let fx = Fixture::new();
        let text = "[Default Applications]\ntext/html=brave.desktop\n";
        fx.write("home/.config/mimeapps.list", text);
        release_html(&fx.xdg, &id("dev.soldunov.wye"), Some(&id("firefox"))).expect("released");
        assert_eq!(read(&fx), text);
    }
}
