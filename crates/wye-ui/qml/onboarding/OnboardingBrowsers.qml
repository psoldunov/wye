// ONB-03: the browsers step: the Primary browser popup, set to the Picker at first with the browser Wye replaced listed
// first, and the checklist of detected browsers and profiles for the picker, the first six browsers pre-checked.
pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.kde.kirigamiaddons.formcard as FormCard
import dev.soldunov.wye.ui

OnboardingPage {
    id: page

    heading: qsTr("Choose your browsers")
    lead: qsTr("The primary browser opens links no rule handles. The picker offers the browsers you check.")

    FormCard.FormCard {
        FormCard.FormComboBoxDelegate {
            currentIndex: Math.max(0, (page.view.primary ?? []).findIndex(choice => choice.checked))
            description: qsTr("Choose the Picker to be asked each time.")
            enabled: page.view.writable ?? true
            model: page.view.primary ?? []
            text: qsTr("Primary browser")
            textRole: "name"

            onActivated: index => OnboardingBackend.setPrimary(JSON.stringify(page.view.primary[index].target))
        }
    }

    // The same section header the Settings window's cards use.
    FormCard.FormHeader {
        title: qsTr("Browsers in the picker")
    }

    FormCard.FormCard {
        enabled: page.view.writable ?? true

        WyeChecklist {
            Layout.fillWidth: true
            rows: page.view.checklist ?? []

            onToggled: (key, checked) => OnboardingBackend.toggleBrowser(key, checked)
        }
    }

    QQC2.Label {
        Layout.fillWidth: true
        color: Kirigami.Theme.disabledTextColor
        text: qsTr("Set a hotkey for each browser later, in Settings.")
        wrapMode: Text.WordWrap
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
