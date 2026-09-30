// WyeSheet (BLK-11): a modal panel over the settings window. The window behind is dimmed and inert. Own header with a
// title, a scrolling body, and a footer that stays pinned: an optional note, optional leading controls, and the buttons.
// The primary button uses the accent colour and stays disabled until the content is valid. Escape closes the sheet, not
// the window behind it.
//
// API
//   title: string          header title ("New Rule")
//   note: string          optional dimmed note in the footer
//   primaryText: string    the primary button ("Done", "Save"); empty for none
//   primaryEnabled: bool   false until the content is valid
//   secondaryText: string  the other button ("Cancel"); empty for none
//   primaryTriggered() / secondaryTriggered()   a button was pressed; the sheet does not close itself: call close()
//   default property       the body: items, laid out in a column that scrolls
//   footerLeading          items at the footer's left ("+" of the shown browsers sheet, "Browse…")
//   sheetWidth: real       default about 380 px (docs/spec/05-browsers.md)
//   open() / close()       as any popup
pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import dev.soldunov.wye.ui

QQC2.Dialog {
    id: sheet

    property string note
    property string primaryText
    property bool primaryEnabled: true
    property string secondaryText
    property real sheetWidth: Kirigami.Units.gridUnit * 21
    default property alias body: bodyColumn.data
    property alias footerLeading: leadingSlot.data
    signal primaryTriggered
    signal secondaryTriggered

    readonly property real available: (parent?.height ?? Kirigami.Units.gridUnit * 30) - Kirigami.Units.gridUnit * 4

    parent: QQC2.Overlay.overlay
    x: Math.round(((parent?.width ?? width) - width) / 2)
    y: Math.round(((parent?.height ?? height) - height) / 2)
    width: Math.min(sheetWidth, (parent?.width ?? sheetWidth) - Kirigami.Units.gridUnit * 2)
    height: Math.min(implicitHeight, available)
    modal: true
    dim: true
    padding: 0
    closePolicy: QQC2.Popup.CloseOnEscape

    // While a sheet is open, Escape closes it and not the window behind (SET-07).
    onOpened: SettingsBackend.popupOpened()
    onClosed: SettingsBackend.popupClosed()
    Component.onDestruction: {
        if (opened) {
            SettingsBackend.popupClosed();
        }
    }

    header: Kirigami.Heading {
        level: 3
        padding: Kirigami.Units.largeSpacing
        text: sheet.title
        horizontalAlignment: Text.AlignHCenter
    }

    contentItem: QQC2.ScrollView {
        contentWidth: availableWidth

        ColumnLayout {
            id: bodyColumn

            width: parent.width
            spacing: Kirigami.Units.smallSpacing
        }
    }

    footer: ColumnLayout {
        spacing: 0

        Kirigami.Separator {
            Layout.fillWidth: true
        }

        RowLayout {
            Layout.margins: Kirigami.Units.largeSpacing
            spacing: Kirigami.Units.smallSpacing

            RowLayout {
                id: leadingSlot

                spacing: Kirigami.Units.smallSpacing
            }

            QQC2.Label {
                Layout.fillWidth: true
                color: Kirigami.Theme.disabledTextColor
                elide: Text.ElideRight
                font: Kirigami.Theme.smallFont
                text: sheet.note
                visible: sheet.note !== ""
            }

            Item {
                Layout.fillWidth: true
                visible: sheet.note === ""
            }

            QQC2.Button {
                text: sheet.secondaryText
                visible: sheet.secondaryText !== ""
                onClicked: sheet.secondaryTriggered()
            }

            QQC2.Button {
                enabled: sheet.primaryEnabled
                highlighted: true
                text: sheet.primaryText
                visible: sheet.primaryText !== ""
                onClicked: sheet.primaryTriggered()
            }
        }
    }
}
