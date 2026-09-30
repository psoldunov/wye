// WyeEmptyState (BLK-12): a centred, large, dimmed title ("No Rules"), a short explanation and a usage hint.
//
// API
//   title: string          the large title
//   explanation: string    the short explanation
//   hint: string           the usage hint ("Click + to add a rule."), under the explanation
//   Fills its parent; put it where the list would be and show it while the list is empty.
pragma ComponentBehavior: Bound
import QtQuick
import org.kde.kirigami as Kirigami

Item {
    id: empty

    property string title
    property string explanation
    property string hint

    implicitHeight: message.implicitHeight + Kirigami.Units.gridUnit * 2

    Kirigami.PlaceholderMessage {
        id: message

        anchors {
            left: parent.left
            right: parent.right
            verticalCenter: parent.verticalCenter
            margins: Kirigami.Units.gridUnit
        }
        explanation: empty.hint === "" ? empty.explanation : empty.explanation + "\n" + empty.hint
        text: empty.title
    }
}
