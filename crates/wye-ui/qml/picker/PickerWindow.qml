// The picker (02-picker.md). A placeholder until U05 builds it: it already
// is the frameless layer-shell overlay the design asks for (design B), asks
// for blur through the shim, and follows the surface contract of
// crates/wye-ui/src/route.rs:
//   handle("show", requestId, PickerRequest JSON)  show or replace (PICK-27)
//   handle("close", requestId, "")                 close that request
import QtQuick
import org.kde.kirigami as Kirigami
import org.kde.layershell as LayerShell
import dev.soldunov.wye.ui

Window {
    id: window

    property string requestId
    property var request: ({})

    function handle(action, key, argument) {
        if (action === "show") {
            requestId = key;
            request = JSON.parse(argument);
            show();
            requestActivate();
            effects.blurBehind(window, true);
        } else if (action === "close" && key === requestId) {
            hide();
            requestId = "";
        }
    }

    title: qsTr("Choose a browser")
    flags: Qt.FramelessWindowHint
    color: "transparent"
    width: Kirigami.Units.gridUnit * 24
    height: Kirigami.Units.gridUnit * 8

    LayerShell.Window.layer: LayerShell.Window.LayerOverlay
    LayerShell.Window.keyboardInteractivity: LayerShell.Window.KeyboardInteractivityExclusive
    LayerShell.Window.wantsToBeOnActiveScreen: true

    PickerBackend {
        id: backend
    }

    WindowEffects {
        id: effects
    }

    Rectangle {
        anchors.fill: parent
        radius: Kirigami.Units.cornerRadius
        color: Kirigami.Theme.backgroundColor

        SurfacePlaceholder {
            anchors.centerIn: parent
            width: parent.width - Kirigami.Units.gridUnit * 2
            text: window.title
            explanation: window.request.url ? window.request.url.full : ""
        }
    }
}
