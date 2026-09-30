// The About window (17-dialogs.md, DLG-ABT-01, DLG-ABT-02): an About page (AboutContent.qml) with Wye's name, icon, version,
// homepage, issue tracker, licence and credits, and a Troubleshooting section below it: what Wye detected in this session
// (`GetTroubleshooting`) with a button that copies it for bug reports. Surface contract (crates/wye-ui/src/route.rs):
//   handle("show", "about", argument)  open, or raise, the one window (SET-04)
// Under `wye-ui --self-test` the argument is a JSON object: {fixture, scheme} (fixtures/about.json). The data is
// `AboutBackend` (crates/wye-ui/src/bridge/about.rs).
pragma ComponentBehavior: Bound
import QtQuick
import org.kde.kirigami as Kirigami
import dev.soldunov.wye.ui

Kirigami.ApplicationWindow {
    id: window

    // The `window` and `argument` of the last ShowWindow.
    property string windowName
    property string argument
    function parseArgument(text) {
        if (!text.startsWith("{")) {
            return {};
        }
        try {
            return JSON.parse(text);
        } catch (error) {
            console.warn("about: the argument is not JSON:", error);
            return {};
        }
    }

    function handle(action, key, argument) {
        if (action !== "show") {
            return;
        }
        windowName = key;
        window.argument = argument;
        const request = parseArgument(argument);
        if (request.fixture !== undefined) {
            if (!backend.loadFixture(JSON.stringify(request.fixture))) {
                console.error("about: the fixture is not valid service data");
            }
        }
        if (request.scheme !== undefined) {
            Qt.styleHints.colorScheme = request.scheme === "dark" ? Qt.Dark : Qt.Light;
        }
        backend.refresh();
        show();
        raise();
        requestActivate();
    }

    title: qsTr("About Wye")
    minimumWidth: Kirigami.Units.gridUnit * 20
    minimumHeight: Kirigami.Units.gridUnit * 16
    width: Kirigami.Units.gridUnit * 36
    height: Kirigami.Units.gridUnit * 28

    AboutBackend {
        id: backend
    }

    WyeErrorText {
        id: errors
    }

    CopyHelper {
        id: clipboard
    }

    Shortcut {
        sequence: "Escape"

        onActivated: window.close()
    }

    Shortcut {
        sequence: "Ctrl+W"

        onActivated: window.close()
    }

    onVisibleChanged: {
        if (visible) {
            backend.refresh();
        }
    }

    pageStack.initialPage: AboutContent {
        aboutData: backend.aboutJson === "" ? ({}) : JSON.parse(backend.aboutJson)
        error: errors.describe(backend.errorKind, backend.error)
        troubleshooting: backend.troubleshooting

        onCopyRequested: text => {
            clipboard.copyText(text);
            window.showPassiveNotification(qsTr("Troubleshooting information copied to the clipboard."), "short");
        }
        onLinkRequested: url => backend.openLink(url)
    }
}
