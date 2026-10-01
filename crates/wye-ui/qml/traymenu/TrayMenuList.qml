pragma ComponentBehavior: Bound

// A panel of tray-menu rows (TRAY-08): the menu itself, or a submenu
// beside its parent (TRAY-15). Only interactive rows take the highlight.
//
// Drawn as a Plasma menu (qqc2-desktop-style's Menu): the Window colours, a
// rounded frame in the theme's frame contrast, a soft shadow, and as wide as
// its widest row, within a minimum and a maximum.
import QtQuick
import org.kde.kirigami as Kirigami

Item {
    id: list

    // Rows of TrayMenuBackend.rows.
    property var entries: []
    property int currentIndex: -1
    // Whether this panel opened to the left of its parent (a submenu with no
    // room on the right); the next level then prefers the left too.
    property bool leftward: false
    readonly property int padding: Kirigami.Units.smallSpacing
    readonly property var currentEntry: currentIndex >= 0 && currentIndex < entries.length ? entries[currentIndex] : null
    // Whether a row has a check mark: then every row keeps the check column.
    readonly property bool hasChecks: entries.some(entry => entry.kind === "radio" || entry.checked)
    // The widest row's content, so the menu fits it (as QMenu does).
    readonly property real widestRow: {
        let widest = 0;
        for (let i = 0; i < rows.count; ++i) {
            const item = rows.itemAt(i) as TrayMenuRow;
            if (item) {
                widest = Math.max(widest, item.contentWidth);
            }
        }
        return widest;
    }

    // The pointer or the keyboard moved onto a row.
    signal pointed(int index)
    // A row was clicked.
    signal chosen(int index)

    // Move the highlight by `step` to the next interactive row, wrapping.
    function move(step) {
        const count = entries.length;
        for (let tried = 1; tried <= count; ++tried) {
            const start = currentIndex < 0 ? (step > 0 ? -1 : 0) : currentIndex;
            const index = ((start + step * tried) % count + count) % count;
            const entry = entries[index];
            if (entry.selectable || entry.opens) {
                currentIndex = index;
                pointed(index);
                return;
            }
        }
    }

    // The row's top edge, relative to the panel.
    function rowY(index) {
        const item = rows.itemAt(index);
        return item ? item.y + padding : 0;
    }

    Accessible.role: Accessible.PopupMenu
    // Plus a grid unit of air at the right, as a QMenu leaves after its longest label.
    width: Math.round(Math.min(Kirigami.Units.gridUnit * 24, Math.max(Kirigami.Units.gridUnit * 12, widestRow + padding * 2 + Kirigami.Units.gridUnit)))
    height: column.implicitHeight + padding * 2

    Kirigami.ShadowedRectangle {
        anchors.fill: parent
        radius: Kirigami.Units.cornerRadius
        color: Kirigami.Theme.backgroundColor
        border.color: Kirigami.ColorUtils.linearInterpolation(Kirigami.Theme.backgroundColor, Kirigami.Theme.textColor, Kirigami.Theme.frameContrast)
        border.width: 1
        shadow.xOffset: 0
        shadow.yOffset: 2
        shadow.color: Qt.rgba(0, 0, 0, 0.3)
        shadow.size: Kirigami.Units.smallSpacing * 2
    }

    // Clicks between rows do not close the menu.
    MouseArea {
        anchors.fill: parent
        acceptedButtons: Qt.AllButtons
    }

    Column {
        id: column

        x: list.padding
        y: list.padding
        width: list.width - list.padding * 2

        Repeater {
            id: rows

            model: list.entries

            TrayMenuRow {
                required property var modelData
                required property int index

                width: column.width
                entry: modelData
                checkColumn: list.hasChecks
                current: index === list.currentIndex
                onPointed: {
                    list.currentIndex = index;
                    list.pointed(index);
                }
                onChosen: list.chosen(index)
            }
        }
    }
}
