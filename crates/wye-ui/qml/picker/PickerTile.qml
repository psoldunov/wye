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
    property int pitch: Kirigami.Units.gridUnit * 3
    property int iconSize: Kirigami.Units.iconSizes.large
    property int badgeSize: Kirigami.Units.iconSizes.small
    property bool showName: true
    // Hotkey character size (02-picker.md) and highlight corner radius.
    readonly property int hotkeyPixels: 12
    readonly property int highlightRadius: 12

    signal hovered()
    signal chosen(bool middle, int modifiers)
    signal menuRequested()

    implicitWidth: pitch
    implicitHeight: column.implicitHeight + Kirigami.Units.smallSpacing * 2
    opacity: modelData.dimmed ? 0.35 : 1
    Accessible.role: Accessible.Button
    Accessible.name: modelData.name

    // PICK-07.
    Rectangle {
        anchors.fill: parent
        radius: tile.highlightRadius
        color: Kirigami.Theme.highlightColor
        visible: tile.selected
    }

    Column {
        id: column

        anchors.horizontalCenter: parent.horizontalCenter
        anchors.top: parent.top
        anchors.topMargin: Kirigami.Units.smallSpacing
        spacing: Kirigami.Units.smallSpacing / 2

        QQC2.Label {
            anchors.horizontalCenter: parent.horizontalCenter
            text: tile.modelData.hotkey || " "
            font.pixelSize: tile.hotkeyPixels
            color: tile.selected ? Kirigami.Theme.highlightedTextColor : Kirigami.Theme.textColor
            opacity: 0.6
        }

        Item {
            anchors.horizontalCenter: parent.horizontalCenter
            width: tile.iconSize
            height: tile.iconSize

            Kirigami.Icon {
                anchors.fill: parent
                source: tile.modelData.icon || "internet-web-browser"
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

        // PICK-05, PICK-10.
        QQC2.Label {
            anchors.horizontalCenter: parent.horizontalCenter
            width: tile.pitch - Kirigami.Units.smallSpacing
            visible: tile.showName
            text: tile.modelData.name
            elide: Text.ElideRight
            maximumLineCount: 1
            horizontalAlignment: Text.AlignHCenter
            font: tile.iconSize >= Kirigami.Units.iconSizes.large ? Kirigami.Theme.defaultFont : Kirigami.Theme.smallFont
            color: tile.selected ? Kirigami.Theme.highlightedTextColor : Kirigami.Theme.textColor
        }
    }

    MouseArea {
        anchors.fill: parent
        hoverEnabled: true
        acceptedButtons: Qt.LeftButton | Qt.MiddleButton | Qt.RightButton
        onEntered: tile.hovered()
        onClicked: mouse => {
            if (mouse.button === Qt.RightButton) {
                tile.menuRequested();
            } else {
                tile.chosen(mouse.button === Qt.MiddleButton, mouse.modifiers);
            }
        }
    }
}
