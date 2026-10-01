// WyeShortcutChips (BLK-16, KEY-02): where an action takes several shortcuts, each shows as a chip with a remove "×",
// followed by a "+" chip that records another. Recording follows WyeShortcutRecorder: Escape cancels; modifiers alone wait.
//
// API
//   bindings: var          the stored bindings, for example ["Return", "KP_Enter", "space"]; bind to the configuration
//   recorded(string binding)  the user recorded a binding with the "+" chip (stored form)
//   changed(var bindings)  the user added or removed one; `bindings` is the whole new list. With `autoApply` off, only
//                          removing emits it: the page checks a new binding first (KEY-21) and applies it itself
//   autoApply: bool        add a recorded binding to the list at once (default true)
//   recording: bool        listening for keys for the "+" chip
pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import dev.soldunov.wye.ui

Flow {
    id: chips

    property var bindings: []
    property bool recording: false
    property bool autoApply: true
    signal changed(var bindings)
    signal recorded(string binding)

    readonly property var labels: JSON.parse(SettingsBackend.bindingLabels(JSON.stringify(bindings)))

    spacing: Kirigami.Units.smallSpacing

    Repeater {
        model: chips.labels

        Kirigami.Chip {
            id: chip

            required property int index
            required property string modelData

            checkable: false
            closable: true
            text: chip.modelData
            onRemoved: chips.changed(chips.bindings.filter((_, at) => at !== chip.index))
        }
    }

    Kirigami.Chip {
        id: addChip

        checkable: false
        closable: false
        focusPolicy: Qt.StrongFocus
        icon.name: "list-add"
        text: chips.recording ? qsTr("Press keys…") : ""
        Accessible.name: qsTr("Add a key")

        QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
        QQC2.ToolTip.text: chips.recording ? qsTr("Press the key. Escape cancels.") : qsTr("Add a key")
        QQC2.ToolTip.visible: hovered || chips.recording

        onClicked: {
            chips.recording = true;
            addChip.forceActiveFocus();
        }
        onActiveFocusChanged: {
            if (!activeFocus) {
                chips.recording = false;
            }
        }

        // While recording, Escape cancels the recording and does not close the window or sheet (SET-07).
        Keys.onShortcutOverride: event => event.accepted = chips.recording
        Keys.onPressed: event => {
            if (!chips.recording) {
                return;
            }
            event.accepted = true;
            const result = JSON.parse(SettingsBackend.recordKey(event.key, event.text, event.nativeScanCode, event.modifiers, false));
            if (result.kind === "binding") {
                chips.recording = false;
                chips.recorded(result.stored);
                if (chips.autoApply && chips.bindings.indexOf(result.stored) < 0) {
                    chips.changed(chips.bindings.concat([result.stored]));
                }
            } else if (result.kind === "cancel" || result.kind === "clear") {
                chips.recording = false;
            }
        }
    }
}
