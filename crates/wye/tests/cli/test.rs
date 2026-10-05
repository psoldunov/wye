//! `wye test`, the dry run (IN-08).

use crate::support::{Desktop, ONE};

#[test]
fn shows_steps_and_target_without_launching() {
    let desktop = Desktop::new();
    desktop.config("[browsers]\nprimary = { app = \"fake-one.desktop\" }\n");
    let run = desktop
        .wye(&["test", "https://github.com/x?utm_source=a&id=1"])
        .expect_code(0);
    let lines: Vec<&str> = run.stdout.lines().collect();
    assert_eq!(
        lines.first(),
        Some(&"Link: https://github.com/x?utm_source=a&id=1")
    );
    assert!(lines.iter().any(|line| line.starts_with("  ")), "{run:#?}");
    assert!(
        lines.contains(&"Result: https://github.com/x?id=1"),
        "{run:#?}"
    );
    assert!(
        lines.contains(&"Opens in: Fake One (fake-one.desktop)"),
        "{run:#?}"
    );
    let command = lines
        .iter()
        .find(|line| line.starts_with("Command: "))
        .unwrap();
    assert!(
        command.ends_with("/bin/fake-one 'https://github.com/x?id=1'"),
        "{command}"
    );
    assert!(desktop.never_launched(ONE));
}

#[test]
fn names_the_picker_stand_in() {
    let desktop = Desktop::new();
    let run = desktop
        .wye(&["test", "https://example.com/"])
        .expect_code(0);
    assert!(run.stdout.contains("Opens in: Picker\n"), "{run:#?}");
    assert!(
        run.stdout
            .contains("wye open would use Fake One (fake-one.desktop)"),
        "{run:#?}"
    );
    assert!(desktop.never_launched(ONE));
}

#[test]
fn source_and_keys_reach_the_rules() {
    let desktop = Desktop::new();
    desktop.config(
        "[browsers]\nprimary = { app = \"fake-one.desktop\" }\n\n[[rules]]\nname = \"Chat\"\n\
         target = { app = \"fake-two.desktop\" }\nsource-apps = [\"chat.desktop\"]\n\
         held-keys = [\"Ctrl\"]\nopen-in-background = true\n",
    );
    let plain = desktop
        .wye(&["test", "https://example.com/"])
        .expect_code(0);
    assert!(plain.stdout.contains("Opens in: Fake One"), "{plain:#?}");
    let matched = desktop
        .wye(&[
            "test",
            "https://example.com/",
            "--source",
            "chat.desktop",
            "--keys",
            "Ctrl",
        ])
        .expect_code(0);
    assert!(
        matched
            .stdout
            .contains("Opens in: Fake Two (fake-two.desktop), background"),
        "{matched:#?}"
    );
}

#[test]
fn rejected_links_exit_with_2() {
    let desktop = Desktop::new();
    let run = desktop.wye(&["test", "mailto:x"]).expect_code(2);
    assert!(run.stderr.starts_with("wye: "), "{run:#?}");
}

#[test]
fn unknown_keys_are_invalid_input() {
    let desktop = Desktop::new();
    desktop
        .wye(&["test", "https://example.com/", "--keys", "Hyper"])
        .expect_code(2);
}

// DEF-08, DEF-09: a web app mapping to a non-browser app does not take back
// a link from that app, nor a sign-in page.
#[test]
fn an_app_mapping_skips_links_from_the_app_and_sign_in_pages() {
    let desktop = Desktop::new();
    desktop.add_app("fake-figma.desktop", "Fake Figma");
    desktop.config(
        "[browsers]\nprimary = { app = \"fake-one.desktop\" }\n\n\
         [apps.figma]\ncustom = \"fake-figma.desktop\"\n",
    );

    let plain = desktop
        .wye(&["test", "https://www.figma.com/design/abc/File"])
        .expect_code(0);
    assert!(plain.stdout.contains("Opens in: Fake Figma"), "{plain:#?}");

    let from_app = desktop
        .wye(&[
            "test",
            "https://www.figma.com/design/abc/File",
            "--source",
            "fake-figma.desktop",
        ])
        .expect_code(0);
    assert!(
        from_app.stdout.contains("Opens in: Fake One"),
        "{from_app:#?}"
    );
    assert!(
        from_app.stdout.contains("skipped: the link came from"),
        "{from_app:#?}"
    );

    let sign_in = desktop
        .wye(&["test", "https://www.figma.com/app_auth/x/grant"])
        .expect_code(0);
    assert!(
        sign_in.stdout.contains("Opens in: Fake One"),
        "{sign_in:#?}"
    );
    assert!(
        sign_in.stdout.contains("sign-in pages open in a browser"),
        "{sign_in:#?}"
    );
}
