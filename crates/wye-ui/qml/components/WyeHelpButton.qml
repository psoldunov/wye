// WyeHelpButton (BLK-08): a small round "?" button that opens a popover explaining a setting.
//
// API
//   helpId: string         the text's ID in crates/wye-ui/src/settings/help.rs (docs/spec/19-help-texts.md)
//   body: string           or the text itself; wins over helpId
// Texts with a placeholder ("<alternative-browser key>") are filled in from the configuration when the popover opens.
// The popover is a window of its own (Popup.Window), so a long text is never cut off by the edge of the Settings window.
pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import dev.soldunov.wye.ui

QQC2.ToolButton {
    id: button

    property string helpId
    property string body
    // A click asked for the popover while another popup was still fading (BLK-08).
    property bool wanted: false

    // As tall as a line of text, so the button does not make its row taller than the title.
    implicitHeight: Kirigami.Units.iconSizes.small + Kirigami.Units.smallSpacing * 2
    implicitWidth: implicitHeight
    padding: Kirigami.Units.smallSpacing
    display: QQC2.AbstractButton.IconOnly
    icon.height: Kirigami.Units.iconSizes.small
    icon.name: "help-contextual-symbolic"
    icon.width: Kirigami.Units.iconSizes.small
    text: qsTr("Show help")
    visible: helpId !== "" || body !== ""
    Accessible.name: text

    // Open once no popup is fading in or out. On Wayland a popup window opened while another one fades out (the press
    // on this button closes the other help popover) is made that one's child, and the compositor takes it down with
    // it: the popover never showed and stayed half-open, so no later click opened it again.
    function openWhenSettled() {
        if (button.wanted && SettingsBackend.settling === 0) {
            button.wanted = false;
            popover.open();
        }
    }

    onClicked: {
        // Still on screen, fading out after the press: the click closes it.
        if (popover.visible) {
            button.wanted = false;
            popover.close();
        } else {
            button.wanted = true;
            button.openWhenSettled();
        }
    }

    Connections {
        function onSettlingChanged() {
            button.openWhenSettled();
        }

        enabled: button.wanted
        target: SettingsBackend
    }

    // Should a popup never settle, the popover still opens.
    Timer {
        interval: Kirigami.Units.longDuration * 4
        running: button.wanted

        onTriggered: {
            button.wanted = false;
            popover.open();
        }
    }

    QQC2.ToolTip.text: text
    QQC2.ToolTip.visible: hovered && !popover.opened
    QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay

    QQC2.Popup {
        id: popover

        popupType: QQC2.Popup.Window
        x: Math.round((button.width - width) / 2)
        y: button.height + Kirigami.Units.smallSpacing
        width: Math.min(Kirigami.Units.gridUnit * 22, helpLabel.implicitWidth + leftPadding + rightPadding)
        padding: Kirigami.Units.largeSpacing
        closePolicy: QQC2.Popup.CloseOnEscape | QQC2.Popup.CloseOnPressOutside

        onAboutToShow: helpLabel.text = button.body !== "" ? button.body : SettingsBackend.helpText(button.helpId)

        // While the popover is open, Escape closes it and not the window (SET-07).
        WyePopupTracker {
            popup: popover
        }

        contentItem: QQC2.Label {
            id: helpLabel

            wrapMode: Text.Wrap
        }
    }
}
