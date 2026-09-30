// WyeCallout (BLK-09): an info card with a round close button. Closing hides it for good: the ID goes into the service's
// UI state (`UpdateUiState`, uiState.dismissedCallouts), so it stays hidden across runs.
//
// API
//   calloutId: string      the callout's identity in the UI state ("general-links", "apps-read-first")
//   title: string          bold title ("Please Read"); empty for none
//   text: string           rich text: <b>, <code>, <a> (links go through Wye, BLK-17)
//   closeLeading: bool     the close button leads the text (the General page); otherwise it sits top-right with the
//                          title (the Apps page)
pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import dev.soldunov.wye.ui

Item {
    id: callout

    property string calloutId
    property string title
    property string text
    property bool closeLeading: false

    readonly property bool dismissed: {
        // Hidden until the UI state is known, so a dismissed callout never flashes up.
        if (SettingsBackend.generation < 0 || !SettingsBackend.loaded) {
            return true;
        }
        return SettingsBackend.isCalloutDismissed(callout.calloutId);
    }

    Layout.fillWidth: true
    Layout.leftMargin: Kirigami.Units.largeSpacing
    Layout.rightMargin: Kirigami.Units.largeSpacing
    implicitHeight: dismissed ? 0 : card.implicitHeight
    visible: !dismissed

    Rectangle {
        id: card

        anchors {
            left: parent.left
            right: parent.right
            top: parent.top
        }
        color: Qt.alpha(Kirigami.Theme.highlightColor, 0.12)
        border.color: Qt.alpha(Kirigami.Theme.highlightColor, 0.5)
        border.width: 1
        implicitHeight: content.implicitHeight + Kirigami.Units.largeSpacing * 2
        radius: Kirigami.Units.cornerRadius

        RowLayout {
            id: content

            anchors {
                fill: parent
                margins: Kirigami.Units.largeSpacing
            }
            spacing: Kirigami.Units.largeSpacing

            QQC2.ToolButton {
                Layout.alignment: Qt.AlignTop
                display: QQC2.AbstractButton.IconOnly
                icon.name: "dialog-close"
                text: qsTr("Dismiss")
                visible: callout.closeLeading
                onClicked: SettingsBackend.dismissCallout(callout.calloutId)
            }

            ColumnLayout {
                Layout.fillWidth: true
                spacing: Kirigami.Units.smallSpacing

                Kirigami.Heading {
                    Layout.fillWidth: true
                    level: 4
                    text: callout.title
                    type: Kirigami.Heading.Type.Primary
                    visible: callout.title !== ""
                }

                QQC2.Label {
                    Layout.fillWidth: true
                    linkColor: Kirigami.Theme.linkColor
                    text: callout.text
                    textFormat: Text.RichText
                    wrapMode: Text.WordWrap
                    onLinkActivated: link => SettingsBackend.openLink(link)
                }
            }

            QQC2.ToolButton {
                Layout.alignment: Qt.AlignTop
                display: QQC2.AbstractButton.IconOnly
                icon.name: "dialog-close"
                text: qsTr("Dismiss")
                visible: !callout.closeLeading
                onClicked: SettingsBackend.dismissCallout(callout.calloutId)
            }
        }
    }
}
