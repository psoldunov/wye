pragma ComponentBehavior: Bound

// The tray-menu popup for the toggle-menu shortcut (TRAY-08, spec decision
// #14): the tray's own menu, drawn in a transparent layer-shell overlay with
// its corner at the pointer, or centred when the pointer is unknown. A
// second toggle, a click outside, Escape or focus loss closes it. Choosing
// an item sends it to the service (ActivateTrayItem) and closes the menu.
// Submenus nest to any depth (TRAY-15: More, then Recent Links): level 0 is
// the menu, levels 1..N are lists that open beside their parent row.
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
    // The span submenus keep to. The popup covers the whole output, panels included (the layer shell's work area is
    // not known here), and the menu opens beside the tray icon, so against the panel the icon is on: no submenu goes
    // past the menu's edge on that side, as Plasma's own menus do not cover the panel (TRAY-15). Unplaced, the screen
    // is the limit.
    readonly property bool menuLow: backend.placed && menu.y + menu.height / 2 >= popup.height / 2
    readonly property real levelTop: backend.placed && !menuLow ? Math.min(menu.y, popup.height / 2) : popup.margin
    readonly property real levelBottom: menuLow ? Math.max(menu.y + menu.height, popup.height / 2) : popup.height - popup.margin
    property bool wasActive: false
    // The level the keyboard is in: 0 is the menu, N the Nth submenu.
    property int depth: 0
    // TRAY-21: Ctrl or Shift is held, so a radio row opens its browser
    // rather than making it primary (TRAY-20); the radio rows hide their
    // marks. Learnt from key events and pointer motion: a key held before
    // the popup opened shows once the pointer moves or another key goes.
    property bool openHeld: false

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
        openHeld = false;
        closeSubmenus(1);
        chooseScreen();
        wasActive = false;
        show();
        requestActivate();
        keyboard.forceActiveFocus();
    }

    function dismiss() {
        closeSubmenus(1);
        openHeld = false;
        hide();
    }

    // TRAY-21: whether `modifiers` hold Ctrl or Shift (Alt and Super do not
    // count, TRAY-20).
    function trackModifiers(modifiers) {
        openHeld = (modifiers & (Qt.ControlModifier | Qt.ShiftModifier)) !== 0;
    }

    // The modifiers after key event `event`: whether a press of Ctrl or Shift
    // already carries its own modifier, and a release still does, differs
    // between platforms, so the key itself decides.
    function trackKey(event, pressed) {
        let modifiers = event.modifiers;
        const own = event.key === Qt.Key_Control ? Qt.ControlModifier : event.key === Qt.Key_Shift ? Qt.ShiftModifier : 0;
        if (own !== 0) {
            modifiers = pressed ? modifiers | own : modifiers & ~own;
        }
        trackModifiers(modifiers);
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

    // How many submenus deep `entries` nest: the levels the popup needs.
    function nesting(entries) {
        let deepest = 0;
        for (const entry of entries) {
            if (entry.opens) {
                deepest = Math.max(deepest, 1 + nesting(entry.children));
            }
        }
        return deepest;
    }

    // The list at `level`: the menu at 0, else a submenu level.
    function listAt(level) {
        return level === 0 ? menu : levels.itemAt(level - 1);
    }

    // Open the submenu of row `index` of the list at `level`, closing every
    // level below it first.
    function openSubmenu(level, index) {
        closeSubmenus(level + 1);
        const parent = listAt(level);
        const entry = parent.entries[index];
        const next = listAt(level + 1);
        if (!entry || !entry.opens || !next) {
            return;
        }
        next.parentList = parent;
        next.entries = entry.children;
        next.currentIndex = -1;
        next.anchorY = parent.y + parent.rowY(index);
        next.visible = true;
    }

    // Hide and empty the list at `level` and every one below it.
    function closeSubmenus(level) {
        for (let i = Math.max(level, 1); i <= levels.count; ++i) {
            const list = listAt(i);
            if (list) {
                list.visible = false;
                list.entries = [];
            }
        }
        depth = Math.max(0, Math.min(depth, level - 1));
    }

    // Run row `index` of the list at `level`: open its submenu, or send it
    // and close.
    function choose(level, index) {
        const entry = listAt(level).entries[index];
        if (!entry) {
            return;
        }
        if (entry.opens) {
            openSubmenu(level, index);
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

    // Open the submenu of the keyboard's row and move into it.
    function descend() {
        openSubmenu(depth, listAt(depth).currentIndex);
        depth += 1;
        listAt(depth).move(1);
    }

    function keyPressed(event) {
        const list = listAt(depth);
        const entry = list.currentEntry;
        switch (event.key) {
        case Qt.Key_Down:
            list.move(1);
            return true;
        case Qt.Key_Up:
            list.move(-1);
            return true;
        case Qt.Key_Right:
            if (entry && entry.opens) {
                descend();
            }
            return true;
        case Qt.Key_Left:
            if (depth > 0) {
                closeSubmenus(depth);
            }
            return true;
        case Qt.Key_Escape:
            if (depth > 0) {
                closeSubmenus(depth);
            } else {
                dismiss();
            }
            return true;
        case Qt.Key_Return:
        case Qt.Key_Enter:
        case Qt.Key_Space:
            if (entry && entry.opens) {
                descend();
            } else if (entry) {
                activate(entry);
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
        hoverEnabled: true
        onPressed: popup.dismiss()
        onPositionChanged: mouse => popup.trackModifiers(mouse.modifiers)
    }

    Item {
        id: keyboard

        focus: true
        Keys.onPressed: event => {
            popup.trackKey(event, true);
            event.accepted = popup.keyPressed(event);
        }
        Keys.onReleased: event => {
            popup.trackKey(event, false);
            event.accepted = false;
        }
    }

    TrayMenuList {
        id: menu

        x: backend.placed ? popup.clamp(backend.placementX, popup.margin, popup.width - menu.width - popup.margin) : (popup.width - menu.width) / 2
        y: backend.placed ? popup.clamp(backend.placementY, popup.margin, popup.height - menu.height - popup.margin) : (popup.height - menu.height) / 2
        entries: JSON.parse(backend.rows || "[]")
        openHeld: popup.openHeld
        onModifiersMoved: modifiers => popup.trackModifiers(modifiers)
        onPointed: row => {
            popup.depth = 0;
            popup.openSubmenu(0, row);
        }
        onChosen: row => popup.choose(0, row)
    }

    // TRAY-15: one list per nesting level, each beside its parent row on the
    // side with room. The levels persist while submenus open and close, so a
    // parent keeps its highlight.
    Repeater {
        id: levels

        model: popup.nesting(menu.entries)

        TrayMenuList {
            id: level

            required property int index
            // The list this one opens from, and the row's top edge in the popup.
            property TrayMenuList parentList: null
            property real anchorY: 0
            readonly property bool roomRight: level.parentList !== null && level.parentList.x + level.parentList.width + level.width <= popup.width - popup.margin
            readonly property bool roomLeft: level.parentList !== null && level.parentList.x - level.width >= popup.margin

            visible: false
            // Left when the right has no room, or when the parent went left and
            // the left has room, so a chain near the right edge keeps going left.
            leftward: !roomRight || (level.parentList !== null && level.parentList.leftward && roomLeft)
            x: level.parentList === null ? 0 : (leftward ? level.parentList.x - level.width : level.parentList.x + level.parentList.width)
            y: popup.clamp(level.anchorY - level.padding, popup.levelTop, popup.levelBottom - level.height)
            openHeld: popup.openHeld
            onModifiersMoved: modifiers => popup.trackModifiers(modifiers)
            onPointed: row => {
                popup.depth = level.index + 1;
                popup.openSubmenu(level.index + 1, row);
            }
            onChosen: row => popup.choose(level.index + 1, row)
        }
    }
}
