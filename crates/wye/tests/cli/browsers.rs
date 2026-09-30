//! `wye browsers`.

use crate::support::{Desktop, WYE};

#[test]
fn lists_browsers_but_not_wye() {
    let desktop = Desktop::new();
    desktop.install_wye();
    desktop.write(
        "data/applications/editor.desktop",
        "[Desktop Entry]\nType=Application\nName=Editor\nExec=editor %f\n",
    );
    let run = desktop.wye(&["browsers"]).expect_code(0);
    let one = run.stdout.find("Fake One\n  ID: fake-one.desktop\n");
    let two = run.stdout.find("Fake Two\n  ID: fake-two.desktop\n");
    assert!(one.is_some() && two.is_some() && one < two, "{run:#?}");
    assert!(run.stdout.contains("  Private window: no\n"), "{run:#?}");
    assert!(!run.stdout.contains(WYE), "{run:#?}");
    assert!(!run.stdout.contains("Editor"), "{run:#?}");
}
