//! Browser families and where known browsers keep their profiles (DISC-04,
//! DISC-06, DISC-07).
//!
//! Paths marked `// verify` come from packaging conventions rather than from
//! an inspected installation.

use std::path::{Path, PathBuf};

use wye_core::DesktopId;

use crate::entry::DesktopEntry;
use crate::exec::ExecTemplate;
use crate::xdg::XdgDirs;

/// The browser engine family, which decides private-window, profile and
/// new-window support.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BrowserFamily {
    Chromium,
    Firefox,
    Other,
}

impl BrowserFamily {
    /// The command-line flag that opens a private window (DISC-05).
    #[must_use]
    pub fn private_flag(self) -> Option<&'static str> {
        match self {
            Self::Chromium => Some("--incognito"),
            Self::Firefox => Some("--private-window"),
            Self::Other => None,
        }
    }

    /// The flag that forces a new window (LAUNCH-05).
    #[must_use]
    pub fn new_window_flag(self) -> Option<&'static str> {
        match self {
            Self::Chromium | Self::Firefox => Some("--new-window"),
            Self::Other => None,
        }
    }
}

/// How an app is installed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Packaging {
    Native,
    Flatpak,
    Snap,
}

/// Where a config directory is relative to.
#[derive(Debug, Clone, Copy)]
enum Base {
    Home,
    ConfigHome,
}

use Base::{ConfigHome, Home};

/// A known browser: its desktop IDs, family and candidate config
/// directories (Chromium: the directory holding `Local State`; Firefox: the
/// directory holding `profiles.ini`), most likely first.
struct KnownBrowser {
    ids: &'static [&'static str],
    family: BrowserFamily,
    dirs: &'static [(Base, &'static str)],
}

const fn chromium(
    ids: &'static [&'static str],
    dirs: &'static [(Base, &'static str)],
) -> KnownBrowser {
    KnownBrowser {
        ids,
        family: BrowserFamily::Chromium,
        dirs,
    }
}

const fn firefox(
    ids: &'static [&'static str],
    dirs: &'static [(Base, &'static str)],
) -> KnownBrowser {
    KnownBrowser {
        ids,
        family: BrowserFamily::Firefox,
        dirs,
    }
}

#[rustfmt::skip]
const KNOWN: &[KnownBrowser] = &[
    chromium(&["google-chrome.desktop"], &[(ConfigHome, "google-chrome")]),
    chromium(&["google-chrome-beta.desktop"], &[(ConfigHome, "google-chrome-beta")]),
    chromium(&["google-chrome-unstable.desktop"], &[(ConfigHome, "google-chrome-unstable")]),
    chromium(&["com.google.Chrome.desktop"], &[(Home, ".var/app/com.google.Chrome/config/google-chrome")]),
    chromium(&["chromium.desktop", "chromium-browser.desktop"], &[(ConfigHome, "chromium")]),
    chromium(&["org.chromium.Chromium.desktop"], &[(Home, ".var/app/org.chromium.Chromium/config/chromium")]),
    chromium(&["chromium_chromium.desktop"], &[(Home, "snap/chromium/common/chromium")]), // verify
    chromium(
        &["io.github.ungoogled_software.ungoogled_chromium.desktop"],
        &[(Home, ".var/app/io.github.ungoogled_software.ungoogled_chromium/config/chromium")], // verify
    ),
    chromium(&["brave-browser.desktop", "brave.desktop"], &[(ConfigHome, "BraveSoftware/Brave-Browser")]),
    chromium(&["com.brave.Browser.desktop"], &[(Home, ".var/app/com.brave.Browser/config/BraveSoftware/Brave-Browser")]),
    chromium(&["brave_brave.desktop"], &[(Home, "snap/brave/current/.config/BraveSoftware/Brave-Browser")]), // verify
    chromium(&["vivaldi-stable.desktop", "vivaldi.desktop"], &[(ConfigHome, "vivaldi")]),
    chromium(&["com.vivaldi.Vivaldi.desktop"], &[(Home, ".var/app/com.vivaldi.Vivaldi/config/vivaldi")]),
    chromium(&["microsoft-edge.desktop"], &[(ConfigHome, "microsoft-edge")]),
    chromium(&["microsoft-edge-beta.desktop"], &[(ConfigHome, "microsoft-edge-beta")]),
    chromium(&["microsoft-edge-dev.desktop"], &[(ConfigHome, "microsoft-edge-dev")]),
    chromium(&["com.microsoft.Edge.desktop"], &[(Home, ".var/app/com.microsoft.Edge/config/microsoft-edge")]),
    chromium(&["opera.desktop"], &[(ConfigHome, "opera")]),
    chromium(&["com.opera.Opera.desktop"], &[(Home, ".var/app/com.opera.Opera/config/opera")]), // verify
    firefox(
        &["firefox.desktop", "firefox-esr.desktop"],
        // Firefox reads ~/.mozilla/firefox and, since it adopted the XDG
        // layout, $XDG_CONFIG_HOME/mozilla/firefox. // verify
        &[(Home, ".mozilla/firefox"), (ConfigHome, "mozilla/firefox")],
    ),
    firefox(
        &["org.mozilla.firefox.desktop"],
        &[(Home, ".var/app/org.mozilla.firefox/.mozilla/firefox"), (Home, ".var/app/org.mozilla.firefox/config/mozilla/firefox")], // verify
    ),
    firefox(&["firefox_firefox.desktop"], &[(Home, "snap/firefox/common/.mozilla/firefox")]),
    firefox(&["zen.desktop", "zen-browser.desktop", "zen-beta.desktop"], &[(Home, ".zen")]), // verify
    firefox(&["app.zen_browser.zen.desktop"], &[(Home, ".var/app/app.zen_browser.zen/.zen")]), // verify
    firefox(&["librewolf.desktop"], &[(Home, ".librewolf")]),
    firefox(&["io.gitlab.librewolf-community.desktop"], &[(Home, ".var/app/io.gitlab.librewolf-community/.librewolf")]),
    firefox(&["floorp.desktop"], &[(Home, ".floorp")]),
    firefox(&["one.ablaze.floorp.desktop"], &[(Home, ".var/app/one.ablaze.floorp/.floorp")]), // verify
    firefox(&["waterfox.desktop"], &[(Home, ".waterfox")]),
    firefox(&["net.waterfox.waterfox.desktop"], &[(Home, ".var/app/net.waterfox.waterfox/.waterfox")]), // verify
];

/// Program-name prefixes for the `Exec` fallback, checked in order.
const PROGRAM_PREFIXES: &[(&str, BrowserFamily)] = &[
    ("google-chrome", BrowserFamily::Chromium),
    ("chrome", BrowserFamily::Chromium),
    ("chromium", BrowserFamily::Chromium),
    ("ungoogled-chromium", BrowserFamily::Chromium),
    ("brave", BrowserFamily::Chromium),
    ("vivaldi", BrowserFamily::Chromium),
    ("microsoft-edge", BrowserFamily::Chromium),
    ("opera", BrowserFamily::Chromium),
    ("thorium", BrowserFamily::Chromium),
    ("helium", BrowserFamily::Chromium), // verify
    ("firefox", BrowserFamily::Firefox),
    ("librewolf", BrowserFamily::Firefox),
    ("zen", BrowserFamily::Firefox),
    ("floorp", BrowserFamily::Firefox),
    ("waterfox", BrowserFamily::Firefox),
    ("icecat", BrowserFamily::Firefox),
    ("mullvad-browser", BrowserFamily::Firefox),
];

fn known(id: &DesktopId) -> Option<&'static KnownBrowser> {
    KNOWN
        .iter()
        .find(|browser| browser.ids.contains(&id.as_str()))
}

/// Detects the family from the desktop ID, then from the program `Exec`
/// runs (DISC-04). The `Exec` fallback looks through `env` assignments and
/// at Flatpak's `--command=`.
#[must_use]
pub fn detect_family(id: &DesktopId, exec: Option<&ExecTemplate>) -> BrowserFamily {
    known(id)
        .map(|browser| browser.family)
        .or_else(|| exec.and_then(|template| family_from_program(&program_name(template)?)))
        .unwrap_or(BrowserFamily::Other)
}

/// The candidate config directories of a known browser that exist on disk,
/// most likely first. Unknown browsers have none.
#[must_use]
pub fn config_dirs(id: &DesktopId, xdg: &XdgDirs) -> Vec<PathBuf> {
    known(id)
        .map(|browser| {
            browser
                .dirs
                .iter()
                .map(|(base, rel)| match base {
                    Base::Home => xdg.home.join(rel),
                    Base::ConfigHome => xdg.config_home.join(rel),
                })
                .filter(|dir| dir.is_dir())
                .collect()
        })
        .unwrap_or_default()
}

/// Detects Flatpak and Snap from `Exec` and from where the entry lives.
#[must_use]
pub fn detect_packaging(entry: &DesktopEntry, exec: Option<&ExecTemplate>) -> Packaging {
    let words = exec.map(ExecTemplate::words).unwrap_or_default();
    let program = words.first().map(|word| basename(word));
    let path = entry.path.to_string_lossy();
    if program == Some("flatpak") || path.contains("/flatpak/exports/") {
        Packaging::Flatpak
    } else if words.first().is_some_and(|word| word.starts_with("/snap/"))
        || path.contains("/snapd/desktop/")
    {
        Packaging::Snap
    } else {
        Packaging::Native
    }
}

/// The name of the program an `Exec` line really runs.
fn program_name(exec: &ExecTemplate) -> Option<String> {
    let words = exec.words();
    let mut rest = words.iter().map(String::as_str);
    let mut program = rest.next()?;
    if basename(program) == "env" {
        program = rest.find(|word| !word.contains('=') && !word.starts_with('-'))?;
    }
    if basename(program) == "flatpak" {
        let command = words
            .iter()
            .find_map(|word| word.strip_prefix("--command="))?;
        return Some(basename(command).to_owned());
    }
    Some(basename(program).to_owned())
}

fn family_from_program(program: &str) -> Option<BrowserFamily> {
    let program = program.to_ascii_lowercase();
    PROGRAM_PREFIXES
        .iter()
        .find(|(prefix, _)| program.starts_with(prefix))
        .map(|(_, family)| *family)
}

fn basename(path: &str) -> &str {
    Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(path)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    fn id(value: &str) -> DesktopId {
        DesktopId::new(value).unwrap()
    }

    fn exec(line: &str) -> ExecTemplate {
        ExecTemplate::parse(line).unwrap()
    }

    #[test]
    fn detects_known_ids() {
        assert_eq!(
            detect_family(&id("google-chrome"), None),
            BrowserFamily::Chromium
        );
        assert_eq!(
            detect_family(&id("com.brave.Browser"), None),
            BrowserFamily::Chromium
        );
        assert_eq!(
            detect_family(&id("app.zen_browser.zen"), None),
            BrowserFamily::Firefox
        );
        assert_eq!(
            detect_family(&id("firefox_firefox"), None),
            BrowserFamily::Firefox
        );
        assert_eq!(
            detect_family(&id("org.gnome.Epiphany"), None),
            BrowserFamily::Other
        );
    }

    #[test]
    fn falls_back_to_program_name() {
        let cases = [
            ("/opt/thorium/thorium-browser %U", BrowserFamily::Chromium),
            (
                "env GDK_BACKEND=x11 /usr/bin/Firefox-Nightly %u",
                BrowserFamily::Firefox,
            ),
            (
                "/usr/bin/flatpak run --command=librewolf io.example.Wolf @@u %u @@",
                BrowserFamily::Firefox,
            ),
            ("epiphany %U", BrowserFamily::Other),
        ];
        for (line, family) in cases {
            assert_eq!(
                detect_family(&id("custom"), Some(&exec(line))),
                family,
                "{line}"
            );
        }
    }

    #[test]
    fn lists_existing_config_dirs() {
        let home = tempfile::tempdir().unwrap();
        let xdg = XdgDirs {
            home: home.path().to_path_buf(),
            config_home: home.path().join(".config"),
            config_dirs: vec![],
            data_home: home.path().join(".local/share"),
            data_dirs: vec![],
            current_desktops: vec![],
            search_path: vec![],
        };
        assert!(config_dirs(&id("firefox"), &xdg).is_empty());
        fs::create_dir_all(home.path().join(".config/mozilla/firefox")).unwrap();
        fs::create_dir_all(home.path().join(".mozilla/firefox")).unwrap();
        assert_eq!(
            config_dirs(&id("firefox"), &xdg),
            vec![
                home.path().join(".mozilla/firefox"),
                home.path().join(".config/mozilla/firefox")
            ]
        );
        assert!(config_dirs(&id("unknown"), &xdg).is_empty());
    }

    #[test]
    fn detects_packaging() {
        let entry = |path: &str| {
            DesktopEntry::parse(id("x"), PathBuf::from(path), "[Desktop Entry]\n").unwrap()
        };
        let native = entry("/usr/share/applications/x.desktop");
        assert_eq!(
            detect_packaging(&native, Some(&exec("/usr/bin/flatpak run x @@u %u @@"))),
            Packaging::Flatpak
        );
        assert_eq!(
            detect_packaging(&native, Some(&exec("/snap/bin/firefox %u"))),
            Packaging::Snap
        );
        assert_eq!(
            detect_packaging(&native, Some(&exec("x %u"))),
            Packaging::Native
        );
        let exported = entry("/var/lib/flatpak/exports/share/applications/x.desktop");
        assert_eq!(detect_packaging(&exported, None), Packaging::Flatpak);
    }
}
