use super::*;
use crate::test_support::{CHROME, FIREFOX, Fixture};

const URL: &str = "https://example.com/";

fn id(value: &str) -> DesktopId {
    DesktopId::new(value).unwrap()
}

fn inventory(fx: &Fixture) -> Inventory {
    Inventory::scan(&fx.xdg, &id("wye-test"))
}

fn build(inventory: &Inventory, target: &Target) -> Result<LaunchCommand, LaunchError> {
    build_command(
        inventory,
        &LaunchRequest {
            target,
            url: URL,
            background: false,
            new_window: false,
        },
    )
}

fn argv(command: &LaunchCommand) -> Vec<&str> {
    std::iter::once(command.program.as_str())
        .chain(command.args.iter().map(String::as_str))
        .collect()
}

fn browsers() -> Fixture {
    let fx = Fixture::new();
    fx.system_entry("firefox.desktop", FIREFOX);
    fx.system_entry("google-chrome.desktop", CHROME);
    fx.system_entry(
        "org.mozilla.firefox.desktop",
        "[Desktop Entry]\nName=Firefox (Flatpak)\nType=Application\n\
         Exec=/usr/bin/flatpak run --branch=stable --arch=x86_64 --command=firefox \
         --file-forwarding org.mozilla.firefox @@u %u @@\n\
         MimeType=x-scheme-handler/https;\n",
    );
    fx.system_entry(
        "term.desktop",
        "[Desktop Entry]\nName=Lynx\nType=Application\nExec=lynx %u\nTerminal=true\n",
    );
    fx.system_entry(
        "epiphany.desktop",
        "[Desktop Entry]\nName=Web\nType=Application\nExec=epiphany %U\n\
         MimeType=x-scheme-handler/https;\n",
    );
    fx.write(
        "home/.config/google-chrome/Local State",
        r#"{"profile":{"info_cache":{"Profile 1":{"name":"Work"}}}}"#,
    );
    fx.write(
        "home/.mozilla/firefox/profiles.ini",
        "[Profile0]\nName=main\nIsRelative=1\nPath=Profiles/abc.main\n",
    );
    fx.write(
        "home/.var/app/org.mozilla.firefox/.mozilla/firefox/profiles.ini",
        "[Profile0]\nName=flat\nIsRelative=1\nPath=xyz.flat\n",
    );
    fx
}

#[test]
fn launches_apps_through_exec() {
    let fx = browsers();
    let inv = inventory(&fx);
    let command = build(&inv, &Target::App(id("firefox"))).unwrap();
    assert_eq!(argv(&command), vec!["firefox", "--name", "firefox", URL]);
    assert!(command.remove_env.is_empty());

    let custom = build(
        &inv,
        &Target::Custom(CustomApp::Desktop(id("google-chrome"))),
    )
    .unwrap();
    assert_eq!(argv(&custom), vec!["/usr/bin/google-chrome-stable", URL]);

    let exe = build(
        &inv,
        &Target::Custom(CustomApp::Executable("/opt/x".into())),
    )
    .unwrap();
    assert_eq!(argv(&exe), vec!["/opt/x", URL]);
}

#[test]
fn opens_private_windows() {
    let fx = browsers();
    let inv = inventory(&fx);
    let firefox = build(&inv, &Target::Private(id("firefox"))).unwrap();
    assert_eq!(argv(&firefox), vec!["firefox", "--private-window", URL]);
    let chrome = build(&inv, &Target::Private(id("google-chrome"))).unwrap();
    assert_eq!(
        argv(&chrome),
        vec!["/usr/bin/google-chrome-stable", "--incognito", URL]
    );
    assert_eq!(
        build(&inv, &Target::Private(id("epiphany"))),
        Err(LaunchError::NoPrivateMode(id("epiphany")))
    );
}

#[test]
fn opens_profiles() {
    let fx = browsers();
    let inv = inventory(&fx);
    let profile = |app: &str, profile: &str| Target::Profile {
        app: id(app),
        id: profile.into(),
    };
    let chrome = build(&inv, &profile("google-chrome", "Profile 1")).unwrap();
    assert_eq!(
        argv(&chrome),
        vec![
            "/usr/bin/google-chrome-stable",
            "--profile-directory=Profile 1",
            URL
        ]
    );

    let firefox = build(&inv, &profile("firefox", "Profiles/abc.main")).unwrap();
    let path = fx.xdg.home.join(".mozilla/firefox/Profiles/abc.main");
    assert_eq!(
        argv(&firefox),
        vec![
            "firefox",
            "--name",
            "firefox",
            "--profile",
            path.to_str().unwrap(),
            URL
        ]
    );

    let flatpak = build(&inv, &profile("org.mozilla.firefox", "xyz.flat")).unwrap();
    let path = fx
        .xdg
        .home
        .join(".var/app/org.mozilla.firefox/.mozilla/firefox/xyz.flat");
    let tail: Vec<&str> = argv(&flatpak).into_iter().rev().take(6).rev().collect();
    assert_eq!(
        tail,
        vec![
            "org.mozilla.firefox",
            "--profile",
            path.to_str().unwrap(),
            "@@u",
            URL,
            "@@"
        ]
    );

    assert!(matches!(
        build(&inv, &profile("firefox", "missing")),
        Err(LaunchError::UnknownProfile { .. })
    ));
    assert_eq!(
        build(&inv, &profile("epiphany", "x")),
        Err(LaunchError::NoProfileSupport(id("epiphany")))
    );
}

#[test]
fn applies_new_window_and_background() {
    let fx = browsers();
    let inv = inventory(&fx);
    let request = |target| LaunchRequest {
        target,
        url: URL,
        background: true,
        new_window: true,
    };
    let chrome_target = Target::App(id("google-chrome"));
    let chrome = build_command(&inv, &request(&chrome_target)).unwrap();
    assert_eq!(
        argv(&chrome),
        vec!["/usr/bin/google-chrome-stable", "--new-window", URL]
    );
    assert_eq!(
        chrome.remove_env,
        vec!["XDG_ACTIVATION_TOKEN", "DESKTOP_STARTUP_ID"]
    );

    let web_target = Target::App(id("epiphany"));
    let web = build_command(&inv, &request(&web_target)).unwrap();
    assert_eq!(argv(&web), vec!["epiphany", URL]);
}

#[test]
fn refuses_unlaunchable_targets() {
    let fx = browsers();
    let inv = inventory(&fx);
    assert!(matches!(
        build(&inv, &Target::Picker),
        Err(LaunchError::NotConcrete(_))
    ));
    assert!(matches!(
        build(&inv, &Target::Default),
        Err(LaunchError::NotConcrete(_))
    ));
    assert_eq!(
        build(&inv, &Target::App(id(WYE_DESKTOP_ID))),
        Err(LaunchError::SelfLaunch)
    );
    assert_eq!(
        build(&inv, &Target::App(id("missing"))),
        Err(LaunchError::NotInstalled(id("missing")))
    );
    assert_eq!(
        build(&inv, &Target::App(id("term"))),
        Err(LaunchError::Terminal(id("term")))
    );
}

#[test]
fn spawns_detached_processes() {
    let command = LaunchCommand {
        program: "true".into(),
        args: vec![URL.into()],
        remove_env: ACTIVATION_ENV.to_vec(),
    };
    spawn(&command).unwrap();
    let missing = LaunchCommand {
        program: "/nonexistent/wye-test-program".into(),
        args: vec![],
        remove_env: vec![],
    };
    assert!(spawn(&missing).is_err());
}
