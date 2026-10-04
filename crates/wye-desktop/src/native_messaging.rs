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
//!
//! Zen is detected by `~/.zen` but reads `~/.mozilla/native-messaging-hosts/`,
//! the directory Firefox uses; a manifest under `~/.zen` is never read. Wye 1.0.0
//! wrote one there, and [`remove`] deletes it.

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

/// The Chromium extension's ID: the Chrome Web Store item's, which the `key` in
/// `frontends/extension/manifest.chromium.json` fixes for the unpacked and the
/// sideloaded build too (the first 128 bits of the key's SHA-256, written with
/// the letters `a` to `p`). The store's own build carries no `key` and gets
/// the same ID from the item's key (BEXT-04).
pub const CHROMIUM_EXTENSION_ID: &str = "jdcifhpoallkdjnbflfienpboodjfjei";

/// The ID of the unpacked Chromium extension Wye 1.0.0 shipped, whose key is
/// lost. Still allowed, so a browser that has that build loaded keeps working
/// after an upgrade (BEXT-04).
pub const LEGACY_CHROMIUM_EXTENSION_ID: &str = "lphepmclmllmbbkjkdhjbdgbjfpmmdnn";

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
    /// The browser directory that holds the manifest directory, when it is not
    /// `root` (a fork that reads another browser's directory).
    hosts_root: Option<&'static str>,
    /// The manifest directory inside `hosts_root`, or `root` without one.
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
        hosts_root: None,
        hosts: CHROMIUM_HOSTS,
    }
}

const fn firefox(browser: &'static str, base: Base, root: &'static str) -> Location {
    Location {
        browser,
        dialect: Dialect::Firefox,
        base,
        root,
        hosts_root: None,
        hosts: FIREFOX_HOSTS,
    }
}

/// A Firefox fork detected by `root` that reads the manifests Firefox reads,
/// from `hosts_root`.
const fn firefox_reading(
    browser: &'static str,
    root: &'static str,
    hosts_root: &'static str,
) -> Location {
    Location {
        hosts_root: Some(hosts_root),
        ..firefox(browser, Base::Home, root)
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
    // Detected by `~/.zen`, reads `~/.mozilla/native-messaging-hosts/`: verified
    // with Zen 1.22.3b (2026-10-04); a manifest under `~/.zen` is ignored.
    firefox_reading("Zen", ".zen", ".mozilla"),
];

/// What happened to one manifest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// Written now: it was missing or said something else.
    Written,
    /// Already there as Wye would write it; not touched.
    Current,
    /// Deleted.
    Removed,
    /// A symlink is in its place, so something else (home-manager, for
    /// example) owns the file: left alone, neither written nor deleted.
    Symlink,
}

/// One manifest Wye wrote, found current, removed or left alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    /// The browser it is for, for example `Firefox`.
    pub browser: &'static str,
    /// The manifest file.
    pub path: PathBuf,
    /// What happened to it.
    pub outcome: Outcome,
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
            "allowed_origins": [
                format!("chrome-extension://{CHROMIUM_EXTENSION_ID}/"),
                format!("chrome-extension://{LEGACY_CHROMIUM_EXTENSION_ID}/"),
            ],
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

/// Make sure every browser whose directory exists has a manifest starting
/// `host`; returns every such browser's manifest with its [`Outcome`]:
/// written now, already current, or left alone as a symlink (BEXT-04).
///
/// A manifest is written only where it is missing or says something else,
/// so a run that changes nothing touches no file. A symlink in a
/// manifest's place is left alone: whatever manages the link (home-manager,
/// for example) owns the file.
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
            write_if_changed(&path, &text)
                .map_err(|source| NativeMessagingError::Io {
                    path: path.clone(),
                    source,
                })
                .map(|outcome| Manifest {
                    browser: location.browser,
                    path,
                    outcome,
                })
        })
        .collect()
}

/// Like [`install`], returning only the manifests written now (BEXT-04).
///
/// # Errors
///
/// As [`install`].
pub fn refresh(xdg: &XdgDirs, host: &Path) -> Result<Vec<Manifest>, NativeMessagingError> {
    Ok(install(xdg, host)?
        .into_iter()
        .filter(|manifest| manifest.outcome == Outcome::Written)
        .collect())
}

/// Write `text` to `path` unless it is there already or `path` is a
/// symlink.
fn write_if_changed(path: &Path, text: &str) -> io::Result<Outcome> {
    if is_symlink(path) {
        return Ok(Outcome::Symlink);
    }
    if std::fs::read_to_string(path).is_ok_and(|current| current == text) {
        return Ok(Outcome::Current);
    }
    atomic::write(path, text.as_bytes(), Some(path)).map(|()| Outcome::Written)
}

fn is_symlink(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|meta| meta.file_type().is_symlink())
}

/// Remove every manifest Wye wrote; returns what was removed, and the
/// symlinks left alone in a manifest's place (something else owns them,
/// as [`install`] assumes).
///
/// # Errors
///
/// The first file that exists but could not be removed.
pub fn remove(xdg: &XdgDirs) -> Result<Vec<Manifest>, NativeMessagingError> {
    let mut removed = Vec::new();
    let mut seen = Vec::new();
    for (location, path) in LOCATIONS
        .iter()
        .flat_map(|location| {
            // Wye 1.0.0 wrote Zen's manifest under `~/.zen`, where Zen never reads it.
            let stale = location.hosts_root.map(|_| Location {
                hosts_root: None,
                ..*location
            });
            [Some(*location), stale]
        })
        .flatten()
        .map(|location| (location, manifest_path(xdg, &location)))
    {
        if seen.contains(&path) {
            continue;
        }
        seen.push(path.clone());
        let outcome = if is_symlink(&path) {
            Outcome::Symlink
        } else {
            match std::fs::remove_file(&path) {
                Ok(()) => Outcome::Removed,
                Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                Err(source) => return Err(NativeMessagingError::Io { path, source }),
            }
        };
        removed.push(Manifest {
            browser: location.browser,
            path,
            outcome,
        });
    }
    Ok(removed)
}

/// The directories whose appearance may bring a browser to write a
/// manifest for: the first component of each browser's directory under
/// its base, such as `~/.config/BraveSoftware`, `~/.mozilla` or `~/.zen`.
/// The service watches for them so a browser installed while it runs gets
/// its manifest without a restart (BEXT-04).
#[must_use]
pub fn watched_names(xdg: &XdgDirs) -> Vec<PathBuf> {
    let mut names: Vec<PathBuf> = Vec::new();
    for location in LOCATIONS {
        let Some(first) = Path::new(location.root).components().next() else {
            continue;
        };
        let name = base_dir(xdg, location).join(first);
        if !names.contains(&name) {
            names.push(name);
        }
    }
    names
}

/// The manifests Wye would write: one per browser directory that exists.
fn detected(xdg: &XdgDirs) -> impl Iterator<Item = (&'static Location, PathBuf)> + '_ {
    let mut seen = Vec::new();
    LOCATIONS
        .iter()
        .filter(|location| root(xdg, location).is_dir())
        .map(|location| (location, manifest_path(xdg, location)))
        .filter(move |(_, path)| {
            // Firefox and Zen share a manifest: the first browser keeps it.
            let fresh = !seen.contains(path);
            seen.push(path.clone());
            fresh
        })
}

fn base_dir(xdg: &XdgDirs, location: &Location) -> PathBuf {
    match location.base {
        Base::Home => xdg.home.clone(),
        Base::ConfigHome => xdg.config_home.clone(),
    }
}

fn root(xdg: &XdgDirs, location: &Location) -> PathBuf {
    base_dir(xdg, location).join(location.root)
}

fn manifest_path(xdg: &XdgDirs, location: &Location) -> PathBuf {
    base_dir(xdg, location)
        .join(location.hosts_root.unwrap_or(location.root))
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

    /// Chromium names the extension by its fixed origin (the current ID first,
    /// then the one Wye 1.0.0 shipped), Firefox by its gecko ID; each manifest
    /// carries only its own family's key.
    #[test]
    fn each_family_allows_only_its_own_extension_id_bext_04() {
        let families = [
            (
                Dialect::Chromium,
                "allowed_origins",
                json!([
                    "chrome-extension://jdcifhpoallkdjnbflfienpboodjfjei/",
                    "chrome-extension://lphepmclmllmbbkjkdhjbdgbjfpmmdnn/",
                ]),
                "allowed_extensions",
            ),
            (
                Dialect::Firefox,
                "allowed_extensions",
                json!(["wye@soldunov.dev"]),
                "allowed_origins",
            ),
        ];
        for (dialect, allowed, ids, absent) in families {
            let manifest = manifest(dialect, Path::new(HOST));
            assert_eq!(manifest["name"], HOST_NAME);
            assert_eq!(manifest["path"], HOST);
            assert_eq!(manifest["type"], "stdio");
            assert_eq!(manifest[allowed], ids, "{dialect:?}");
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

    /// The service refreshes the manifests at every start; one that is
    /// already current is not rewritten, so a start that changes nothing
    /// touches nothing.
    #[test]
    fn refresh_writes_only_what_is_missing_or_different_bext_04() {
        let fixture = Fixture::new();
        fs::create_dir_all(fixture.xdg.config_home.join("chromium")).unwrap();
        fs::create_dir_all(fixture.xdg.home.join(".mozilla")).unwrap();

        let first = refresh(&fixture.xdg, Path::new(HOST)).unwrap();
        assert_eq!(first.len(), 2);
        let chromium = first[0].path.clone();
        let stamp = fs::metadata(&chromium).unwrap().modified().unwrap();

        assert!(refresh(&fixture.xdg, Path::new(HOST)).unwrap().is_empty());
        assert_eq!(
            fs::metadata(&chromium).unwrap().modified().unwrap(),
            stamp,
            "a current manifest is not rewritten"
        );
        assert_eq!(
            install(&fixture.xdg, Path::new(HOST)).unwrap().len(),
            2,
            "install still lists every browser it covers"
        );

        fs::write(&chromium, "{}").unwrap();
        let changed = refresh(&fixture.xdg, Path::new(HOST)).unwrap();
        assert_eq!(changed.len(), 1);
        assert_eq!(changed[0].path, chromium);
        let moved = refresh(&fixture.xdg, Path::new("/new/wye-native-host")).unwrap();
        assert_eq!(moved.len(), 2, "a new host path rewrites both");
    }

    /// A symlinked manifest belongs to whatever made the link: install
    /// reports it left alone, refresh skips it, remove keeps it.
    #[test]
    fn a_symlinked_manifest_is_left_alone_bext_04() {
        let fixture = Fixture::new();
        fs::create_dir_all(fixture.xdg.home.join(".mozilla/native-messaging-hosts")).unwrap();
        let target = fixture.path("managed.json");
        fs::write(&target, "{\"managed\":true}").unwrap();
        let link = fixture
            .xdg
            .home
            .join(".mozilla/native-messaging-hosts/dev.soldunov.wye.json");
        std::os::unix::fs::symlink(&target, &link).unwrap();

        assert!(refresh(&fixture.xdg, Path::new(HOST)).unwrap().is_empty());
        let installed = install(&fixture.xdg, Path::new(HOST)).unwrap();
        assert_eq!(
            installed,
            [Manifest {
                browser: "Firefox",
                path: link.clone(),
                outcome: Outcome::Symlink,
            }]
        );
        let removed = remove(&fixture.xdg).unwrap();
        assert_eq!(removed, installed);

        assert!(is_symlink(&link));
        assert_eq!(fs::read_to_string(&target).unwrap(), "{\"managed\":true}");
    }

    #[test]
    fn install_tells_written_from_current_bext_04() {
        let fixture = Fixture::new();
        fs::create_dir_all(fixture.xdg.config_home.join("chromium")).unwrap();
        let outcomes = |host: &str| -> Vec<Outcome> {
            install(&fixture.xdg, Path::new(host))
                .unwrap()
                .into_iter()
                .map(|manifest| manifest.outcome)
                .collect()
        };
        assert_eq!(outcomes(HOST), [Outcome::Written]);
        assert_eq!(outcomes(HOST), [Outcome::Current]);
        assert_eq!(outcomes("/new/wye-native-host"), [Outcome::Written]);
    }

    /// A manifest Wye 1.0.0 wrote (the old origin only) is rewritten, so an
    /// upgrade adds the store ID.
    #[test]
    fn install_rewrites_a_wye_1_0_0_manifest_bext_04() {
        let fixture = Fixture::new();
        let hosts = fixture
            .xdg
            .config_home
            .join("chromium/NativeMessagingHosts");
        fs::create_dir_all(&hosts).unwrap();
        let path = hosts.join(format!("{HOST_NAME}.json"));
        let old = json!({
            "name": HOST_NAME,
            "description": DESCRIPTION,
            "path": HOST,
            "type": "stdio",
            "allowed_origins": [format!("chrome-extension://{LEGACY_CHROMIUM_EXTENSION_ID}/")],
        });
        fs::write(&path, format!("{old:#}\n")).unwrap();

        let installed = install(&fixture.xdg, Path::new(HOST)).unwrap();
        assert_eq!(installed[0].outcome, Outcome::Written);
        let text = fs::read_to_string(&path).unwrap();
        let written: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(
            written["allowed_origins"],
            json!([
                format!("chrome-extension://{CHROMIUM_EXTENSION_ID}/"),
                format!("chrome-extension://{LEGACY_CHROMIUM_EXTENSION_ID}/"),
            ])
        );
    }

    /// The top-level directories the service watches, one per browser
    /// directory's first component, each listed once.
    #[test]
    fn watched_names_are_the_top_level_browser_directories_bext_04() {
        let fixture = Fixture::new();
        let names = watched_names(&fixture.xdg);
        let config = &fixture.xdg.config_home;
        let home = &fixture.xdg.home;
        for expected in [
            config.join("BraveSoftware"),
            config.join("chromium"),
            config.join("mozilla"),
            home.join(".mozilla"),
            home.join(".zen"),
        ] {
            assert!(names.contains(&expected), "{}", expected.display());
        }
        assert!(!names.contains(&config.join("BraveSoftware/Brave-Browser")));
        let unique: std::collections::BTreeSet<_> = names.iter().collect();
        assert_eq!(unique.len(), names.len(), "{names:?}");
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

    /// Zen is detected by `~/.zen` but reads Firefox's directory (verified
    /// with Zen 1.22.3b): the manifest goes there, not under `~/.zen`.
    #[test]
    fn zen_alone_gets_its_manifest_in_the_mozilla_directory_bext_04() {
        let fixture = Fixture::new();
        fs::create_dir_all(fixture.xdg.home.join(".zen")).unwrap();

        let written = install(&fixture.xdg, Path::new(HOST)).unwrap();

        let path = fixture
            .xdg
            .home
            .join(".mozilla/native-messaging-hosts/dev.soldunov.wye.json");
        assert_eq!(written.len(), 1);
        assert_eq!(written[0].browser, "Zen");
        assert_eq!(written[0].path, path);
        assert!(path.is_file());
        assert!(
            !fixture
                .xdg
                .home
                .join(".zen/native-messaging-hosts")
                .exists()
        );
    }

    #[test]
    fn firefox_and_zen_share_one_manifest_listed_once_bext_04() {
        let fixture = Fixture::new();
        fs::create_dir_all(fixture.xdg.home.join(".mozilla")).unwrap();
        fs::create_dir_all(fixture.xdg.home.join(".zen")).unwrap();

        let written = install(&fixture.xdg, Path::new(HOST)).unwrap();

        assert_eq!(written.len(), 1);
        assert_eq!(written[0].browser, "Firefox");
        assert_eq!(
            refresh(&fixture.xdg, Path::new("/new/host")).unwrap().len(),
            1
        );
    }

    #[test]
    fn remove_deletes_the_shared_manifest_and_the_stale_zen_one_bext_04() {
        let fixture = Fixture::new();
        fs::create_dir_all(fixture.xdg.home.join(".mozilla")).unwrap();
        install(&fixture.xdg, Path::new(HOST)).unwrap();
        let stale = fixture
            .xdg
            .home
            .join(".zen/native-messaging-hosts/dev.soldunov.wye.json");
        fs::create_dir_all(stale.parent().unwrap()).unwrap();
        fs::write(&stale, "{}").unwrap();

        let removed = remove(&fixture.xdg).unwrap();

        let browsers: Vec<_> = removed.iter().map(|manifest| manifest.browser).collect();
        assert_eq!(browsers, ["Firefox", "Zen"]);
        assert_eq!(removed[1].path, stale);
        assert!(!stale.exists());
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
