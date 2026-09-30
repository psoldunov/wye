// HistoryRow (DLG-HIS-02, DLG-HIS-03): one opened link. The target's icon; the final link with its host emphasised and the
// rest dimmed and cut in the middle; a second line with the source app, the target, the time, why, and what changed the
// link. The original link is in the tooltip when the link was changed. Double-click or Enter opens it through the picker;
// the context menu and the "…" button offer the other actions.
//
// API
//   modelData: var         the row: `Row` of crates/wye-ui/src/history/view.rs (id, time, host, rest, finalUrl, originalUrl,
//                          changed, source, sourceName, targetName, icon, sameTarget, reason, reasonKind, badges)
//   pickerRequested(id)        Open in Picker (DLG-HIS-03)
//   sameTargetRequested(id)    Open in <target> Again
//   copyRequested(text)        Copy Link, Copy Original Link
//   ruleRequested(id)          Create Rule…
//   deleteRequested(id)        Delete Entry
pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

QQC2.ItemDelegate {
    id: row

    required property var modelData
    signal pickerRequested(real id)
    signal sameTargetRequested(real id)
    signal copyRequested(string text)
    signal ruleRequested(real id)
    signal deleteRequested(real id)

    // "14:32" today, the short date and time otherwise.
    readonly property string timeLabel: {
        const when = new Date(row.modelData.time * 1000);
        const sameDay = when.toDateString() === new Date().toDateString();
        return sameDay ? Qt.formatTime(when, Locale.ShortFormat) : Qt.formatDateTime(when, Locale.ShortFormat);
    }
    // "from Slack · Firefox · 14:32"
    readonly property string details: [row.modelData.sourceName ? qsTr("from %1").arg(row.modelData.sourceName) : "", row.modelData.targetName, row.timeLabel].filter(part => part !== "").join(" · ")
    readonly property color reasonTint: {
        switch (row.modelData.reasonKind) {
        case "rule":
            return Kirigami.Theme.highlightColor;
        case "mapping":
            return Kirigami.Theme.positiveTextColor;
        case "alternative":
            return Kirigami.Theme.neutralTextColor;
        default:
            return Kirigami.Theme.disabledTextColor;
        }
    }

    Accessible.name: row.modelData.host + row.modelData.rest + ", " + row.details + ", " + row.modelData.reason
    hoverEnabled: true
    QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
    QQC2.ToolTip.text: qsTr("Original link: %1").arg(row.modelData.originalUrl)
    QQC2.ToolTip.visible: row.hovered && row.modelData.changed
    width: ListView.view ? ListView.view.width : implicitWidth

    // DLG-HIS-03: double-click or Enter opens the link through the picker.
    onDoubleClicked: row.pickerRequested(row.modelData.id)
    Keys.onEnterPressed: row.pickerRequested(row.modelData.id)
    Keys.onReturnPressed: row.pickerRequested(row.modelData.id)

    TapHandler {
        acceptedButtons: Qt.RightButton
        onTapped: menu.popup()
    }

    contentItem: RowLayout {
        spacing: Kirigami.Units.largeSpacing

        Kirigami.Icon {
            Layout.alignment: Qt.AlignTop
            Layout.preferredHeight: Kirigami.Units.iconSizes.medium
            Layout.preferredWidth: Kirigami.Units.iconSizes.medium
            source: row.modelData.icon
        }

        ColumnLayout {
            Layout.fillWidth: true
            spacing: 0

            RowLayout {
                Layout.fillWidth: true
                spacing: 0

                QQC2.Label {
                    Layout.maximumWidth: row.width * 0.5
                    elide: Text.ElideRight
                    font.weight: Font.Bold
                    text: row.modelData.host
                }

                QQC2.Label {
                    Layout.fillWidth: true
                    elide: Text.ElideMiddle
                    opacity: 0.7
                    text: row.modelData.rest
                }
            }

            RowLayout {
                Layout.fillWidth: true
                spacing: Kirigami.Units.smallSpacing

                QQC2.Label {
                    Layout.fillWidth: true
                    color: Kirigami.Theme.disabledTextColor
                    elide: Text.ElideRight
                    font: Kirigami.Theme.smallFont
                    text: row.details
                }

                HistoryBadge {
                    text: row.modelData.reason
                    tint: row.reasonTint
                    visible: text !== ""
                }

                Repeater {
                    model: row.modelData.badges

                    HistoryBadge {
                        required property string modelData

                        text: modelData
                        tint: Kirigami.Theme.neutralTextColor
                    }
                }
            }
        }

        QQC2.ToolButton {
            Layout.alignment: Qt.AlignVCenter
            display: QQC2.AbstractButton.IconOnly
            icon.name: "overflow-menu"
            text: qsTr("Actions")
            QQC2.ToolTip.text: text
            QQC2.ToolTip.visible: hovered
            onClicked: menu.popup()
        }
    }

    // DLG-HIS-03
    QQC2.Menu {
        id: menu

        QQC2.MenuItem {
            icon.name: "view-list-text"
            text: qsTr("Open in Picker")
            onTriggered: row.pickerRequested(row.modelData.id)
        }

        QQC2.MenuItem {
            icon.name: "document-open"
            text: qsTr("Open in %1 Again").arg(row.modelData.targetName)
            visible: row.modelData.sameTarget
            height: visible ? implicitHeight : 0
            onTriggered: row.sameTargetRequested(row.modelData.id)
        }

        QQC2.MenuSeparator {}

        QQC2.MenuItem {
            icon.name: "edit-copy"
            text: qsTr("Copy Link")
            onTriggered: row.copyRequested(row.modelData.finalUrl)
        }

        QQC2.MenuItem {
            icon.name: "edit-copy"
            text: qsTr("Copy Original Link")
            visible: row.modelData.changed
            height: visible ? implicitHeight : 0
            onTriggered: row.copyRequested(row.modelData.originalUrl)
        }

        QQC2.MenuItem {
            icon.name: "list-add"
            text: qsTr("Create Rule…")
            onTriggered: row.ruleRequested(row.modelData.id)
        }

        QQC2.MenuSeparator {}

        QQC2.MenuItem {
            icon.name: "edit-delete"
            text: qsTr("Delete Entry")
            onTriggered: row.deleteRequested(row.modelData.id)
        }
    }
}
