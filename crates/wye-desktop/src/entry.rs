//! Desktop entries: the `[Desktop Entry]` group and its
//! `[Desktop Action <id>]` groups ([Desktop Entry Specification]).
//!
//! Only the keys Wye uses are read. Localised keys such as `Name[de]` are
//! ignored for now; the unlocalised value is used everywhere.
//!
//! [Desktop Entry Specification]: https://specifications.freedesktop.org/desktop-entry-spec/latest/

use std::path::{Path, PathBuf};

use wye_core::DesktopId;

use crate::keyfile::{self, Group};

const MAIN_GROUP: &str = "Desktop Entry";
const ACTION_PREFIX: &str = "Desktop Action ";

/// A parsed desktop entry.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "the booleans mirror the Desktop Entry Specification's keys one to one"
)]
pub struct DesktopEntry {
    pub id: DesktopId,
    /// The file the entry was read from.
    pub path: PathBuf,
    /// `Type`, normally `Application`.
    pub entry_type: Option<String>,
    /// `Name`; falls back to the application ID when the key is missing.
    pub name: String,
    pub generic_name: Option<String>,
    pub icon: Option<String>,
    /// `Exec` after string unescaping; see [`crate::exec`] for its own quoting.
    pub exec: Option<String>,
    pub try_exec: Option<String>,
    pub hidden: bool,
    pub no_display: bool,
    pub only_show_in: Vec<String>,
    pub not_show_in: Vec<String>,
    pub mime_types: Vec<String>,
    pub categories: Vec<String>,
    pub keywords: Vec<String>,
    pub dbus_activatable: bool,
    pub terminal: bool,
    /// The actions listed in `Actions` that have a `[Desktop Action <id>]`
    /// group with a `Name`, in the order `Actions` lists them.
    pub actions: Vec<DesktopAction>,
}

/// An additional way to launch an app, such as "New Private Window".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopAction {
    pub id: String,
    pub name: String,
    /// Missing for D-Bus-activated actions.
    pub exec: Option<String>,
}

/// A desktop entry that could not be read.
#[derive(Debug, thiserror::Error)]
pub enum EntryError {
    #[error("cannot read {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("{path} has no [Desktop Entry] group")]
    MissingGroup { path: PathBuf },
}

impl DesktopEntry {
    /// Reads and parses the entry at `path`.
    ///
    /// # Errors
    ///
    /// Returns [`EntryError::Io`] when the file cannot be read and
    /// [`EntryError::MissingGroup`] when it is not a desktop entry.
    pub fn load(id: DesktopId, path: &Path) -> Result<Self, EntryError> {
        let bytes = std::fs::read(path).map_err(|source| EntryError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        Self::parse(id, path.to_path_buf(), &String::from_utf8_lossy(&bytes))
    }

    /// Parses entry text.
    ///
    /// # Errors
    ///
    /// Returns [`EntryError::MissingGroup`] when `text` has no
    /// `[Desktop Entry]` group.
    pub fn parse(id: DesktopId, path: PathBuf, text: &str) -> Result<Self, EntryError> {
        let groups = keyfile::parse(text);
        let Some(main) = groups.iter().find(|group| group.name == MAIN_GROUP) else {
            return Err(EntryError::MissingGroup { path });
        };
        let actions = main
            .list("Actions")
            .into_iter()
            .filter_map(|action_id| parse_action(&groups, action_id))
            .collect();

        Ok(Self {
            name: main
                .string("Name")
                .unwrap_or_else(|| id.app_id().to_owned()),
            entry_type: main.string("Type"),
            generic_name: main.string("GenericName"),
            icon: main.string("Icon"),
            exec: main.string("Exec"),
            try_exec: main.string("TryExec"),
            hidden: main.bool("Hidden"),
            no_display: main.bool("NoDisplay"),
            only_show_in: main.list("OnlyShowIn"),
            not_show_in: main.list("NotShowIn"),
            mime_types: main.list("MimeType"),
            categories: main.list("Categories"),
            keywords: main.list("Keywords"),
            dbus_activatable: main.bool("DBusActivatable"),
            terminal: main.bool("Terminal"),
            actions,
            id,
            path,
        })
    }

    /// True when `MimeType` lists `x-scheme-handler/http` or `https`
    /// (DISC-01).
    #[must_use]
    pub fn handles_web(&self) -> bool {
        self.mime_types
            .iter()
            .any(|mime| mime == "x-scheme-handler/http" || mime == "x-scheme-handler/https")
    }

    /// Applies `OnlyShowIn`/`NotShowIn` to the session's desktops
    /// (`$XDG_CURRENT_DESKTOP`); names compare case-sensitively, as the
    /// specification's registered names do.
    #[must_use]
    pub fn shown_in(&self, current_desktops: &[String]) -> bool {
        let listed = |list: &[String]| list.iter().any(|name| current_desktops.contains(name));
        if listed(&self.not_show_in) {
            return false;
        }
        self.only_show_in.is_empty() || listed(&self.only_show_in)
    }

    #[must_use]
    pub fn action(&self, id: &str) -> Option<&DesktopAction> {
        self.actions.iter().find(|action| action.id == id)
    }
}

fn parse_action(groups: &[Group], id: String) -> Option<DesktopAction> {
    let group = groups
        .iter()
        .find(|group| group.name.strip_prefix(ACTION_PREFIX) == Some(id.as_str()))?;
    Some(DesktopAction {
        name: group.string("Name")?,
        exec: group.string("Exec"),
        id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIREFOX: &str = r"[Desktop Entry]
Version=1.0
Name=Firefox
Name[de]=Feuerfuchs
GenericName=Web Browser
Icon=firefox
Exec=firefox --name firefox %u
TryExec=firefox
Type=Application
Terminal=false
MimeType=text/html;x-scheme-handler/http;x-scheme-handler/https;
Categories=Network;WebBrowser;
Keywords=web\sbrowser;internet\;www;
Actions=new-window;new-private-window;ghost;
StartupWMClass=firefox

[Desktop Action new-window]
Name=New Window
Exec=firefox --new-window %u

[Desktop Action new-private-window]
Name=New Private Window
Exec=firefox --private-window %u

[Desktop Action unlisted]
Name=Unlisted
Exec=firefox
";

    fn entry(text: &str) -> DesktopEntry {
        DesktopEntry::parse(
            DesktopId::new("firefox.desktop").unwrap(),
            PathBuf::from("/usr/share/applications/firefox.desktop"),
            text,
        )
        .unwrap()
    }

    #[test]
    fn parses_main_group() {
        let e = entry(FIREFOX);
        assert_eq!(e.name, "Firefox");
        assert_eq!(e.generic_name.as_deref(), Some("Web Browser"));
        assert_eq!(e.exec.as_deref(), Some("firefox --name firefox %u"));
        assert_eq!(e.try_exec.as_deref(), Some("firefox"));
        assert_eq!(e.entry_type.as_deref(), Some("Application"));
        assert!(!e.terminal && !e.hidden && !e.no_display && !e.dbus_activatable);
        assert_eq!(e.categories, vec!["Network", "WebBrowser"]);
        assert_eq!(e.keywords, vec!["web browser", "internet;www"]);
        assert!(e.handles_web());
    }

    #[test]
    fn keeps_listed_actions_in_order() {
        let e = entry(FIREFOX);
        let ids: Vec<&str> = e.actions.iter().map(|a| a.id.as_str()).collect();
        assert_eq!(ids, vec!["new-window", "new-private-window"]);
        assert_eq!(
            e.action("new-private-window").unwrap().exec.as_deref(),
            Some("firefox --private-window %u")
        );
    }

    #[test]
    fn falls_back_to_app_id_for_name() {
        let e = entry("[Desktop Entry]\nHidden=true\n");
        assert_eq!(e.name, "firefox");
        assert!(e.hidden);
        assert!(!e.handles_web());
    }

    #[test]
    fn rejects_files_without_main_group() {
        let result = DesktopEntry::parse(
            DesktopId::new("x").unwrap(),
            PathBuf::from("/x.desktop"),
            "[Other]\nName=x\n",
        );
        assert!(matches!(result, Err(EntryError::MissingGroup { .. })));
    }

    #[test]
    fn applies_show_in() {
        let mut e = entry(FIREFOX);
        let gnome = vec!["GNOME".to_owned()];
        assert!(e.shown_in(&gnome));
        e.only_show_in = vec!["KDE".into()];
        assert!(!e.shown_in(&gnome));
        e.only_show_in = vec!["KDE".into(), "GNOME".into()];
        assert!(e.shown_in(&gnome));
        e.not_show_in = vec!["GNOME".into()];
        assert!(!e.shown_in(&gnome));
    }

    #[test]
    fn loads_from_disk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("firefox.desktop");
        std::fs::write(&path, FIREFOX).unwrap();
        let e = DesktopEntry::load(DesktopId::new("firefox").unwrap(), &path).unwrap();
        assert_eq!(e.path, path);
        let missing = DesktopEntry::load(e.id.clone(), &dir.path().join("none.desktop"));
        assert!(matches!(missing, Err(EntryError::Io { .. })));
    }
}
