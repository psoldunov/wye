pragma ComponentBehavior: Bound

// A tile's context menu (PICK-30), built from the entries the backend
// gives for that tile: [{action, label}], an empty action being a separator.
//
// An item popup, inside the picker's window: a menu in a window of its own
// would take the keyboard focus, and the picker cancels when it loses focus
// (PICK-23). The picker's window covers the output, so nothing clips it.
import QtQuick
import QtQuick.Controls as QQC2

QQC2.Menu {
    id: menu

    property int tileIndex: -1
    // The tile's icon, shown on "Open".
    property string tileIcon

    signal action(int index, string name)

    // Open the menu for tile `index` with its `entries` and `icon`; at the
    // pointer (a right click), or under `anchor` when given (only the
    // self-test does).
    function show(index, entries, icon, anchor) {
        while (count > 0) {
            takeItem(0).destroy();
        }
        tileIndex = index;
        tileIcon = icon;
        for (const entry of entries) {
            const item = entry.action === "" ? separator.createObject(menu) : entryItem.createObject(menu, {
                "text": entry.label,
                "name": entry.action
            });
            addItem(item);
        }
        if (anchor) {
            popup(anchor, 0, anchor.height);
        } else {
            popup();
        }
    }

    // An icon for each action (src/picker/state.rs, `action_name`).
    function iconFor(name) {
        switch (name) {
        case "open":
            // A themed name; a picture file gets a generic icon.
            return tileIcon !== "" && !tileIcon.startsWith("/") ? tileIcon : "document-open";
        case "open-private":
            return "view-private";
        case "open-new-window":
            return "window-new";
        case "open-background":
            return "tab-new-background";
        case "make-primary":
            return "starred-symbolic";
        }
        return "";
    }

    popupType: QQC2.Popup.Item

    Component {
        id: separator

        QQC2.MenuSeparator {}
    }

    Component {
        id: entryItem

        QQC2.MenuItem {
            property string name

            icon.name: menu.iconFor(name)
            onTriggered: menu.action(menu.tileIndex, name)
        }
    }
}
