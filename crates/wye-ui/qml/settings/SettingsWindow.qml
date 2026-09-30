// The Settings window with its pages and sheets (03-settings-window.md). A placeholder until U09 and U11 builds it. Surface contract
// (crates/wye-ui/src/route.rs):
//   handle("show", window name, argument)  open, or raise the one window (SET-04)
import QtQuick
import org.kde.kirigami as Kirigami
import dev.soldunov.wye.ui

Kirigami.ApplicationWindow {
    id: window

    // The `window` and `argument` of the last ShowWindow.
    property string windowName
    property string argument

    function handle(action, key, argument) {
        if (action !== "show") {
            return;
        }
        windowName = key;
        window.argument = argument;
        show();
        raise();
        requestActivate();
    }

    title: qsTr("Settings")
    minimumWidth: Kirigami.Units.gridUnit * 20
    minimumHeight: Kirigami.Units.gridUnit * 16
    width: Kirigami.Units.gridUnit * 36
    height: Kirigami.Units.gridUnit * 28

    SettingsBackend {
        id: settingsBackend
    }

    RulesBackend {
        id: rulesBackend
    }

    AppChooserBackend {
        id: appChooserBackend
    }

    TesterBackend {
        id: testerBackend
    }

    pageStack.initialPage: Kirigami.Page {
        title: window.title

        SurfacePlaceholder {
            anchors.centerIn: parent
            width: parent.width - Kirigami.Units.gridUnit * 4
            text: window.title
            explanation: window.argument !== "" ? window.windowName + ": " + window.argument : window.windowName
        }
    }
}
