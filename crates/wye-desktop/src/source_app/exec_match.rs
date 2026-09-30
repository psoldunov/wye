//! Matching a running process to a desktop entry by the program it runs
//! (13-linux-platform.md, "Source-app detection details", step 3, last
//! fallback): the process's executable and `argv[0]` are compared with the
//! program in every entry's `Exec`.
//!
//! Programs are compared by resolved, canonical path first, so a symlink in
//! `/usr/bin` and the binary it points at match. When no path matches, the
//! file name alone is used, which covers wrappers that re-execute a program
//! from another place. A match must be unique: two entries that run the same
//! program (two profiles of one browser, say) do not name a source app, unless
//! one of them is called after the program.

use std::path::{Path, PathBuf};

use wye_core::DesktopId;

use crate::discovery::Inventory;
use crate::entry::DesktopEntry;
use crate::exec::ExecTemplate;
use crate::xdg;

/// Programs that start other programs: matching on them would pin every
/// script, launcher and Flatpak or Snap app to one entry. (Sandboxed apps
/// are found through `.flatpak-info` and the Snap cgroup instead.)
const GENERIC: &[&str] = &[
    "env",
    "sh",
    "bash",
    "dash",
    "zsh",
    "fish",
    "flatpak",
    "snap",
    "gtk-launch",
    "xdg-open",
    "gio",
    "systemd-run",
    "python",
    "python3",
    "perl",
    "ruby",
    "node",
    "java",
];

/// Interpreters whose `exe` says nothing about the script they run; the
/// script is `argv[1]`.
const INTERPRETERS: &[&str] = &[
    "sh", "bash", "dash", "zsh", "python", "python3", "perl", "ruby", "node",
];

/// What `/proc/<pid>` says about the program a process runs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProcessProgram {
    /// The target of `/proc/<pid>/exe`.
    pub exe: Option<PathBuf>,
    /// `/proc/<pid>/cmdline`, split at NUL.
    pub argv: Vec<String>,
}

/// An index from programs to the desktop entries that run them.
#[derive(Debug, Clone, Default)]
pub struct ExecMatcher {
    by_path: Vec<(PathBuf, DesktopId)>,
    by_name: Vec<(String, DesktopId)>,
}

impl ExecMatcher {
    /// Indexes `entries`; bare program names are looked up on `search_path`.
    #[must_use]
    pub fn new<'a>(
        entries: impl IntoIterator<Item = &'a DesktopEntry>,
        search_path: &[PathBuf],
    ) -> Self {
        let mut matcher = Self::default();
        for entry in entries {
            let Some(program) = entry_program(entry) else {
                continue;
            };
            let name = file_name(&program);
            if GENERIC.contains(&name.as_str()) {
                continue;
            }
            if let Some(resolved) = resolve(&program, search_path) {
                matcher.by_path.push((resolved, entry.id.clone()));
            }
            matcher.by_name.push((name, entry.id.clone()));
        }
        matcher
    }

    /// Indexes every app of `inventory`.
    #[must_use]
    pub fn from_inventory(inventory: &Inventory) -> Self {
        Self::new(
            inventory.apps().map(|app| &app.entry),
            inventory.search_path(),
        )
    }

    /// The desktop entry that runs the process's program, when exactly one
    /// does (see the module documentation).
    #[must_use]
    pub fn find(&self, process: &ProcessProgram) -> Option<DesktopId> {
        let exe_name = process.exe.as_deref().map(file_name_of);
        let interpreted = exe_name.as_deref().is_some_and(is_interpreter);
        let programs: Vec<String> = if interpreted {
            process.argv.get(1).cloned().into_iter().collect()
        } else {
            process
                .exe
                .iter()
                .map(|exe| exe.to_string_lossy().into_owned())
                .chain(process.argv.first().cloned())
                .collect()
        };
        let by_path: Vec<PathBuf> = programs
            .iter()
            .filter(|program| program.contains('/'))
            .map(|program| canonical(Path::new(program)))
            .collect();
        for path in &by_path {
            let ids = self.ids_for_path(path);
            if !ids.is_empty() {
                return pick(&ids, &file_name_of(path));
            }
        }
        for program in &programs {
            let name = file_name(program);
            let ids = self.ids_for_name(&name);
            if !ids.is_empty() {
                return pick(&ids, &name);
            }
        }
        None
    }

    fn ids_for_path(&self, path: &Path) -> Vec<&DesktopId> {
        self.by_path
            .iter()
            .filter(|(known, _)| known == path)
            .map(|(_, id)| id)
            .collect()
    }

    fn ids_for_name(&self, name: &str) -> Vec<&DesktopId> {
        self.by_name
            .iter()
            .filter(|(known, _)| known == name)
            .map(|(_, id)| id)
            .collect()
    }
}

/// `python3`, `python3.12`, `bash` … : an interpreter, with or without a
/// version suffix.
fn is_interpreter(name: &str) -> bool {
    INTERPRETERS.iter().any(|interpreter| {
        name.strip_prefix(interpreter)
            .is_some_and(|rest| rest.chars().all(|c| c.is_ascii_digit() || c == '.'))
    })
}

/// One candidate, or the one named after the program; several others are
/// ambiguous.
fn pick(ids: &[&DesktopId], program_name: &str) -> Option<DesktopId> {
    let mut unique: Vec<&DesktopId> = ids.to_vec();
    unique.sort();
    unique.dedup();
    match unique.as_slice() {
        [only] => Some((*only).clone()),
        several => several
            .iter()
            .find(|id| id.app_id().eq_ignore_ascii_case(program_name))
            .map(|id| (**id).clone()),
    }
}

/// The program an entry's `Exec` runs: the first word, past `env` and its
/// `VAR=value` arguments.
fn entry_program(entry: &DesktopEntry) -> Option<String> {
    let words = ExecTemplate::parse(entry.exec.as_deref()?).ok()?.words();
    let mut rest = words.iter();
    let first = rest.next()?;
    if file_name(first) != "env" {
        return Some(first.clone());
    }
    rest.find(|word| !word.contains('=') && !word.starts_with('-'))
        .cloned()
}

/// The program as an absolute canonical path, when it can be found.
fn resolve(program: &str, search_path: &[PathBuf]) -> Option<PathBuf> {
    if Path::new(program).is_absolute() {
        return Some(canonical(Path::new(program)));
    }
    xdg::find_program(program, search_path).map(|found| canonical(&found))
}

fn canonical(path: &Path) -> PathBuf {
    let text = path.to_string_lossy();
    let path = Path::new(text.trim_end_matches(" (deleted)"));
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

fn file_name(program: &str) -> String {
    file_name_of(Path::new(program))
}

fn file_name_of(path: &Path) -> String {
    path.file_name()
        .map(|name| {
            name.to_string_lossy()
                .trim_end_matches(" (deleted)")
                .to_owned()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::unix::fs::{PermissionsExt, symlink};

    use super::*;

    fn entry(id: &str, exec: &str) -> DesktopEntry {
        DesktopEntry::parse(
            DesktopId::new(id).unwrap(),
            PathBuf::from("/x"),
            &format!("[Desktop Entry]\nName=X\nType=Application\nExec={exec}\n"),
        )
        .unwrap()
    }

    fn process(exe: &str, argv: &[&str]) -> ProcessProgram {
        ProcessProgram {
            exe: Some(PathBuf::from(exe)),
            argv: argv.iter().map(|a| (*a).to_owned()).collect(),
        }
    }

    fn id_of(matcher: &ExecMatcher, process: &ProcessProgram) -> Option<String> {
        matcher.find(process).map(|id| id.as_str().to_owned())
    }

    fn matcher(entries: &[DesktopEntry]) -> ExecMatcher {
        ExecMatcher::new(entries, &[])
    }

    #[test]
    fn matches_an_absolute_exec_by_path() {
        let m = matcher(&[entry("slack.desktop", "/usr/lib/slack/slack -s %U")]);
        assert_eq!(
            id_of(
                &m,
                &process("/usr/lib/slack/slack", &["/usr/lib/slack/slack"])
            )
            .as_deref(),
            Some("slack.desktop")
        );
    }

    #[test]
    fn resolves_bare_programs_on_the_search_path_and_follows_symlinks() {
        let root = tempfile::tempdir().unwrap();
        let real = root.path().join("opt/Slack/slack");
        fs::create_dir_all(real.parent().unwrap()).unwrap();
        fs::write(&real, "#!/bin/sh\n").unwrap();
        fs::set_permissions(&real, fs::Permissions::from_mode(0o755)).unwrap();
        let bin = root.path().join("bin");
        fs::create_dir_all(&bin).unwrap();
        symlink(&real, bin.join("slack")).unwrap();
        // A second entry runs the same file name from elsewhere, so only the
        // path can tell them apart.
        let entries = [
            entry("slack.desktop", "slack %U"),
            entry("other.desktop", "/somewhere/else/slack"),
        ];
        let m = ExecMatcher::new(&entries, &[bin]);
        let running = process(real.to_str().unwrap(), &["slack"]);
        assert_eq!(id_of(&m, &running).as_deref(), Some("slack.desktop"));
    }

    #[test]
    fn falls_back_to_the_file_name_for_wrappers() {
        // The entry runs a wrapper script; the process is the real binary.
        let m = matcher(&[entry("firefox.desktop", "/usr/bin/firefox %u")]);
        let running = process(
            "/usr/lib/firefox/firefox",
            &["/usr/lib/firefox/firefox", "--new-window"],
        );
        assert_eq!(id_of(&m, &running).as_deref(), Some("firefox.desktop"));
        // argv[0] set by `exec -a`.
        let renamed = process("/opt/real/bin-1234", &["firefox"]);
        assert_eq!(id_of(&m, &renamed).as_deref(), Some("firefox.desktop"));
    }

    #[test]
    fn looks_past_env_assignments() {
        let m = matcher(&[entry(
            "app.desktop",
            "env GDK_BACKEND=x11 QT_QPA_PLATFORM=xcb /opt/app/app %u",
        )]);
        assert_eq!(
            id_of(&m, &process("/opt/app/app", &["app"])).as_deref(),
            Some("app.desktop")
        );
    }

    #[test]
    fn generic_launchers_name_nothing() {
        let m = matcher(&[
            entry("a.desktop", "flatpak run org.example.A"),
            entry("b.desktop", "sh -c 'b %u'"),
            entry("c.desktop", "env FOO=1 python3 /opt/c.py"),
            entry("d.desktop", "gtk-launch d"),
        ]);
        assert_eq!(id_of(&m, &process("/usr/bin/flatpak", &["flatpak"])), None);
        assert_eq!(id_of(&m, &process("/bin/sh", &["sh"])), None);
        assert_eq!(
            id_of(
                &m,
                &process("/usr/bin/python3.12", &["python3", "/opt/c.py"])
            ),
            None
        );
    }

    #[test]
    fn an_interpreted_script_is_matched_by_the_script() {
        let m = matcher(&[entry("tool.desktop", "/usr/bin/tool %u")]);
        let running = process("/usr/bin/python3.12", &["python3", "/usr/bin/tool", "--x"]);
        // A versioned interpreter counts too.
        assert_eq!(id_of(&m, &running).as_deref(), Some("tool.desktop"));
        let via_python = process("/usr/bin/python3", &["python3", "/usr/bin/tool"]);
        assert_eq!(id_of(&m, &via_python).as_deref(), Some("tool.desktop"));
        let bare = process("/bin/bash", &["bash", "/usr/bin/tool"]);
        assert_eq!(id_of(&m, &bare).as_deref(), Some("tool.desktop"));
        let no_script = process("/bin/bash", &["bash"]);
        assert_eq!(id_of(&m, &no_script), None);
    }

    #[test]
    fn ambiguous_programs_name_nothing_unless_one_entry_is_named_after_them() {
        let two = matcher(&[
            entry("firefox-work.desktop", "/usr/bin/firefox -P work %u"),
            entry("firefox-home.desktop", "/usr/bin/firefox -P home %u"),
        ]);
        assert_eq!(
            id_of(&two, &process("/usr/bin/firefox", &["firefox"])),
            None
        );

        let named = matcher(&[
            entry("firefox.desktop", "/usr/bin/firefox %u"),
            entry(
                "firefox-private.desktop",
                "/usr/bin/firefox --private-window %u",
            ),
        ]);
        assert_eq!(
            id_of(&named, &process("/usr/bin/firefox", &["firefox"])).as_deref(),
            Some("firefox.desktop")
        );
    }

    #[test]
    fn unknown_programs_and_empty_processes_match_nothing() {
        let m = matcher(&[entry("slack.desktop", "/usr/bin/slack")]);
        assert_eq!(id_of(&m, &process("/usr/bin/zoom", &["zoom"])), None);
        assert_eq!(m.find(&ProcessProgram::default()), None);
        assert_eq!(
            ExecMatcher::default().find(&process("/usr/bin/slack", &["slack"])),
            None
        );
    }

    #[test]
    fn a_deleted_executable_still_matches() {
        let m = matcher(&[entry("slack.desktop", "/usr/bin/slack")]);
        assert_eq!(
            id_of(&m, &process("/usr/bin/slack (deleted)", &["slack"])).as_deref(),
            Some("slack.desktop")
        );
    }

    #[test]
    fn entries_without_exec_are_skipped() {
        let no_exec = DesktopEntry::parse(
            DesktopId::new("x.desktop").unwrap(),
            PathBuf::from("/x"),
            "[Desktop Entry]\nName=X\n",
        )
        .unwrap();
        let m = matcher(&[no_exec]);
        assert_eq!(m.find(&process("/usr/bin/x", &["x"])), None);
    }
}
