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

    // Switch the wheel off on every combo box inside `item`.
    function stopWheel(item: Item) {
        for (const child of item.children) {
            if (child instanceof QQC2.ComboBox) {
                child.wheelEnabled = false;
            } else {
                page.stopWheel(child);
            }
        }
    }

    heading: qsTr("Choose your browsers")
    lead: qsTr("The primary browser opens links no rule handles. The picker offers the browsers you check.")

    FormCard.FormCard {
        FormCard.FormComboBoxDelegate {
            id: primaryCombo

            description: qsTr("Choose the Picker to be asked each time.")
            enabled: page.view.writable ?? true
            model: page.view.primary ?? []
            text: qsTr("Primary browser")
            textRole: "name"

            // Scrolling the page over the box must not change the primary browser, as in Settings: the delegate keeps
            // its combo box to itself, so its wheel is switched off where it is.
            Component.onCompleted: page.stopWheel(primaryCombo)
            onActivated: index => OnboardingBackend.setPrimary(JSON.stringify(page.view.primary[index].target))
        }
    }

    // The chosen primary browser, set again whenever the view changes: a click in the popup assigns `currentIndex`
    // (ending a plain binding), and the new model then resets it to the first entry, which showed the replaced browser
    // after the Picker was chosen (ONB-03). Delayed, so it lands after that reset.
    Binding {
        delayed: true
        property: "currentIndex"
        target: primaryCombo
        value: Math.max(0, (page.view.primary ?? []).findIndex(choice => choice.checked))
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

        // The close button sets `visible` outright, which ends the binding: put it back, or the next error would
        // never show (ONB-03).
        onVisibleChanged: if (!visible) {
            OnboardingBackend.clearError();
            visible = Qt.binding(() => OnboardingBackend.error !== "");
        }
    }

    WyeErrorText {
        id: errors
    }
}
