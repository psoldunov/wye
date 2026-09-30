use std::os::unix::fs::symlink;

use super::*;
use crate::test_support::write_file;

fn id(value: &str) -> DesktopId {
    DesktopId::new(value).unwrap()
}

#[test]
fn parses_app_units() {
    let cases = [
        (
            "app-gnome-org.gnome.Nautilus-4321.scope",
            Some("org.gnome.Nautilus.desktop"),
        ),
        (
            "app-flatpak-com.slack.Slack-12345.scope",
            Some("com.slack.Slack.desktop"),
        ),
        (
            "app-org.kde.konsole@a1b2c3.service",
            Some("org.kde.konsole.desktop"),
        ),
        (
            "app-gnome-google\\x2dchrome-9876.scope",
            Some("google-chrome.desktop"),
        ),
        ("app-Hyprland-firefox-1234.scope", Some("firefox.desktop")),
        ("app-firefox-1234.scope", Some("firefox.desktop")),
        (
            "app-gnome-google-chrome-1234.scope",
            Some("google-chrome.desktop"),
        ),
        (
            "app-kde-org.kde.dolphin.service",
            Some("org.kde.dolphin.desktop"),
        ),
        ("session-2.scope", None),
        ("app.slice", None),
        ("app-gnome-bad\\x2-1.scope", None),
        ("user@1000.service", None),
    ];
    for (unit, expected) in cases {
        assert_eq!(
            parse_app_unit(unit).as_ref().map(DesktopId::as_str),
            expected,
            "{unit}"
        );
    }
}

/// A fake `/proc` process entry.
struct Proc<'a> {
    pid: u32,
    parent: u32,
    comm: &'a str,
    cgroup: Option<&'a str>,
    environ: Option<&'a [u8]>,
    exe: Option<&'a str>,
}

impl<'a> Proc<'a> {
    fn new(pid: u32, parent: u32, comm: &'a str) -> Self {
        Self {
            pid,
            parent,
            comm,
            cgroup: None,
            environ: None,
            exe: None,
        }
    }

    fn write(&self, root: &Path) {
        let dir = root.join(self.pid.to_string());
        write_file(&dir.join("comm"), &format!("{}\n", self.comm));
        write_file(
            &dir.join("stat"),
            &format!("{} ({}) S {} 1 1 0 -1\n", self.pid, self.comm, self.parent),
        );
        if let Some(cgroup) = self.cgroup {
            write_file(&dir.join("cgroup"), cgroup);
        }
        if let Some(environ) = self.environ {
            fs::write(dir.join("environ"), environ).unwrap();
        }
        if let Some(exe) = self.exe {
            symlink(exe, dir.join("exe")).unwrap();
        }
    }
}

fn proc_tree(procs: &[Proc<'_>]) -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    for proc in procs {
        proc.write(root.path());
    }
    root
}

#[test]
fn walks_past_launch_helpers_to_the_scope() {
    let root = proc_tree(&[
        Proc::new(300, 200, "xdg-open"),
        Proc::new(200, 150, "gio-launch-desk"),
        Proc::new(150, 100, "sh"),
        Proc {
            cgroup: Some(
                "0::/user.slice/user-1000.slice/user@1000.service/app.slice/\
                 app-gnome-com.slack.Slack-4242.scope\n",
            ),
            exe: Some("/usr/lib/slack/slack"),
            ..Proc::new(100, 1, "slack")
        },
    ]);
    let source = detect(root.path(), 300);
    assert_eq!(source.desktop_id, Some(id("com.slack.Slack")));
    assert_eq!(source.executable.as_deref(), Some("slack"));
}

#[test]
fn falls_back_to_gio_environment_and_comm() {
    let root = proc_tree(&[
        Proc::new(50, 40, "bash"),
        Proc {
            cgroup: Some("0::/user.slice/user-1000.slice/session-2.scope\n"),
            environ: Some(b"HOME=/h\0GIO_LAUNCHED_DESKTOP_FILE=/usr/share/applications/org.gnome.Terminal.desktop\0"),
            ..Proc::new(40, 1, "my (odd) app")
        },
    ]);
    let source = detect(root.path(), 50);
    assert_eq!(source.desktop_id, Some(id("org.gnome.Terminal")));
    assert_eq!(source.executable.as_deref(), Some("my (odd) app"));
}

#[test]
fn portal_and_systemd_mean_unknown() {
    let portal = proc_tree(&[
        Proc::new(20, 10, "xdg-open"),
        Proc::new(10, 1, "xdg-desktop-por"),
    ]);
    assert!(detect(portal.path(), 20).is_unknown());

    let systemd = proc_tree(&[Proc::new(20, 10, "sh"), Proc::new(10, 1, "systemd")]);
    assert!(detect(systemd.path(), 20).is_unknown());

    let only_helpers = proc_tree(&[Proc::new(20, 1, "xdg-open")]);
    assert!(detect(only_helpers.path(), 20).is_unknown());

    let missing = tempfile::tempdir().unwrap();
    assert!(detect(missing.path(), 1234).is_unknown());
    assert!(detect(missing.path(), 1).is_unknown());
}

#[test]
fn strips_deleted_suffix_from_exe() {
    let root = proc_tree(&[Proc {
        exe: Some("/opt/app/app-bin (deleted)"),
        ..Proc::new(30, 1, "app-bin")
    }]);
    assert_eq!(
        detect(root.path(), 30).executable.as_deref(),
        Some("app-bin")
    );
}

#[test]
fn falls_back_to_own_cgroup_when_reparented() {
    // `gio open` exited and Wye was reparented to `systemd --user`; it still
    // sits in the source app's scope.
    let slack_scope = "0::/user.slice/user-1000.slice/user@1000.service/app.slice/\
                       app-gnome-com.slack.Slack-4242.scope\n";
    let root = proc_tree(&[Proc::new(900, 1, "systemd")]);
    write_file(&root.path().join("self/cgroup"), slack_scope);
    let source = detect(root.path(), 900);
    assert_eq!(source.desktop_id, Some(id("com.slack.Slack")));
    assert_eq!(source.executable, None);

    // Missing parents fall back the same way.
    assert_eq!(
        detect(root.path(), 4321).desktop_id,
        Some(id("com.slack.Slack"))
    );

    // A parent chain that names an app wins over the cgroup.
    let chain = proc_tree(&[Proc::new(70, 1, "thunderbird")]);
    write_file(&chain.path().join("self/cgroup"), slack_scope);
    assert_eq!(
        detect(chain.path(), 70).executable.as_deref(),
        Some("thunderbird")
    );
    assert_eq!(detect(chain.path(), 70).desktop_id, None);
}

#[test]
fn ignores_wyes_own_scope_and_non_app_units() {
    let own = proc_tree(&[Proc::new(900, 1, "systemd")]);
    write_file(
        &own.path().join("self/cgroup"),
        "0::/user.slice/user-1000.slice/user@1000.service/app.slice/\
         app-gnome-dev.soldunov.wye-777.scope\n",
    );
    assert!(detect(own.path(), 900).is_unknown());

    let session = proc_tree(&[Proc::new(900, 1, "systemd")]);
    write_file(
        &session.path().join("self/cgroup"),
        "0::/user.slice/user-1000.slice/session-2.scope\n",
    );
    assert!(detect(session.path(), 900).is_unknown());
}
