// The hotkey popup of one row of the shown browsers sheet (SHOWN-04). It lists "None", a to z, 0 to 9 (keys the picker's own
// actions use are left out, KEY-12) and "Other Key…", which records any single key. A hotkey is unique: choosing one that is
// in use clears it on the other row. When the picker's hotkey scheme is not "Assigned per browser" the popup is disabled and
// shows the key the scheme gives.
//
// API
//   row: var               the row's data ({key, hotkey, shownHotkey, …}); set by WyeChecklist
pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import dev.soldunov.wye.ui

QQC2.ComboBox {
    id: combo

    property var row: ({
            "key": "",
            "hotkey": null,
            "shownHotkey": null
        })

    readonly property var config: SettingsBackend.configJson === "" ? ({}) : JSON.parse(SettingsBackend.configJson)
    readonly property bool perBrowser: (config.picker?.hotkeys ?? "per-target") === "per-target"
    readonly property var choices: SettingsBackend.generation >= 0 ? JSON.parse(SettingsBackend.hotkeyChoices()) : []
    readonly property string none: qsTr("None")
    readonly property string other: qsTr("Other Key…")
    readonly property string hotkeyLabel: {
        const key = perBrowser ? row.hotkey : row.shownHotkey;
        if (key === null || key === undefined || key === "") {
            return none;
        }
        return perBrowser ? JSON.parse(SettingsBackend.bindingLabels(JSON.stringify([key])))[0] : key;
    }

    Accessible.name: qsTr("Hotkey")
    Layout.preferredWidth: Kirigami.Units.gridUnit * 6
    currentIndex: -1
    displayText: hotkeyLabel
    enabled: perBrowser
    model: [none].concat(choices.map(choice => choice.label), [other])
    // The list is a window of its own, so it is never cut off by the sheet or the window.
    popup.popupType: QQC2.Popup.Window
    // Scrolling the list over the box must not change the hotkey.
    wheelEnabled: false

    onActivated: index => {
        if (index === 0) {
            SettingsBackend.shownSetHotkey(row.key, "");
        } else if (index === model.length - 1) {
            recorder.open();
        } else {
            SettingsBackend.shownSetHotkey(row.key, choices[index - 1].key);
        }
        combo.currentIndex = -1;
    }

    // While the list is open, Escape closes it and not the sheet (SET-07).
    Connections {
        function onOpened() {
            SettingsBackend.popupOpened();
        }

        function onClosed() {
            SettingsBackend.popupClosed();
        }

        target: combo.popup
    }

    Component.onDestruction: {
        if (combo.popup.opened) {
            SettingsBackend.popupClosed();
        }
    }

    // "Other Key…": press any single key.
    QQC2.Popup {
        id: recorder

        property string problem

        padding: Kirigami.Units.largeSpacing
        parent: QQC2.Overlay.overlay
        x: Math.round((parent.width - width) / 2)
        y: Math.round((parent.height - height) / 2)
        closePolicy: QQC2.Popup.CloseOnEscape | QQC2.Popup.CloseOnPressOutside
        modal: true

        onAboutToShow: problem = ""
        onOpened: {
            SettingsBackend.popupOpened();
            capture.recording = true;
            capture.forceActiveFocus();
        }
        onClosed: SettingsBackend.popupClosed()

        contentItem: ColumnLayout {
            spacing: Kirigami.Units.smallSpacing

            QQC2.Label {
                text: qsTr("Press the key for this browser.")
            }

            WyeShortcutRecorder {
                id: capture

                Layout.alignment: Qt.AlignHCenter
                single: true

                onCleared: {
                    SettingsBackend.shownSetHotkey(combo.row.key, "");
                    recorder.close();
                }
                onRecorded: binding => {
                    const result = JSON.parse(SettingsBackend.checkHotkey(binding));
                    if (result.ok) {
                        SettingsBackend.shownSetHotkey(combo.row.key, result.key);
                        recorder.close();
                    } else {
                        recorder.problem = result.error;
                        capture.recording = true;
                        capture.forceActiveFocus();
                    }
                }
            }

            QQC2.Label {
                Layout.maximumWidth: Kirigami.Units.gridUnit * 16
                color: Kirigami.Theme.negativeTextColor
                text: recorder.problem
                visible: recorder.problem !== ""
                wrapMode: Text.WordWrap
            }
        }
    }
}
