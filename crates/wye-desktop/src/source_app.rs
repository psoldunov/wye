//! Detecting the app a link came from ([13-linux-platform.md], "Source-app
//! detection details").
//!
//! [13-linux-platform.md]: ../../../docs/spec/13-linux-platform.md

use std::fs;
use std::path::{Path, PathBuf};

use wye_core::{DesktopId, SourceApp};

use crate::discovery::Inventory;
use crate::launch::WYE_DESKTOP_ID;
use crate::loop_guard::OPENERS;

mod exec_match;
mod sandbox;

pub use exec_match::{ExecMatcher, ProcessProgram};

/// Processes between the source app and Wye: launch helpers and shells.
const SKIPPED: &[&str] = &[
    "xdg-open",
    "gio",
    "gio-launch-desktop",
    "sh",
    "bash",
    "dash",
    "zsh",
    "fish",
    "env",
    "nohup",
    "xdg-settings",
];

/// Flatpak apps open links through the `OpenURI` portal; its caller is lost.
const PORTAL: &str = "xdg-desktop-portal";

/// The kernel truncates `comm` to 15 bytes.
const COMM_LEN: usize = 15;

/// How many ancestors to inspect before giving up.
const MAX_HOPS: usize = 64;

/// Walks the parent chain starting at `start_pid` (normally Wye's parent)
/// under `proc_root` (normally `/proc`), skipping launch helpers and shells.
/// Stops at PID 1 or `systemd`. When the first remaining process is
/// `xdg-desktop-portal`, or nothing could be read, the chain names no app.
///
/// The chain comes up empty when the helper that started Wye (`gio open`,
/// say) has already exited and Wye was reparented to `systemd --user`. Wye
/// still sits in the cgroup it was started in, so the app unit in
/// `<proc_root>/self/cgroup` is used then, unless that unit is Wye's own
/// (a launcher that started Wye in a scope of its own). Otherwise the
/// source is unknown.
///
/// The desktop ID of the app comes from, in order: its systemd scope,
/// `GIO_LAUNCHED_DESKTOP_FILE`, the Flatpak sandbox it runs in, its Snap
/// unit. [`detect_with`] adds matching its program against desktop entries.
#[must_use]
pub fn detect(proc_root: &Path, start_pid: u32) -> SourceApp {
    detect_using(proc_root, start_pid, None)
}

/// Like [`detect`], and when none of those name the app, the program it runs
/// is matched against the `Exec` of the desktop entries in `matcher`: the
/// executable and `argv[0]`, by resolved path and then by file name
/// (13-linux-platform.md, step 3).
#[must_use]
pub fn detect_with(proc_root: &Path, start_pid: u32, matcher: &ExecMatcher) -> SourceApp {
    detect_using(proc_root, start_pid, Some(matcher))
}

/// [`detect_with`] over the apps of `inventory`.
#[must_use]
pub fn detect_in(proc_root: &Path, start_pid: u32, inventory: &Inventory) -> SourceApp {
    detect_with(
        proc_root,
        start_pid,
        &ExecMatcher::from_inventory(inventory),
    )
}

fn detect_using(proc_root: &Path, start_pid: u32, matcher: Option<&ExecMatcher>) -> SourceApp {
    walk_parents(proc_root, start_pid, matcher)
        .or_else(|| own_scope(proc_root))
        .unwrap_or_default()
}

/// The source app from the parent chain, or `None` when it names none.
fn walk_parents(
    proc_root: &Path,
    start_pid: u32,
    matcher: Option<&ExecMatcher>,
) -> Option<SourceApp> {
    let mut pid = start_pid;
    for _ in 0..MAX_HOPS {
        if pid <= 1 {
            break;
        }
        let Some(comm) = read_comm(proc_root, pid) else {
            break;
        };
        if comm_is(&comm, "systemd") || comm_is(&comm, PORTAL) {
            break;
        }
        if SKIPPED.iter().any(|name| comm_is(&comm, name)) {
            match parent_pid(proc_root, pid) {
                Some(parent) => {
                    pid = parent;
                    continue;
                }
                None => break,
            }
        }
        return Some(describe(proc_root, pid, comm, matcher));
    }
    None
}

/// The app unit Wye itself runs in, unless it is Wye's own or an opener's
/// (`app-Hyprland-wye@1.service`, or a scope named after `xdg-open`): those
/// units belong to what forwarded the link, not to the app it came from.
fn own_scope(proc_root: &Path) -> Option<SourceApp> {
    let text = fs::read_to_string(proc_root.join("self").join("cgroup")).ok()?;
    let desktop_id = desktop_id_from_cgroup(&text)?;
    if desktop_id.as_str() == WYE_DESKTOP_ID || OPENERS.contains(&desktop_id.app_id()) {
        return None;
    }
    Some(SourceApp {
        desktop_id: Some(desktop_id),
        executable: None,
    })
}

/// Parses a systemd unit named after the desktop-launcher convention:
/// `app[-<launcher>]-<ApplicationID>-<RANDOM>.scope` or
/// `app[-<launcher>]-<ApplicationID>[@<RANDOM>].service`, where `-` inside
/// the application ID is escaped as `\x2d`.
///
/// Units that do not escape `-` are handled best effort: the first segment
/// is taken as the launcher and the rest as the ID.
#[must_use]
pub fn parse_app_unit(unit: &str) -> Option<DesktopId> {
    let body = if let Some(scope) = unit.strip_suffix(".scope") {
        scope.rsplit_once('-')?.0
    } else {
        let service = unit.strip_suffix(".service")?;
        service.split_once('@').map_or(service, |(name, _)| name)
    };
    let rest = body.strip_prefix("app-")?;
    let app_id = rest
        .split_once('-')
        .map_or(rest, |(_launcher, app_id)| app_id);
    DesktopId::new(unescape_unit(app_id)?).ok()
}

/// The source app for one process: its desktop ID and its executable's file
/// name. The ID is tried from the systemd scope, `GIO_LAUNCHED_DESKTOP_FILE`,
/// the Flatpak sandbox, the Snap unit and, with a `matcher`, the program's
/// desktop entry.
fn describe(proc_root: &Path, pid: u32, comm: String, matcher: Option<&ExecMatcher>) -> SourceApp {
    let dir = pid_dir(proc_root, pid);
    let cgroup = fs::read_to_string(dir.join("cgroup")).unwrap_or_default();
    let exe_path = fs::read_link(dir.join("exe")).ok();
    let desktop_id = desktop_id_from_cgroup(&cgroup)
        .or_else(|| desktop_id_from_environ(&dir.join("environ")))
        .or_else(|| sandbox::flatpak_desktop_id(&dir))
        .or_else(|| sandbox::snap_desktop_id(&cgroup))
        .or_else(|| {
            matcher?.find(&ProcessProgram {
                exe: exe_path.clone(),
                argv: read_argv(&dir.join("cmdline")),
            })
        });
    let executable = exe_path
        .and_then(|target| {
            let name = target.file_name()?.to_string_lossy().into_owned();
            Some(name.trim_end_matches(" (deleted)").to_owned())
        })
        .filter(|name| !name.is_empty())
        .or(Some(comm));
    SourceApp {
        desktop_id,
        executable,
    }
}

/// The unified-hierarchy (`0::`) cgroup path's innermost `app-…` unit.
fn desktop_id_from_cgroup(text: &str) -> Option<DesktopId> {
    cgroup_units(text)
        .filter(|unit| unit.starts_with("app-"))
        .find_map(parse_app_unit)
}

/// The units of the unified-hierarchy (`0::`) cgroup path, innermost first.
fn cgroup_units(text: &str) -> impl Iterator<Item = &str> {
    text.lines()
        .find_map(|line| line.strip_prefix("0::"))
        .unwrap_or_default()
        .rsplit('/')
}

/// `/proc/<pid>/cmdline` split at NUL.
fn read_argv(path: &Path) -> Vec<String> {
    fs::read(path)
        .map(|bytes| {
            bytes
                .split(|byte| *byte == 0)
                .filter(|arg| !arg.is_empty())
                .map(|arg| String::from_utf8_lossy(arg).into_owned())
                .collect()
        })
        .unwrap_or_default()
}

fn desktop_id_from_environ(path: &Path) -> Option<DesktopId> {
    let environ = fs::read(path).ok()?;
    let value = environ
        .split(|byte| *byte == 0)
        .find_map(|var| var.strip_prefix(b"GIO_LAUNCHED_DESKTOP_FILE="))?;
    let file = PathBuf::from(String::from_utf8_lossy(value).into_owned());
    DesktopId::new(file.file_name()?.to_string_lossy().into_owned()).ok()
}

fn read_comm(proc_root: &Path, pid: u32) -> Option<String> {
    let comm = fs::read_to_string(pid_dir(proc_root, pid).join("comm")).ok()?;
    Some(comm.trim_end_matches('\n').to_owned())
}

/// The parent PID from `stat`: the fourth field, counted after the last `)`
/// because the command name in parentheses may contain spaces and `)`.
fn parent_pid(proc_root: &Path, pid: u32) -> Option<u32> {
    let stat = fs::read_to_string(pid_dir(proc_root, pid).join("stat")).ok()?;
    let (_, after_name) = stat.rsplit_once(')')?;
    after_name.split_whitespace().nth(1)?.parse().ok()
}

fn pid_dir(proc_root: &Path, pid: u32) -> PathBuf {
    proc_root.join(pid.to_string())
}

/// True when `comm` is `name` as the kernel would record it.
fn comm_is(comm: &str, name: &str) -> bool {
    let truncated = name.get(..COMM_LEN).unwrap_or(name);
    comm == truncated
}

/// Decodes systemd's `\xNN` unit-name escapes.
fn unescape_unit(escaped: &str) -> Option<String> {
    let mut bytes = Vec::with_capacity(escaped.len());
    let mut rest = escaped.as_bytes();
    while let Some((&byte, tail)) = rest.split_first() {
        if byte == b'\\' && tail.first() == Some(&b'x') {
            let digits = tail.get(1..3)?;
            if !digits.iter().all(u8::is_ascii_hexdigit) {
                return None;
            }
            bytes.push(u8::from_str_radix(std::str::from_utf8(digits).ok()?, 16).ok()?);
            rest = tail.get(3..)?;
        } else {
            bytes.push(byte);
            rest = tail;
        }
    }
    String::from_utf8(bytes).ok()
}

#[cfg(test)]
mod tests;
