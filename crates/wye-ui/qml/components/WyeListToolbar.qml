// WyeListToolbar (BLK-13): a bar attached to the bottom of a list card: "+" at the left, "⋯" menu at the right.
//
// API
//   addText: string        the "+" button's tooltip and accessible name ("Add Rule")
//   addTriggered()         "+" was pressed
//   addEnabled: bool       default true
//   default property       the "⋯" menu's entries: QQC2.MenuItem / QQC2.MenuSeparator / Action
//   moreEnabled: bool      the menu button, default true
pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

QQC2.ToolBar {
    id: bar

    property string addText: qsTr("Add")
    property bool addEnabled: true
    property bool moreEnabled: true
    default property alias menuItems: moreMenu.contentData
    signal addTriggered

    Layout.fillWidth: true
    position: QQC2.ToolBar.Footer

    RowLayout {
        anchors.fill: parent
        spacing: Kirigami.Units.smallSpacing

        QQC2.ToolButton {
            display: QQC2.AbstractButton.IconOnly
            enabled: bar.addEnabled
            icon.name: "list-add"
            text: bar.addText
            QQC2.ToolTip.text: text
            QQC2.ToolTip.visible: hovered
            onClicked: bar.addTriggered()
        }

        Item {
            Layout.fillWidth: true
        }

        QQC2.ToolButton {
            display: QQC2.AbstractButton.IconOnly
            enabled: bar.moreEnabled
            icon.name: "overflow-menu"
            text: qsTr("More")
            QQC2.ToolTip.text: text
            QQC2.ToolTip.visible: hovered
            onClicked: moreMenu.popup()

            QQC2.Menu {
                id: moreMenu

                y: parent.height
            }
        }
    }
}
