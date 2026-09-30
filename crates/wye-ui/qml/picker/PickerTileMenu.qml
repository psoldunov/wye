pragma ComponentBehavior: Bound

// A tile's context menu (PICK-30), built from the entries the backend
// gives for that tile: [{action, label}], an empty action being a separator.
import QtQuick
import QtQuick.Controls as QQC2

QQC2.Menu {
    id: menu

    property int tileIndex: -1

    signal action(int index, string name)

    function show(index, entries) {
        while (count > 0) {
            takeItem(0).destroy();
        }
        tileIndex = index;
        for (const entry of entries) {
            const item = entry.action === "" ? separator.createObject(menu) : entryItem.createObject(menu, {
                "text": entry.label,
                "name": entry.action
            });
            addItem(item);
        }
        popup();
    }

    Component {
        id: separator

        QQC2.MenuSeparator {}
    }

    Component {
        id: entryItem

        QQC2.MenuItem {
            property string name

            onTriggered: menu.action(menu.tileIndex, name)
        }
    }
}
