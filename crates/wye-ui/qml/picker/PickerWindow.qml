pragma ComponentBehavior: Bound

// The picker (02-picker.md). A transparent layer-shell overlay covering the
// output, with the panel near the pointer (PICK-02) or centred; a click
// outside the panel, Escape or focus loss cancels (PICK-23). All logic is in
// PickerBackend (src/bridge/picker.rs, src/picker/); this file draws.
//
// Surface contract (crates/wye-ui/src/route.rs):
//   handle("show", requestId, PickerRequest JSON)  show, or replace (PICK-27)
//   handle("close", requestId, "")                 the service closed it
//
// Under `wye-ui --self-test` a request may carry `selfTest: {scheme, menu}`
// (fixtures/picker.json): the colour scheme to draw in, and a menu to open
// (`overflow`, or `tile` for the first tile's).
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
    readonly property int overflowSize: 14
    // How long to wait for the compositor's activation token (PICK-29).
    readonly property int tokenWait: 500
    // The tiles as the backend describes them.
    readonly property var tiles: JSON.parse(backend.tiles || "[]")
    readonly property bool hasHotkeys: tiles.some(tile => tile.hotkey !== "")
    // PICK-11: a caption under the icons at every size.
    readonly property font nameFont: Kirigami.Theme.smallFont
    readonly property int tilePadding: Kirigami.Units.smallSpacing + Kirigami.Units.smallSpacing / 2
    // PICK-05: every tile is as wide as the longest name needs, from the
    // size's pitch up to twice that; a longer name is cut with an ellipsis.
    readonly property int tileWidth: {
        const pitch = backend.pitch;
        if (!backend.showNames) {
            return pitch;
        }
        let widest = 0;
        for (const tile of tiles) {
            widest = Math.max(widest, nameMetrics.advanceWidth(tile.name));
        }
        return Math.round(clamp(Math.ceil(widest) + 2 * tilePadding + 2, pitch, pitch * 2));
    }
    // PICK-13: up to eight tiles per row, fewer when the row would not fit
    // the output.
    readonly property int columns: {
        const room = width - 2 * (Kirigami.Units.largeSpacing + panelPadding + tilesRow.spacing) - overflowButton.implicitWidth;
        const fit = Math.floor((room + tileGrid.columnSpacing) / (tileWidth + tileGrid.columnSpacing));
        return clamp(fit, 1, backend.columns);
    }
    // PICK-04: the hotkey character's size; `hotkeyMetrics` and every tile use it.
    readonly property int hotkeyPixels: 10
    // PICK-08: the "⋯" button is centred on the icons.
    readonly property real iconCentre: tilePadding + (hasHotkeys ? hotkeyMetrics.height + Kirigami.Units.smallSpacing : 0) + backend.iconSize / 2
    property bool blurred: false
    property bool wasActive: false
    // PICK-24: the first tile stays selected until the pointer really
    // moves; a panel that opens under a resting pointer must not select the
    // tile under it.
    property var hoverOrigin: null
    property bool pointerMoved: false

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
        // PICK-27: a menu belongs to the request it was opened for.
        overflowMenu.close();
        tileMenu.close();
        hoverOrigin = null;
        pointerMoved = false;
        chooseScreen();
        if (!visible) {
            wasActive = false;
        }
        show();
        requestActivate();
        panel.forceActiveFocus();
        updateBlur();
        applySelfTest(argument);
    }

    function applySelfTest(argument) {
        const request = JSON.parse(argument);
        const test = request.selfTest;
        if (test === undefined) {
            return;
        }
        if (test.scheme !== undefined) {
            Qt.styleHints.colorScheme = test.scheme === "dark" ? Qt.Dark : Qt.Light;
        }
        if (test.menu === "overflow") {
            Qt.callLater(openMore);
        } else if (test.menu === "tile") {
            Qt.callLater(() => tileMenu.openFor(0, tileRepeater.itemAt(0)));
        }
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

    // PICK-22: hover selects, once the pointer has moved (PICK-24).
    function tileHovered(index, point) {
        if (!pointerMoved) {
            if (hoverOrigin === null) {
                hoverOrigin = point;
                return;
            }
            if (Math.abs(point.x - hoverOrigin.x) < 3 && Math.abs(point.y - hoverOrigin.y) < 3) {
                return;
            }
            pointerMoved = true;
        }
        backend.hover(index);
    }

    // PICK-08, KEY-22: the "⋯" menu, under its button.
    function openMore() {
        overflowMenu.popup(overflowButton, 0, overflowButton.height + Kirigami.Units.smallSpacing);
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
    // PICK-15: KWin gives a surface the window type its scope names, and zooms and fades every
    // new surface of the normal type (the type of an unknown scope) into view. It animates no
    // utility window, so the panel appears at once. (An on-screen display is not animated
    // either, but KWin takes the keyboard focus straight back from one, which cancels the
    // picker, PICK-23.)
    LayerShell.Window.scope: "utility"

    PickerBackend {
        id: backend

        onCloseRequested: {
            tokenTimer.stop();
            overflowMenu.close();
            tileMenu.close();
            window.hide();
        }
        onTokenRequested: appId => window.requestToken(appId)
        onMoreRequested: window.openMore()
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

    FontMetrics {
        id: nameMetrics

        font: window.nameFont
    }

    FontMetrics {
        id: hotkeyMetrics

        font.pixelSize: window.hotkeyPixels
        font.weight: Font.DemiBold
    }

    // PICK-23: a click outside the panel cancels.
    MouseArea {
        anchors.fill: parent
        acceptedButtons: Qt.AllButtons
        onPressed: backend.cancel()
    }

    Kirigami.ShadowedRectangle {
        id: panel

        Kirigami.Theme.colorSet: Kirigami.Theme.Window
        Kirigami.Theme.inherit: false
        x: backend.placed ? window.clamp(backend.placementX - width / 2, Kirigami.Units.largeSpacing, window.width - width - Kirigami.Units.largeSpacing) : (window.width - width) / 2
        y: backend.placed ? window.clamp(backend.placementY - height / 2, Kirigami.Units.largeSpacing, window.height - height - Kirigami.Units.largeSpacing) : (window.height - height) / 2
        width: Math.ceil(content.implicitWidth) + 2 * window.panelPadding
        height: Math.ceil(content.implicitHeight) + 2 * window.panelPadding
        radius: window.panelRadius
        // PICK-01, PICK-12: translucent over blur, opaque without it (the
        // popover colour, "No blur available").
        color: Qt.alpha(Kirigami.Theme.backgroundColor, window.blurred ? 0.88 : 1)
        border.width: 1
        border.color: Qt.alpha(Kirigami.Theme.textColor, 0.2)
        shadow.size: Kirigami.Units.gridUnit
        shadow.yOffset: 2
        shadow.color: Qt.rgba(0, 0, 0, 0.35)
        focus: true
        Accessible.role: Accessible.Dialog
        Accessible.name: window.title
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
                id: tilesRow

                // Centred when the link below is wider.
                Layout.fillWidth: false
                Layout.alignment: Qt.AlignHCenter
                spacing: Kirigami.Units.smallSpacing

                // PICK-03, PICK-13: rows of up to eight tiles.
                GridLayout {
                    id: tileGrid

                    visible: window.tiles.length > 0
                    columns: window.columns
                    rowSpacing: Kirigami.Units.smallSpacing
                    columnSpacing: Kirigami.Units.smallSpacing / 2

                    Repeater {
                        id: tileRepeater

                        model: window.tiles

                        PickerTile {
                            selected: index === backend.selected
                            tileWidth: window.tileWidth
                            padding: window.tilePadding
                            iconSize: backend.iconSize
                            badgeSize: backend.badgeSize
                            showName: backend.showNames
                            showHotkey: window.hasHotkeys
                            hotkeyHeight: hotkeyMetrics.height
                            hotkeyPixels: window.hotkeyPixels
                            nameFont: window.nameFont
                            onHovered: point => window.tileHovered(index, point)
                            onChosen: (middle, modifiers) => backend.activate(index, middle, modifiers)
                            onMenuRequested: tileMenu.openFor(index, null)
                        }
                    }
                }

                // No tile to show: the "⋯" menu still opens the link.
                ColumnLayout {
                    Layout.margins: Kirigami.Units.smallSpacing
                    visible: window.tiles.length === 0
                    spacing: 0

                    QQC2.Label {
                        text: qsTr("No browsers to show")
                        font.weight: Font.DemiBold
                    }

                    QQC2.Label {
                        text: qsTr("Use ⋯ to open the link another way")
                        font: Kirigami.Theme.smallFont
                        opacity: 0.7
                    }
                }

                // PICK-08: the "⋯" button, centred on the icons.
                QQC2.RoundButton {
                    id: overflowButton

                    Layout.alignment: window.tiles.length > 0 ? Qt.AlignTop : Qt.AlignVCenter
                    Layout.topMargin: window.tiles.length > 0 ? window.iconCentre - height / 2 : 0
                    implicitWidth: window.overflowSize + Kirigami.Units.smallSpacing * 3
                    implicitHeight: implicitWidth
                    flat: true
                    focusPolicy: Qt.NoFocus
                    icon.name: "view-more-horizontal-symbolic"
                    icon.width: window.overflowSize
                    icon.height: window.overflowSize
                    Accessible.name: qsTr("More targets")
                    QQC2.ToolTip.visible: hovered && !overflowMenu.visible
                    QQC2.ToolTip.text: qsTr("More targets")
                    QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
                    onClicked: window.openMore()
                }
            }

            // PICK-14: what the held modifiers do.
            QQC2.Label {
                Layout.fillWidth: true
                Layout.preferredWidth: tilesRow.implicitWidth
                visible: backend.hint !== ""
                text: backend.hint
                wrapMode: Text.WordWrap
                horizontalAlignment: Text.AlignHCenter
                color: Kirigami.Theme.highlightColor
                font.weight: Font.DemiBold
            }

            // PICK-09: centred; as wide as the link, up to the tiles' width
            // or a limit, whichever is larger. Past that the line cuts the
            // link.
            PickerUrlLine {
                // A layout fills by default; this one keeps its own width.
                Layout.fillWidth: false
                Layout.alignment: Qt.AlignHCenter
                Layout.maximumWidth: Math.max(tilesRow.implicitWidth, Kirigami.Units.gridUnit * 22)
                Layout.topMargin: Kirigami.Units.smallSpacing
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
                Layout.preferredWidth: tilesRow.implicitWidth
                visible: backend.preview
                text: qsTr("Preview: choosing a browser opens nothing")
                wrapMode: Text.WordWrap
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
            onClosed: panel.forceActiveFocus()
        }

        PickerTileMenu {
            id: tileMenu

            // PICK-30: at the pointer, or under `anchor`.
            function openFor(index, anchor) {
                const tile = window.tiles[index];
                if (tile === undefined) {
                    return;
                }
                show(index, JSON.parse(backend.tileMenu(index)), tile.icon, anchor);
            }

            onAction: (index, name) => backend.tileAction(index, name)
            onClosed: panel.forceActiveFocus()
        }
    }
}
