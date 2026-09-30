// One tile (PICK-04 to PICK-07, PICK-10, PICK-14): hotkey, icon with an
// optional profile badge, name. Left click opens, middle click opens in the
// background (PICK-32), right click opens the context menu (PICK-30).
import QtQuick
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

Item {
    id: tile

    // The Repeater's element: {name, icon, hotkey, badgeImage, badgeInitial,
    // badgeColor, dimmed} (src/picker/qml.rs).
    required property var modelData
    required property int index
    property bool selected: false
    // The tile's width; the window gives every tile the same one, wide
    // enough for the longest name within limits (PICK-05).
    property int tileWidth: Kirigami.Units.gridUnit * 4
    property int iconSize: Kirigami.Units.iconSizes.large
    property int badgeSize: Kirigami.Units.iconSizes.small
    property bool showName: true
    // False when no tile has a hotkey: the row above the icons goes away.
    property bool showHotkey: true
    property int hotkeyHeight: 0
    property font nameFont: Kirigami.Theme.defaultFont
    // Space around the content inside the highlight.
    property int padding: Kirigami.Units.smallSpacing + Kirigami.Units.smallSpacing / 2
    // Hotkey character size (02-picker.md; the window's, which measures it
    // for `hotkeyHeight`) and highlight corner radius.
    property int hotkeyPixels: 12
    readonly property int highlightRadius: 12
    // Where the icon's centre is, from the tile's top; the "⋯" button lines
    // up with it (PICK-08).
    readonly property real iconCentre: padding + (showHotkey ? hotkeyHeight + column.spacing : 0) + iconSize / 2

    // The pointer moved over the tile, at `point` in window coordinates.
    signal hovered(point point)
    signal chosen(bool middle, int modifiers)
    signal menuRequested()

    implicitWidth: tileWidth
    implicitHeight: column.implicitHeight + padding * 2
    opacity: modelData.dimmed ? 0.35 : 1
    Accessible.role: Accessible.Button
    Accessible.name: modelData.name
    Accessible.description: modelData.hotkey ? qsTr("Hotkey %1").arg(modelData.hotkey) : ""
    Accessible.focusable: true
    Accessible.focused: selected
    Accessible.onPressAction: tile.chosen(false, Qt.NoModifier)

    Behavior on opacity {
        NumberAnimation {
            duration: Kirigami.Units.shortDuration
        }
    }

    // PICK-07: the selection, in the accent colour; a little stronger while
    // pressed.
    Rectangle {
        anchors.fill: parent
        radius: tile.highlightRadius
        color: Qt.alpha(Kirigami.Theme.highlightColor, mouse.pressed ? 0.45 : 0.28)
        border.width: 1
        border.color: Qt.alpha(Kirigami.Theme.highlightColor, 0.85)
        opacity: tile.selected ? 1 : 0

        Behavior on opacity {
            NumberAnimation {
                duration: Kirigami.Units.shortDuration
                easing.type: Easing.OutCubic
            }
        }
    }

    Column {
        id: column

        anchors.horizontalCenter: parent.horizontalCenter
        anchors.top: parent.top
        anchors.topMargin: tile.padding
        spacing: Kirigami.Units.smallSpacing

        // PICK-04: small, dimmed, centred.
        QQC2.Label {
            anchors.horizontalCenter: parent.horizontalCenter
            height: tile.hotkeyHeight
            visible: tile.showHotkey
            text: tile.modelData.hotkey
            font.pixelSize: tile.hotkeyPixels
            font.weight: Font.DemiBold
            verticalAlignment: Text.AlignVCenter
            opacity: tile.selected ? 0.9 : 0.55
        }

        Item {
            anchors.horizontalCenter: parent.horizontalCenter
            width: tile.iconSize
            height: tile.iconSize

            Kirigami.Icon {
                anchors.fill: parent
                source: tile.modelData.icon || "internet-web-browser"
                fallback: "internet-web-browser"
            }

            // PICK-06: over the icon's bottom-left corner.
            PickerBadge {
                x: -width / 4
                y: parent.height - height * 3 / 4
                size: tile.badgeSize
                image: tile.modelData.badgeImage
                initial: tile.modelData.badgeInitial
                tint: tile.modelData.badgeColor
            }
        }

        // PICK-05, PICK-10: one line, cut at the end; the tooltip has the
        // whole name.
        QQC2.Label {
            id: name

            anchors.horizontalCenter: parent.horizontalCenter
            width: tile.tileWidth - tile.padding * 2
            visible: tile.showName
            text: tile.modelData.name
            elide: Text.ElideRight
            maximumLineCount: 1
            horizontalAlignment: Text.AlignHCenter
            font: tile.nameFont
        }
    }

    MouseArea {
        id: mouse

        anchors.fill: parent
        hoverEnabled: true
        acceptedButtons: Qt.LeftButton | Qt.MiddleButton | Qt.RightButton
        onEntered: tile.hovered(mapToItem(null, mouseX, mouseY))
        onPositionChanged: event => tile.hovered(mapToItem(null, event.x, event.y))
        onClicked: event => {
            if (event.button === Qt.RightButton) {
                tile.menuRequested();
            } else {
                tile.chosen(event.button === Qt.MiddleButton, event.modifiers);
            }
        }
    }

    QQC2.ToolTip.visible: mouse.containsMouse && (name.truncated || !tile.showName)
    QQC2.ToolTip.text: modelData.name
    QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
}
