// WyeHelpButton (BLK-08): a small round "?" button that opens a popover explaining a setting.
//
// API
//   helpId: string         the text's ID in crates/wye-ui/src/settings/help.rs (docs/spec/19-help-texts.md)
//   text: string           or the text itself; wins over helpId
// Texts with a placeholder ("<alternative-browser key>") are filled in from the configuration when the popover opens.
pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import dev.soldunov.wye.ui

QQC2.ToolButton {
    id: button

    property string helpId
    property string body

    display: QQC2.AbstractButton.IconOnly
    icon.name: "help-contextual-symbolic"
    text: qsTr("Show help")
    visible: helpId !== "" || body !== ""

    onClicked: {
        if (popover.opened) {
            popover.close();
        } else {
            popover.open();
        }
    }

    QQC2.ToolTip.text: text
    QQC2.ToolTip.visible: hovered && !popover.opened
    QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay

    QQC2.Popup {
        id: popover

        y: button.height + Kirigami.Units.smallSpacing
        width: Math.min(Kirigami.Units.gridUnit * 22, (button.QQC2.Overlay.overlay?.width ?? Kirigami.Units.gridUnit * 22) - Kirigami.Units.gridUnit * 2)
        padding: Kirigami.Units.largeSpacing
        closePolicy: QQC2.Popup.CloseOnEscape | QQC2.Popup.CloseOnPressOutside

        onOpened: SettingsBackend.popupOpened()
        onClosed: SettingsBackend.popupClosed()
        onAboutToShow: helpLabel.text = button.body !== "" ? button.body : SettingsBackend.helpText(button.helpId)

        contentItem: QQC2.Label {
            id: helpLabel

            wrapMode: Text.WordWrap
        }
    }
}
