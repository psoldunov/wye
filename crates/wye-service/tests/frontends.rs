//! The frontends (ADV-12) on a private bus: which of the GNOME Shell
//! picker, the GTK host and the Qt host the service calls, in which order,
//! and how a request moves when a host leaves (PICK-27, PKS-07, PIPE-13).
//! A GNOME session is one where `org.gnome.Shell` has an owner. Skips
//! without `dbus-daemon`.

mod support;

use std::collections::HashMap;
use std::time::Duration;

use support::hosts::{
    Call, PICKER, URL, broken_gtk_windows, cli, fake_ui, gnome_picker, gnome_session, gtk_host,
    gtk_windows, opened_a_window, release_until, shown_count, two, with_frontend, wye,
};
use support::{Service, eventually};
use wye_api::Error;
use wye_api::names::{GNOME_BUS_NAME, GTK_BUS_NAME};
use wye_service::run;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn gnome_prefers_live_shell_and_falls_back_to_qt_without_losing_a_request() {
    let Some(service) = Service::start(PICKER).await else {
        return;
    };
    run::use_environment(&service.ctx, service.desktop.environment_on("ubuntu:GNOME"));
    let _session = gnome_session(&service).await;
    let (qt, _qt_connection) = fake_ui(&service).await;
    let (shell, shell_connection) = gnome_picker(&service).await;
    let proxy = wye(&service).await;
    proxy.open_link(URL, cli()).await.expect("routed");
    shown_count(&shell, 1).await;
    proxy.toggle_menu().await.expect("Shell menu");
    assert!(shell.calls().contains(&Call::Menu));
    assert!(qt.calls().is_empty(), "shell owns the picker and menu");
    let first = shell.shown().remove(0).0;
    release_until(
        &shell_connection,
        GNOME_BUS_NAME,
        "the handover to Qt",
        || async { !qt.shown().is_empty() },
    )
    .await;
    assert_eq!(qt.shown()[0].0, first, "pending request handed to Qt");
    proxy
        .open_link("https://example.com/new", cli())
        .await
        .expect("routed");
    shown_count(&qt, 2).await;
    let second = qt.shown()[1].0.clone();
    assert!(matches!(
        proxy.picker_cancelled(&first).await,
        Err(Error::NotFound(_))
    ));
    proxy
        .picker_chose(&second, &two(), HashMap::new())
        .await
        .expect("chosen");
    assert_eq!(
        service.launched(),
        [vec![
            "fake-two".to_owned(),
            "https://example.com/new".to_owned()
        ]]
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn adv12_auto_on_gnome_without_the_extension_uses_the_gtk_picker() {
    // The extension is off: the GTK host shows the picker and the tray
    // popup as well as the windows, so one toolkit serves all of them.
    let Some(service) = Service::start(PICKER).await else {
        return;
    };
    let _session = gnome_session(&service).await;
    let (qt, _qt_connection) = fake_ui(&service).await;
    let (gtk, _gtk_connection) = gtk_host(&service).await;
    let proxy = wye(&service).await;
    proxy.open_link(URL, cli()).await.expect("routed");
    shown_count(&gtk, 1).await;
    proxy.toggle_menu().await.expect("GTK menu");
    proxy.show_window("settings", "").await.expect("GTK window");
    assert!(gtk.calls().contains(&Call::Menu));
    assert!(opened_a_window(&gtk));
    assert!(qt.calls().is_empty(), "{:?}", qt.calls());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn gnome_windows_use_gtk_and_qt_when_gtk_is_missing() {
    let Some(service) = Service::start(PICKER).await else {
        return;
    };
    let _session = gnome_session(&service).await;
    let (qt, _qt_connection) = fake_ui(&service).await;
    let (gtk, gtk_connection) = gtk_windows(&service).await;
    let proxy = wye(&service).await;
    proxy
        .show_window("settings", "general")
        .await
        .expect("GTK window");
    assert_eq!(gtk.calls().len(), 1);
    assert!(qt.calls().is_empty());
    gtk_connection
        .release_name(GTK_BUS_NAME)
        .await
        .expect("released");
    proxy.show_window("history", "").await.expect("Qt window");
    assert_eq!(qt.calls().len(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn broken_gtk_does_not_silently_switch_to_qt() {
    let Some(service) = Service::start(PICKER).await else {
        return;
    };
    let _session = gnome_session(&service).await;
    let (qt, _qt_connection) = fake_ui(&service).await;
    let _gtk_connection = broken_gtk_windows(&service).await;
    let result = wye(&service).await.show_window("settings", "").await;
    assert!(result.is_err(), "GTK failure must be reported");
    assert!(
        qt.calls().is_empty(),
        "Qt must not hide an installed GTK failure"
    );
}

/// With every host present, Qt alone shows the picker, the menu and the
/// windows. `shell_session`: whether GNOME Shell runs.
async fn only_qt_serves(config: &str, desktop: &str, shell_session: bool) {
    let Some(service) = Service::start(config).await else {
        return;
    };
    run::use_environment(&service.ctx, service.desktop.environment_on(desktop));
    let _session = if shell_session {
        Some(gnome_session(&service).await)
    } else {
        None
    };
    let (qt, _qt_connection) = fake_ui(&service).await;
    let (shell, _shell_connection) = gnome_picker(&service).await;
    let (gtk, _gtk_connection) = gtk_host(&service).await;
    let proxy = wye(&service).await;
    proxy.open_link(URL, cli()).await.expect("routed");
    shown_count(&qt, 1).await;
    proxy.show_window("settings", "").await.expect("Qt window");
    proxy.toggle_menu().await.expect("Qt menu");
    assert!(qt.calls().contains(&Call::Menu));
    assert!(opened_a_window(&qt));
    assert!(shell.calls().is_empty());
    assert!(gtk.calls().is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn kde_keeps_qt_even_when_gnome_hosts_are_present() {
    only_qt_serves(PICKER, "KDE", false).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn adv12_auto_keeps_qt_on_desktops_that_only_name_gnome() {
    // Budgie and GNOME Flashback put GNOME in XDG_CURRENT_DESKTOP but run
    // no GNOME Shell: Automatic keeps Qt there, as before ADV-12.
    only_qt_serves(PICKER, "Budgie:GNOME", false).await;
    only_qt_serves(PICKER, "GNOME-Flashback:GNOME", false).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn adv12_kde_frontend_uses_qt_on_gnome() {
    only_qt_serves(&with_frontend("kde"), "GNOME", true).await;
}

/// The one host that runs, of the frontend not chosen.
#[derive(Clone, Copy)]
enum Other {
    Gtk,
    Qt,
}

/// ADV-12: without its own frontend, the chosen one hands the picker and
/// the windows to the other one.
async fn the_other_frontend_takes_over(frontend: &str, other: Other) {
    let Some(service) = Service::start(&with_frontend(frontend)).await else {
        return;
    };
    run::use_environment(&service.ctx, service.desktop.environment_on("KDE"));
    let (ui, _connection) = match other {
        Other::Gtk => gtk_host(&service).await,
        Other::Qt => fake_ui(&service).await,
    };
    let proxy = wye(&service).await;
    proxy.open_link(URL, cli()).await.expect("routed");
    shown_count(&ui, 1).await;
    proxy.show_window("settings", "").await.expect("a window");
    assert!(opened_a_window(&ui));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn adv12_kde_frontend_falls_back_to_gnome_without_qt() {
    the_other_frontend_takes_over("kde", Other::Gtk).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn adv12_gnome_frontend_without_gtk_uses_qt() {
    the_other_frontend_takes_over("gnome", Other::Qt).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn adv12_gnome_frontend_uses_shell_then_gtk_on_kde() {
    let Some(service) = Service::start(&with_frontend("gnome")).await else {
        return;
    };
    run::use_environment(&service.ctx, service.desktop.environment_on("KDE"));
    let (qt, _qt_connection) = fake_ui(&service).await;
    let (gtk, _gtk_connection) = gtk_host(&service).await;
    let (shell, shell_connection) = gnome_picker(&service).await;
    let proxy = wye(&service).await;
    proxy.open_link(URL, cli()).await.expect("routed");
    shown_count(&shell, 1).await;
    proxy.show_window("settings", "").await.expect("GTK window");
    assert_eq!(gtk.calls().len(), 1, "windows are GTK's");
    let first = shell.shown().remove(0).0;
    release_until(
        &shell_connection,
        GNOME_BUS_NAME,
        "the handover to GTK",
        || async { !gtk.shown().is_empty() },
    )
    .await;
    assert_eq!(gtk.shown()[0].0, first, "pending request handed to GTK");
    proxy.toggle_menu().await.expect("GTK menu");
    assert!(gtk.calls().contains(&Call::Menu));
    assert!(qt.calls().is_empty(), "Qt is only the last resort");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn adv12_gnome_frontend_falls_back_to_qt_when_gtk_has_no_picker() {
    let Some(service) = Service::start(&with_frontend("gnome")).await else {
        return;
    };
    run::use_environment(&service.ctx, service.desktop.environment_on("sway"));
    let (qt, _qt_connection) = fake_ui(&service).await;
    // A GTK host that serves windows only: `UnknownInterface` for the picker.
    let (gtk, _gtk_connection) = gtk_windows(&service).await;
    let proxy = wye(&service).await;
    proxy.open_link(URL, cli()).await.expect("routed");
    shown_count(&qt, 1).await;
    proxy.toggle_menu().await.expect("Qt menu");
    assert!(qt.calls().contains(&Call::Menu));
    proxy.show_window("settings", "").await.expect("GTK window");
    assert_eq!(gtk.calls().len(), 1, "windows stay GTK's");
    assert!(!opened_a_window(&qt));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn adv12_a_frontend_change_applies_to_the_next_window() {
    let Some(service) = Service::start(PICKER).await else {
        return;
    };
    run::use_environment(&service.ctx, service.desktop.environment_on("KDE"));
    let (qt, _qt_connection) = fake_ui(&service).await;
    let (gtk, _gtk_connection) = gtk_host(&service).await;
    let proxy = wye(&service).await;
    proxy.show_window("settings", "").await.expect("Qt window");
    assert_eq!(qt.calls().len(), 1);
    service
        .desktop
        .replace("config/wye/config.toml", &with_frontend("gnome"));
    eventually("the next window to open on GTK", || async {
        proxy.show_window("about", "").await.expect("a window");
        !gtk.calls().is_empty()
    })
    .await;
    proxy.open_link(URL, cli()).await.expect("routed");
    shown_count(&gtk, 1).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn adv12_closing_reaches_every_running_host() {
    // PKS-07 with the GNOME frontend: Shell shows the picker; Qt may hold an
    // older request and is closed too; a GTK host without a picker has none
    // to close and does not fail the call.
    let Some(service) = Service::start(&with_frontend("gnome")).await else {
        return;
    };
    run::use_environment(&service.ctx, service.desktop.environment_on("KDE"));
    let (qt, _qt_connection) = fake_ui(&service).await;
    let (gtk, _gtk_connection) = gtk_windows(&service).await;
    let (shell, _shell_connection) = gnome_picker(&service).await;
    let proxy = wye(&service).await;
    proxy.open_link(URL, cli()).await.expect("routed");
    shown_count(&shell, 1).await;
    let first = shell.shown().remove(0).0;

    service.fakes.lock.set(true);
    eventually("the picker to close everywhere", || async {
        shell.closed(&first) && qt.closed(&first)
    })
    .await;
    service.fakes.lock.set(false);
    shown_count(&shell, 2).await;
    assert!(qt.shown().is_empty(), "Shell keeps the picker");
    assert!(gtk.calls().is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn pks07_after_the_unlock_the_link_waits_for_the_shell_picker() {
    // GNOME switches its extensions off while the screen is locked: the
    // Shell picker leaves the bus with the lock and comes back a moment
    // after the unlock. Nothing shows on Qt meanwhile; the held link's
    // picker waits for the Shell.
    let Some(service) = Service::start(PICKER).await else {
        return;
    };
    let _session = gnome_session(&service).await;
    let (qt, _qt_connection) = fake_ui(&service).await;
    let (shell, shell_connection) = gnome_picker(&service).await;
    let proxy = wye(&service).await;
    proxy.open_link(URL, cli()).await.expect("routed");
    shown_count(&shell, 1).await;
    let first = shell.shown().remove(0).0;

    service.fakes.lock.set(true);
    eventually("the picker to close", || async { shell.closed(&first) }).await;
    shell_connection
        .release_name(GNOME_BUS_NAME)
        .await
        .expect("extension off");
    service.fakes.lock.set(false);
    tokio::time::sleep(Duration::from_millis(300)).await;
    shell_connection
        .request_name(GNOME_BUS_NAME)
        .await
        .expect("extension on again");
    shown_count(&shell, 2).await;
    let (second, request) = shell.shown().remove(1);
    assert_ne!(first, second);
    assert_eq!(request.url.full, URL);
    assert!(qt.shown().is_empty(), "{:?}", qt.calls());
    assert!(service.launched().is_empty(), "no stand-in");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn pick27_a_request_replaced_on_another_host_closes_there() {
    // The GNOME frontend without the Shell: GTK shows the first request.
    // Then the Shell comes and shows the next one; GTK's picker would stay
    // behind with a request nobody answers, so it is closed.
    let Some(service) = Service::start(&with_frontend("gnome")).await else {
        return;
    };
    let (gtk, _gtk_connection) = gtk_host(&service).await;
    let proxy = wye(&service).await;
    proxy
        .open_link("https://example.com/old", cli())
        .await
        .expect("routed");
    shown_count(&gtk, 1).await;
    let old = gtk.shown().remove(0).0;
    let (shell, _shell_connection) = gnome_picker(&service).await;
    proxy.open_link(URL, cli()).await.expect("routed");
    shown_count(&shell, 1).await;
    eventually("the old picker to close on GTK", || async {
        gtk.closed(&old)
    })
    .await;
    let new = shell.shown().remove(0).0;
    assert!(!shell.closed(&new));
    assert_eq!(gtk.shown().len(), 1, "GTK never shows the new request");
}
