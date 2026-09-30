// WyeConfirmDialog: a small modal question with a primary and a secondary answer ("Delete stored history?"). Built on
// WyeSheet, so it dims the window and Escape answers it as declined.
//
// API
//   title: string          the header
//   message: string        the question, wrapping
//   confirmText: string    the primary button ("Delete History")
//   declineText: string    the other button ("Keep History")
//   confirmed() / declined()   the answer; Escape and clicking outside decline
//   open()
pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

WyeSheet {
    id: dialog

    property string message
    property string confirmText
    property string declineText
    signal confirmed
    signal declined

    // True once an answer was given, so closing after it is not a second, declining one.
    property bool answered: false

    primaryText: confirmText
    secondaryText: declineText
    sheetWidth: Kirigami.Units.gridUnit * 18

    onAboutToShow: answered = false
    onClosed: {
        if (!answered) {
            declined();
        }
    }
    onPrimaryTriggered: {
        answered = true;
        confirmed();
        close();
    }
    onSecondaryTriggered: {
        answered = true;
        declined();
        close();
    }

    QQC2.Label {
        Layout.fillWidth: true
        Layout.margins: Kirigami.Units.largeSpacing
        text: dialog.message
        wrapMode: Text.WordWrap
    }
}
