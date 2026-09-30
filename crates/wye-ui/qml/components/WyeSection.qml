// WyeSection (BLK-14): a bold section title with a small "+" button beside it, a dimmed subtitle, then a list card that
// shows a placeholder ("No Matchers") while it is empty.
//
// API
//   title: string          bold section title
//   subtitle: string       dimmed text under the title
//   emptyText: string      the placeholder inside the empty card
//   count: int             how many entries the list holds; 0 shows the placeholder
//   addText: string        the "+" button's tooltip and accessible name
//   addEnabled: bool       default true
//   addTriggered()         "+" was pressed
//   default property       the list's rows (WyeRow variants), inside the card
pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.kde.kirigamiaddons.formcard as FormCard

ColumnLayout {
    id: section

    property string title
    property string subtitle
    property string emptyText
    property string addText: qsTr("Add")
    property bool addEnabled: true
    property int count: 0
    default property alias rows: card.delegates
    signal addTriggered

    Layout.fillWidth: true
    spacing: 0

    RowLayout {
        Layout.leftMargin: Kirigami.Units.largeSpacing + Kirigami.Units.smallSpacing
        Layout.rightMargin: Kirigami.Units.largeSpacing
        Layout.topMargin: Kirigami.Units.largeSpacing

        QQC2.Label {
            font.weight: Font.Bold
            text: section.title
        }

        QQC2.ToolButton {
            display: QQC2.AbstractButton.IconOnly
            enabled: section.addEnabled
            icon.name: "list-add"
            text: section.addText
            QQC2.ToolTip.text: text
            QQC2.ToolTip.visible: hovered
            onClicked: section.addTriggered()
        }

        Item {
            Layout.fillWidth: true
        }
    }

    QQC2.Label {
        Layout.bottomMargin: Kirigami.Units.smallSpacing
        Layout.fillWidth: true
        Layout.leftMargin: Kirigami.Units.largeSpacing + Kirigami.Units.smallSpacing
        Layout.rightMargin: Kirigami.Units.largeSpacing
        color: Kirigami.Theme.disabledTextColor
        font: Kirigami.Theme.smallFont
        text: section.subtitle
        visible: section.subtitle !== ""
        wrapMode: Text.WordWrap
    }

    FormCard.FormCard {
        id: card

        maximumWidth: section.width - Kirigami.Units.largeSpacing * 2

        QQC2.Label {
            Layout.fillWidth: true
            Layout.margins: Kirigami.Units.largeSpacing
            color: Kirigami.Theme.disabledTextColor
            horizontalAlignment: Text.AlignHCenter
            text: section.emptyText
            visible: section.count === 0
        }
    }
}
