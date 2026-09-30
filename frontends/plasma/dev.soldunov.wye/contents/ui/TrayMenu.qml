/*
    SPDX-FileCopyrightText: 2026 Philipp Soldunov <69530789+psoldunov@users.noreply.github.com>
    SPDX-License-Identifier: MIT

    The tray menu (01-tray-menu.md), a native menu built from the `Tray`
    model's items: section headers, separators, radio items (TRAY-11),
    submenus (TRAY-15), icons (TRAY-14) and the shortcut column (TRAY-13,
    KEY-51). Choosing an item emits `chosen(id)`; the service does the rest.
*/
pragma ComponentBehavior: Bound

import QtQuick

import org.kde.plasma.extras as PlasmaExtras

Item {
    id: builder

    /*! The item the menu opens next to. */
    property Item visualParent: null
    /*! The `Tray` model's `items`. */
    property var items: []
    /*! TRAY-10: the latest answer to ClipboardHasUrl, or null to trust the model. */
    property var clipboardHasUrl: null
    /*! The menu is on screen (TRAY-06). */
    readonly property bool open: rootMenu.status === PlasmaExtras.Menu.Open

    /*! Every object built for the current items, destroyed on rebuild. */
    property var built: []

    signal chosen(string id)

    visible: false

    function popup() {
        builder.rebuild();
        rootMenu.openRelative();
    }

    function close() {
        rootMenu.close();
    }

    function rebuild() {
        for (const object of builder.built) {
            object.destroy();
        }
        builder.built = [];
        rootMenu.clearMenuItems();
        for (const item of (builder.items || [])) {
            builder.add(rootMenu, item);
        }
    }

    /*! Qt draws text after a tab as the right-aligned shortcut column. */
    function text(item) {
        const label = String(item.label || "").replace(/&/g, "&&");
        return item.shortcut ? label + "\t" + item.shortcut : label;
    }

    function isEnabled(item) {
        if (item.id === "open-clipboard" && builder.clipboardHasUrl !== null) {
            return builder.clipboardHasUrl === true;
        }
        return item.enabled !== false;
    }

    function add(menu, item) {
        let entry = null;
        switch (item.kind) {
        case "separator":
            entry = menuItem.createObject(menu, {
                "separator": true
            });
            break;
        case "header":
            entry = menuItem.createObject(menu, {
                "section": true,
                "text": String(item.label || "")
            });
            break;
        case "submenu":
            entry = menuItem.createObject(menu, {
                "text": builder.text(item),
                "icon": item.icon || "",
                "enabled": builder.isEnabled(item)
            });
            builder.addSubmenu(entry, item.children || []);
            break;
        default:
            entry = choosable.createObject(menu, {
                "itemId": String(item.id),
                "text": builder.text(item),
                "icon": item.icon || "",
                "enabled": builder.isEnabled(item),
                "checkable": item.kind === "radio",
                "checked": item.kind === "radio" && item.checked === true
            });
            break;
        }
        builder.built.push(entry);
        menu.addMenuItem(entry);
    }

    /*! A menu whose visual parent is an item's action becomes that item's submenu. */
    function addSubmenu(entry, children) {
        const submenu = subMenu.createObject(entry);
        for (const child of children) {
            builder.add(submenu, child);
        }
        submenu.visualParent = entry.action;
        builder.built.push(submenu);
    }

    Component {
        id: menuItem

        PlasmaExtras.MenuItem {}
    }

    Component {
        id: choosable

        PlasmaExtras.MenuItem {
            property string itemId: ""

            onClicked: builder.chosen(itemId)
        }
    }

    Component {
        id: subMenu

        PlasmaExtras.Menu {}
    }

    PlasmaExtras.Menu {
        id: rootMenu

        visualParent: builder.visualParent
        placement: PlasmaExtras.Menu.BottomPosedLeftAlignedPopup
    }
}
