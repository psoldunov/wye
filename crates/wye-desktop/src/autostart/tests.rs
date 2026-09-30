use std::os::unix::fs::symlink;

use super::*;
use crate::entry::DesktopEntry;
use crate::exec::ExecTemplate;
use crate::test_support::Fixture;

fn parse(text: &str) -> DesktopEntry {
    DesktopEntry::parse(
        wye_core::DesktopId::new(WYE_DESKTOP_ID).unwrap(),
        PathBuf::from("/x"),
        text,
    )
    .unwrap()
}

fn exec_words(text: &str) -> Vec<String> {
    ExecTemplate::parse(parse(text).exec.as_deref().unwrap())
        .unwrap()
        .words()
}

// GEN-01
#[test]
fn the_entry_starts_the_service_with_the_given_executable() {
    let text = entry_text(Path::new("/usr/bin/wye"));
    let entry = parse(&text);
    assert_eq!(entry.entry_type.as_deref(), Some("Application"));
    assert_eq!(entry.name, "Wye");
    assert_eq!(
        entry.exec.as_deref(),
        Some("/usr/bin/wye service --activate")
    );
    assert_eq!(entry.icon.as_deref(), Some("dev.soldunov.wye"));
    assert!(!entry.terminal);
    assert!(text.contains("X-GNOME-Autostart-enabled=true\n"));
    assert!(text.starts_with("[Desktop Entry]\n"));
}

#[test]
fn the_exec_path_survives_spaces_and_special_characters() {
    for path in [
        "/nix/store/abc-wye-0.1.0/bin/wye",
        "/opt/my apps/wye",
        "/opt/100%/wye",
        "/opt/a\"b/wye",
        "/opt/a$b`c/wye",
        "/opt/back\\slash/wye",
        "/opt/(x) & y; z/wye",
    ] {
        let words = exec_words(&entry_text(Path::new(path)));
        assert_eq!(words, [path, "service", "--activate"], "{path}");
    }
}

#[test]
fn enable_writes_the_entry_under_the_config_home() {
    let fx = Fixture::new();
    enable(&fx.xdg, Path::new("/usr/bin/wye")).unwrap();
    let path = entry_path(&fx.xdg);
    assert_eq!(
        path,
        fx.xdg
            .config_home
            .join("autostart/dev.soldunov.wye.desktop")
    );
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        entry_text(Path::new("/usr/bin/wye"))
    );
    assert!(is_enabled(&fx.xdg));
}

#[test]
fn enabling_again_with_another_executable_replaces_the_entry() {
    let fx = Fixture::new();
    enable(&fx.xdg, Path::new("/usr/bin/wye")).unwrap();
    enable(&fx.xdg, Path::new("/opt/wye/bin/wye")).unwrap();
    let text = std::fs::read_to_string(entry_path(&fx.xdg)).unwrap();
    assert!(text.contains("Exec=/opt/wye/bin/wye service --activate\n"));
    assert_eq!(text.matches("[Desktop Entry]").count(), 1);
}

#[test]
fn enabling_an_unchanged_entry_writes_nothing() {
    let fx = Fixture::new();
    enable(&fx.xdg, Path::new("/usr/bin/wye")).unwrap();
    let before = std::fs::metadata(entry_path(&fx.xdg))
        .unwrap()
        .modified()
        .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(20));
    enable(&fx.xdg, Path::new("/usr/bin/wye")).unwrap();
    let after = std::fs::metadata(entry_path(&fx.xdg))
        .unwrap()
        .modified()
        .unwrap();
    assert_eq!(before, after);
}

#[test]
fn disable_removes_the_entry() {
    let fx = Fixture::new();
    assert!(!disable(&fx.xdg).unwrap(), "nothing to remove");
    assert!(!is_enabled(&fx.xdg));
    enable(&fx.xdg, Path::new("/usr/bin/wye")).unwrap();
    assert!(disable(&fx.xdg).unwrap());
    assert!(!entry_path(&fx.xdg).exists());
    assert!(!is_enabled(&fx.xdg));
    assert!(!disable(&fx.xdg).unwrap());
}

#[test]
fn an_entry_the_session_would_skip_is_not_enabled() {
    let fx = Fixture::new();
    let write = |extra: &str| {
        fx.write(
            "home/.config/autostart/dev.soldunov.wye.desktop",
            &format!("[Desktop Entry]\nType=Application\nName=Wye\nExec=wye\n{extra}"),
        );
    };
    write("");
    assert!(is_enabled(&fx.xdg));
    write("X-GNOME-Autostart-enabled=false\n");
    assert!(!is_enabled(&fx.xdg));
    write("Hidden=true\n");
    assert!(!is_enabled(&fx.xdg));
    fx.write(
        "home/.config/autostart/dev.soldunov.wye.desktop",
        "not an entry",
    );
    assert!(!is_enabled(&fx.xdg));
}

#[test]
fn a_symlinked_entry_is_left_alone() {
    let fx = Fixture::new();
    let target = fx.write("store/wye.desktop", "[Desktop Entry]\nExec=managed\n");
    let link = entry_path(&fx.xdg);
    std::fs::create_dir_all(link.parent().unwrap()).unwrap();
    symlink(&target, &link).unwrap();
    assert!(matches!(
        enable(&fx.xdg, Path::new("/usr/bin/wye")),
        Err(AutostartError::Managed { .. })
    ));
    assert!(matches!(
        disable(&fx.xdg),
        Err(AutostartError::Managed { .. })
    ));
    assert!(
        std::fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(
        std::fs::read_to_string(&target).unwrap(),
        "[Desktop Entry]\nExec=managed\n"
    );
    // The managed entry still counts as enabled.
    assert!(is_enabled(&fx.xdg));
}
