// One row of the tray-menu popup (TRAY-08): a dimmed header, a separator,
// or an item with its check mark (TRAY-11), icon (TRAY-14), label and fixed
// shortcut (TRAY-13); a submenu row ends in an arrow (TRAY-15).
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

Item {
    id: row

    // A row of TrayMenuBackend.rows.
    required property var entry
    property bool current: false

    readonly property bool separator: entry.kind === "separator"
    readonly property bool header: entry.kind === "header"
    readonly property bool interactive: entry.selectable || entry.opens
    readonly property bool highlighted: current && interactive
    readonly property color textColor: highlighted ? Kirigami.Theme.highlightedTextColor : Kirigami.Theme.textColor

    signal chosen
    signal pointed

    implicitHeight: separator ? Kirigami.Units.smallSpacing * 2 + 1 : Math.max(Kirigami.Units.iconSizes.smallMedium, label.implicitHeight) + Kirigami.Units.smallSpacing * 2

    Rectangle {
        anchors.fill: parent
        visible: row.highlighted
        radius: Kirigami.Units.cornerRadius
        color: Kirigami.Theme.highlightColor
    }

    Rectangle {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        anchors.leftMargin: Kirigami.Units.smallSpacing
        anchors.rightMargin: Kirigami.Units.smallSpacing
        visible: row.separator
        height: 1
        color: Qt.alpha(Kirigami.Theme.textColor, 0.2)
    }

    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: Kirigami.Units.smallSpacing
        anchors.rightMargin: Kirigami.Units.largeSpacing
        visible: !row.separator
        spacing: Kirigami.Units.smallSpacing

        // TRAY-11: the checked radio item.
        Kirigami.Icon {
            Layout.preferredWidth: Kirigami.Units.iconSizes.small
            Layout.preferredHeight: Kirigami.Units.iconSizes.small
            visible: !row.header
            source: row.entry.checked ? "checkmark" : ""
            color: row.textColor
        }

        // TRAY-14: the browser's full-colour icon.
        Kirigami.Icon {
            Layout.preferredWidth: Kirigami.Units.iconSizes.smallMedium
            Layout.preferredHeight: Kirigami.Units.iconSizes.smallMedium
            visible: row.entry.icon !== ""
            source: row.entry.icon
        }

        QQC2.Label {
            id: label

            Layout.fillWidth: true
            text: row.entry.label
            elide: Text.ElideRight
            color: row.textColor
            opacity: row.header || !row.entry.enabled ? 0.6 : 1
        }

        // TRAY-13, KEY-51.
        QQC2.Label {
            visible: row.entry.shortcut !== ""
            text: row.entry.shortcut
            color: row.textColor
            opacity: 0.6
        }

        Kirigami.Icon {
            Layout.preferredWidth: Kirigami.Units.iconSizes.small
            Layout.preferredHeight: Kirigami.Units.iconSizes.small
            visible: row.entry.kind === "submenu"
            source: "go-next-symbolic"
            color: row.textColor
            opacity: row.entry.opens ? 1 : 0.6
        }
    }

    MouseArea {
        anchors.fill: parent
        enabled: row.interactive
        hoverEnabled: true
        onEntered: row.pointed()
        onClicked: row.chosen()
    }
}
