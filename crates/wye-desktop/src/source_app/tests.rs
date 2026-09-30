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

#[test]
fn ignores_an_openers_scope() {
    // A compositor started Wye as `wye`, `xdg-open` or `gio` in its own unit;
    // that unit names the opener, not the app the link came from.
    for unit in [
        "app-Hyprland-wye@4321.service",
        "app-Hyprland-xdg\\x2dopen-4321.scope",
        "app-gnome-gio-4321.scope",
    ] {
        let root = proc_tree(&[Proc::new(900, 1, "systemd")]);
        write_file(
            &root.path().join("self/cgroup"),
            &format!("0::/user.slice/user-1000.slice/user@1000.service/app.slice/{unit}\n"),
        );
        assert!(detect(root.path(), 900).is_unknown(), "{unit}");
    }
}

// Step 3: sandboxes and the program's own entry.

fn desktop_entry(id: &str, exec: &str) -> crate::entry::DesktopEntry {
    crate::entry::DesktopEntry::parse(
        id_of(id),
        PathBuf::from("/x"),
        &format!("[Desktop Entry]\nName=X\nType=Application\nExec={exec}\n"),
    )
    .unwrap()
}

fn id_of(value: &str) -> DesktopId {
    DesktopId::new(value).unwrap()
}

const SESSION_CGROUP: &str = "0::/user.slice/user-1000.slice/session-2.scope\n";

#[test]
fn a_flatpak_sandbox_names_its_app() {
    let root = proc_tree(&[
        Proc::new(300, 200, "xdg-open"),
        Proc {
            cgroup: Some(SESSION_CGROUP),
            exe: Some("/app/bin/firefox"),
            ..Proc::new(200, 1, "firefox")
        },
    ]);
    write_file(
        &root.path().join("200/root/.flatpak-info"),
        "[Application]\nname=org.mozilla.firefox\n",
    );
    let source = detect(root.path(), 300);
    assert_eq!(source.desktop_id, Some(id("org.mozilla.firefox")));
    assert_eq!(source.executable.as_deref(), Some("firefox"));
}

#[test]
fn a_snap_unit_names_its_app() {
    let root = proc_tree(&[Proc {
        cgroup: Some(
            "0::/user.slice/user-1000.slice/user@1000.service/app.slice/\
             snap.firefox.firefox-21ddf5b7-5c8d-4a8d-9e2b-0c4f9a1b2c3d.scope\n",
        ),
        exe: Some("/snap/firefox/4848/usr/lib/firefox/firefox"),
        ..Proc::new(60, 1, "firefox")
    }]);
    assert_eq!(
        detect(root.path(), 60).desktop_id,
        Some(id("firefox_firefox"))
    );
}

#[test]
fn the_systemd_scope_and_gio_environment_beat_the_sandbox_and_program_steps() {
    let scope = "0::/user.slice/user-1000.slice/user@1000.service/app.slice/\
                 app-gnome-com.slack.Slack-4242.scope\n";
    let matcher = ExecMatcher::new(
        &[desktop_entry("other.desktop", "/usr/lib/slack/slack")],
        &[],
    );
    let by_scope = proc_tree(&[Proc {
        cgroup: Some(scope),
        exe: Some("/usr/lib/slack/slack"),
        ..Proc::new(10, 1, "slack")
    }]);
    write_file(
        &by_scope.path().join("10/root/.flatpak-info"),
        "[Application]\nname=not.used\n",
    );
    assert_eq!(
        detect_with(by_scope.path(), 10, &matcher).desktop_id,
        Some(id("com.slack.Slack"))
    );

    let by_env = proc_tree(&[Proc {
        cgroup: Some(SESSION_CGROUP),
        environ: Some(b"GIO_LAUNCHED_DESKTOP_FILE=/usr/share/applications/gio.desktop\0"),
        exe: Some("/usr/lib/slack/slack"),
        ..Proc::new(10, 1, "slack")
    }]);
    write_file(
        &by_env.path().join("10/root/.flatpak-info"),
        "[Application]\nname=not.used\n",
    );
    assert_eq!(
        detect_with(by_env.path(), 10, &matcher).desktop_id,
        Some(id("gio"))
    );

    // The sandbox beats the program match.
    let by_flatpak = proc_tree(&[Proc {
        cgroup: Some(SESSION_CGROUP),
        exe: Some("/usr/lib/slack/slack"),
        ..Proc::new(10, 1, "slack")
    }]);
    write_file(
        &by_flatpak.path().join("10/root/.flatpak-info"),
        "[Application]\nname=com.slack.Slack\n",
    );
    assert_eq!(
        detect_with(by_flatpak.path(), 10, &matcher).desktop_id,
        Some(id("com.slack.Slack"))
    );
}

#[test]
fn the_program_is_matched_against_desktop_entries_when_nothing_else_names_the_app() {
    let matcher = ExecMatcher::new(
        &[
            desktop_entry("slack.desktop", "/usr/lib/slack/slack -s %U"),
            desktop_entry("thunderbird.desktop", "thunderbird %u"),
        ],
        &[],
    );
    let root = proc_tree(&[
        Proc::new(300, 200, "xdg-open"),
        Proc {
            cgroup: Some(SESSION_CGROUP),
            exe: Some("/usr/lib/slack/slack"),
            ..Proc::new(200, 1, "slack")
        },
    ]);
    std::fs::write(
        root.path().join("200/cmdline"),
        b"/usr/lib/slack/slack\0--type=x\0",
    )
    .unwrap();
    let source = detect_with(root.path(), 300, &matcher);
    assert_eq!(source.desktop_id, Some(id("slack")));
    assert_eq!(source.executable.as_deref(), Some("slack"));

    // Without a matcher the ID stays unknown, as before.
    let plain = detect(root.path(), 300);
    assert_eq!(plain.desktop_id, None);
    assert_eq!(plain.executable.as_deref(), Some("slack"));

    // argv[0] is used when the executable is a wrapper target elsewhere.
    let wrapped = proc_tree(&[Proc {
        cgroup: Some(SESSION_CGROUP),
        exe: Some("/opt/tb/thunderbird-bin"),
        ..Proc::new(70, 1, "thunderbird-bin")
    }]);
    std::fs::write(wrapped.path().join("70/cmdline"), b"thunderbird\0").unwrap();
    assert_eq!(
        detect_with(wrapped.path(), 70, &matcher).desktop_id,
        Some(id("thunderbird"))
    );

    // A program no entry runs stays unknown.
    let unknown = proc_tree(&[Proc {
        exe: Some("/usr/bin/zoom"),
        ..Proc::new(80, 1, "zoom")
    }]);
    assert_eq!(detect_with(unknown.path(), 80, &matcher).desktop_id, None);
}

#[test]
fn detect_in_uses_the_apps_of_an_inventory() {
    let fx = crate::test_support::Fixture::new();
    fx.system_entry(
        "tool.desktop",
        "[Desktop Entry]\nName=Tool\nType=Application\nExec=/usr/lib/tool/tool %u\n",
    );
    let inventory = Inventory::scan(&fx.xdg, &id("wye-test"));
    let root = proc_tree(&[Proc {
        exe: Some("/usr/lib/tool/tool"),
        ..Proc::new(90, 1, "tool")
    }]);
    assert_eq!(
        detect_in(root.path(), 90, &inventory).desktop_id,
        Some(id("tool"))
    );
}
