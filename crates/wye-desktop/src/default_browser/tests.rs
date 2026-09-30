use std::os::unix::fs::{PermissionsExt, symlink};

use super::*;
use crate::test_support::Fixture;

fn id(value: &str) -> DesktopId {
    DesktopId::new(value).unwrap()
}

fn wye() -> DesktopId {
    id("dev.soldunov.wye")
}

/// Installs a minimal desktop entry for each ID, so lookups find them.
fn install(fx: &Fixture, ids: &[&str]) {
    for id in ids {
        fx.system_entry(
            &format!("{id}.desktop"),
            &format!("[Desktop Entry]\nType=Application\nName={id}\nExec={id} %u\n"),
        );
    }
}

#[test]
fn follows_lookup_order() {
    let fx = Fixture::new();
    install(&fx, &["chromium", "firefox", "brave", "google-chrome"]);
    fx.write(
        "sys/applications/mimeapps.list",
        "[Default Applications]\nx-scheme-handler/https=chromium.desktop\n",
    );
    assert_eq!(current_default(&fx.xdg), Some(id("chromium")));

    fx.write(
        "etc/xdg/mimeapps.list",
        "[Default Applications]\nx-scheme-handler/https=bad id;firefox.desktop;\n",
    );
    assert_eq!(current_default(&fx.xdg), Some(id("firefox")));

    fx.write(
        "home/.config/mimeapps.list",
        "[Added Associations]\nx-scheme-handler/https=brave.desktop\n\
         [Default Applications]\ntext/html=brave.desktop\n",
    );
    assert_eq!(current_default(&fx.xdg), Some(id("firefox")));

    fx.write(
        "home/.config/gnome-mimeapps.list",
        "[Default Applications]\nx-scheme-handler/https=google-chrome.desktop\n",
    );
    assert_eq!(current_default(&fx.xdg), Some(id("google-chrome")));
    assert_eq!(default_for(&fx.xdg, "text/html"), Some(id("brave")));
    assert_eq!(default_for(&fx.xdg, "x-scheme-handler/mailto"), None);
}

#[test]
fn sets_default_preserving_other_lines() {
    let fx = Fixture::new();
    let original = "# managed by hand\n[Added Associations]\nx-scheme-handler/http=firefox.desktop;\n\n\
                    [Default Applications]\n# browsers\nx-scheme-handler/http=firefox.desktop\n\
                    image/png=eog.desktop\n\n[Removed Associations]\ntext/plain=x.desktop\n";
    let path = fx.write("home/.config/mimeapps.list", original);
    install(&fx, &["dev.soldunov.wye"]);

    set_default(&fx.xdg, &wye(), false).unwrap();
    let text = fs::read_to_string(&path).unwrap();
    assert_eq!(
        text,
        "# managed by hand\n[Added Associations]\nx-scheme-handler/http=firefox.desktop;\n\n\
         [Default Applications]\n# browsers\nx-scheme-handler/http=dev.soldunov.wye.desktop\n\
         image/png=eog.desktop\nx-scheme-handler/https=dev.soldunov.wye.desktop\n\n\
         [Removed Associations]\ntext/plain=x.desktop\n"
    );
    assert!(is_default(&fx.xdg, &wye()));
    assert_eq!(default_for(&fx.xdg, "text/html"), None);
}

#[test]
fn creates_file_and_group() {
    let fx = Fixture::new();
    set_default(&fx.xdg, &wye(), true).unwrap();
    let path = fx.xdg.config_home.join("mimeapps.list");
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        "[Default Applications]\n\
         x-scheme-handler/http=dev.soldunov.wye.desktop\n\
         x-scheme-handler/https=dev.soldunov.wye.desktop\n\
         text/html=dev.soldunov.wye.desktop\n\
         application/xhtml+xml=dev.soldunov.wye.desktop\n\n\
         [Added Associations]\n\
         text/html=dev.soldunov.wye.desktop;\n\
         application/xhtml+xml=dev.soldunov.wye.desktop;\n",
        "DEF-07: KService needs the association too"
    );
    remove_html_association(&fx.xdg, &wye()).unwrap();
    assert!(
        !fs::read_to_string(&path)
            .unwrap()
            .contains("[Added Associations]\ntext/html"),
        "removed again"
    );

    fs::write(&path, "[Added Associations]\ntext/html=a.desktop").unwrap();
    set_default(&fx.xdg, &wye(), false).unwrap();
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        "[Added Associations]\ntext/html=a.desktop\n\n[Default Applications]\n\
         x-scheme-handler/http=dev.soldunov.wye.desktop\n\
         x-scheme-handler/https=dev.soldunov.wye.desktop\n"
    );
}

#[test]
fn updates_shadowing_desktop_file() {
    let fx = Fixture::new();
    let gnome = fx.write(
        "home/.config/gnome-mimeapps.list",
        "[Default Applications]\nx-scheme-handler/https=google-chrome.desktop\nimage/png=eog.desktop\n",
    );
    let unrelated = fx.write(
        "home/.config/kde-mimeapps.list",
        "[Default Applications]\nx-scheme-handler/https=konqueror.desktop\n",
    );
    install(&fx, &["dev.soldunov.wye"]);
    set_default(&fx.xdg, &wye(), false).unwrap();
    assert_eq!(
        fs::read_to_string(&gnome).unwrap(),
        "[Default Applications]\nx-scheme-handler/https=dev.soldunov.wye.desktop\nimage/png=eog.desktop\n"
    );
    assert!(
        fs::read_to_string(&unrelated)
            .unwrap()
            .contains("konqueror")
    );
    assert!(is_default(&fx.xdg, &wye()));
}

#[test]
fn refuses_managed_files() {
    let fx = Fixture::new();
    let store = fx.write("nix/store/mimeapps.list", "[Default Applications]\n");
    fs::create_dir_all(&fx.xdg.config_home).unwrap();
    let link = fx.xdg.config_home.join("mimeapps.list");
    symlink(&store, &link).unwrap();
    assert!(matches!(
        set_default(&fx.xdg, &wye(), false),
        Err(DefaultBrowserError::Managed { .. })
    ));
    assert!(
        fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(
        fs::read_to_string(&store).unwrap(),
        "[Default Applications]\n"
    );

    fs::remove_file(&link).unwrap();
    fs::write(&link, "").unwrap();
    fs::set_permissions(&link, fs::Permissions::from_mode(0o444)).unwrap();
    assert!(matches!(
        set_default(&fx.xdg, &wye(), false),
        Err(DefaultBrowserError::Managed { .. })
    ));
}

#[test]
fn keeps_permissions_and_leaves_no_temp_files() {
    let fx = Fixture::new();
    let path = fx.write("home/.config/mimeapps.list", "");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
    set_default(&fx.xdg, &wye(), false).unwrap();
    let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o640);
    let entries: Vec<_> = fs::read_dir(&fx.xdg.config_home).unwrap().collect();
    assert_eq!(entries.len(), 1);
}

#[test]
fn set_keys_handles_missing_trailing_newline() {
    assert_eq!(set_keys("[G]\na=1", "G", &[("b", "2")]), "[G]\na=1\nb=2\n");
    assert_eq!(set_keys("[G]\na=1", "G", &[("a", "2")]), "[G]\na=2");
    assert_eq!(set_keys("", "G", &[("a", "1")]), "[G]\na=1\n");
}

#[test]
fn skips_listed_apps_that_are_not_installed() {
    let fx = Fixture::new();
    install(&fx, &["firefox", "chromium"]);
    fx.user_entry("hidden.desktop", "[Desktop Entry]\nHidden=true\n");
    fx.write(
        "home/.config/mimeapps.list",
        "[Default Applications]\n\
         x-scheme-handler/https=removed.desktop;hidden.desktop;firefox.desktop;\n\
         x-scheme-handler/http=removed.desktop;\n",
    );
    assert_eq!(current_default(&fx.xdg), Some(id("firefox")));
    // Only missing apps in the first file: the next file decides.
    assert_eq!(default_for(&fx.xdg, HTTP), None);
    fx.write(
        "etc/xdg/mimeapps.list",
        "[Default Applications]\nx-scheme-handler/http=chromium.desktop\n",
    );
    assert_eq!(default_for(&fx.xdg, HTTP), Some(id("chromium")));
}

#[test]
fn listed_default_ignores_whether_the_entry_is_installed() {
    let fx = Fixture::new();
    install(&fx, &["firefox"]);
    fx.write(
        "home/.config/mimeapps.list",
        "[Default Applications]\n\
         x-scheme-handler/https=dev.soldunov.wye.desktop;firefox.desktop;\n\
         x-scheme-handler/http=dev.soldunov.wye.desktop\n",
    );
    // The installed filter skips the missing Wye entry; the listed one does not.
    assert_eq!(current_default(&fx.xdg), Some(id("firefox")));
    assert_eq!(listed_default(&fx.xdg), Some(wye()));
    assert_eq!(listed_default_for(&fx.xdg, HTTP), Some(wye()));
    assert!(is_default(&fx.xdg, &wye()));
    assert_eq!(listed_default_for(&fx.xdg, "text/html"), None);
    // Invalid IDs are still skipped, and lookup order still applies.
    fx.write(
        "home/.config/gnome-mimeapps.list",
        "[Default Applications]\nx-scheme-handler/https=bad id;firefox.desktop\n",
    );
    assert_eq!(listed_default(&fx.xdg), Some(id("firefox")));
}
