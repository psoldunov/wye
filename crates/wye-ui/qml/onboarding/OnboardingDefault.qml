// ONB-02: the default-browser step: "Make Wye your default browser", the current default named, and Make Default, which
// turns into a checkmark and "Wye is your default browser" on success. Skip (the window's footer button) is allowed.
pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.kde.kirigamiaddons.formcard as FormCard
import dev.soldunov.wye.ui

OnboardingPage {
    id: page

    heading: qsTr("Make Wye your default browser")
    lead: qsTr("When Wye is your default browser, every link you click in another app goes through it. Wye then sends the link to the right place.")

    FormCard.FormCard {
        FormCard.FormTextDelegate {
            icon.name: page.view.isDefault ? "emblem-ok-symbolic" : "dialog-information-symbolic"
            text: page.view.isDefault ? qsTr("Wye is your default browser") : (page.view.currentDefault ? qsTr("Currently: %1").arg(page.view.currentDefault) : qsTr("No default browser is set"))
        }

        FormCard.FormButtonDelegate {
            icon.name: "emblem-default-symbolic"
            text: qsTr("Make Default")
            visible: !page.view.isDefault

            onClicked: OnboardingBackend.makeDefault()
        }
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

    QQC2.Label {
        Layout.fillWidth: true
        color: Kirigami.Theme.disabledTextColor
        text: qsTr("You can skip this. Wye then only sees links from the clipboard and the browser extension.")
        visible: !page.view.isDefault
        wrapMode: Text.WordWrap
    }

    WyeErrorText {
        id: errors
    }
}
