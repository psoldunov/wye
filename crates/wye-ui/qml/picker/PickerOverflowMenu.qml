pragma ComponentBehavior: Bound

// The "⋯" menu (PICK-08): Open In › every target that is not a tile
// (PICK-28), Copy Link, Create Rule… (PICK-31), Settings….
import QtQuick
import QtQuick.Controls as QQC2

QQC2.Menu {
    id: menu

    // [{kind: "header" | "item", label, icon, group, item}]
    // (src/picker/qml.rs).
    property var entries: []

    signal openIn(int group, int item)
    signal action(string name)

    QQC2.Menu {
        id: openInMenu

        title: qsTr("Open In")
        enabled: menu.entries.length > 0

        Instantiator {
            model: menu.entries

            delegate: QQC2.MenuItem {
                required property var modelData

                text: modelData.label
                icon.name: modelData.icon
                enabled: modelData.kind === "item"
                font.bold: modelData.kind === "header"
                onTriggered: menu.openIn(modelData.group, modelData.item)
            }

            onObjectAdded: (index, object) => openInMenu.insertItem(index, object)
            onObjectRemoved: (index, object) => openInMenu.removeItem(object)
        }
    }

    QQC2.MenuSeparator {}

    QQC2.MenuItem {
        text: qsTr("Copy Link")
        icon.name: "edit-copy"
        onTriggered: menu.action("copy-link")
    }

    QQC2.MenuItem {
        text: qsTr("Create Rule…")
        icon.name: "list-add"
        onTriggered: menu.action("create-rule")
    }

    QQC2.MenuSeparator {}

    QQC2.MenuItem {
        text: qsTr("Settings…")
        icon.name: "configure"
        onTriggered: menu.action("settings")
    }
}
