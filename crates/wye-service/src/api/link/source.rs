//! The source app of a link that came over D-Bus (PIPE-01, 13-linux-platform
//! "Source-app detection").
//!
//! `org.freedesktop.Application.Open` is called by the app itself (through
//! KIO or GIO) or by an opener it ran (`kde-open`, `xdg-open`, `gio`).
//! Detection starts at the caller's PID, skipping openers up the parent
//! chain, and then follows the same rules as `wye open`. A caller that is
//! `xdg-desktop-portal` (Flatpak apps through `OpenURI`) hides the app; the
//! focused window stands in for it.

use std::path::Path;

use wye_core::{DesktopId, SourceApp};
use wye_desktop::loop_guard::OPENERS;
use wye_desktop::{Inventory, source_app};
use zbus::fdo::DBusProxy;
use zbus::names::BusName;

use crate::platform::FocusSource;

/// `xdg-desktop-portal` as the kernel's 15-byte `comm` shows it.
const PORTAL_COMM: &str = "xdg-desktop-por";

/// How far up the chain openers are skipped.
const MAX_OPENERS: usize = 8;

/// The process ID of the connection named `sender`.
pub(crate) async fn caller_pid(connection: &zbus::Connection, sender: &str) -> Option<u32> {
    let name = BusName::try_from(sender).ok()?;
    let bus = DBusProxy::new(connection).await.ok()?;
    match bus.get_connection_credentials(name).await {
        Ok(credentials) => credentials.process_id(),
        Err(error) => {
            tracing::info!(%error, sender, "cannot read the caller's credentials");
            None
        }
    }
}

/// The app behind process `pid`: `None` when the caller is the portal and
/// the focused window has to be asked instead.
pub(crate) fn from_pid(proc_root: &Path, pid: u32, inventory: &Inventory) -> Option<SourceApp> {
    let mut current = pid;
    for _ in 0..MAX_OPENERS {
        let Some(comm) = comm(proc_root, current) else {
            break;
        };
        if comm == PORTAL_COMM {
            return None;
        }
        if !OPENERS.iter().any(|opener| truncated(opener) == comm) {
            break;
        }
        current = parent_pid(proc_root, current)?;
    }
    Some(source_app::detect_in(proc_root, current, inventory))
}

/// The focused window's app, standing in for a caller that hides the
/// source (step 4).
pub(crate) async fn from_focus(focus: &dyn FocusSource) -> SourceApp {
    let Some(app) = focus.focused().await else {
        return SourceApp::default();
    };
    SourceApp {
        desktop_id: app
            .desktop_id
            .and_then(|id| DesktopId::new(with_suffix(&id)).ok()),
        executable: app.resource_class,
    }
}

fn with_suffix(id: &str) -> String {
    if id.ends_with(DesktopId::SUFFIX) {
        id.to_owned()
    } else {
        format!("{id}{}", DesktopId::SUFFIX)
    }
}

fn comm(proc_root: &Path, pid: u32) -> Option<String> {
    let text = std::fs::read_to_string(proc_root.join(pid.to_string()).join("comm")).ok()?;
    Some(text.trim_end_matches('\n').to_owned())
}

fn parent_pid(proc_root: &Path, pid: u32) -> Option<u32> {
    let status = std::fs::read_to_string(proc_root.join(pid.to_string()).join("status")).ok()?;
    status
        .lines()
        .find_map(|line| line.strip_prefix("PPid:"))
        .and_then(|value| value.trim().parse().ok())
}

/// A program name as `comm` shows it.
fn truncated(name: &str) -> &str {
    name.get(..15).unwrap_or(name)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    fn no_apps() -> Inventory {
        Inventory::from_apps(Vec::new(), Vec::new())
    }

    /// A fake process under `root`.
    fn process(root: &Path, pid: u32, comm: &str, parent: u32, cgroup: &str) {
        let dir = root.join(pid.to_string());
        fs::create_dir_all(&dir).expect("dir");
        fs::write(dir.join("comm"), format!("{comm}\n")).expect("comm");
        fs::write(
            dir.join("status"),
            format!("Name:\t{comm}\nPPid:\t{parent}\n"),
        )
        .expect("status");
        fs::write(dir.join("stat"), format!("{pid} ({comm}) S {parent} 0 0\n")).expect("stat");
        fs::write(dir.join("cgroup"), format!("0::{cgroup}\n")).expect("cgroup");
        fs::write(dir.join("environ"), "").expect("environ");
    }

    #[test]
    fn openers_between_the_app_and_wye_are_skipped() {
        let root = tempfile::tempdir().expect("temp dir");
        let slice = "/user.slice/user-1000.slice/user@1000.service/app.slice";
        process(
            root.path(),
            100,
            "chat",
            1,
            &format!("{slice}/app-org.example.Chat-1.scope"),
        );
        process(
            root.path(),
            200,
            "kde-open",
            100,
            &format!("{slice}/app-org.example.Chat-1.scope"),
        );
        let source = from_pid(root.path(), 200, &no_apps()).expect("an app");
        assert_eq!(
            source.desktop_id,
            DesktopId::new("org.example.Chat.desktop").ok()
        );
    }

    #[test]
    fn the_portal_hides_the_app() {
        let root = tempfile::tempdir().expect("temp dir");
        process(
            root.path(),
            300,
            PORTAL_COMM,
            1,
            "/user.slice/xdg-desktop-portal.service",
        );
        assert_eq!(from_pid(root.path(), 300, &no_apps()), None);
    }

    #[test]
    fn focused_ids_gain_the_desktop_suffix() {
        assert_eq!(with_suffix("org.kde.dolphin"), "org.kde.dolphin.desktop");
        assert_eq!(with_suffix("firefox.desktop"), "firefox.desktop");
    }
}
