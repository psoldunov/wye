// ONB-01: the welcome step: Wye's icon and the two-sentence explanation. "Get Started" is the window's footer button.
pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

OnboardingPage {
    id: page

    Kirigami.Icon {
        Layout.alignment: Qt.AlignHCenter
        Layout.topMargin: Kirigami.Units.gridUnit * 2
        Layout.preferredHeight: Kirigami.Units.iconSizes.enormous
        Layout.preferredWidth: Kirigami.Units.iconSizes.enormous
        source: "dev.soldunov.wye"
    }

    Kirigami.Heading {
        Layout.alignment: Qt.AlignHCenter
        level: 1
        text: qsTr("Welcome to Wye")
    }

    QQC2.Label {
        Layout.alignment: Qt.AlignHCenter
        Layout.fillWidth: true
        horizontalAlignment: Text.AlignHCenter
        text: qsTr("Wye opens every link in the browser you want. It becomes your default browser and forwards each link to the right place.")
        wrapMode: Text.WordWrap
    }
}
