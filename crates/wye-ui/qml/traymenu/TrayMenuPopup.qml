pragma ComponentBehavior: Bound

// The tray-menu popup for the toggle-menu shortcut (TRAY-08, spec decision
// #14): the tray's own menu, drawn in a transparent layer-shell overlay with
// its corner at the pointer, or centred when the pointer is unknown. A
// second toggle, a click outside, Escape or focus loss closes it. Choosing
// an item sends it to the service (ActivateTrayItem) and closes the menu.
// The logic is in TrayMenuBackend (src/bridge/tray_menu.rs, src/tray_menu/).
//
// Surface contract (crates/wye-ui/src/route.rs):
//   handle("toggle", "", ShowMenu payload)  show it, or hide it when shown
import QtQuick
import org.kde.kirigami as Kirigami
import org.kde.layershell as LayerShell
import dev.soldunov.wye.ui

Window {
    id: popup

    readonly property int margin: Kirigami.Units.largeSpacing
    property bool wasActive: false
    // Whether the keyboard is in the submenu.
    property bool inSubmenu: false

    function handle(action, key, argument) {
        if (action !== "toggle") {
            return;
        }
        if (visible) {
            dismiss();
            return;
        }
        if (!backend.load(argument)) {
            return;
        }
        menu.currentIndex = -1;
        closeSubmenu();
        chooseScreen();
        wasActive = false;
        show();
        requestActivate();
        keyboard.forceActiveFocus();
    }

    function dismiss() {
        closeSubmenu();
        hide();
    }

    // The output the pointer is on (as the picker, PICK-02).
    function chooseScreen() {
        if (!backend.placed) {
            return;
        }
        // qmllint disable missing-property
        const screens = Qt.application.screens;
        // qmllint enable missing-property
        for (let i = 0; i < screens.length; ++i) {
            if (screens[i].name === backend.placementOutput) {
                popup.screen = screens[i];
                return;
            }
        }
    }

    function clamp(value, low, high) {
        return Math.max(low, Math.min(value, high));
    }

    function openSubmenu(index) {
        const entry = menu.entries[index];
        if (!entry || !entry.opens) {
            closeSubmenu();
            return;
        }
        submenu.entries = entry.children;
        submenu.currentIndex = -1;
        submenu.anchorY = menu.y + menu.rowY(index);
        submenu.visible = true;
    }

    function closeSubmenu() {
        submenu.visible = false;
        submenu.entries = [];
        inSubmenu = false;
    }

    // Run a top-level `entry`: open its submenu, or send it and close.
    function choose(entry, index) {
        if (entry.opens) {
            openSubmenu(index);
        } else {
            activate(entry);
        }
    }

    // Send `entry` to the service and close.
    function activate(entry) {
        if (entry.selectable && backend.activate(entry.id)) {
            dismiss();
        }
    }

    function keyPressed(event) {
        const list = inSubmenu ? submenu : menu;
        switch (event.key) {
        case Qt.Key_Down:
            list.move(1);
            return true;
        case Qt.Key_Up:
            list.move(-1);
            return true;
        case Qt.Key_Right:
            if (!inSubmenu && menu.currentEntry && menu.currentEntry.opens) {
                openSubmenu(menu.currentIndex);
                inSubmenu = true;
                submenu.move(1);
            }
            return true;
        case Qt.Key_Left:
            if (inSubmenu) {
                closeSubmenu();
            }
            return true;
        case Qt.Key_Escape:
            if (inSubmenu) {
                closeSubmenu();
            } else {
                dismiss();
            }
            return true;
        case Qt.Key_Return:
        case Qt.Key_Enter:
        case Qt.Key_Space:
            if (inSubmenu) {
                if (submenu.currentEntry) {
                    activate(submenu.currentEntry);
                }
            } else if (menu.currentEntry) {
                choose(menu.currentEntry, menu.currentIndex);
                if (menu.currentEntry.opens) {
                    inSubmenu = true;
                    submenu.move(1);
                }
            }
            return true;
        default:
            break;
        }
        // KEY-51: the fixed accelerators of the top-level items.
        const id = backend.accelerator(event.text);
        if (id !== "" && backend.activate(id)) {
            dismiss();
            return true;
        }
        return false;
    }

    title: qsTr("Wye")
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
            dismiss();
        }
    }

    LayerShell.Window.layer: LayerShell.Window.LayerOverlay
    LayerShell.Window.anchors: LayerShell.Window.AnchorTop | LayerShell.Window.AnchorBottom | LayerShell.Window.AnchorLeft | LayerShell.Window.AnchorRight
    LayerShell.Window.exclusionZone: -1
    LayerShell.Window.keyboardInteractivity: LayerShell.Window.KeyboardInteractivityExclusive
    LayerShell.Window.wantsToBeOnActiveScreen: !backend.placed
    LayerShell.Window.scope: "wye-tray-menu"

    TrayMenuBackend {
        id: backend
    }

    // A click outside the menu closes it.
    MouseArea {
        anchors.fill: parent
        acceptedButtons: Qt.AllButtons
        onPressed: popup.dismiss()
    }

    Item {
        id: keyboard

        focus: true
        Keys.onPressed: event => {
            event.accepted = popup.keyPressed(event);
        }
    }

    TrayMenuList {
        id: menu

        x: backend.placed ? popup.clamp(backend.placementX, popup.margin, popup.width - menu.width - popup.margin) : (popup.width - menu.width) / 2
        y: backend.placed ? popup.clamp(backend.placementY, popup.margin, popup.height - menu.height - popup.margin) : (popup.height - menu.height) / 2
        entries: JSON.parse(backend.rows || "[]")
        onPointed: index => {
            popup.inSubmenu = false;
            popup.openSubmenu(index);
        }
        onChosen: index => popup.choose(menu.entries[index], index)
    }

    // TRAY-15: beside the menu, on the side with room.
    TrayMenuList {
        id: submenu

        property real anchorY: 0

        visible: false
        x: menu.x + menu.width + submenu.width <= popup.width ? menu.x + menu.width : menu.x - submenu.width
        y: popup.clamp(submenu.anchorY - submenu.padding, popup.margin, popup.height - submenu.height - popup.margin)
        onPointed: popup.inSubmenu = true
        onChosen: index => popup.activate(submenu.entries[index])
    }
}
