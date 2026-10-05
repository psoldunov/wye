use std::fs;

use super::*;
use crate::test_support::{CHROME, EDITOR, FIREFOX, Fixture};

fn id(value: &str) -> DesktopId {
    DesktopId::new(value).unwrap()
}

fn wye() -> DesktopId {
    id("dev.soldunov.wye")
}

fn ids(inventory: &Inventory) -> Vec<&str> {
    inventory.apps().map(|app| app.id().as_str()).collect()
}

#[test]
fn scans_nested_dirs_and_respects_precedence() {
    let fx = Fixture::new();
    fx.system_entry("firefox.desktop", FIREFOX);
    fx.user_entry(
        "firefox.desktop",
        &FIREFOX.replace("Name=Firefox", "Name=My Firefox"),
    );
    fx.system_entry("kde4/editor.desktop", EDITOR);
    fx.system_entry("dev.soldunov.wye.desktop", CHROME);
    fx.system_entry("notes.txt", EDITOR);

    let inventory = Inventory::scan(&fx.xdg, &wye());
    assert_eq!(
        ids(&inventory),
        vec!["firefox.desktop", "kde4-editor.desktop"]
    );
    assert_eq!(
        inventory.get(&id("firefox")).unwrap().entry.name,
        "My Firefox"
    );
    assert!(inventory.warnings().is_empty());
}

#[test]
fn applies_visibility_rules() {
    let fx = Fixture::new();
    let base = "[Desktop Entry]\nName=X\nExec=x\n";
    fx.user_entry("hidden-app.desktop", "[Desktop Entry]\nHidden=true\n");
    fx.system_entry("hidden-app.desktop", &format!("{base}Type=Application\n"));
    fx.system_entry("link.desktop", &format!("{base}Type=Link\n"));
    fx.system_entry(
        "nodisplay.desktop",
        &format!("{base}Type=Application\nNoDisplay=true\n"),
    );
    fx.system_entry(
        "tryexec-missing.desktop",
        &format!("{base}Type=Application\nTryExec=nope\n"),
    );
    fx.system_entry(
        "tryexec-ok.desktop",
        &format!("{base}Type=Application\nTryExec=tool\n"),
    );
    fx.system_entry(
        "kde-only.desktop",
        &format!("{base}Type=Application\nOnlyShowIn=KDE;\n"),
    );
    fx.system_entry(
        "not-gnome.desktop",
        &format!("{base}Type=Application\nNotShowIn=GNOME;\n"),
    );
    fx.system_entry("broken.desktop", "no group here\n");
    fx.program("tool");

    let inventory = Inventory::scan(&fx.xdg, &wye());
    assert_eq!(ids(&inventory), vec!["tryexec-ok.desktop"]);
    assert_eq!(inventory.warnings().len(), 1);
}

#[test]
fn classifies_browsers() {
    let fx = Fixture::new();
    fx.system_entry("firefox.desktop", FIREFOX);
    fx.system_entry("google-chrome.desktop", CHROME);
    fx.system_entry("editor.desktop", EDITOR);
    fx.system_entry(
        "chrome-abc-Default.desktop",
        "[Desktop Entry]\nName=Web App\nType=Application\nExec=google-chrome --app-id=abc\n",
    );
    fx.write(
        "home/.config/google-chrome/Local State",
        r#"{"profile":{"info_cache":{"Default":{"name":"Me"}}}}"#,
    );

    let inventory = Inventory::scan(&fx.xdg, &wye());
    let firefox = inventory.get(&id("firefox")).unwrap();
    assert_eq!(firefox.family, BrowserFamily::Firefox);
    assert_eq!(
        firefox.private,
        Some(PrivateMode::Action("new-private-window".into()))
    );
    assert!(firefox.profiles.is_empty());

    let chrome = inventory.get(&id("google-chrome")).unwrap();
    assert_eq!(chrome.family, BrowserFamily::Chromium);
    assert_eq!(chrome.private, Some(PrivateMode::Flag("--incognito")));
    assert_eq!(chrome.profiles.len(), 1);
    assert_eq!(chrome.packaging, Packaging::Native);

    let editor = inventory.get(&id("editor")).unwrap();
    assert!(!editor.handles_web && editor.private.is_none());
    let web_app = inventory.get(&id("chrome-abc-Default")).unwrap();
    assert_eq!(web_app.family, BrowserFamily::Other);

    let names: Vec<&str> = inventory
        .web_handlers()
        .iter()
        .map(|app| app.entry.name.as_str())
        .collect();
    assert_eq!(names, vec!["Firefox", "Google Chrome"]);
}

#[test]
fn reports_availability() {
    let fx = Fixture::new();
    fx.system_entry("firefox.desktop", FIREFOX);
    fx.system_entry("editor.desktop", EDITOR);
    fx.write(
        "home/.mozilla/firefox/profiles.ini",
        "[Profile0]\nName=main\nIsRelative=1\nPath=abc.main\n",
    );
    let tool = fx.program("tool");
    let inventory = Inventory::scan(&fx.xdg, &wye());

    let profile = |app: &str, profile: &str| Target::Profile {
        app: id(app),
        id: profile.into(),
    };
    let exe = |value: &str| Target::Custom(CustomApp::Executable(value.into()));
    let available = [
        Target::Picker,
        Target::Default,
        Target::App(id("firefox")),
        Target::Custom(CustomApp::Desktop(id("editor"))),
        Target::Private(id("firefox")),
        profile("firefox", "abc.main"),
        exe("tool"),
        exe(tool.to_str().unwrap()),
    ];
    let unavailable = [
        Target::App(id("chromium")),
        Target::Private(id("editor")),
        profile("firefox", "other"),
        exe("missing"),
        exe("/nonexistent/tool"),
    ];
    for target in available {
        assert!(inventory.is_available(&target), "{target}");
    }
    for target in unavailable {
        assert!(!inventory.is_available(&target), "{target}");
    }
}

// DEF-08, DEF-09: only apps that handle web links count as browsers.
#[test]
fn tells_browsers_from_other_apps() {
    let fx = Fixture::new();
    fx.system_entry("firefox.desktop", FIREFOX);
    fx.system_entry("editor.desktop", EDITOR);
    let inventory = Inventory::scan(&fx.xdg, &wye());

    let profile = Target::Profile {
        app: id("firefox"),
        id: "abc.main".into(),
    };
    let exe = Target::Custom(CustomApp::Executable("tool".into()));
    let browsers = [
        Target::App(id("firefox")),
        Target::Custom(CustomApp::Desktop(id("firefox"))),
        Target::Private(id("firefox")),
        profile,
    ];
    let others = [
        Target::Picker,
        Target::Default,
        Target::App(id("editor")),
        Target::Custom(CustomApp::Desktop(id("editor"))),
        Target::App(id("missing")),
        exe,
    ];
    for target in browsers {
        assert!(inventory.is_browser(&target), "{target}");
    }
    for target in others {
        assert!(!inventory.is_browser(&target), "{target}");
    }
}

#[test]
fn finds_single_entries() {
    let fx = Fixture::new();
    fx.system_entry("firefox.desktop", FIREFOX);
    fx.system_entry("org.kde/konsole.desktop", EDITOR);
    fx.system_entry("gone.desktop", EDITOR);
    fx.user_entry("gone.desktop", "[Desktop Entry]\nHidden=true\n");
    fs::create_dir_all(fx.path("sys/applications/firefox")).unwrap();

    assert_eq!(find_entry(&fx.xdg, &id("firefox")).unwrap().name, "Firefox");
    assert_eq!(
        find_entry(&fx.xdg, &id("org.kde-konsole")).unwrap().name,
        "Editor"
    );
    assert!(find_entry(&fx.xdg, &id("gone")).is_none());
    assert!(find_entry(&fx.xdg, &id("missing")).is_none());
}

#[test]
fn openers_are_not_web_handlers_or_targets() {
    // DEF-06: these would send the link straight back to Wye.
    let fx = Fixture::new();
    fx.system_entry("firefox.desktop", FIREFOX);
    let web = "Type=Application\nMimeType=x-scheme-handler/http;x-scheme-handler/https;\n";
    fx.system_entry(
        "opener.desktop",
        &format!("[Desktop Entry]\nName=Opener\nExec=xdg-open %u\n{web}"),
    );
    fx.system_entry(
        "old-wye.desktop",
        &format!("[Desktop Entry]\nName=Old Wye\nExec=/usr/local/bin/wye open %U\n{web}"),
    );
    fx.system_entry(
        "wrapped.desktop",
        &format!("[Desktop Entry]\nName=Wrapped\nExec=env A=1 gio open %u\n{web}"),
    );
    fx.program("xdg-open");
    let inventory = Inventory::scan(&fx.xdg, &wye());

    let handlers: Vec<&str> = inventory
        .web_handlers()
        .iter()
        .map(|app| app.id().as_str())
        .collect();
    assert_eq!(handlers, vec!["firefox.desktop"]);
    for name in ["opener", "old-wye", "wrapped"] {
        let app = inventory.get(&id(name)).unwrap();
        assert!(app.forwards_links && !app.handles_web, "{name}");
        assert!(app.private.is_none() && app.profiles.is_empty(), "{name}");
        assert!(!inventory.is_available(&Target::App(id(name))), "{name}");
        assert!(
            !inventory.is_available(&Target::Custom(CustomApp::Desktop(id(name)))),
            "{name}"
        );
    }
    let exe = |value: &str| Target::Custom(CustomApp::Executable(value.into()));
    assert!(!inventory.is_available(&exe("xdg-open")));
    let current = std::env::current_exe().unwrap();
    assert!(!inventory.is_available(&exe(current.to_str().unwrap())));
    assert!(!inventory.get(&id("firefox")).unwrap().forwards_links);
}

#[test]
fn ids_never_resolve_outside_applications() {
    let fx = Fixture::new();
    // `sys/escape.desktop` sits beside `sys/applications`, not inside it.
    fx.write("sys/escape.desktop", EDITOR);
    fx.write("sys/applications/sub/ok.desktop", EDITOR);
    fs::create_dir_all(fx.path("sys/applications/sub/nested")).unwrap();
    assert!(find_entry(&fx.xdg, &id("..-escape")).is_none());
    assert!(find_entry(&fx.xdg, &id("sub-..-..-escape")).is_none());
    assert!(find_entry(&fx.xdg, &id(".-sub-ok")).is_none());
    assert!(find_entry(&fx.xdg, &id("sub--ok")).is_none());
    assert!(find_entry(&fx.xdg, &id("sub-nested-..-ok")).is_none());
    assert_eq!(find_entry(&fx.xdg, &id("sub-ok")).unwrap().name, "Editor");
}

// DISC-03, DISC-08, TGT-02: the inventory as a target catalogue.
#[test]
fn the_catalogue_carries_localised_names_badges_and_abilities() {
    let fx = Fixture::new();
    fx.system_entry(
        "firefox.desktop",
        &FIREFOX.replace("Name=Firefox", "Name=Firefox\nName[de]=Feuerfuchs"),
    );
    fx.system_entry("google-chrome.desktop", CHROME);
    fx.system_entry(
        "spotify.desktop",
        "[Desktop Entry]\nName=Spotify\nName[de]=Spotify DE\nIcon=spotify\nType=Application\nExec=spotify %U\n",
    );
    fx.system_entry("editor.desktop", EDITOR);
    fs::create_dir_all(fx.xdg.home.join(".config/google-chrome")).unwrap();
    fx.write(
        "home/.config/google-chrome/Local State",
        r#"{"profile":{"info_cache":{"Profile 1":{"name":"Work"}}}}"#,
    );
    let inventory = Inventory::scan(&fx.xdg, &wye());
    let german = Locale::parse("de_DE.UTF-8").unwrap();
    let catalog = inventory.catalog(&german, &[id("spotify"), id("missing"), id("firefox")]);

    let names: Vec<_> = catalog.handlers.iter().map(|h| h.name.as_str()).collect();
    assert_eq!(
        names,
        ["Feuerfuchs", "Google Chrome"],
        "sorted by the localised name"
    );
    let firefox = &catalog.handlers[0];
    assert!(firefox.browser && firefox.private && firefox.new_window);
    assert_eq!(firefox.icon.as_deref(), Some("firefox"));
    assert!(firefox.profiles.is_empty());
    let chrome = &catalog.handlers[1];
    assert_eq!(chrome.profiles.len(), 1);
    assert_eq!(chrome.profiles[0].name, "Work");
    assert!(
        chrome.profiles[0].badge.is_some(),
        "every profile has a badge"
    );

    // Only installed non-handlers are added; a handler is already a handler.
    let extra: Vec<_> = catalog.apps.iter().map(|a| a.name.as_str()).collect();
    assert_eq!(extra, ["Spotify DE"]);
    assert!(catalog.custom.is_empty());

    // Without a locale the plain names are used.
    let plain = inventory.catalog(&Locale::none(), &[]);
    assert_eq!(plain.handlers[0].name, "Firefox");
    assert!(plain.apps.is_empty());
}

#[test]
fn web_handlers_sort_by_their_name_in_the_locale() {
    let fx = Fixture::new();
    fx.system_entry(
        "a.desktop",
        &FIREFOX.replace("Name=Firefox", "Name=Alpha\nName[de]=Zeta"),
    );
    fx.system_entry("b.desktop", &CHROME.replace("Google Chrome", "Beta"));
    let inventory = Inventory::scan(&fx.xdg, &wye());
    let order = |locale: &Locale| -> Vec<String> {
        inventory
            .web_handlers_in(locale)
            .into_iter()
            .map(|(_, name)| name)
            .collect()
    };
    assert_eq!(order(&Locale::none()), ["Alpha", "Beta"]);
    assert_eq!(order(&Locale::parse("de").unwrap()), ["Beta", "Zeta"]);
}
