// WyeCopyButton (KEY-41): a small button that copies a text to the clipboard: the CLI command to bind in a compositor
// ("wye menu"). It reads "Copied" for a moment afterwards.
//
// API
//   payload: string        the text to copy
pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls as QQC2

QQC2.Button {
    id: button

    property string payload

    icon.name: copied.running ? "checkmark" : "edit-copy"
    text: copied.running ? qsTr("Copied") : qsTr("Copy")

    onClicked: {
        clipboard.text = button.payload;
        clipboard.selectAll();
        clipboard.copy();
        copied.restart();
    }

    // QML has no clipboard of its own; a hidden text field copies for us.
    TextEdit {
        id: clipboard

        visible: false
    }

    Timer {
        id: copied

        interval: 1500
    }
}
