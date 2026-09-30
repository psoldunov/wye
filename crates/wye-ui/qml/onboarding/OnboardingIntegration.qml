// ONB-04: the desktop integration step: the Launch at login switch (on), and the desktop's note about its tray.
pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.kde.kirigamiaddons.formcard as FormCard
import dev.soldunov.wye.ui

OnboardingPage {
    id: page

    heading: qsTr("Start with your desktop")
    lead: qsTr("Wye works best when it is already running when you click a link.")

    FormCard.FormCard {
        enabled: page.view.writable ?? true

        FormCard.FormSwitchDelegate {
            id: loginSwitch

            // GEN-01: the Nix modules decide login start; the switch shows
            // what they set (launchAtLogin is loginManagedOn then).
            readonly property bool managed: page.view.loginManaged ?? false

            checked: page.view.launchAtLogin ?? true
            enabled: !managed
            text: qsTr("Launch at login")

            onToggled: OnboardingBackend.setLaunchAtLogin(checked)
        }

        // Why the switch is locked, in a row of its own so the reason is not greyed out with the switch.
        FormCard.FormTextDelegate {
            icon.name: "object-locked-symbolic"
            text: loginSwitch.checked ? qsTr("Your Nix configuration starts Wye at login. Change it there.") : qsTr("Your Nix configuration does not start Wye at login. Change it there.")
            textItem.wrapMode: Text.Wrap
            visible: loginSwitch.managed
        }
    }

    Kirigami.InlineMessage {
        Layout.fillWidth: true
        text: page.view.desktopNote ?? ""
        type: Kirigami.MessageType.Information
        visible: (page.view.desktopNote ?? "") !== ""
    }

    Kirigami.InlineMessage {
        Layout.fillWidth: true
        showCloseButton: true
        text: errors.describe(OnboardingBackend.errorKind, OnboardingBackend.error)
        type: Kirigami.MessageType.Error
        visible: OnboardingBackend.error !== ""

        onVisibleChanged: if (!visible) {
            OnboardingBackend.clearError()
        }
    }

    WyeErrorText {
        id: errors
    }
}
