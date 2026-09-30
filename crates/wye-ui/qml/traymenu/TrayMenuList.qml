pragma ComponentBehavior: Bound

// A panel of tray-menu rows (TRAY-08): the menu itself, or the submenu
// beside it (TRAY-15). Only interactive rows take the highlight.
import QtQuick
import org.kde.kirigami as Kirigami

Rectangle {
    id: list

    // Rows of TrayMenuBackend.rows.
    property var entries: []
    property int currentIndex: -1
    readonly property int padding: Kirigami.Units.smallSpacing
    readonly property var currentEntry: currentIndex >= 0 && currentIndex < entries.length ? entries[currentIndex] : null

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

    Kirigami.Theme.colorSet: Kirigami.Theme.View
    Kirigami.Theme.inherit: false
    width: Kirigami.Units.gridUnit * 18
    height: column.implicitHeight + padding * 2
    radius: Kirigami.Units.cornerRadius
    color: Kirigami.Theme.backgroundColor
    border.width: 1
    border.color: Qt.alpha(Kirigami.Theme.textColor, 0.15)

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
