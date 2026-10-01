// The picker keys sheet (15-keyboard.md, KEY-20 to KEY-22): every picker action with its bindings as removable chips and a
// "+" chip that records another (KEY-02), the three held-modifier actions with modifier choosers (KEY-01), Reset to
// Defaults (KEY-04) and Done. Changes apply at once. A key belongs to one action and cannot also be a target hotkey; the
// three held-modifier actions need different sets. On a clash the sheet says "Already used by …" and offers Replace (KEY-21).
pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import dev.soldunov.wye.ui

WyeSheet {
    id: sheet

    // The action names as KEY-22 draws them, with their keys in `picker.keys`.
    readonly property var actions: [
        {
            "key": "open",
            "title": qsTr("Open selected target")
        },
        {
            "key": "cancel",
            "title": qsTr("Cancel")
        },
        {
            "key": "next",
            "title": qsTr("Select next")
        },
        {
            "key": "previous",
            "title": qsTr("Select previous")
        },
        {
            "key": "first",
            "title": qsTr("Select first")
        },
        {
            "key": "last",
            "title": qsTr("Select last")
        },
        {
            "key": "copy-link",
            "title": qsTr("Copy link and close")
        },
        {
            "key": "more",
            "title": qsTr("Show more targets")
        },
        {
            "key": "create-rule",
            "title": qsTr("Create rule from link…")
        }
    ]
    readonly property var holdActions: [
        {
            "key": "private-modifier",
            "title": qsTr("Open in private window")
        },
        {
            "key": "background-modifier",
            "title": qsTr("Open in background")
        },
        {
            "key": "new-window-modifier",
            "title": qsTr("Open in new window")
        }
    ]
    readonly property var config: SettingsBackend.configJson === "" ? ({}) : JSON.parse(SettingsBackend.configJson)
    readonly property var keys: config.picker?.keys ?? ({})

    // A key or a modifier set that something else uses, waiting for Replace or Cancel (KEY-21).
    property var clash: null

    // KEY-02, KEY-21: add a recorded binding, or ask.
    function tryKey(action, binding) {
        const answer = JSON.parse(SettingsBackend.checkPickerKey(action, binding));
        if (answer.status === "free") {
            SettingsBackend.setPickerKey(action, binding, false);
            clash = null;
        } else if (answer.status === "clash") {
            clash = {
                "kind": "key",
                "action": action,
                "value": binding,
                "message": answer.message
            };
        }
    }

    // KEY-01, KEY-21: change a held-modifier set, or ask.
    function tryModifiers(which, names) {
        const answer = JSON.parse(SettingsBackend.checkPickerModifiers(which, JSON.stringify(names)));
        if (answer.status === "free") {
            SettingsBackend.setPickerModifiers(which, JSON.stringify(names), false);
            clash = null;
        } else if (answer.status === "clash") {
            clash = {
                "kind": "modifiers",
                "action": which,
                "value": names,
                "message": answer.message
            };
        }
    }

    function replace() {
        if (clash === null) {
            return;
        }
        if (clash.kind === "key") {
            SettingsBackend.setPickerKey(clash.action, clash.value, true);
        } else {
            SettingsBackend.setPickerModifiers(clash.action, JSON.stringify(clash.value), true);
        }
        clash = null;
    }

    primaryText: qsTr("Done")
    sheetWidth: Kirigami.Units.gridUnit * 30
    title: qsTr("Picker Keys")

    onAboutToShow: clash = null
    onPrimaryTriggered: close()

    // KEY-04
    footerLeading: QQC2.Button {
        enabled: SettingsBackend.writable
        icon.name: "edit-reset"
        text: qsTr("Reset to Defaults")

        onClicked: {
            sheet.clash = null;
            SettingsBackend.resetSection("picker.keys");
        }
    }

    // KEY-21: the clash, as the desktop's own warning message, with Replace and Cancel.
    Kirigami.InlineMessage {
        Layout.fillWidth: true
        Layout.margins: Kirigami.Units.largeSpacing
        text: sheet.clash?.message ?? ""
        type: Kirigami.MessageType.Warning
        visible: sheet.clash !== null

        actions: [
            Kirigami.Action {
                icon.name: "document-replace"
                text: qsTr("Replace")

                onTriggered: sheet.replace()
            },
            Kirigami.Action {
                icon.name: "dialog-cancel"
                text: qsTr("Cancel")

                onTriggered: sheet.clash = null
            }
        ]
    }

    // KEY-20
    WyeGroupCard {
        title: qsTr("Actions")

        Repeater {
            model: sheet.actions

            WyeRow {
                id: action

                required property var modelData

                title: modelData.title

                WyeShortcutChips {
                    Layout.preferredWidth: Kirigami.Units.gridUnit * 12
                    autoApply: false
                    bindings: sheet.keys[action.modelData.key] ?? []

                    onChanged: list => SettingsBackend.setValue("picker.keys." + action.modelData.key, JSON.stringify(list))
                    onRecorded: binding => sheet.tryKey(action.modelData.key, binding)
                }
            }
        }
    }

    // KEY-13, KEY-20: hold while choosing
    WyeGroupCard {
        title: qsTr("Hold while choosing")

        Repeater {
            model: sheet.holdActions

            WyeModifierRow {
                id: hold

                required property var modelData

                modifiers: sheet.keys[modelData.key] ?? []
                title: modelData.title

                onModified: names => sheet.tryModifiers(modelData.key, names)
            }
        }
    }
}
