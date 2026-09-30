// The tray-menu popup for the toggle-menu shortcut (TRAY-08, spec decision
// #14). A placeholder until U13 builds it. Surface contract
// (crates/wye-ui/src/route.rs):
//   handle("toggle", "", TrayMenu JSON)  show it, or hide it when shown
import QtQuick
import org.kde.kirigami as Kirigami
import org.kde.layershell as LayerShell
import dev.soldunov.wye.ui

Window {
    id: popup

    property var menu: ({})

    function handle(action, key, argument) {
        if (action !== "toggle") {
            return;
        }
        if (visible) {
            hide();
            return;
        }
        menu = JSON.parse(argument);
        show();
        requestActivate();
    }

    title: qsTr("Wye")
    flags: Qt.FramelessWindowHint
    color: "transparent"
    width: Kirigami.Units.gridUnit * 14
    height: Kirigami.Units.gridUnit * 10

    LayerShell.Window.layer: LayerShell.Window.LayerOverlay
    LayerShell.Window.keyboardInteractivity: LayerShell.Window.KeyboardInteractivityExclusive
    LayerShell.Window.wantsToBeOnActiveScreen: true

    TrayMenuBackend {
        id: backend
    }

    Rectangle {
        anchors.fill: parent
        radius: Kirigami.Units.cornerRadius
        color: Kirigami.Theme.backgroundColor

        SurfacePlaceholder {
            anchors.centerIn: parent
            width: parent.width - Kirigami.Units.gridUnit * 2
            text: popup.title
            explanation: qsTr("%n menu item(s)", "", popup.menu.items ? popup.menu.items.length : 0)
        }
    }
}
