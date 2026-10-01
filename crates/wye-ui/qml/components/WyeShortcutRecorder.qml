// WyeShortcutRecorder (BLK-16, KEY-02): a button showing the shortcut, or "Record Shortcut" until one is set. Click it,
// press the keys, done. Escape cancels, Backspace clears. Bindings are stored with XKB key names ("Ctrl+Shift+o", KEY-03)
// and shown in desktop style ("Ctrl+Shift+O"). A modifier on its own does not end the recording. While it listens the
// button stays pressed and reads "Press keys…".
//
// API
//   binding: string        the stored binding; empty for none. Bind it to the configuration.
//   emptyText: string      what the button reads while no shortcut is set (default "Record Shortcut"; "None" where the
//                          row says what the shortcut does)
//   single: bool           record one key without modifiers (a target hotkey, SHOWN-04) instead of a combination
//   recorded(string binding)   a binding was recorded (stored form)
//   cleared()              Backspace cleared it
//   recording: bool        listening for keys (read-only in use)
pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import dev.soldunov.wye.ui

QQC2.Button {
    id: button

    property string binding
    property string emptyText: qsTr("Record Shortcut")
    property bool single: false
    property bool recording: false
    signal recorded(string binding)
    signal cleared

    readonly property string label: binding === "" ? "" : JSON.parse(SettingsBackend.bindingLabels(JSON.stringify([binding])))[0]

    Accessible.description: qsTr("Click, then press the keys. Escape cancels, Backspace clears.")
    Accessible.name: binding === "" ? emptyText : label
    checked: recording
    focusPolicy: Qt.StrongFocus
    icon.name: "configure-shortcuts"
    text: recording ? qsTr("Press keys…") : (binding === "" ? emptyText : label)

    QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
    QQC2.ToolTip.text: recording ? qsTr("Press the keys. Escape cancels, Backspace clears.") : qsTr("Click to record a shortcut")
    QQC2.ToolTip.visible: hovered || recording

    onClicked: {
        recording = true;
        forceActiveFocus();
    }
    onActiveFocusChanged: {
        if (!activeFocus) {
            recording = false;
        }
    }

    // While recording, every key is the recorder's: Escape must cancel the recording, not close the window (SET-07).
    Keys.onShortcutOverride: event => event.accepted = recording
    Keys.onPressed: event => {
        if (!recording) {
            return;
        }
        event.accepted = true;
        const result = JSON.parse(SettingsBackend.recordKey(event.key, event.text, event.nativeScanCode, event.modifiers, single));
        switch (result.kind) {
        case "cancel":
            recording = false;
            break;
        case "clear":
            recording = false;
            button.cleared();
            break;
        case "binding":
            recording = false;
            button.recorded(result.stored);
            break;
        default:
            break;
        }
    }
}
