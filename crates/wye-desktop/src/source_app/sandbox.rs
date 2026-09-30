//! Sandboxed apps (13-linux-platform.md, "Source-app detection details",
//! step 3): a Flatpak app names itself in `/.flatpak-info`, a Snap app runs
//! in a `snap.<name>.<app>-<uuid>.scope` unit.

use std::fs;
use std::path::Path;

use wye_core::DesktopId;

use super::{cgroup_units, unescape_unit};
use crate::keyfile;

/// The Flatpak application ID of the process, read from
/// `<pid dir>/root/.flatpak-info` (`[Application]` `name=`), which exists
/// only in a Flatpak sandbox. Flatpak exports the desktop entry as
/// `<application ID>.desktop`.
#[must_use]
pub(super) fn flatpak_desktop_id(pid_dir: &Path) -> Option<DesktopId> {
    let text = fs::read_to_string(pid_dir.join("root").join(".flatpak-info")).ok()?;
    let name = keyfile::parse(&text)
        .iter()
        .find(|group| group.name == "Application")?
        .get("name")?
        .trim()
        .to_owned();
    DesktopId::new(name).ok()
}

/// The desktop ID of a Snap app from its cgroup: `snap.<name>.<app>-<uuid>.scope`
/// is the entry `<name>_<app>.desktop`, which is how snapd exports it.
#[must_use]
pub(super) fn snap_desktop_id(cgroup: &str) -> Option<DesktopId> {
    cgroup_units(cgroup)
        .filter(|unit| unit.starts_with("snap."))
        .find_map(parse_snap_unit)
}

/// Parses one `snap.` unit name.
#[must_use]
pub(super) fn parse_snap_unit(unit: &str) -> Option<DesktopId> {
    let unit = unescape_unit(unit)?;
    let body = if let Some(scope) = unit.strip_suffix(".scope") {
        strip_uuid(scope)
    } else {
        unit.strip_suffix(".service")?
    };
    let mut parts = body.strip_prefix("snap.")?.split('.');
    let (name, app) = (parts.next()?, parts.next()?);
    if name.is_empty() || app.is_empty() {
        return None;
    }
    DesktopId::new(format!("{name}_{app}")).ok()
}

/// Removes the random suffix of a scope name: a UUID
/// (`-xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx`) or else one `-<random>` part.
fn strip_uuid(scope: &str) -> &str {
    const SHAPE: [usize; 5] = [8, 4, 4, 4, 12];
    let uuid_len = SHAPE.iter().sum::<usize>() + SHAPE.len();
    if let Some(start) = scope.len().checked_sub(uuid_len)
        && let Some(tail) = scope.get(start..)
        && let Some(head) = scope.get(..start)
    {
        let groups: Vec<&str> = tail.strip_prefix('-').unwrap_or("").split('-').collect();
        let shaped = groups.len() == SHAPE.len()
            && groups.iter().zip(SHAPE).all(|(group, len)| {
                group.len() == len && group.bytes().all(|b| b.is_ascii_hexdigit())
            });
        if shaped {
            return head;
        }
    }
    scope.rsplit_once('-').map_or(scope, |(head, _)| head)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(unit: &str) -> Option<String> {
        parse_snap_unit(unit).map(|id| id.as_str().to_owned())
    }

    #[test]
    fn parses_snap_units() {
        for (unit, expected) in [
            (
                "snap.firefox.firefox-21ddf5b7-5c8d-4a8d-9e2b-0c4f9a1b2c3d.scope",
                Some("firefox_firefox.desktop"),
            ),
            (
                "snap.spotify.spotify-2cea2c2a-0f8b-4ca5-a1f0-f1a1f4a07b5c.scope",
                Some("spotify_spotify.desktop"),
            ),
            (
                "snap.chromium.chromium-1234.scope",
                Some("chromium_chromium.desktop"),
            ),
            (
                "snap.foo.my-app-21ddf5b7-5c8d-4a8d-9e2b-0c4f9a1b2c3d.scope",
                Some("foo_my-app.desktop"),
            ),
            ("snap.code.code.service", Some("code_code.desktop")),
            ("snap.lxd.daemon.service", Some("lxd_daemon.desktop")),
            ("snap.firefox.scope", None),
            ("snap..x-1.scope", None),
            ("snapd.service", None),
            ("app-gnome-firefox-1234.scope", None),
        ] {
            assert_eq!(snap(unit).as_deref(), expected, "{unit}");
        }
    }

    #[test]
    fn finds_the_snap_unit_in_a_cgroup_path() {
        let cgroup = "0::/user.slice/user-1000.slice/user@1000.service/app.slice/\
                      snap.firefox.firefox-21ddf5b7-5c8d-4a8d-9e2b-0c4f9a1b2c3d.scope\n";
        assert_eq!(
            snap_desktop_id(cgroup)
                .map(|id| id.as_str().to_owned())
                .as_deref(),
            Some("firefox_firefox.desktop")
        );
        assert!(snap_desktop_id("0::/user.slice/session-2.scope\n").is_none());
        assert!(snap_desktop_id("").is_none());
    }

    #[test]
    fn reads_the_flatpak_application_id() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(flatpak_desktop_id(dir.path()), None);
        let info = dir.path().join("root/.flatpak-info");
        fs::create_dir_all(info.parent().unwrap()).unwrap();
        fs::write(
            &info,
            "[Application]\nname=org.mozilla.firefox\nruntime=runtime/org.freedesktop.Platform/x86_64/24.08\n\n[Instance]\napp-path=/x\n",
        )
        .unwrap();
        assert_eq!(
            flatpak_desktop_id(dir.path())
                .map(|id| id.as_str().to_owned())
                .as_deref(),
            Some("org.mozilla.firefox.desktop")
        );
        fs::write(&info, "[Instance]\nname=not.this\n").unwrap();
        assert_eq!(flatpak_desktop_id(dir.path()), None);
        fs::write(&info, "[Application]\nname=\n").unwrap();
        assert_eq!(flatpak_desktop_id(dir.path()), None);
    }
}
