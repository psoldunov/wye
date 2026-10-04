//! What to watch and what a changed path means (SET-06 reload, DEF-03,
//! DISC-02, BEXT-04). Pure apart from checking which directories exist.
//!
//! Every directory is watched on its own (not recursively): editors and
//! home-manager replace files by renaming, which the directory sees, and
//! Nix profiles swap symlinks, which only the symlink's parent sees.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::api::link::Environment;

/// Files that name the default browser (DEF-03).
const MIMEAPPS: &str = "mimeapps.list";
const KDEGLOBALS: &str = "kdeglobals";
/// Files that list a browser's profiles (DISC-02).
const PROFILE_FILES: [&str; 2] = ["Local State", "profiles.ini"];

/// What a change means.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Kind {
    /// `config.toml` changed: reload (SET-06).
    Config,
    /// A `mimeapps.list` or `kdeglobals` changed (DEF-03).
    Registration,
    /// Apps or profiles changed: rescan (DISC-02).
    Inventory,
    /// A browser's top-level directory appeared or moved: write the
    /// extension's host manifests (BEXT-04).
    ExtensionHosts,
    /// A watched directory or a profile symlink appeared or moved: watch
    /// again.
    Rearm,
}

/// The directories to watch for one environment and how to read events.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Plan {
    /// Existing directories to watch.
    pub dirs: Vec<PathBuf>,
    config_dir: PathBuf,
    config_file: PathBuf,
    config_home: PathBuf,
    applications: Vec<PathBuf>,
    /// Symlinks on the way to an applications directory (Nix profiles).
    links: Vec<PathBuf>,
    browser_dirs: Vec<PathBuf>,
    /// The browsers' top-level directories, in the home and configuration
    /// directories (BEXT-04).
    extension_names: Vec<PathBuf>,
}

impl Plan {
    /// The plan for `environment`, with the config directories of the
    /// installed browsers (`browser_dirs`).
    pub fn new(environment: &Environment, browser_dirs: &[PathBuf]) -> Self {
        let config_file = environment.config.clone();
        let config_dir = config_file
            .parent()
            .map_or_else(PathBuf::new, Path::to_path_buf);
        let config_home = environment.xdg.config_home.clone();
        let applications = environment.xdg.applications_dirs();
        let links: Vec<PathBuf> = applications
            .iter()
            .flat_map(|dir| symlinked_ancestors(dir))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let extension_names = wye_desktop::native_messaging::watched_names(&environment.xdg);
        // The home directory is watched for the browsers' directories that
        // appear in it; every other change there classifies to nothing.
        let wanted = [
            config_dir.clone(),
            config_home.clone(),
            environment.xdg.home.clone(),
        ]
        .into_iter()
        .chain(applications.iter().cloned())
        .chain(
            links
                .iter()
                .filter_map(|link| link.parent().map(Path::to_path_buf)),
        )
        .chain(browser_dirs.iter().cloned());
        let mut seen = BTreeSet::new();
        let dirs = wanted
            .filter(|dir| dir.is_dir() && seen.insert(dir.clone()))
            .collect();
        Self {
            dirs,
            config_dir,
            config_file,
            config_home,
            applications,
            links,
            browser_dirs: browser_dirs.to_vec(),
            extension_names,
        }
    }

    /// The paths to check for changes when no watcher can be started:
    /// every file Wye reads that is named up front, the application
    /// directories (their modification time changes when an entry is added,
    /// removed or replaced) and the browsers' top-level directories (seen
    /// when they appear). [`Plan::classify`] reads each of them.
    pub fn polled(&self) -> Vec<PathBuf> {
        let registration = [MIMEAPPS, KDEGLOBALS].map(|name| self.config_home.join(name));
        let profiles = self
            .browser_dirs
            .iter()
            .flat_map(|dir| PROFILE_FILES.map(|name| dir.join(name)));
        std::iter::once(self.config_file.clone())
            .chain(registration)
            .chain(self.applications.iter().cloned())
            .chain(profiles)
            .chain(self.extension_names.iter().cloned())
            .collect()
    }

    /// What a change at `path` means; empty when nothing Wye reads.
    pub fn classify(&self, path: &Path) -> BTreeSet<Kind> {
        let mut kinds = BTreeSet::new();
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let parent = path.parent().unwrap_or(Path::new(""));
        if path == self.config_file {
            kinds.insert(Kind::Config);
        }
        if path == self.config_dir {
            kinds.extend([Kind::Config, Kind::Rearm]);
        }
        let in_config_dirs =
            parent == self.config_home || self.applications.iter().any(|dir| dir == parent);
        if in_config_dirs && is_registration_file(&name, parent == self.config_home) {
            kinds.insert(Kind::Registration);
        }
        if self.applications.iter().any(|dir| dir == parent) && name.ends_with(".desktop") {
            kinds.insert(Kind::Inventory);
        }
        if self.applications.iter().any(|dir| dir == path)
            || self.links.iter().any(|link| link == path)
        {
            kinds.extend([Kind::Inventory, Kind::Rearm]);
        }
        if self.browser_dirs.iter().any(|dir| dir == parent)
            && PROFILE_FILES.contains(&name.as_str())
        {
            kinds.insert(Kind::Inventory);
        }
        if self.extension_names.iter().any(|dir| dir == path) {
            kinds.insert(Kind::ExtensionHosts);
        }
        kinds
    }
}

/// `mimeapps.list`, `<desktop>-mimeapps.list`, and `kdeglobals` in the
/// configuration home.
fn is_registration_file(name: &str, in_config_home: bool) -> bool {
    name == MIMEAPPS
        || name.ends_with(&format!("-{MIMEAPPS}"))
        || (in_config_home && name == KDEGLOBALS)
}

/// The symlinks among `dir` and its ancestors, such as
/// `/run/current-system` or `~/.nix-profile`.
fn symlinked_ancestors(dir: &Path) -> Vec<PathBuf> {
    dir.ancestors()
        .filter(|ancestor| {
            std::fs::symlink_metadata(ancestor).is_ok_and(|meta| meta.file_type().is_symlink())
        })
        .map(Path::to_path_buf)
        .collect()
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use super::*;

    fn environment(root: &Path) -> Environment {
        let vars = [
            ("HOME", root.join("home")),
            ("XDG_CONFIG_HOME", root.join("config")),
            ("XDG_DATA_HOME", root.join("data")),
            ("XDG_DATA_DIRS", root.join("profile/share")),
            ("XDG_STATE_HOME", root.join("state")),
        ];
        Environment::from_lookup(|name| {
            vars.iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| OsString::from(value))
        })
        .expect("home")
    }

    fn kinds(plan: &Plan, path: &Path) -> Vec<Kind> {
        plan.classify(path).into_iter().collect()
    }

    #[test]
    fn each_file_means_its_own_thing() {
        let dir = tempfile::tempdir().expect("temp dir");
        let root = dir.path();
        for sub in ["config/wye", "data/applications", "browser"] {
            std::fs::create_dir_all(root.join(sub)).expect("dir");
        }
        let plan = Plan::new(&environment(root), &[root.join("browser")]);
        assert!(plan.dirs.contains(&root.join("config/wye")));
        assert!(plan.dirs.contains(&root.join("data/applications")));

        assert_eq!(
            kinds(&plan, &root.join("config/wye/config.toml")),
            [Kind::Config]
        );
        assert_eq!(kinds(&plan, &root.join("config/wye/other.toml")), []);
        assert_eq!(
            kinds(&plan, &root.join("config/mimeapps.list")),
            [Kind::Registration]
        );
        assert_eq!(
            kinds(&plan, &root.join("config/kde-mimeapps.list")),
            [Kind::Registration]
        );
        assert_eq!(
            kinds(&plan, &root.join("config/kdeglobals")),
            [Kind::Registration]
        );
        assert_eq!(kinds(&plan, &root.join("config/other")), []);
        assert_eq!(
            kinds(&plan, &root.join("data/applications/a.desktop")),
            [Kind::Inventory]
        );
        assert_eq!(
            kinds(&plan, &root.join("browser/Local State")),
            [Kind::Inventory]
        );
        assert_eq!(
            kinds(&plan, &root.join("config/wye")),
            [Kind::Config, Kind::Rearm]
        );
    }

    /// A browser's top-level directory appearing in the configuration or
    /// home directory means its host manifest can be written; nothing else
    /// in the home directory means anything (BEXT-04).
    #[test]
    fn a_new_browser_directory_means_extension_hosts_bext_04() {
        let dir = tempfile::tempdir().expect("temp dir");
        let root = dir.path();
        for sub in ["config", "home"] {
            std::fs::create_dir_all(root.join(sub)).expect("dir");
        }
        let plan = Plan::new(&environment(root), &[]);
        assert!(plan.dirs.contains(&root.join("home")), "{:?}", plan.dirs);
        assert!(plan.dirs.contains(&root.join("config")), "{:?}", plan.dirs);

        for browser in [
            "config/BraveSoftware",
            "config/chromium",
            "config/google-chrome",
            "home/.mozilla",
            "home/.zen",
            "home/.librewolf",
        ] {
            assert_eq!(
                kinds(&plan, &root.join(browser)),
                [Kind::ExtensionHosts],
                "{browser}"
            );
        }
        for other in [
            "home/.bash_history",
            "home/.zsh_history.new",
            "home/Downloads",
            "home/.config",
            "config/BraveSoftware/Brave-Browser",
            "home/.mozilla/firefox",
        ] {
            assert_eq!(kinds(&plan, &root.join(other)), [], "{other}");
        }
        assert_eq!(
            kinds(&plan, &root.join("config/mimeapps.list")),
            [Kind::Registration]
        );
    }

    #[test]
    fn every_polled_path_means_something() {
        let dir = tempfile::tempdir().expect("temp dir");
        let root = dir.path();
        let plan = Plan::new(&environment(root), &[root.join("browser")]);
        let polled = plan.polled();
        assert!(polled.contains(&root.join("config/wye/config.toml")));
        assert!(polled.contains(&root.join("browser/profiles.ini")));
        assert!(polled.contains(&root.join("home/.mozilla")));
        for path in polled {
            assert!(!plan.classify(&path).is_empty(), "{}", path.display());
        }
    }

    #[test]
    fn a_profile_symlink_swap_is_seen_in_its_parent() {
        let dir = tempfile::tempdir().expect("temp dir");
        let root = dir.path();
        let generation = root.join("store/gen-1/share/applications");
        std::fs::create_dir_all(&generation).expect("dir");
        std::os::unix::fs::symlink(root.join("store/gen-1"), root.join("profile")).expect("linked");
        let plan = Plan::new(&environment(root), &[]);
        assert!(plan.dirs.contains(&root.to_path_buf()), "{:?}", plan.dirs);
        assert_eq!(
            kinds(&plan, &root.join("profile")),
            [Kind::Inventory, Kind::Rearm]
        );
    }
}
