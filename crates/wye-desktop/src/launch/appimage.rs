//! The session's environment for launched apps when Wye runs from its
//! `AppImage` (LAUNCH-08).
//!
//! The `AppImage` runtime and sharun give Wye's own programs the environment
//! they need: `$APPDIR/bin` first on `PATH`, `$APPDIR/share` on
//! `XDG_DATA_DIRS`, `GIO_*` and `GTK_*` paths into the image, and a private
//! `XDG_CACHE_HOME`. A launched app must not get any of it: `flatpak run`
//! would exec the `bwrap` shim in `$APPDIR/bin` and die. This module works
//! out the changes that give the app the environment the session had before
//! the `AppImage` started, see `docs/investigations/appimage-flatpak-launch.md`.

use std::ffi::{OsStr, OsString};
use std::os::unix::ffi::{OsStrExt as _, OsStringExt as _};
use std::path::{Component, Path, PathBuf};

/// One environment change: `Some` sets the variable, `None` removes it.
pub(super) type EnvChange = (OsString, Option<OsString>);

/// What a rule does to a variable.
enum Edit {
    Set(OsString),
    Remove,
}

impl Edit {
    fn into_value(self) -> Option<OsString> {
        match self {
            Self::Set(value) => Some(value),
            Self::Remove => None,
        }
    }
}

/// Variables only the `AppImage` runtime and sharun set (LAUNCH-08). Listed
/// one by one: a `HOST_*` wildcard would also take the user's own variables.
const MARKERS: [&str; 17] = [
    "APPDIR",
    "APPIMAGE",
    "APPOFFSET",
    "APPIMAGE_ARCH",
    "APPIMAGE_UID",
    "ARGV0",
    "OWD",
    "SHARUN_DIR",
    "URUNTIME",
    "URUNTIME_DIR",
    "HOSTPATH",
    "HOST_HOME",
    "HOST_XDG_CONFIG_HOME",
    "HOST_XDG_DATA_HOME",
    "HOST_XDG_CACHE_HOME",
    "HOST_XDG_STATE_HOME",
    "HOST_KERNEL_VERSION",
];

/// Variables sharun saved as `HOST_*` and may have moved (LAUNCH-08): the
/// variable and the one that holds its original value.
const RESTORES: [(&str, &str); 5] = [
    ("HOME", "HOST_HOME"),
    ("XDG_CONFIG_HOME", "HOST_XDG_CONFIG_HOME"),
    ("XDG_DATA_HOME", "HOST_XDG_DATA_HOME"),
    ("XDG_CACHE_HOME", "HOST_XDG_CACHE_HOME"),
    ("XDG_STATE_HOME", "HOST_XDG_STATE_HOME"),
];

/// The changes for this process: none unless it runs from an `AppImage`
/// (LAUNCH-08). Does the IO for [`bundle_roots`], which decides.
pub(super) fn current_changes() -> Vec<EnvChange> {
    let (Some(appdir), Some(sharun_dir)) =
        (std::env::var_os("APPDIR"), std::env::var_os("SHARUN_DIR"))
    else {
        return Vec::new();
    };
    let Ok(exe) = std::env::current_exe() else {
        return Vec::new();
    };
    let appdir = PathBuf::from(appdir);
    let sharun_dir = PathBuf::from(sharun_dir);
    let appdir_canonical = std::fs::canonicalize(&appdir).unwrap_or_else(|_| appdir.clone());
    let sharun_canonical =
        std::fs::canonicalize(&sharun_dir).unwrap_or_else(|_| sharun_dir.clone());
    match bundle_roots(
        &appdir,
        &appdir_canonical,
        Some(&sharun_dir),
        Some(&sharun_canonical),
        &exe,
    ) {
        Some(roots) => session_changes(std::env::vars_os(), &roots),
        None => Vec::new(),
    }
}

/// The spellings of the `AppImage` directory the sweep matches against, or
/// `None` when this process does not run from an `AppImage` (LAUNCH-08).
///
/// A stray `APPDIR` must change nothing. Without this gate, `APPDIR=/usr`
/// in the environment of a packaged `/usr/bin/wye` would strip `/usr/bin`
/// from every launched app's `PATH`. The gate passes only when all hold:
///
/// - `APPDIR` and `SHARUN_DIR` are both set and both raw values are absolute
///   paths with at least one normal component, so the root and a relative
///   path never count. sharun sets both to its own directory in every mode,
///   including a directly extracted `AppDir`, and Wye's `AppRun` never
///   unsets `SHARUN_DIR`; a stray `APPDIR` alone does not come with it.
/// - Their canonical forms are equal. A stray `APPDIR` rarely matches a
///   `SHARUN_DIR` that sharun wrote.
/// - `exe` lies inside that canonical directory.
///
/// The result holds the raw `APPDIR`, its canonical form and the raw
/// `SHARUN_DIR`, without duplicates: sharun fills some variables from its
/// canonical directory, which differs from the raw one when `TMPDIR` is a
/// symlink.
fn bundle_roots(
    appdir: &Path,
    appdir_canonical: &Path,
    sharun_dir: Option<&Path>,
    sharun_canonical: Option<&Path>,
    exe: &Path,
) -> Option<Vec<PathBuf>> {
    let sharun_dir = sharun_dir?;
    let sharun_canonical = sharun_canonical?;
    let usable = [appdir, appdir_canonical, sharun_dir, sharun_canonical]
        .into_iter()
        .all(is_real_directory_path);
    if !usable || appdir_canonical != sharun_canonical || !exe.starts_with(appdir_canonical) {
        return None;
    }
    let mut roots: Vec<PathBuf> = Vec::new();
    for root in [appdir, appdir_canonical, sharun_dir] {
        if !roots.iter().any(|known| known == root) {
            roots.push(root.to_path_buf());
        }
    }
    Some(roots)
}

/// True for an absolute path with at least one normal component.
fn is_real_directory_path(path: &Path) -> bool {
    path.is_absolute()
        && path
            .components()
            .any(|component| matches!(component, Component::Normal(_)))
}

/// The changes that turn `vars` into the session's own environment
/// (LAUNCH-08), in input order, at most one per variable and none for a
/// variable that stays as it is. `roots` are the spellings of the `AppImage`
/// directory. Per variable, the first rule that applies wins:
///
/// 1. A marker is removed.
/// 2. A moved variable gets its saved original back, when `HOST_HOME` is an
///    absolute path (sharun derives the other originals from it), the
///    original is absolute and it differs from the current value.
/// 3. `GSETTINGS_BACKEND=keyfile` is removed when rule 2 restores `HOME` or
///    `XDG_CONFIG_HOME`. quick-sharun's `05-gsettings-backend.hook` exports
///    it only in portable home/config mode, and a native GTK app launched
///    with it would ignore dconf. Any other value stays.
/// 4. Any other variable loses its colon-separated components inside any of
///    `roots`, or is removed when nothing else is left.
pub(super) fn session_changes(
    vars: impl IntoIterator<Item = (OsString, OsString)>,
    roots: &[PathBuf],
) -> Vec<EnvChange> {
    let vars: Vec<(OsString, OsString)> = vars.into_iter().collect();
    let lookup = |name: &str| {
        vars.iter()
            .find(|(candidate, _)| candidate == name)
            .map(|(_, value)| value.as_os_str())
    };
    let host_home_usable = lookup("HOST_HOME").is_some_and(is_absolute);
    let config_restored = vars.iter().any(|(name, value)| {
        (name == "HOME" || name == "XDG_CONFIG_HOME")
            && restore(name, value, host_home_usable, &lookup).is_some()
    });
    vars.iter()
        .filter_map(|(name, value)| {
            let edit = if MARKERS.iter().any(|marker| name == marker) {
                Some(Edit::Remove)
            } else {
                restore(name, value, host_home_usable, &lookup)
                    .or_else(|| drop_keyfile_backend(name, value, config_restored))
                    .or_else(|| sweep(value, roots))
            };
            edit.map(|edit| (name.clone(), edit.into_value()))
        })
        .collect()
}

/// Rule 3: `GSETTINGS_BACKEND=keyfile` goes when the config home is restored.
fn drop_keyfile_backend(name: &OsStr, value: &OsStr, config_restored: bool) -> Option<Edit> {
    (config_restored && name == "GSETTINGS_BACKEND" && value == "keyfile").then_some(Edit::Remove)
}

/// Rule 2: the original of a moved variable, when it applies.
fn restore<'a>(
    name: &OsStr,
    value: &OsStr,
    host_home_usable: bool,
    lookup: &impl Fn(&str) -> Option<&'a OsStr>,
) -> Option<Edit> {
    if !host_home_usable {
        return None;
    }
    let (_, twin) = RESTORES.iter().find(|(moved, _)| name == *moved)?;
    lookup(twin)
        .filter(|original| is_absolute(original) && *original != value)
        .map(|original| Edit::Set(original.to_owned()))
}

/// Rule 4: `value` without its components inside any of `roots`; `None` when
/// none is inside, [`Edit::Remove`] when nothing is left.
fn sweep(value: &OsStr, roots: &[PathBuf]) -> Option<Edit> {
    let parts: Vec<&[u8]> = value.as_bytes().split(|byte| *byte == b':').collect();
    if !parts.iter().any(|part| inside(part, roots)) {
        return None;
    }
    let kept: Vec<&[u8]> = parts
        .into_iter()
        .filter(|part| !inside(part, roots))
        .collect();
    if kept.iter().all(|part| part.is_empty()) {
        return Some(Edit::Remove);
    }
    Some(Edit::Set(OsString::from_vec(kept.join(&b':'))))
}

/// True for a non-empty absolute path under any of `roots`, compared by whole
/// components: `/tmp/.mount_Wye12` is not inside `/tmp/.mount_Wye1`.
fn inside(component: &[u8], roots: &[PathBuf]) -> bool {
    let path = Path::new(OsStr::from_bytes(component));
    !component.is_empty() && path.is_absolute() && roots.iter().any(|root| path.starts_with(root))
}

fn is_absolute(value: &OsStr) -> bool {
    Path::new(value).is_absolute()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    type Vars = Vec<(OsString, OsString)>;

    fn vars(pairs: &[(&str, &str)]) -> Vars {
        pairs
            .iter()
            .map(|(name, value)| (name.into(), value.into()))
            .collect()
    }

    fn changes(pairs: &[(&str, &str)], appdir: &str) -> BTreeMap<String, Option<String>> {
        changes_in(pairs, &[appdir])
    }

    fn changes_in(pairs: &[(&str, &str)], roots: &[&str]) -> BTreeMap<String, Option<String>> {
        let roots: Vec<PathBuf> = roots.iter().map(PathBuf::from).collect();
        session_changes(vars(pairs), &roots)
            .into_iter()
            .map(|(name, value)| {
                (
                    name.into_string().expect("UTF-8 name"),
                    value.map(|value| value.into_string().expect("UTF-8 value")),
                )
            })
            .collect()
    }

    #[allow(
        clippy::unnecessary_wraps,
        reason = "reads as the `Some` side of a change next to the `None` of a removal"
    )]
    fn set(value: &str) -> Option<String> {
        Some(value.to_owned())
    }

    #[test]
    fn the_investigated_environment_is_restored() {
        let appdir = "/tmp/.mount_Wye1remp";
        let got = changes(
            &[
                ("APPDIR", appdir),
                (
                    "PATH",
                    "/home/deck/.local/bin:/tmp/.mount_Wye1remp/bin:/usr/local/bin:/usr/bin",
                ),
                (
                    "XDG_DATA_DIRS",
                    "/tmp/.mount_Wye1remp/share:/usr/local/share:/usr/share",
                ),
                (
                    "GIO_LAUNCH_DESKTOP",
                    "/tmp/.mount_Wye1remp/bin/gio-launch-desktop",
                ),
                ("GTK_EXE_PREFIX", appdir),
                ("XDG_CACHE_HOME", "/home/deck/.cache/AppImage-Cache"),
                ("HOME", "/home/deck"),
                ("HOST_HOME", "/home/deck"),
                ("HOST_XDG_CACHE_HOME", "/home/deck/.cache"),
                ("HOST_XDG_CONFIG_HOME", "/home/deck/.config"),
                ("URUNTIME", "/home/deck/Downloads/Wye-1.1.0-x86_64.AppImage"),
                ("SHARUN_DIR", appdir),
                ("DISPLAY", ":0"),
                ("DBUS_SESSION_BUS_ADDRESS", "unix:path=/run/user/1000/bus"),
            ],
            appdir,
        );
        let expected: BTreeMap<String, Option<String>> = [
            ("APPDIR", None),
            ("PATH", set("/home/deck/.local/bin:/usr/local/bin:/usr/bin")),
            ("XDG_DATA_DIRS", set("/usr/local/share:/usr/share")),
            ("GIO_LAUNCH_DESKTOP", None),
            ("GTK_EXE_PREFIX", None),
            ("XDG_CACHE_HOME", set("/home/deck/.cache")),
            ("HOST_HOME", None),
            ("HOST_XDG_CACHE_HOME", None),
            ("HOST_XDG_CONFIG_HOME", None),
            ("URUNTIME", None),
            ("SHARUN_DIR", None),
        ]
        .into_iter()
        .map(|(name, value)| (name.to_owned(), value))
        .collect();
        assert_eq!(got, expected);
    }

    #[test]
    fn every_marker_is_removed() {
        let pairs: Vec<(&str, &str)> = MARKERS.iter().map(|name| (*name, "x")).collect();
        let got = changes(&pairs, "/tmp/.mount_Wye1");
        assert_eq!(got.len(), MARKERS.len());
        assert!(got.values().all(Option::is_none));
    }

    #[test]
    fn components_match_whole_path_components() {
        let got = changes(
            &[
                ("PATH", "/tmp/.mount_Wye12/bin:/usr/bin"),
                ("OTHER", "/tmp/.mount_Wye1"),
            ],
            "/tmp/.mount_Wye1",
        );
        assert_eq!(got, BTreeMap::from([("OTHER".to_owned(), None)]));
    }

    #[test]
    fn a_trailing_slash_on_the_appdir_still_matches() {
        let got = changes(
            &[("PATH", "/tmp/.mount_Wye1/bin:/usr/bin")],
            "/tmp/.mount_Wye1/",
        );
        assert_eq!(got, BTreeMap::from([("PATH".to_owned(), set("/usr/bin"))]));
    }

    #[test]
    fn the_sweep_removes_filters_and_leaves_alone() {
        let appdir = "/tmp/.mount_Wye1";
        let got = changes(
            &[
                ("ALL_INSIDE", "/tmp/.mount_Wye1/a:/tmp/.mount_Wye1/b"),
                ("EMPTY_LEFTOVERS", "/tmp/.mount_Wye1/a::"),
                ("MIXED", "/usr/bin:/tmp/.mount_Wye1/a::/opt/bin"),
                ("RELATIVE", "tmp/.mount_Wye1/bin:/tmp/.mount_Wye1/bin"),
                ("UNTOUCHED", "/usr/bin::/opt"),
                ("RELATIVE_ONLY", "tmp/.mount_Wye1/bin"),
                ("EMPTY", ""),
            ],
            appdir,
        );
        let expected = BTreeMap::from([
            ("ALL_INSIDE".to_owned(), None),
            ("EMPTY_LEFTOVERS".to_owned(), None),
            ("MIXED".to_owned(), set("/usr/bin::/opt/bin")),
            ("RELATIVE".to_owned(), set("tmp/.mount_Wye1/bin")),
        ]);
        assert_eq!(got, expected);
    }

    #[test]
    fn changes_follow_the_input_order() {
        let got = session_changes(
            vars(&[("B", "/a/x"), ("A", "/a/y"), ("C", "/z")]),
            &[PathBuf::from("/a")],
        );
        let names: Vec<_> = got.iter().map(|(name, _)| name.to_str()).collect();
        assert_eq!(names, [Some("B"), Some("A")]);
    }

    #[test]
    fn a_missing_variable_stays_missing() {
        let got = changes(
            &[("HOME", "/home/deck"), ("HOST_HOME", "/home/deck")],
            "/tmp/a",
        );
        assert_eq!(got, BTreeMap::from([("HOST_HOME".to_owned(), None)]));
    }

    #[test]
    fn restores_apply_when_the_original_is_usable() {
        let got = changes(
            &[
                ("HOME", "/tmp/appimage-home"),
                ("XDG_CONFIG_HOME", "/tmp/appimage-config"),
                ("XDG_DATA_HOME", "/tmp/appimage-data"),
                ("XDG_STATE_HOME", "/tmp/appimage-state"),
                ("HOST_HOME", "/home/deck"),
                ("HOST_XDG_CONFIG_HOME", "/home/deck/.config"),
                ("HOST_XDG_DATA_HOME", "/home/deck/.local/share"),
                ("HOST_XDG_STATE_HOME", "/home/deck/.local/state"),
            ],
            "/tmp/.mount_Wye1",
        );
        assert_eq!(got["HOME"], set("/home/deck"));
        assert_eq!(got["XDG_CONFIG_HOME"], set("/home/deck/.config"));
        assert_eq!(got["XDG_DATA_HOME"], set("/home/deck/.local/share"));
        assert_eq!(got["XDG_STATE_HOME"], set("/home/deck/.local/state"));
    }

    #[test]
    fn restores_are_skipped_without_a_usable_host_home() {
        for host_home in ["", "relative/home"] {
            let got = changes(
                &[
                    ("HOME", "/tmp/appimage-home"),
                    ("XDG_CACHE_HOME", "/tmp/appimage-cache"),
                    ("HOST_HOME", host_home),
                    ("HOST_XDG_CACHE_HOME", "/home/deck/.cache"),
                ],
                "/tmp/.mount_Wye1",
            );
            assert!(!got.contains_key("HOME"), "{host_home:?}");
            assert!(!got.contains_key("XDG_CACHE_HOME"), "{host_home:?}");
        }
    }

    #[test]
    fn restores_are_skipped_for_a_relative_or_equal_original() {
        let got = changes(
            &[
                ("HOME", "/home/deck"),
                ("XDG_CACHE_HOME", "/tmp/appimage-cache"),
                ("XDG_CONFIG_HOME", "/home/deck/.config"),
                ("HOST_HOME", "/home/deck"),
                ("HOST_XDG_CACHE_HOME", ".cache"),
                ("HOST_XDG_CONFIG_HOME", "/home/deck/.config"),
            ],
            "/tmp/.mount_Wye1",
        );
        let expected = BTreeMap::from([
            ("HOST_HOME".to_owned(), None),
            ("HOST_XDG_CACHE_HOME".to_owned(), None),
            ("HOST_XDG_CONFIG_HOME".to_owned(), None),
        ]);
        assert_eq!(got, expected);
    }

    #[test]
    fn a_restorable_variable_that_is_not_restored_is_still_swept() {
        let got = changes(
            &[("XDG_DATA_HOME", "/tmp/.mount_Wye1/data")],
            "/tmp/.mount_Wye1",
        );
        assert_eq!(got, BTreeMap::from([("XDG_DATA_HOME".to_owned(), None)]));
    }

    #[test]
    fn a_value_that_is_not_utf8_is_filtered_byte_for_byte() {
        let value = b"/usr/\xff/bin:/tmp/.mount_Wye1/bin:/opt/\xfe".to_vec();
        let got = session_changes(
            [(OsString::from("PATH"), OsString::from_vec(value))],
            &[PathBuf::from("/tmp/.mount_Wye1")],
        );
        let expected = OsString::from_vec(b"/usr/\xff/bin:/opt/\xfe".to_vec());
        assert_eq!(got, [(OsString::from("PATH"), Some(expected))]);
    }

    fn gate(
        appdir: &str,
        appdir_canonical: &str,
        sharun: Option<(&str, &str)>,
        exe: &str,
    ) -> Option<Vec<PathBuf>> {
        bundle_roots(
            Path::new(appdir),
            Path::new(appdir_canonical),
            sharun.map(|(raw, _)| Path::new(raw)),
            sharun.map(|(_, canonical)| Path::new(canonical)),
            Path::new(exe),
        )
    }

    #[test]
    fn a_stray_appdir_above_a_packaged_binary_changes_nothing() {
        assert_eq!(gate("/usr", "/usr", None, "/usr/bin/wye"), None);
    }

    #[test]
    fn a_mismatched_or_relative_appdir_or_sharun_dir_changes_nothing() {
        // (label, appdir, appdir canonical, sharun (raw, canonical), exe)
        type Case = (
            &'static str,
            &'static str,
            &'static str,
            (&'static str, &'static str),
            &'static str,
        );
        let cases: [Case; 4] = [
            (
                "appdir without a matching sharun dir (/usr)",
                "/usr",
                "/usr",
                ("/tmp/.mount_X", "/tmp/.mount_X"),
                "/usr/bin/wye",
            ),
            (
                "appdir without a matching sharun dir (mounts)",
                "/tmp/.mount_X",
                "/tmp/.mount_X",
                ("/tmp/.mount_Y", "/tmp/.mount_Y"),
                "/tmp/.mount_X/bin/wye",
            ),
            (
                "relative appdir",
                "mount_X",
                "/tmp/mount_X",
                ("/tmp/mount_X", "/tmp/mount_X"),
                "/tmp/mount_X/bin/wye",
            ),
            (
                "relative sharun dir",
                "/tmp/mount_X",
                "/tmp/mount_X",
                ("mount_X", "/tmp/mount_X"),
                "/tmp/mount_X/bin/wye",
            ),
        ];
        for (label, appdir, canonical, sharun, exe) in cases {
            assert_eq!(gate(appdir, canonical, Some(sharun), exe), None, "{label}");
        }
    }

    #[test]
    fn the_root_as_appdir_changes_nothing() {
        let got = gate("/", "/", Some(("/", "/")), "/usr/bin/wye");
        assert_eq!(got, None);
    }

    #[test]
    fn an_exe_outside_the_appdir_changes_nothing() {
        let sharun = Some(("/tmp/.mount_Wye1", "/tmp/.mount_Wye1"));
        for exe in ["/usr/bin/wye", "/tmp/.mount_Wye12/bin/wye"] {
            assert_eq!(
                gate("/tmp/.mount_Wye1", "/tmp/.mount_Wye1", sharun, exe),
                None
            );
        }
    }

    #[test]
    fn an_appdir_with_its_sharun_dir_and_exe_inside_is_a_bundle() {
        let got = gate(
            "/tmp/.mount_X",
            "/tmp/.mount_X",
            Some(("/tmp/.mount_X", "/tmp/.mount_X")),
            "/tmp/.mount_X/bin/wye",
        );
        assert_eq!(got, Some(vec![PathBuf::from("/tmp/.mount_X")]));
    }

    #[test]
    fn every_spelling_of_the_appdir_and_sharun_dir_is_a_root() {
        // (label, raw sharun dir, expected roots)
        let cases: [(&str, &str, &[&str]); 2] = [
            (
                "same sharun spelling",
                "/tmp/link/.mount_X",
                &["/tmp/link/.mount_X", "/real/tmp/.mount_X"],
            ),
            (
                "differing sharun spelling",
                "/tmp/other/.mount_X",
                &[
                    "/tmp/link/.mount_X",
                    "/real/tmp/.mount_X",
                    "/tmp/other/.mount_X",
                ],
            ),
        ];
        for (label, sharun_raw, roots) in cases {
            let got = gate(
                "/tmp/link/.mount_X",
                "/real/tmp/.mount_X",
                Some((sharun_raw, "/real/tmp/.mount_X")),
                "/real/tmp/.mount_X/bin/wye",
            );
            let expected = roots.iter().map(PathBuf::from).collect::<Vec<_>>();
            assert_eq!(got, Some(expected), "{label}");
        }
    }

    #[test]
    fn the_sweep_matches_every_root() {
        let got = changes_in(
            &[
                (
                    "PATH",
                    "/real/tmp/.mount_X/bin:/tmp/link/.mount_X/bin:/usr/bin",
                ),
                (
                    "GIO_LAUNCH_DESKTOP",
                    "/real/tmp/.mount_X/bin/gio-launch-desktop",
                ),
                ("GTK_EXE_PREFIX", "/tmp/link/.mount_X"),
            ],
            &["/tmp/link/.mount_X", "/real/tmp/.mount_X"],
        );
        let expected = BTreeMap::from([
            ("PATH".to_owned(), set("/usr/bin")),
            ("GIO_LAUNCH_DESKTOP".to_owned(), None),
            ("GTK_EXE_PREFIX".to_owned(), None),
        ]);
        assert_eq!(got, expected);
    }

    #[test]
    fn the_sweep_with_only_the_raw_root_misses_the_canonical_spelling() {
        let got = changes_in(
            &[(
                "GIO_LAUNCH_DESKTOP",
                "/real/tmp/.mount_X/bin/gio-launch-desktop",
            )],
            &["/tmp/link/.mount_X"],
        );
        assert!(got.is_empty());
    }

    #[test]
    fn only_the_keyfile_backend_goes_when_home_is_restored() {
        // (label, backend, whether the sweep drops it)
        let cases = [("keyfile", "keyfile", true), ("another", "dconf", false)];
        for (label, backend, dropped) in cases {
            let got = changes(
                &[
                    ("HOME", "/x/Wye.AppImage.home"),
                    ("HOST_HOME", "/home/deck"),
                    ("GSETTINGS_BACKEND", backend),
                ],
                "/tmp/.mount_Wye1",
            );
            assert_eq!(got["HOME"], set("/home/deck"), "{label}: HOME");
            if dropped {
                assert_eq!(got["GSETTINGS_BACKEND"], None, "{label}: backend");
            } else {
                assert!(!got.contains_key("GSETTINGS_BACKEND"), "{label}: backend");
            }
        }
    }

    #[test]
    fn the_keyfile_backend_goes_when_the_config_home_is_restored() {
        let got = changes(
            &[
                ("HOME", "/home/deck"),
                ("XDG_CONFIG_HOME", "/x/Wye.AppImage.config"),
                ("HOST_HOME", "/home/deck"),
                ("HOST_XDG_CONFIG_HOME", "/home/deck/.config"),
                ("GSETTINGS_BACKEND", "keyfile"),
            ],
            "/tmp/.mount_Wye1",
        );
        assert_eq!(got["XDG_CONFIG_HOME"], set("/home/deck/.config"));
        assert_eq!(got["GSETTINGS_BACKEND"], None);
    }

    #[test]
    fn the_keyfile_backend_stays_when_nothing_is_restored() {
        let got = changes(
            &[
                ("HOME", "/home/deck"),
                ("HOST_HOME", "/home/deck"),
                ("GSETTINGS_BACKEND", "keyfile"),
            ],
            "/tmp/.mount_Wye1",
        );
        assert!(!got.contains_key("GSETTINGS_BACKEND"));
    }
}
