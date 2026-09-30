pragma ComponentBehavior: Bound

// The picker (02-picker.md). A transparent layer-shell overlay covering the
// output, with the panel near the pointer (PICK-02) or centred; a click
// outside the panel, Escape or focus loss cancels (PICK-23). All logic is in
// PickerBackend (src/bridge/picker.rs, src/picker/); this file draws.
//
// Surface contract (crates/wye-ui/src/route.rs):
//   handle("show", requestId, PickerRequest JSON)  show, or replace (PICK-27)
//   handle("close", requestId, "")                 the service closed it
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.kde.layershell as LayerShell
import dev.soldunov.wye.ui

Window {
    id: window

    // Panel metrics (02-picker.md, "At every size").
    readonly property int panelPadding: 14
    readonly property int panelRadius: 14
    readonly property int overflowSize: 18
    // How long to wait for the compositor's activation token (PICK-29).
    readonly property int tokenWait: 500
    // The tiles as the backend describes them.
    readonly property var tiles: JSON.parse(backend.tiles || "[]")
    property bool blurred: false
    property bool wasActive: false

    function handle(action, key, argument) {
        if (action === "show") {
            open(key, argument);
        } else if (action === "close") {
            backend.dismiss(key);
        }
    }

    function open(key, argument) {
        if (!backend.load(key, argument)) {
            return;
        }
        chooseScreen();
        if (!visible) {
            wasActive = false;
        }
        show();
        requestActivate();
        panel.forceActiveFocus();
        updateBlur();
    }

    // PICK-02: the output the pointer is on.
    function chooseScreen() {
        if (!backend.placed) {
            return;
        }
        // qmllint disable missing-property
        const screens = Qt.application.screens;
        // qmllint enable missing-property
        for (let i = 0; i < screens.length; ++i) {
            if (screens[i].name === backend.placementOutput) {
                window.screen = screens[i];
                return;
            }
        }
    }

    // PICK-01: blur behind the panel, else an opaque panel.
    function updateBlur() {
        if (visible) {
            blurred = backend.blurBehind(window, panel.x, panel.y, panel.width, panel.height, panelRadius);
        }
    }

    // PICK-29: the launched browser gets a token from this input event.
    function requestToken(appId) {
        if (appId === "" || !effects.requestActivationToken(window, appId)) {
            backend.submit("");
            return;
        }
        tokenTimer.restart();
    }

    function clamp(value, low, high) {
        return Math.max(low, Math.min(value, high));
    }

    title: qsTr("Choose a browser")
    flags: Qt.FramelessWindowHint
    color: "transparent"
    // Where the layer shell does not apply (X11, offscreen), cover the
    // screen the same way.
    width: Screen.width
    height: Screen.height
    onActiveChanged: {
        if (active) {
            wasActive = true;
        } else if (wasActive && visible) {
            backend.cancel();
        }
    }

    LayerShell.Window.layer: LayerShell.Window.LayerOverlay
    LayerShell.Window.anchors: LayerShell.Window.AnchorTop | LayerShell.Window.AnchorBottom | LayerShell.Window.AnchorLeft | LayerShell.Window.AnchorRight
    LayerShell.Window.exclusionZone: -1
    LayerShell.Window.keyboardInteractivity: LayerShell.Window.KeyboardInteractivityExclusive
    LayerShell.Window.wantsToBeOnActiveScreen: !backend.placed
    LayerShell.Window.scope: "wye-picker"

    PickerBackend {
        id: backend

        onCloseRequested: {
            tokenTimer.stop();
            window.hide();
        }
        onTokenRequested: appId => window.requestToken(appId)
        onMoreRequested: overflowMenu.popup(overflowButton)
    }

    WindowEffects {
        id: effects

        onActivationTokenReady: token => {
            tokenTimer.stop();
            backend.submit(token);
        }
    }

    Timer {
        id: tokenTimer

        interval: window.tokenWait
        onTriggered: backend.submit("")
    }

    // PICK-23: a click outside the panel cancels.
    MouseArea {
        anchors.fill: parent
        acceptedButtons: Qt.AllButtons
        onPressed: backend.cancel()
    }

    Rectangle {
        id: panel

        Kirigami.Theme.colorSet: Kirigami.Theme.View
        Kirigami.Theme.inherit: false
        x: backend.placed ? window.clamp(backend.placementX - width / 2, Kirigami.Units.largeSpacing, window.width - width - Kirigami.Units.largeSpacing) : (window.width - width) / 2
        y: backend.placed ? window.clamp(backend.placementY - height / 2, Kirigami.Units.largeSpacing, window.height - height - Kirigami.Units.largeSpacing) : (window.height - height) / 2
        width: content.implicitWidth + 2 * window.panelPadding
        height: content.implicitHeight + 2 * window.panelPadding
        radius: window.panelRadius
        // PICK-01, PICK-12: translucent over blur, opaque without it.
        color: Qt.alpha(Kirigami.Theme.backgroundColor, window.blurred ? 0.82 : 1)
        border.width: 1
        border.color: Qt.alpha(Kirigami.Theme.textColor, 0.15)
        focus: true
        onXChanged: window.updateBlur()
        onYChanged: window.updateBlur()
        onWidthChanged: window.updateBlur()
        onHeightChanged: window.updateBlur()
        Keys.onPressed: event => {
            event.accepted = backend.keyPressed(event.key, event.text, event.nativeScanCode, event.modifiers);
        }
        Keys.onReleased: event => backend.modifiersChanged(event.modifiers)

        // Clicks on the panel itself do not cancel.
        MouseArea {
            anchors.fill: parent
            acceptedButtons: Qt.AllButtons
        }

        ColumnLayout {
            id: content

            anchors.centerIn: parent
            spacing: Kirigami.Units.smallSpacing

            RowLayout {
                spacing: Kirigami.Units.smallSpacing

                // PICK-03, PICK-13: rows of up to eight tiles.
                GridLayout {
                    columns: backend.columns
                    rowSpacing: Kirigami.Units.smallSpacing
                    columnSpacing: 0

                    Repeater {
                        model: window.tiles

                        PickerTile {
                            selected: index === backend.selected
                            pitch: backend.pitch
                            iconSize: backend.iconSize
                            badgeSize: backend.badgeSize
                            showName: backend.showNames
                            onHovered: backend.hover(index)
                            onChosen: (middle, modifiers) => backend.activate(index, middle, modifiers)
                            onMenuRequested: tileMenu.openFor(index)
                        }
                    }
                }

                // PICK-08: the "⋯" button, centred on the icons.
                QQC2.RoundButton {
                    id: overflowButton

                    Layout.alignment: Qt.AlignTop
                    Layout.topMargin: Kirigami.Units.gridUnit + (backend.iconSize - height) / 2
                    implicitWidth: window.overflowSize + Kirigami.Units.smallSpacing * 2
                    implicitHeight: implicitWidth
                    flat: true
                    icon.name: "overflow-menu"
                    icon.width: window.overflowSize
                    icon.height: window.overflowSize
                    Accessible.name: qsTr("More targets")
                    onClicked: overflowMenu.popup(overflowButton)
                }
            }

            // PICK-14: what the held modifiers do.
            QQC2.Label {
                Layout.fillWidth: true
                visible: backend.hint !== ""
                text: backend.hint
                horizontalAlignment: Text.AlignHCenter
                opacity: 0.7
            }

            // PICK-09.
            PickerUrlLine {
                Layout.fillWidth: true
                visible: backend.showUrl
                host: backend.urlHost
                rest: backend.urlRest
                full: backend.urlFull
                sourceName: backend.sourceName
                sourceIcon: backend.sourceIcon
            }

            // PKS-06.
            QQC2.Label {
                Layout.fillWidth: true
                visible: backend.preview
                text: qsTr("Preview: choosing a browser opens nothing")
                horizontalAlignment: Text.AlignHCenter
                font: Kirigami.Theme.smallFont
                opacity: 0.6
            }
        }

        PickerOverflowMenu {
            id: overflowMenu

            entries: JSON.parse(backend.openIn || "[]")
            onOpenIn: (group, item) => backend.openInTarget(group, item)
            onAction: name => backend.overflowAction(name)
        }

        PickerTileMenu {
            id: tileMenu

            function openFor(index) {
                show(index, JSON.parse(backend.tileMenu(index)));
            }

            onAction: (index, name) => backend.tileAction(index, name)
        }
    }
}
