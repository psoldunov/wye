// WyeShortcutRecorder (BLK-16, KEY-02): a button showing "Record Shortcut" (dimmed) until a shortcut is set. Click it, press
// the keys, done. Escape cancels, Backspace clears. Bindings are stored with XKB key names ("Ctrl+Shift+o", KEY-03) and
// shown in desktop style ("Ctrl+Shift+O"). A modifier on its own does not end the recording.
//
// API
//   binding: string        the stored binding; empty for none. Bind it to the configuration.
//   single: bool           record one key without modifiers (a target hotkey, SHOWN-04) instead of a combination
//   recorded(string binding)   a binding was recorded (stored form)
//   cleared()              Backspace cleared it
//   recording: bool        listening for keys (read-only in use)
pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls as QQC2
import dev.soldunov.wye.ui

QQC2.Button {
    id: button

    property string binding
    property bool single: false
    property bool recording: false
    signal recorded(string binding)
    signal cleared

    readonly property string label: binding === "" ? "" : JSON.parse(SettingsBackend.bindingLabels(JSON.stringify([binding])))[0]

    focusPolicy: Qt.StrongFocus
    opacity: binding === "" && !recording ? 0.6 : 1
    text: recording ? qsTr("Press keys…") : (binding === "" ? qsTr("Record Shortcut") : label)

    onClicked: {
        recording = true;
        forceActiveFocus();
    }
    onActiveFocusChanged: {
        if (!activeFocus) {
            recording = false;
        }
    }

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
