//! Who handles web links now (DEF-03, ONB-10). Blocking file reads.

use wye_core::DesktopId;
use wye_desktop::{XdgDirs, kdeglobals, listed_default};

/// The default-browser registration as the desktop sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Registration {
    /// Links go to Wye.
    pub is_default: bool,
    /// The app they go to instead, when known.
    pub current: Option<DesktopId>,
}

/// Read `mimeapps.list` and, on KDE, `kdeglobals`: Wye is the default when
/// `mimeapps.list` lists it for `https` and Plasma's own setting (DEF-02)
/// does not name another app.
pub(crate) fn registration(xdg: &XdgDirs, wye: &DesktopId) -> Registration {
    let listed = listed_default(xdg);
    if listed.as_ref() != Some(wye) {
        return Registration {
            is_default: false,
            current: listed,
        };
    }
    let plasma = kdeglobals::is_kde(xdg)
        .then(|| kdeglobals::browser(xdg))
        .flatten()
        // An old-style `!command` value names no desktop entry.
        .and_then(|value| DesktopId::new(value.trim_start_matches('!')).ok())
        .filter(|id| id != wye);
    match plasma {
        Some(other) => Registration {
            is_default: false,
            current: Some(other),
        },
        None => Registration {
            is_default: true,
            current: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::path::Path;

    use super::*;

    fn xdg(root: &Path, desktop: &str) -> XdgDirs {
        let vars = [
            ("HOME", root.join("home")),
            ("XDG_CONFIG_HOME", root.join("config")),
            ("XDG_DATA_HOME", root.join("data")),
            ("XDG_DATA_DIRS", root.join("sysdata")),
            ("XDG_CONFIG_DIRS", root.join("sysconfig")),
            ("XDG_CURRENT_DESKTOP", desktop.into()),
        ];
        XdgDirs::from_lookup(|name| {
            vars.iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| OsString::from(value))
        })
        .expect("home")
    }

    fn id(text: &str) -> DesktopId {
        DesktopId::new(text).expect("valid")
    }

    fn write(root: &Path, file: &str, text: &str) {
        let path = root.join("config").join(file);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("dir");
        std::fs::write(path, text).expect("written");
    }

    const WYE_LISTED: &str = "[Default Applications]\nx-scheme-handler/http=dev.soldunov.wye.desktop\nx-scheme-handler/https=dev.soldunov.wye.desktop\n";

    #[test]
    fn another_listed_browser_took_over() {
        let dir = tempfile::tempdir().expect("temp dir");
        write(
            dir.path(),
            "mimeapps.list",
            "[Default Applications]\nx-scheme-handler/https=firefox.desktop\n",
        );
        let found = registration(&xdg(dir.path(), "GNOME"), &id("dev.soldunov.wye.desktop"));
        assert!(!found.is_default);
        assert_eq!(found.current, Some(id("firefox.desktop")));
    }

    #[test]
    fn wye_listed_is_the_default() {
        let dir = tempfile::tempdir().expect("temp dir");
        write(dir.path(), "mimeapps.list", WYE_LISTED);
        let found = registration(&xdg(dir.path(), "GNOME"), &id("dev.soldunov.wye.desktop"));
        assert!(found.is_default);
    }

    #[test]
    fn plasmas_own_setting_can_take_over() {
        let dir = tempfile::tempdir().expect("temp dir");
        write(dir.path(), "mimeapps.list", WYE_LISTED);
        write(
            dir.path(),
            "kdeglobals",
            "[General]\nBrowserApplication=org.kde.falkon.desktop\n",
        );
        let found = registration(&xdg(dir.path(), "KDE"), &id("dev.soldunov.wye.desktop"));
        assert!(!found.is_default, "DEF-03 on Plasma");
        assert_eq!(found.current, Some(id("org.kde.falkon.desktop")));
        let elsewhere = registration(&xdg(dir.path(), "GNOME"), &id("dev.soldunov.wye.desktop"));
        assert!(elsewhere.is_default, "kdeglobals only counts on KDE");
    }
}
