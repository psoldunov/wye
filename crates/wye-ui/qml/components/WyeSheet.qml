// WyeSheet (BLK-11): a modal panel over the settings window. The window behind is dimmed and inert. Own header with a
// title, a scrolling body, and a footer that stays pinned: an optional note, optional leading controls, and the buttons in
// KDE's order (the primary action, then the one that declines) with the desktop's dialog icons. The primary button is the
// default button, drawn highlighted, and stays disabled until the content is valid. Escape closes the sheet, not the window
// behind it. The sheet is as tall as its content and at most the window's height less a margin; the body scrolls then.
//
// API
//   title: string          header title ("New Rule")
//   note: string           optional dimmed note in the footer, on its own full-width row above the buttons; it wraps
//   primaryText: string    the primary button ("Done", "Save"); empty for none
//   primaryIcon: string    its icon (default "dialog-ok"; "document-save" for Save, "edit-delete" for a deletion)
//   secondaryIcon: string  the other button's icon (default "dialog-cancel")
//   primaryEnabled: bool   false until the content is valid
//   secondaryText: string  the other button ("Cancel"); empty for none
//   tertiaryText: string   an optional third button, after the secondary ("Cancel" beside "Save" and "Discard"); empty for none
//   tertiaryIcon: string   its icon (default "dialog-cancel")
//   primaryTriggered() / secondaryTriggered() / tertiaryTriggered()   a button was pressed; the sheet does not close itself: call close()
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
    property string primaryIcon: "dialog-ok"
    property string secondaryIcon: "dialog-cancel"
    property string tertiaryText
    property string tertiaryIcon: "dialog-cancel"
    property real sheetWidth: Kirigami.Units.gridUnit * 21
    default property alias body: bodyColumn.data
    property alias footerLeading: leadingSlot.data
    signal primaryTriggered
    signal secondaryTriggered
    signal tertiaryTriggered

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
    WyePopupTracker {
        popup: sheet
    }

    header: ColumnLayout {
        spacing: 0

        Kirigami.Heading {
            Layout.fillWidth: true
            Layout.leftMargin: Kirigami.Units.largeSpacing * 2
            Layout.rightMargin: Kirigami.Units.largeSpacing * 2
            Layout.topMargin: Kirigami.Units.largeSpacing
            Layout.bottomMargin: Kirigami.Units.largeSpacing
            elide: Text.ElideRight
            level: 2
            text: sheet.title
        }

        Kirigami.Separator {
            Layout.fillWidth: true
        }
    }

    contentItem: QQC2.ScrollView {
        id: scroller

        // Room between the header's line and the first item, and above the footer's line.
        bottomPadding: Kirigami.Units.largeSpacing
        contentWidth: availableWidth
        topPadding: Kirigami.Units.smallSpacing

        ColumnLayout {
            id: bodyColumn

            width: scroller.availableWidth
            spacing: Kirigami.Units.smallSpacing
        }
    }

    footer: ColumnLayout {
        spacing: 0

        Kirigami.Separator {
            Layout.fillWidth: true
        }

        // The note has a row of its own, the sheet's full width, above the buttons.
        QQC2.Label {
            Layout.fillWidth: true
            Layout.leftMargin: Kirigami.Units.largeSpacing
            Layout.rightMargin: Kirigami.Units.largeSpacing
            Layout.topMargin: Kirigami.Units.largeSpacing
            color: Kirigami.Theme.disabledTextColor
            font: Kirigami.Theme.smallFont
            text: sheet.note
            visible: sheet.note !== ""
            wrapMode: Text.Wrap
        }

        RowLayout {
            Layout.margins: Kirigami.Units.largeSpacing
            spacing: Kirigami.Units.smallSpacing

            RowLayout {
                id: leadingSlot

                spacing: Kirigami.Units.smallSpacing
            }

            Item {
                Layout.fillWidth: true
            }

            // KDE's order: the affirmative action first, then the one that declines.
            QQC2.Button {
                Accessible.defaultButton: true
                enabled: sheet.primaryEnabled
                highlighted: true
                icon.name: sheet.primaryIcon
                text: sheet.primaryText
                visible: sheet.primaryText !== ""
                onClicked: sheet.primaryTriggered()
            }

            QQC2.Button {
                icon.name: sheet.secondaryIcon
                text: sheet.secondaryText
                visible: sheet.secondaryText !== ""
                onClicked: sheet.secondaryTriggered()
            }

            QQC2.Button {
                icon.name: sheet.tertiaryIcon
                text: sheet.tertiaryText
                visible: sheet.tertiaryText !== ""
                onClicked: sheet.tertiaryTriggered()
            }
        }
    }
}
