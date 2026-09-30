// RuleList: the Rules page's list (RUL-03 to RUL-07). Each row: a drag handle, the rule name with a dimmed one-line
// summary under it, the target's icon and name, a switch that turns the rule off, and a delete button. A turned-off rule is
// dimmed. Clicking a row edits it; right-click opens a menu (Edit, Duplicate, Delete); Delete deletes the focused row.
//
// API
//   rows: var                  RulesBackend.rows(): [{index, name, summary, enabled, target}]
//   editable: bool             false for a read-only configuration: no dragging, switching or deleting
//   editRequested(int index)   / toggled(int index, bool on) / deleteRequested(int index) / duplicateRequested(int index)
//   moved(int from, int to)    a drag ended
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

    function sync() {
        const list = root.rows || [];
        listModel.clear();
        list.forEach(row => listModel.append(root.flat(row)));
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

            width: list.width
            height: rowItem.implicitHeight

            QQC2.ItemDelegate {
                id: rowItem

                width: holder.width
                padding: Kirigami.Units.smallSpacing
                Accessible.name: holder.name
                Accessible.description: holder.summary

                onClicked: root.editRequested(holder.ruleIndex)
                Keys.onDeletePressed: {
                    if (root.editable) {
                        root.deleteRequested(holder.ruleIndex);
                    }
                }

                TapHandler {
                    acceptedButtons: Qt.RightButton
                    onTapped: rowMenu.popup()
                }

                QQC2.Menu {
                    id: rowMenu

                    QQC2.MenuItem {
                        icon.name: "document-edit"
                        text: qsTr("Edit…")
                        onTriggered: root.editRequested(holder.ruleIndex)
                    }
                    QQC2.MenuItem {
                        enabled: root.editable
                        icon.name: "edit-copy"
                        text: qsTr("Duplicate")
                        onTriggered: root.duplicateRequested(holder.ruleIndex)
                    }
                    QQC2.MenuSeparator {}
                    QQC2.MenuItem {
                        enabled: root.editable
                        icon.name: "edit-delete"
                        text: qsTr("Delete")
                        onTriggered: root.deleteRequested(holder.ruleIndex)
                    }
                }

                contentItem: RowLayout {
                    spacing: Kirigami.Units.smallSpacing

                    Kirigami.ListItemDragHandle {
                        enabled: root.editable
                        listItem: rowItem
                        listView: list

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
                        spacing: 0
                        opacity: holder.ruleOn ? 1 : 0.5

                        QQC2.Label {
                            Layout.fillWidth: true
                            elide: Text.ElideRight
                            text: holder.name
                        }
                        QQC2.Label {
                            Layout.fillWidth: true
                            color: Kirigami.Theme.disabledTextColor
                            elide: Text.ElideRight
                            font: Kirigami.Theme.smallFont
                            text: holder.summary
                        }
                    }

                    Kirigami.Icon {
                        Layout.preferredHeight: Kirigami.Units.iconSizes.small
                        Layout.preferredWidth: Kirigami.Units.iconSizes.small
                        opacity: holder.ruleOn ? 1 : 0.5
                        source: holder.shown.icon !== "" ? holder.shown.icon : "application-x-executable"
                    }
                    QQC2.Label {
                        Layout.maximumWidth: Kirigami.Units.gridUnit * 7
                        elide: Text.ElideRight
                        opacity: holder.ruleOn ? 1 : 0.5
                        text: holder.shown.label
                    }

                    QQC2.Switch {
                        checked: holder.ruleOn
                        enabled: root.editable
                        Accessible.name: qsTr("Rule on")
                        onToggled: {
                            root.toggled(holder.ruleIndex, checked);
                            checked = Qt.binding(() => holder.ruleOn);
                        }
                    }

                    QQC2.ToolButton {
                        display: QQC2.AbstractButton.IconOnly
                        enabled: root.editable
                        icon.name: "edit-delete"
                        text: qsTr("Delete Rule")
                        QQC2.ToolTip.text: text
                        QQC2.ToolTip.visible: hovered
                        onClicked: root.deleteRequested(holder.ruleIndex)
                    }
                }
            }
        }
    }
}
