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

    // One row: the state, and Make Default beside it as the step's primary button (the footer's Continue is not
    // highlighted while this is shown). On success the button goes and the row turns into the checkmark.
    // (A plain delegate with its own row: FormTextDelegate's `leading`/`trailing` items did not show here.)
    FormCard.FormCard {
        FormCard.AbstractFormDelegate {
            Accessible.name: stateLabel.text
            Layout.fillWidth: true
            background: null
            focusPolicy: Qt.NoFocus
            hoverEnabled: false

            contentItem: RowLayout {
                spacing: Kirigami.Units.largeSpacing

                Kirigami.Icon {
                    implicitHeight: Kirigami.Units.iconSizes.smallMedium
                    implicitWidth: Kirigami.Units.iconSizes.smallMedium
                    color: page.view.isDefault ? Kirigami.Theme.positiveTextColor : Kirigami.Theme.textColor
                    isMask: true
                    source: page.view.isDefault ? "checkmark" : "internet-web-browser-symbolic"
                }

                QQC2.Label {
                    id: stateLabel

                    Layout.fillWidth: true
                    text: page.view.isDefault ? qsTr("Wye is your default browser") : (page.view.currentDefault ? qsTr("Currently: %1").arg(page.view.currentDefault) : qsTr("No default browser is set"))
                    wrapMode: Text.Wrap
                }

                QQC2.Button {
                    Accessible.defaultButton: true
                    highlighted: true
                    icon.name: "emblem-default-symbolic"
                    text: qsTr("Make Default")
                    visible: !page.view.isDefault

                    onClicked: OnboardingBackend.makeDefault()
                }
            }
        }
    }

    Kirigami.InlineMessage {
        Layout.fillWidth: true
        showCloseButton: true
        text: errors.describe(OnboardingBackend.errorKind, OnboardingBackend.error)
        type: Kirigami.MessageType.Error
        visible: OnboardingBackend.error !== ""

        // The close button sets `visible` outright, which ends the binding: put it back, or the next error would
        // never show.
        onVisibleChanged: if (!visible) {
            OnboardingBackend.clearError();
            visible = Qt.binding(() => OnboardingBackend.error !== "");
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
