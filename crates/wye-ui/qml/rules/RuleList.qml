// RuleList: the Rules page's list (RUL-03 to RUL-07). Each row: a drag handle, the rule name with a dimmed one-line
// summary under it, the target's icon and name, a switch that turns the rule off, and a delete button. A turned-off rule is
// dimmed. Clicking a row (or Return/Space on it) edits it; right-click or the Menu key opens a menu (Edit, Duplicate,
// Move Up, Move Down, Delete); Delete deletes the focused row; Alt+Up and Alt+Down move it, so the order does not need a
// mouse (RUL-04).
//
// API
//   rows: var                  RulesBackend.rows(): [{index, name, summary, enabled, target}]
//   editable: bool             false for a read-only configuration: no dragging, switching or deleting
//   editRequested(int index)   / toggled(int index, bool on) / deleteRequested(int index) / duplicateRequested(int index)
//   moved(int from, int to)    a drag ended, or Move Up / Move Down
pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import dev.soldunov.wye.ui

Item {
    id: root

    property var rows: []
    property bool editable: true
    property int dragStart: -1

    signal editRequested(int index)
    signal toggled(int index, bool on)
    signal deleteRequested(int index)
    signal duplicateRequested(int index)
    signal moved(int from, int to)

    implicitHeight: list.contentHeight
    implicitWidth: list.implicitWidth

    function flat(row) {
        return {
            key: row.index + ":" + row.name,
            ruleIndex: row.index,
            name: row.name,
            summary: row.summary,
            ruleOn: !!row.enabled,
            target: JSON.stringify(row.target ?? {})
        };
    }

    // Updates the model in place, row by row, rather than clearing it: a cleared model destroys every delegate, and the
    // row, switch or button that had the keyboard focus with it (RUL-04, RUL-05).
    function sync() {
        const list = root.rows || [];
        list.forEach((row, at) => {
            if (at < listModel.count) {
                listModel.set(at, root.flat(row));
            } else {
                listModel.append(root.flat(row));
            }
        });
        if (listModel.count > list.length) {
            listModel.remove(list.length, listModel.count - list.length);
        }
    }

    // Move the rule at `from` by `step` rows (Move Up / Move Down, Alt+arrows). The model moves first, so the focused
    // delegate moves with its rule and the next Alt+arrow moves it again (RUL-04); the saved order then fills in each
    // row. The later sync puts the list back if the change was refused.
    function shift(from: int, step: int) {
        const to = from + step;
        if (root.editable && to >= 0 && to < listModel.count) {
            listModel.move(from, to, 1);
            root.moved(from, to);
            Qt.callLater(root.sync);
        }
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
            required property int ruleIndex
            required property string name
            required property string summary
            required property bool ruleOn
            required property string target

            // Reads the generation first, so the label follows the inventory.
            readonly property var shown: SettingsBackend.generation >= 0 ? JSON.parse(SettingsBackend.targetLabel("rule", holder.target, "")) : ({
                    "label": "",
                    "icon": ""
                })
            // A turned-off rule is dimmed (RUL-07); its switch and buttons are not.
            readonly property real contentOpacity: holder.ruleOn ? 1 : 0.5

            width: list.width
            height: rowItem.implicitHeight

            // The inset hairline between rows, as between the rows of a card (BLK-01).
            Kirigami.Separator {
                anchors {
                    top: parent.top
                    left: parent.left
                    right: parent.right
                    leftMargin: Kirigami.Units.largeSpacing
                    rightMargin: Kirigami.Units.largeSpacing
                }
                visible: holder.index > 0
            }

            QQC2.ItemDelegate {
                id: rowItem

                width: holder.width
                leftPadding: Kirigami.Units.smallSpacing
                rightPadding: Kirigami.Units.largeSpacing
                topPadding: Kirigami.Units.smallSpacing
                bottomPadding: Kirigami.Units.smallSpacing
                // The row's own highlight is the keyboard focus: the list has no current item.
                highlighted: visualFocus
                Accessible.name: holder.ruleOn ? holder.name : qsTr("%1 (off)").arg(holder.name)
                Accessible.description: holder.summary

                onClicked: root.editRequested(holder.ruleIndex)
                Keys.onDeletePressed: {
                    if (root.editable) {
                        root.deleteRequested(holder.ruleIndex);
                    }
                }
                Keys.onMenuPressed: rowMenu.popup(rowItem, Kirigami.Units.gridUnit, rowItem.height)
                Keys.onPressed: event => {
                    if (event.modifiers & Qt.AltModifier && (event.key === Qt.Key_Up || event.key === Qt.Key_Down)) {
                        root.shift(holder.ruleIndex, event.key === Qt.Key_Up ? -1 : 1);
                        event.accepted = true;
                    }
                }

                TapHandler {
                    acceptedButtons: Qt.RightButton
                    onTapped: rowMenu.popup()
                }

                QQC2.Menu {
                    id: rowMenu

                    QQC2.MenuItem {
                        icon.name: "document-edit-symbolic"
                        text: qsTr("Edit…")
                        onTriggered: root.editRequested(holder.ruleIndex)
                    }
                    QQC2.MenuItem {
                        enabled: root.editable
                        icon.name: "edit-copy-symbolic"
                        text: qsTr("Duplicate")
                        onTriggered: root.duplicateRequested(holder.ruleIndex)
                    }
                    QQC2.MenuSeparator {}
                    QQC2.MenuItem {
                        enabled: root.editable && holder.index > 0
                        icon.name: "go-up-symbolic"
                        text: qsTr("Move Up")
                        onTriggered: root.shift(holder.ruleIndex, -1)
                    }
                    QQC2.MenuItem {
                        enabled: root.editable && holder.index < listModel.count - 1
                        icon.name: "go-down-symbolic"
                        text: qsTr("Move Down")
                        onTriggered: root.shift(holder.ruleIndex, 1)
                    }
                    QQC2.MenuSeparator {}
                    QQC2.MenuItem {
                        enabled: root.editable
                        icon.name: "edit-delete-symbolic"
                        text: qsTr("Delete")
                        onTriggered: root.deleteRequested(holder.ruleIndex)
                    }
                }

                contentItem: RowLayout {
                    spacing: Kirigami.Units.largeSpacing

                    Kirigami.ListItemDragHandle {
                        enabled: root.editable
                        listItem: rowItem
                        listView: list
                        Accessible.name: qsTr("Drag to reorder")

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
                            // The list follows the configuration once the change is saved (or refused).
                            Qt.callLater(root.sync);
                        }
                    }

                    ColumnLayout {
                        Layout.fillWidth: true
                        Layout.minimumWidth: Kirigami.Units.gridUnit * 5
                        spacing: 0
                        opacity: holder.contentOpacity

                        QQC2.Label {
                            Layout.fillWidth: true
                            elide: Text.ElideRight
                            text: holder.name
                        }
                        QQC2.Label {
                            Layout.fillWidth: true
                            visible: text !== ""
                            color: Kirigami.Theme.disabledTextColor
                            elide: Text.ElideRight
                            font: Kirigami.Theme.smallFont
                            text: holder.summary
                        }
                    }

                    // The target: its icon and name (RUL-07), in a column of one width, so the icons line up from row to
                    // row. A long name is cut; its tooltip has it in full.
                    RowLayout {
                        Layout.preferredWidth: Kirigami.Units.gridUnit * 10
                        Layout.maximumWidth: Kirigami.Units.gridUnit * 10
                        spacing: Kirigami.Units.smallSpacing
                        opacity: holder.contentOpacity

                        Kirigami.Icon {
                            Layout.preferredHeight: Kirigami.Units.iconSizes.smallMedium
                            Layout.preferredWidth: Kirigami.Units.iconSizes.smallMedium
                            source: holder.shown.icon !== "" ? holder.shown.icon : "application-x-executable"
                        }
                        QQC2.Label {
                            id: targetLabel

                            Layout.fillWidth: true
                            elide: Text.ElideRight
                            text: holder.shown.label

                            HoverHandler {
                                id: targetHover
                            }
                            QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
                            QQC2.ToolTip.text: targetLabel.text
                            QQC2.ToolTip.visible: targetHover.hovered && targetLabel.truncated
                        }
                    }

                    QQC2.Switch {
                        checked: holder.ruleOn
                        enabled: root.editable
                        Accessible.name: qsTr("Use “%1”").arg(holder.name)
                        QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
                        QQC2.ToolTip.text: holder.ruleOn ? qsTr("Turn this rule off") : qsTr("Turn this rule on")
                        QQC2.ToolTip.visible: hovered
                        onToggled: {
                            root.toggled(holder.ruleIndex, checked);
                            checked = Qt.binding(() => holder.ruleOn);
                        }
                    }

                    QQC2.ToolButton {
                        id: deleteButton

                        display: QQC2.AbstractButton.IconOnly
                        enabled: root.editable
                        text: qsTr("Delete Rule")
                        Accessible.name: qsTr("Delete “%1”").arg(holder.name)
                        QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
                        QQC2.ToolTip.text: text
                        QQC2.ToolTip.visible: hovered
                        // A neutral trash glyph: Breeze draws `edit-delete` red, which on every row is loud. It turns negative
                        // under the pointer or focus.
                        icon.color: deleteButton.hovered || deleteButton.visualFocus ? Kirigami.Theme.negativeTextColor : Kirigami.Theme.textColor
                        icon.name: "user-trash-symbolic"
                        onClicked: root.deleteRequested(holder.ruleIndex)
                    }
                }
            }
        }
    }
}
