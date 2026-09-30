// The Picker page (07-picker-settings.md): the picker's appearance, when it is skipped, its keys, and Preview Picker.
// PKS-01 to PKS-09.
pragma ComponentBehavior: Bound
import QtQuick
import dev.soldunov.wye.ui

WyePage {
    id: page

    function showKeys() {
        (keysLoader.item as PickerKeysSheet)?.open();
    }

    title: qsTr("Picker")

    Connections {
        function onSheetRequested(name) {
            if (name === "picker-keys") {
                keysLoader.active = true;
                page.showKeys();
            }
        }

        target: SettingsBackend
    }

    WyeGroupCard {
        title: qsTr("Appearance")

        // PKS-01
        WyeRadioRow {
            choices: [
                {
                    "value": "small",
                    "label": qsTr("Small")
                },
                {
                    "value": "medium",
                    "label": qsTr("Medium")
                },
                {
                    "value": "large",
                    "label": qsTr("Large")
                }
            ]
            currentValue: page.value(path, "large")
            path: "picker.icon-size"
            title: qsTr("Icon size")
        }

        // PKS-02
        WyeSwitchRow {
            isOn: page.value(path, true)
            path: "picker.show-names"
            title: qsTr("Show browser names")
        }

        // PKS-03
        WyeSwitchRow {
            isOn: page.value(path, false)
            path: "picker.show-url"
            title: qsTr("Show URL")
        }

        // PKS-04
        WyeSwitchRow {
            isOn: page.value(path, true)
            path: "picker.show-profile-badge"
            title: qsTr("Show profile badge")
        }

    }

    WyeGroupCard {
        title: qsTr("Behaviour")

        // PKS-05, PKS-07: a link that needs the picker is held until the screen unlocks; the service does that.
        WyeSwitchRow {
            isOn: page.value(path, false)
            path: "picker.skip-when-locked"
            subtitle: qsTr("Links will then open in the alternative browser.")
            title: qsTr("Skip picker when screen is locked")
        }
    }

    // PKS-08, PKS-09
    WyeGroupCard {
        title: qsTr("Keys")

        WyeChoiceRow {
            choices: [
                {
                    "value": "per-target",
                    "label": qsTr("Assigned per browser")
                },
                {
                    "value": "numbers",
                    "label": qsTr("Numbers 1–9")
                },
                {
                    "value": "letters",
                    "label": qsTr("Letters from names")
                },
                {
                    "value": "off",
                    "label": qsTr("Off")
                }
            ]
            currentValue: page.value(path, "per-target")
            path: "picker.hotkeys"
            title: qsTr("Target hotkeys")
        }

        WyeButtonRow {
            buttonText: qsTr("Customize…")
            needsConfig: false
            title: qsTr("Picker keys")

            onActivated: {
                keysLoader.active = true;
                page.showKeys();
            }
        }
    }

    // PKS-06: Preview Picker, in a card of its own below the others rather than a loose button under them; choosing a
    // target in the preview opens nothing.
    WyeGroupCard {
        WyeButtonRow {
            buttonIcon: "view-preview"
            buttonText: qsTr("Preview Picker")
            needsConfig: false
            subtitle: qsTr("See the picker as it looks now. Choosing a target in it opens nothing.")
            title: qsTr("Preview")

            onActivated: SettingsBackend.previewPicker()
        }
    }

    overlays: [
        Loader {
            id: keysLoader

            active: false

            sourceComponent: PickerKeysSheet {
                onClosed: keysLoader.active = false
            }
        }
    ]
}
