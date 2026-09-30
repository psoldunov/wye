//! Keeping links from coming back to Wye (DEF-06).
//!
//! Wye is the default browser, so anything that opens a link "in the default
//! browser" opens it in Wye again. Beyond Wye's own desktop ID, that covers
//! the `wye` program itself and the generic openers below: an entry whose
//! `Exec` runs one of them is never a web handler or a target, and Wye never
//! launches one.

use std::path::{Path, PathBuf};

use crate::entry::DesktopEntry;
use crate::exec::ExecTemplate;
use crate::xdg;

/// Programs that hand a link to the default browser (or to whatever a
/// desktop entry names) instead of opening it themselves.
///
/// - `wye`: Wye itself.
/// - `xdg-open`, `gio` (`gio open`), `gvfs-open`, `exo-open`, `kde-open`,
///   `kde-open5`, `kioclient`, `kioclient5`, `kfmclient`: the desktops'
///   openers.
/// - `handlr`, `mimeo`, `mimeopen`: standalone `xdg-open` replacements.
/// - `sensible-browser`, `x-www-browser`: Debian's "the preferred browser"
///   aliases, which may point at Wye.
/// - `gtk-launch`: runs another desktop entry, which Wye cannot vet.
///
/// Not covered: shell wrappers (`sh -c "xdg-open %u"`) and D-Bus calls
/// (`busctl`, `gdbus`) to the `OpenURI` portal; those are too varied to
/// recognise reliably.
pub const OPENERS: &[&str] = &[
    "wye",
    "xdg-open",
    "gio",
    "gvfs-open",
    "exo-open",
    "kde-open",
    "kde-open5",
    "kioclient",
    "kioclient5",
    "kfmclient",
    "handlr",
    "mimeo",
    "mimeopen",
    "sensible-browser",
    "x-www-browser",
    "gtk-launch",
];

/// True when `entry`'s `Exec` runs Wye or a generic opener. An entry
/// without a parsable `Exec` does not forward anything.
#[must_use]
pub fn forwards_links(entry: &DesktopEntry) -> bool {
    entry
        .exec
        .as_deref()
        .and_then(|line| ExecTemplate::parse(line).ok())
        .is_some_and(|exec| runs_opener(&exec.words()))
}

/// True when the command line `argv` runs Wye or a generic opener, looking
/// through a leading `env [NAME=value]... [-flags]`.
#[must_use]
pub fn runs_opener(argv: &[String]) -> bool {
    program(argv).is_some_and(|program| OPENERS.contains(&basename(program)))
}

/// True when `argv`'s program resolves (on `search_path`, following
/// symlinks) to the running executable. Best effort: false when either
/// side cannot be resolved.
#[must_use]
pub fn runs_current_exe(argv: &[String], search_path: &[PathBuf]) -> bool {
    let Some(current) = std::env::current_exe()
        .ok()
        .and_then(|path| path.canonicalize().ok())
    else {
        return false;
    };
    program(argv)
        .and_then(|program| xdg::find_program(program, search_path))
        .and_then(|path| path.canonicalize().ok())
        .is_some_and(|path| path == current)
}

/// The program a command line really runs: its first word, or for `env`
/// the first word that is neither an assignment, an option nor the value of
/// `-u`/`-C` (`--unset`, `--chdir`).
fn program(argv: &[String]) -> Option<&str> {
    let mut words = argv.iter().map(String::as_str);
    let first = words.next()?;
    if basename(first) != "env" {
        return Some(first);
    }
    while let Some(word) = words.next() {
        if matches!(word, "-u" | "--unset" | "-C" | "--chdir") {
            words.next();
        } else if !word.contains('=') && !word.starts_with('-') {
            return Some(word);
        }
    }
    None
}

fn basename(path: &str) -> &str {
    Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(path)
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::symlink;

    use super::*;
    use crate::test_support::Fixture;

    fn argv(words: &[&str]) -> Vec<String> {
        words.iter().map(|word| (*word).to_owned()).collect()
    }

    #[test]
    fn recognises_openers_and_wye() {
        assert!(runs_opener(&argv(&["xdg-open", "https://a"])));
        assert!(runs_opener(&argv(&["/usr/bin/wye", "open", "https://a"])));
        assert!(runs_opener(&argv(&["gio", "open", "https://a"])));
        assert!(runs_opener(&argv(&["env", "A=1", "-u", "B", "kde-open5"])));
        assert!(!runs_opener(&argv(&["firefox", "https://a"])));
        assert!(!runs_opener(&argv(&["/opt/wyewolf", "https://a"])));
        assert!(!runs_opener(&[]));
    }

    #[test]
    fn recognises_the_running_executable() {
        let fx = Fixture::new();
        let current = std::env::current_exe().unwrap();
        let direct = current.to_string_lossy().into_owned();
        assert!(runs_current_exe(&argv(&[&direct]), &[]));

        // A renamed symlink on the search path still resolves to it.
        symlink(&current, fx.path("bin/renamed")).unwrap();
        assert!(runs_current_exe(&argv(&["renamed"]), &fx.xdg.search_path));

        fx.program("other");
        assert!(!runs_current_exe(&argv(&["other"]), &fx.xdg.search_path));
        assert!(!runs_current_exe(&argv(&["missing"]), &fx.xdg.search_path));
    }
}
