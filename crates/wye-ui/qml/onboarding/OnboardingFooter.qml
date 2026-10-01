// OnboardingFooter (ONB-06): Back at the left on every step after the first, the progress dots in the middle, and the
// step's button at the right: Get Started, Continue, or Done. The default-browser step also offers Skip.
//
// API
//   view: var              `View` of crates/wye-ui/src/onboarding/view.rs
//   backRequested()        Back
//   nextRequested()        Get Started, Continue, Done
//   skipRequested()        Skip on the default-browser step
pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

QQC2.ToolBar {
    id: bar

    property var view: ({})
    signal backRequested
    signal nextRequested
    signal skipRequested

    readonly property bool onDefaultStep: (view.step ?? "") === "default-browser"
    // On the default-browser step, until Wye is the default, the page's Make Default is the primary button.
    readonly property bool nextIsPrimary: !onDefaultStep || (view.isDefault ?? false)
    readonly property string nextText: {
        if ((view.step ?? "") === "welcome") {
            return qsTr("Get Started");
        }
        return (view.isLast ?? false) ? qsTr("Done") : qsTr("Continue");
    }

    position: QQC2.ToolBar.Footer

    contentItem: RowLayout {
        spacing: Kirigami.Units.largeSpacing

        // Both sides take the same width, so the dots stay in the middle.
        Item {
            Layout.fillWidth: true
            Layout.preferredHeight: backButton.implicitHeight
            Layout.preferredWidth: 1

            QQC2.Button {
                id: backButton

                icon.name: "go-previous"
                text: qsTr("Back")
                visible: bar.view.canGoBack ?? false

                onClicked: bar.backRequested()
            }
        }

        Row {
            Layout.alignment: Qt.AlignVCenter
            spacing: Kirigami.Units.smallSpacing
            Accessible.name: qsTr("Step %1 of %2").arg((bar.view.stepIndex ?? 0) + 1).arg(bar.view.stepCount ?? 0)

            Repeater {
                model: bar.view.stepCount ?? 0

                Rectangle {
                    required property int index

                    color: index === (bar.view.stepIndex ?? 0) ? Kirigami.Theme.highlightColor : Qt.alpha(Kirigami.Theme.textColor, 0.25)
                    height: Kirigami.Units.smallSpacing * 2
                    radius: height / 2
                    width: height
                }
            }
        }

        Item {
            Layout.fillWidth: true
            Layout.preferredHeight: nextButton.implicitHeight
            Layout.preferredWidth: 1

            RowLayout {
                anchors.right: parent.right
                spacing: Kirigami.Units.smallSpacing

                QQC2.Button {
                    text: qsTr("Skip")
                    visible: bar.onDefaultStep && !(bar.view.isDefault ?? false)
                    QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
                    QQC2.ToolTip.text: qsTr("Keep the current default browser")
                    QQC2.ToolTip.visible: hovered

                    onClicked: bar.skipRequested()
                }

                QQC2.Button {
                    id: nextButton

                    // Breeze draws the default button in the accent colour; `highlighted` alone only draws the focus frame.
                    Accessible.defaultButton: bar.nextIsPrimary
                    highlighted: bar.nextIsPrimary
                    icon.name: (bar.view.isLast ?? false) ? "dialog-ok-apply" : "go-next"
                    text: bar.nextText

                    onClicked: bar.nextRequested()
                }
            }
        }
    }
}
