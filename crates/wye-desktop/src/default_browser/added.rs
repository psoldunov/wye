//! `[Added Associations]` for the HTML types (DEF-07).
//!
//! `KService` (Plasma) only honours a `[Default Applications]` entry for an
//! app associated with the type: one whose desktop entry lists it, or one
//! the user added under `[Added Associations]`. Wye's entry does not claim
//! HTML files, so opting in adds Wye there too, first in the list, and
//! stopping removes it again.

use wye_core::DesktopId;

use super::{
    DefaultBrowserError, HTML_TYPES, MIMEAPPS, ensure_writable, read_existing, remove_key, set_keys,
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
