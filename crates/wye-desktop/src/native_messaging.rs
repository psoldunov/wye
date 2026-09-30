//! Native-messaging host manifests for Wye's browser extension (BEXT-04).
//!
//! A browser starts `wye-native-host` for the extension only when it finds a
//! manifest named after the host in its own directory: `NativeMessagingHosts/`
//! under a Chromium-family browser's config directory, `native-messaging-hosts/`
//! under a Firefox-family browser's home directory. Wye writes one per
//! browser whose directory exists. Flatpak and Snap browsers are not
//! supported: their sandbox cannot run a host outside it.
//!
//! Directories marked `// verify` come from the browsers' documentation or
//! packaging conventions rather than from an inspected installation.

use std::io;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::atomic;
use crate::xdg::XdgDirs;

/// The host's name; the manifest file is `<HOST_NAME>.json`.
pub const HOST_NAME: &str = "dev.soldunov.wye";

/// The program the browsers start.
pub const HOST_PROGRAM: &str = "wye-native-host";

/// The Firefox extension's ID (`browser_specific_settings.gecko.id` in
/// `frontends/extension/manifest.firefox.json`).
pub const FIREFOX_EXTENSION_ID: &str = "wye@soldunov.dev";

/// The Chromium extension's ID, fixed by the `key` in
/// `frontends/extension/manifest.chromium.json` (the first 128 bits of the
/// key's SHA-256, written with the letters `a` to `p`).
pub const CHROMIUM_EXTENSION_ID: &str = "lphepmclmllmbbkjkdhjbdgbjfpmmdnn";

const DESCRIPTION: &str = "Wye: send links from the browser to Wye";

/// The manifest dialect a browser reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dialect {
    /// Chrome, Chromium, Brave, Vivaldi, Edge: `allowed_origins`.
    Chromium,
    /// Firefox and its forks (Waterfox, Floorp, Zen and others): `allowed_extensions`.
    Firefox,
}

/// Where a location's directory is relative to.
#[derive(Debug, Clone, Copy)]
enum Base {
    Home,
    ConfigHome,
}

/// A browser's host-manifest directory.
#[derive(Debug, Clone, Copy)]
struct Location {
    browser: &'static str,
    dialect: Dialect,
    base: Base,
    /// The browser's own directory; the manifest is only written when it
    /// exists (the browser is installed and has run).
    root: &'static str,
    /// The manifest directory inside `root`.
    hosts: &'static str,
}

const CHROMIUM_HOSTS: &str = "NativeMessagingHosts";
const FIREFOX_HOSTS: &str = "native-messaging-hosts";

const fn chromium(browser: &'static str, root: &'static str) -> Location {
    Location {
        browser,
        dialect: Dialect::Chromium,
        base: Base::ConfigHome,
        root,
        hosts: CHROMIUM_HOSTS,
    }
}

const fn firefox(browser: &'static str, base: Base, root: &'static str) -> Location {
    Location {
        browser,
        dialect: Dialect::Firefox,
        base,
        root,
        hosts: FIREFOX_HOSTS,
    }
}

#[rustfmt::skip]
const LOCATIONS: &[Location] = &[
    chromium("Google Chrome", "google-chrome"),
    chromium("Google Chrome Beta", "google-chrome-beta"),
    chromium("Google Chrome Dev", "google-chrome-unstable"),
    chromium("Chromium", "chromium"),
    chromium("Brave", "BraveSoftware/Brave-Browser"),
    chromium("Vivaldi", "vivaldi"),
    chromium("Microsoft Edge", "microsoft-edge"),
    chromium("Microsoft Edge Beta", "microsoft-edge-beta"),
    chromium("Microsoft Edge Dev", "microsoft-edge-dev"),
    chromium("Thorium", "thorium"), // verify
    chromium("Helium", "net.imput.helium"), // verify
    // Firefox and the forks that keep Mozilla's directory.
    firefox("Firefox", Base::Home, ".mozilla"),
    // Firefox with the XDG layout. // verify
    firefox("Firefox", Base::ConfigHome, "mozilla"),
    firefox("LibreWolf", Base::Home, ".librewolf"),
    firefox("Waterfox", Base::Home, ".waterfox"), // verify
    firefox("Floorp", Base::Home, ".floorp"), // verify
    firefox("Zen", Base::Home, ".zen"), // verify
];

/// One manifest Wye wrote or removed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    /// The browser it is for, for example `Firefox`.
    pub browser: &'static str,
    /// The manifest file.
    pub path: PathBuf,
}

/// Why the manifests could not be written or removed.
#[derive(Debug, thiserror::Error)]
pub enum NativeMessagingError {
    /// The host program's path is not absolute; browsers need one.
    #[error("the native host path {0} is not absolute")]
    RelativeHost(PathBuf),
    /// A file could not be written or removed.
    #[error("{path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

/// The manifest `dialect` browsers read, starting `host`.
#[must_use]
pub fn manifest(dialect: Dialect, host: &Path) -> Value {
    let base = json!({
        "name": HOST_NAME,
        "description": DESCRIPTION,
        "path": host.to_string_lossy(),
        "type": "stdio",
    });
    let allowed = match dialect {
        Dialect::Chromium => json!({
            "allowed_origins": [format!("chrome-extension://{CHROMIUM_EXTENSION_ID}/")],
        }),
        Dialect::Firefox => json!({ "allowed_extensions": [FIREFOX_EXTENSION_ID] }),
    };
    merge(base, allowed)
}

fn merge(base: Value, extra: Value) -> Value {
    match (base, extra) {
        (Value::Object(base), Value::Object(extra)) => {
            Value::Object(base.into_iter().chain(extra).collect())
        }
        (base, _) => base,
    }
}

/// Write a manifest starting `host` for every browser whose directory
/// exists; returns what was written (BEXT-04).
///
/// # Errors
///
/// [`NativeMessagingError::RelativeHost`] for a relative `host`, else the
/// first file that could not be written (earlier ones stay written).
pub fn install(xdg: &XdgDirs, host: &Path) -> Result<Vec<Manifest>, NativeMessagingError> {
    if !host.is_absolute() {
        return Err(NativeMessagingError::RelativeHost(host.to_path_buf()));
    }
    detected(xdg)
        .map(|(location, path)| {
            let text = format!("{:#}\n", manifest(location.dialect, host));
            atomic::write(&path, text.as_bytes(), Some(&path))
                .map_err(|source| NativeMessagingError::Io {
                    path: path.clone(),
                    source,
                })
                .map(|()| Manifest {
                    browser: location.browser,
                    path,
                })
        })
        .collect()
}

/// Remove every manifest Wye wrote; returns what was removed.
///
/// # Errors
///
/// The first file that exists but could not be removed.
pub fn remove(xdg: &XdgDirs) -> Result<Vec<Manifest>, NativeMessagingError> {
    let mut removed = Vec::new();
    for (location, path) in LOCATIONS
        .iter()
        .map(|location| (location, manifest_path(xdg, location)))
    {
        match std::fs::remove_file(&path) {
            Ok(()) => removed.push(Manifest {
                browser: location.browser,
                path,
            }),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(source) => return Err(NativeMessagingError::Io { path, source }),
        }
    }
    Ok(removed)
}

/// The manifests Wye would write: one per browser directory that exists.
fn detected(xdg: &XdgDirs) -> impl Iterator<Item = (&'static Location, PathBuf)> + '_ {
    LOCATIONS
        .iter()
        .filter(|location| root(xdg, location).is_dir())
        .map(|location| (location, manifest_path(xdg, location)))
}

fn root(xdg: &XdgDirs, location: &Location) -> PathBuf {
    match location.base {
        Base::Home => xdg.home.join(location.root),
        Base::ConfigHome => xdg.config_home.join(location.root),
    }
}

fn manifest_path(xdg: &XdgDirs, location: &Location) -> PathBuf {
    root(xdg, location)
        .join(location.hosts)
        .join(format!("{HOST_NAME}.json"))
}

/// The host program to name in the manifests: the first `wye-native-host`
/// on the search path, as found there (a profile link such as
/// `/etc/profiles/per-user/…/bin` survives upgrades where the resolved store
/// path would not), else the one next to `current_exe`.
#[must_use]
pub fn find_host(xdg: &XdgDirs, current_exe: Option<&Path>) -> Option<PathBuf> {
    xdg.search_path
        .iter()
        .filter(|dir| dir.is_absolute())
        .map(|dir| dir.join(HOST_PROGRAM))
        .find(|candidate| candidate.is_file())
        .or_else(|| {
            current_exe
                .and_then(Path::parent)
                .map(|dir| dir.join(HOST_PROGRAM))
                .filter(|candidate| candidate.is_file())
        })
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::test_support::Fixture;

    const HOST: &str = "/usr/bin/wye-native-host";

    /// Chromium names the extension by its fixed origin, Firefox by its
    /// gecko ID; each manifest carries only its own family's key.
    #[test]
    fn each_family_allows_only_its_own_extension_id_bext_04() {
        let families = [
            (
                Dialect::Chromium,
                "allowed_origins",
                "chrome-extension://lphepmclmllmbbkjkdhjbdgbjfpmmdnn/",
                "allowed_extensions",
            ),
            (
                Dialect::Firefox,
                "allowed_extensions",
                "wye@soldunov.dev",
                "allowed_origins",
            ),
        ];
        for (dialect, allowed, id, absent) in families {
            let manifest = manifest(dialect, Path::new(HOST));
            assert_eq!(manifest["name"], HOST_NAME);
            assert_eq!(manifest["path"], HOST);
            assert_eq!(manifest["type"], "stdio");
            assert_eq!(manifest[allowed], json!([id]), "{dialect:?}");
            assert!(manifest.get(absent).is_none(), "{dialect:?}");
        }
    }

    #[test]
    fn install_writes_only_for_browsers_that_are_there_bext_04() {
        let fixture = Fixture::new();
        fs::create_dir_all(fixture.xdg.config_home.join("google-chrome")).unwrap();
        fs::create_dir_all(fixture.xdg.home.join(".mozilla/firefox")).unwrap();

        let written = install(&fixture.xdg, Path::new(HOST)).unwrap();

        let browsers: Vec<_> = written.iter().map(|manifest| manifest.browser).collect();
        assert_eq!(browsers, ["Google Chrome", "Firefox"]);
        let chrome = fixture
            .xdg
            .config_home
            .join("google-chrome/NativeMessagingHosts/dev.soldunov.wye.json");
        let firefox = fixture
            .xdg
            .home
            .join(".mozilla/native-messaging-hosts/dev.soldunov.wye.json");
        assert_eq!(written[0].path, chrome);
        assert_eq!(written[1].path, firefox);
        let text: Value = serde_json::from_str(&fs::read_to_string(&firefox).unwrap()).unwrap();
        assert_eq!(text, manifest(Dialect::Firefox, Path::new(HOST)));
        assert!(!fixture.xdg.config_home.join("chromium").exists());
    }

    #[test]
    fn installing_again_replaces_the_host_path() {
        let fixture = Fixture::new();
        fs::create_dir_all(fixture.xdg.config_home.join("chromium")).unwrap();
        install(&fixture.xdg, Path::new("/old/wye-native-host")).unwrap();
        let written = install(&fixture.xdg, Path::new(HOST)).unwrap();
        let text: Value =
            serde_json::from_str(&fs::read_to_string(&written[0].path).unwrap()).unwrap();
        assert_eq!(text["path"], HOST);
    }

    #[test]
    fn a_relative_host_is_refused() {
        let fixture = Fixture::new();
        assert!(matches!(
            install(&fixture.xdg, Path::new("wye-native-host")),
            Err(NativeMessagingError::RelativeHost(_))
        ));
    }

    #[test]
    fn remove_deletes_what_install_wrote_and_nothing_else() {
        let fixture = Fixture::new();
        fs::create_dir_all(fixture.xdg.home.join(".librewolf")).unwrap();
        let other = fixture
            .xdg
            .home
            .join(".librewolf/native-messaging-hosts/other.json");
        fs::create_dir_all(other.parent().unwrap()).unwrap();
        fs::write(&other, "{}").unwrap();
        install(&fixture.xdg, Path::new(HOST)).unwrap();

        let removed = remove(&fixture.xdg).unwrap();

        assert_eq!(removed.len(), 1);
        assert_eq!(removed[0].browser, "LibreWolf");
        assert!(!removed[0].path.exists());
        assert!(other.exists());
        assert!(remove(&fixture.xdg).unwrap().is_empty());
    }

    #[test]
    fn the_host_is_found_on_the_path_before_next_to_wye() {
        let fixture = Fixture::new();
        let beside = fixture.path("store/bin");
        fs::create_dir_all(&beside).unwrap();
        fs::write(beside.join(HOST_PROGRAM), "").unwrap();
        let exe = beside.join("wye");
        assert_eq!(
            find_host(&fixture.xdg, Some(&exe)),
            Some(beside.join(HOST_PROGRAM))
        );
        let on_path = fixture.program(HOST_PROGRAM);
        assert_eq!(find_host(&fixture.xdg, Some(&exe)), Some(on_path));
        assert_eq!(
            find_host(&fixture.xdg, None).map(|p| p.is_file()),
            Some(true)
        );
    }
}
