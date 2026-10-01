// WyeChecklist (BLK-15): rows with a checkbox, an icon, a name, an optional trailing control (a popup), and a drag handle on
// checked rows. Checked rows come first; dragging a handle reorders them. Used for the shown browsers sheet (SHOWN-03);
// rows about 32 px tall, icons 16 px.
//
// API
//   rows: var              array of {key: string, name, icon, checked: bool, missing: bool, removable: bool, …}; `key`
//                          identifies the row. Checked rows must come first. Any extra fields reach `trailing`.
//   trailing: Component    optional per-row control, shown before the drag handle. The component declares
//                          `property var row` and receives the row's data there.
//   toggled(key, checked)  a checkbox was clicked
//   moved(from, to)        a drag ended; indices among the rows (checked ones), in the order shown
//   removeRequested(key)   the delete button of a `removable` row (SHOWN-08)
pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

Item {
    id: root

    property var rows: []
    property Component trailing
    property int dragStart: -1
    // Some row can be removed: every row keeps the remove button's room, so the trailing controls line up.
    readonly property bool anyRemovable: rows.some(row => !!row.removable)
    signal toggled(string key, bool checked)
    signal moved(int from, int to)
    signal removeRequested(string key)

    implicitHeight: list.contentHeight
    implicitWidth: list.implicitWidth

    // A ListModel row cannot hold nested objects, so the row's own data stays in `rows` and the model keeps what the
    // delegate draws, plus the index back into `rows`.
    function flat(row, index) {
        return {
            key: row.key,
            name: row.name,
            icon: row.icon || "",
            checked: !!row.checked,
            missing: !!row.missing,
            removable: !!row.removable,
            source: index
        };
    }

    function sync() {
        const list = root.rows || [];
        const sameKeys = listModel.count === list.length && list.every((row, index) => listModel.get(index).key === row.key);
        if (sameKeys) {
            list.forEach((row, index) => listModel.set(index, root.flat(row, index)));
            return;
        }
        listModel.clear();
        list.forEach((row, index) => listModel.append(root.flat(row, index)));
    }

    onRowsChanged: sync()
    Component.onCompleted: sync()

    ListModel {
        id: listModel
    }

    ListView {
        id: list

        anchors.fill: parent
        implicitHeight: contentHeight
        interactive: false
        model: listModel

        moveDisplaced: Transition {
            YAnimator {
                duration: Kirigami.Units.shortDuration
                easing.type: Easing.InOutQuad
            }
        }

        delegate: Item {
            id: holder

            required property int index
            required property string key
            required property string name
            required property string icon
            required property bool checked
            required property bool missing
            required property bool removable
            required property int source

            width: list.width
            height: rowItem.implicitHeight

            QQC2.ItemDelegate {
                id: rowItem

                width: holder.width
                hoverEnabled: false
                padding: Kirigami.Units.smallSpacing

                contentItem: RowLayout {
                    spacing: Kirigami.Units.smallSpacing

                    QQC2.CheckBox {
                        checked: holder.checked
                        Accessible.name: holder.name
                        onToggled: {
                            root.toggled(holder.key, checked);
                            checked = Qt.binding(() => holder.checked);
                        }
                    }

                    Kirigami.Icon {
                        Layout.preferredHeight: Kirigami.Units.iconSizes.small
                        Layout.preferredWidth: Kirigami.Units.iconSizes.small
                        fallback: "internet-web-browser"
                        source: holder.missing ? "dialog-warning" : (holder.icon !== "" ? holder.icon : "application-x-executable")
                    }

                    QQC2.Label {
                        Layout.fillWidth: true
                        elide: Text.ElideRight
                        opacity: holder.checked ? 1 : 0.7
                        text: holder.name
                    }

                    Loader {
                        id: trailingLoader

                        active: root.trailing !== null
                        sourceComponent: root.trailing
                    }

                    Binding {
                        property: "row"
                        target: trailingLoader.item
                        value: root.rows[holder.source]
                        when: trailingLoader.item !== null
                    }

                    QQC2.ToolButton {
                        Accessible.name: qsTr("Remove %1").arg(holder.name)
                        display: QQC2.AbstractButton.IconOnly
                        enabled: holder.removable
                        icon.name: "list-remove-symbolic"
                        opacity: holder.removable ? 1 : 0
                        text: qsTr("Remove")
                        visible: root.anyRemovable
                        onClicked: root.removeRequested(holder.key)

                        QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
                        QQC2.ToolTip.text: qsTr("Remove from the list")
                        QQC2.ToolTip.visible: hovered
                    }

                    Kirigami.ListItemDragHandle {
                        listItem: rowItem
                        listView: list
                        opacity: holder.checked ? 1 : 0
                        enabled: holder.checked

                        onMoveRequested: (oldIndex, newIndex) => {
                            if (root.dragStart < 0) {
                                root.dragStart = oldIndex;
                            }
                            listModel.move(oldIndex, newIndex, 1);
                        }
                        onDropped: (oldIndex, newIndex) => {
                            const from = root.dragStart;
                            root.dragStart = -1;
                            if (from >= 0 && from !== newIndex) {
                                root.moved(from, newIndex);
                            }
                            // Put the rows back as the data has them if the change was refused.
                            Qt.callLater(root.sync);
                        }
                    }
                }
            }
        }
    }
}
