// WyeCallout (BLK-09): an information message with a close button, drawn as the desktop's own inline message
// (Kirigami.InlineMessage). Closing hides it for good: the ID goes into the service's UI state (`UpdateUiState`,
// uiState.dismissedCallouts), so it stays hidden across runs.
//
// API
//   calloutId: string      the callout's identity in the UI state ("general-links", "apps-read-first")
//   title: string          bold title ("Please Read"); empty for none
//   text: string           rich text: <b>, <code>, <a> (links go through Wye, BLK-17)
pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import dev.soldunov.wye.ui

Item {
    id: callout

    property string calloutId
    property string title
    property string text

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
    implicitHeight: dismissed ? 0 : message.implicitHeight
    visible: !dismissed

    // A callout shown again (another UI state) brings its message back.
    onDismissedChanged: {
        if (!dismissed) {
            message.visible = true;
        }
    }

    Kirigami.InlineMessage {
        id: message

        anchors {
            left: parent.left
            right: parent.right
            top: parent.top
        }
        showCloseButton: true
        text: callout.title === "" ? callout.text : "<b>" + callout.title + "</b><br/>" + callout.text
        type: Kirigami.MessageType.Information
        visible: true

        onLinkActivated: link => SettingsBackend.openLink(link)
        // The close button hides the message itself; remember that, unless the callout as a whole went away.
        onVisibleChanged: {
            if (!message.visible && callout.visible && !callout.dismissed) {
                SettingsBackend.dismissCallout(callout.calloutId);
            }
        }
    }
}
