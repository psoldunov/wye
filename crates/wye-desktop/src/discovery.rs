//! Finding installed apps and browsers (DISC-01, DISC-03 to DISC-07).

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

use wye_core::{Availability, CustomApp, DesktopId, Target};

use crate::entry::{DesktopAction, DesktopEntry};
use crate::exec::ExecTemplate;
use crate::family::{self, BrowserFamily, Packaging};
use crate::loop_guard;
use crate::profiles::{self, Profile};
use crate::xdg::{self, XdgDirs};

/// How deep `applications` subdirectories are followed; guards against
/// symlink loops.
const MAX_DEPTH: usize = 8;

/// Hints that a desktop action opens a private window.
const PRIVATE_HINTS: &[&str] = &["private", "incognito", "inprivate"];

/// How Wye opens a private window for an app (DISC-05).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrivateMode {
    /// Run the desktop action with this ID.
    Action(String),
    /// Add this flag to the entry's `Exec`.
    Flag(&'static str),
}

/// A visible, launchable app.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledApp {
    pub entry: DesktopEntry,
    pub family: BrowserFamily,
    /// `MimeType` lists `x-scheme-handler/http` or `https`, and `Exec` does
    /// not forward links (see `forwards_links`).
    pub handles_web: bool,
    /// `Exec` runs Wye or a generic opener such as `xdg-open`, which would
    /// send links straight back to Wye (DEF-06). Such an app is never a web
    /// handler, never available as a target and never launched.
    pub forwards_links: bool,
    /// Only web handlers get a private mode.
    pub private: Option<PrivateMode>,
    /// Only Chromium- and Firefox-family web handlers have profiles.
    pub profiles: Vec<Profile>,
    pub packaging: Packaging,
}

impl InstalledApp {
    /// Classifies an entry and reads its profiles. Problems reading a
    /// profile store are appended to `warnings`.
    #[must_use]
    pub fn from_entry(entry: DesktopEntry, xdg: &XdgDirs, warnings: &mut Vec<String>) -> Self {
        let exec = entry
            .exec
            .as_deref()
            .and_then(|line| ExecTemplate::parse(line).ok());
        let forwards_links = exec
            .as_ref()
            .is_some_and(|exec| loop_guard::runs_opener(&exec.words()));
        let handles_web = entry.handles_web() && !forwards_links;
        // A non-browser whose Exec happens to run a browser (a Chrome web-app
        // shortcut, say) is not treated as a browser.
        let family = if handles_web {
            family::detect_family(&entry.id, exec.as_ref())
        } else {
            BrowserFamily::Other
        };
        let private = handles_web
            .then(|| private_mode(&entry.actions, family))
            .flatten();
        let profiles = if handles_web {
            read_profiles(&entry.id, family, xdg, warnings)
        } else {
            Vec::new()
        };
        Self {
            packaging: family::detect_packaging(&entry, exec.as_ref()),
            entry,
            family,
            handles_web,
            forwards_links,
            private,
            profiles,
        }
    }

    #[must_use]
    pub fn id(&self) -> &DesktopId {
        &self.entry.id
    }

    #[must_use]
    pub fn profile(&self, id: &str) -> Option<&Profile> {
        self.profiles.iter().find(|profile| profile.id == id)
    }
}

/// Every visible app, keyed by desktop ID.
#[derive(Debug, Clone, Default)]
pub struct Inventory {
    apps: BTreeMap<DesktopId, InstalledApp>,
    search_path: Vec<PathBuf>,
    warnings: Vec<String>,
}

impl Inventory {
    /// Scans every `applications` directory, leaving out `exclude` (Wye's own
    /// entry, DISC-01).
    #[must_use]
    pub fn scan(xdg: &XdgDirs, exclude: &DesktopId) -> Self {
        let mut warnings = Vec::new();
        let mut apps = BTreeMap::new();
        for (id, path) in entry_files(xdg) {
            if &id == exclude {
                continue;
            }
            match DesktopEntry::load(id, &path) {
                Ok(entry) if is_visible(&entry, xdg) => {
                    let app = InstalledApp::from_entry(entry, xdg, &mut warnings);
                    apps.insert(app.id().clone(), app);
                }
                Ok(_) => {}
                Err(error) => warnings.push(error.to_string()),
            }
        }
        Self {
            apps,
            search_path: xdg.search_path.clone(),
            warnings,
        }
    }

    /// Builds an inventory from already classified apps.
    #[must_use]
    pub fn from_apps(apps: Vec<InstalledApp>, search_path: Vec<PathBuf>) -> Self {
        Self {
            apps: apps
                .into_iter()
                .map(|app| (app.id().clone(), app))
                .collect(),
            search_path,
            warnings: Vec::new(),
        }
    }

    #[must_use]
    pub fn get(&self, id: &DesktopId) -> Option<&InstalledApp> {
        self.apps.get(id)
    }

    /// Every visible app, ordered by desktop ID.
    pub fn apps(&self) -> impl Iterator<Item = &InstalledApp> {
        self.apps.values()
    }

    /// Apps that handle web links, sorted by name (TGT-05).
    #[must_use]
    pub fn web_handlers(&self) -> Vec<&InstalledApp> {
        let mut handlers: Vec<&InstalledApp> =
            self.apps.values().filter(|app| app.handles_web).collect();
        handlers.sort_by_cached_key(|app| (app.entry.name.to_lowercase(), app.id().clone()));
        handlers
    }

    /// Entries and profile stores that could not be read during the scan.
    #[must_use]
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }

    #[must_use]
    pub fn search_path(&self) -> &[PathBuf] {
        &self.search_path
    }
}

impl Availability for Inventory {
    fn is_available(&self, target: &Target) -> bool {
        match target {
            Target::Picker | Target::Default => true,
            Target::App(id) | Target::Custom(CustomApp::Desktop(id)) => {
                self.get(id).is_some_and(|app| !app.forwards_links)
            }
            Target::Private(id) => self.get(id).is_some_and(|app| app.private.is_some()),
            Target::Profile { app, id } => {
                self.get(app).is_some_and(|app| app.profile(id).is_some())
            }
            Target::Custom(CustomApp::Executable(program)) => {
                let argv = [program.clone()];
                xdg::find_program(program, &self.search_path).is_some()
                    && !loop_guard::runs_opener(&argv)
                    && !loop_guard::runs_current_exe(&argv, &self.search_path)
            }
        }
    }
}

/// Resolves one desktop ID without scanning everything: the first
/// `applications` directory holding it wins. Returns `None` when it is
/// missing, unreadable or `Hidden` (deleted). Other visibility rules are
/// left to the caller.
#[must_use]
pub fn find_entry(xdg: &XdgDirs, id: &DesktopId) -> Option<DesktopEntry> {
    let path = xdg
        .applications_dirs()
        .iter()
        .find_map(|dir| resolve_id(dir, id.as_str(), 0))?;
    DesktopEntry::load(id.clone(), &path)
        .ok()
        .filter(|entry| !entry.hidden)
}

/// Finds the file for `rest` under `dir`, where each `-` in `rest` may stand
/// for a subdirectory separator. Empty, `.` and `..` segments are skipped,
/// so an ID never resolves outside `dir`.
fn resolve_id(dir: &Path, rest: &str, depth: usize) -> Option<PathBuf> {
    if is_plain_segment(rest) {
        let direct = dir.join(rest);
        if direct.is_file() {
            return Some(direct);
        }
    }
    if depth >= MAX_DEPTH {
        return None;
    }
    rest.match_indices('-').find_map(|(index, _)| {
        let segment = rest.get(..index)?;
        if !is_plain_segment(segment) {
            return None;
        }
        let subdir = dir.join(segment);
        if subdir.is_dir() {
            resolve_id(&subdir, rest.get(index + 1..)?, depth + 1)
        } else {
            None
        }
    })
}

/// A single path component that stays inside its directory.
fn is_plain_segment(segment: &str) -> bool {
    !segment.is_empty() && segment != "." && segment != ".." && !segment.contains('/')
}

/// Every desktop file in precedence order, first file per ID only. A
/// `Hidden` entry still claims its ID, which hides lower-precedence copies.
fn entry_files(xdg: &XdgDirs) -> Vec<(DesktopId, PathBuf)> {
    let mut seen = HashSet::new();
    let mut files = Vec::new();
    for dir in xdg.applications_dirs() {
        let mut found = Vec::new();
        walk(&dir, "", 0, &mut found);
        for (id, path) in found {
            if seen.insert(id.clone()) {
                files.push((id, path));
            }
        }
    }
    files
}

/// Collects `.desktop` files; the ID is the path relative to the
/// `applications` directory with `/` replaced by `-`.
fn walk(dir: &Path, prefix: &str, depth: usize, found: &mut Vec<(DesktopId, PathBuf)>) {
    let Ok(read) = std::fs::read_dir(dir) else {
        return;
    };
    let mut children: Vec<(String, PathBuf)> = read
        .filter_map(Result::ok)
        .filter_map(|child| Some((child.file_name().into_string().ok()?, child.path())))
        .collect();
    children.sort();
    for (name, path) in children {
        let Ok(meta) = std::fs::metadata(&path) else {
            continue;
        };
        if meta.is_dir() && depth < MAX_DEPTH {
            walk(&path, &format!("{prefix}{name}-"), depth + 1, found);
        } else if meta.is_file() && name.ends_with(DesktopId::SUFFIX) {
            if let Ok(id) = DesktopId::new(format!("{prefix}{name}")) {
                found.push((id, path));
            }
        }
    }
}

/// DISC-01: `Type=Application`, not `Hidden` or `NoDisplay`, `TryExec`
/// resolves, and `OnlyShowIn`/`NotShowIn` allow the current desktop.
fn is_visible(entry: &DesktopEntry, xdg: &XdgDirs) -> bool {
    !entry.hidden
        && !entry.no_display
        && entry.entry_type.as_deref() == Some("Application")
        && entry
            .try_exec
            .as_deref()
            .is_none_or(|program| xdg.find_program(program).is_some())
        && entry.shown_in(&xdg.current_desktops)
}

/// DISC-05: a private-window desktop action, else the family flag.
fn private_mode(actions: &[DesktopAction], family: BrowserFamily) -> Option<PrivateMode> {
    actions
        .iter()
        .find(|action| {
            let id = action.id.to_ascii_lowercase();
            let exec = action.exec.as_deref().unwrap_or_default();
            PRIVATE_HINTS.iter().any(|hint| id.contains(hint))
                || [
                    "--private-window",
                    "-private-window",
                    "--incognito",
                    "--inprivate",
                ]
                .iter()
                .any(|flag| exec.split_whitespace().any(|word| word == *flag))
        })
        .map(|action| PrivateMode::Action(action.id.clone()))
        .or_else(|| family.private_flag().map(PrivateMode::Flag))
}

/// Profiles from the first existing config directory (DISC-06, DISC-07).
fn read_profiles(
    id: &DesktopId,
    family: BrowserFamily,
    xdg: &XdgDirs,
    warnings: &mut Vec<String>,
) -> Vec<Profile> {
    let read = match family {
        BrowserFamily::Chromium => profiles::read_chromium,
        BrowserFamily::Firefox => profiles::read_firefox,
        BrowserFamily::Other => return Vec::new(),
    };
    for dir in family::config_dirs(id, xdg) {
        match read(&dir) {
            Ok(found) if !found.is_empty() => return found,
            Ok(_) => {}
            Err(error) => warnings.push(error.to_string()),
        }
    }
    Vec::new()
}

#[cfg(test)]
mod tests;
