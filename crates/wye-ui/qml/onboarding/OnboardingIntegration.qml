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
            // GEN-01: the Nix modules start Wye at login themselves.
            readonly property bool managed: page.view.loginManaged ?? false

            checked: managed || (page.view.launchAtLogin ?? true)
            description: managed ? qsTr("Your Nix configuration starts Wye at login, so this switch changes nothing. Change it there.") : ""
            enabled: !managed
            text: qsTr("Launch at login")

            onToggled: OnboardingBackend.setLaunchAtLogin(checked)
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
