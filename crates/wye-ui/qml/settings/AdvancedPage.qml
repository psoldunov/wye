// The Advanced page (10-advanced.md): URL expansion, the global transform script, global keyboard shortcuts, history, and the
// browser-extension override. ADV-01 to ADV-11.
pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import dev.soldunov.wye.ui

WyePage {
    id: page

    readonly property var bypassKey: page.value("advanced.bypass-key", ["Alt"])
    readonly property var shortcuts: SettingsBackend.shortcutsJson === "" ? ({
            "mechanism": "none",
            "recordable": false,
            "configurable": false,
            "rows": []
        }) : JSON.parse(SettingsBackend.shortcutsJson)

    function showExpansion() {
        expansionLoader.active = true;
        (expansionLoader.item as ExpansionSheet)?.open();
    }

    title: qsTr("Advanced")

    // The shortcut rows show what the configuration holds when the mechanism reports nothing, so they follow it.
    Component.onCompleted: SettingsBackend.loadShortcuts()

    Connections {
        function onGenerationChanged() {
            SettingsBackend.loadShortcuts();
        }

        function onSheetRequested(name) {
            if (name === "expansion") {
                page.showExpansion();
            } else if (name === "history-confirm") {
                clearHistory.open();
            }
        }

        target: SettingsBackend
    }

    // ADV-01, ADV-02
    WyeGroupCard {
        title: qsTr("URL Expansion")

        WyeButtonRow {
            buttonText: qsTr("Configure…")
            hasSwitch: true
            isOn: page.value(path, true)
            path: "advanced.expand-urls"
            title: qsTr("Expand redirect and short URLs")

            onActivated: page.showExpansion()
        }
    }

    // ADV-03, ADV-04: the global script has the scope `global`.
    WyeGroupCard {
        title: qsTr("URL Transformation")

        WyeButtonRow {
            buttonText: qsTr("Edit Script…")
            hasSwitch: true
            isOn: page.value(path, false)
            path: "advanced.transform"
            subtitle: qsTr("Runs after URL expansion and tracking removal.")
            title: qsTr("Transform all URLs before matching rules")

            onActivated: SettingsBackend.showWindow("script-editor", "{\"scope\":\"global\"}")
            // SCR-09: turning the transform on with no script opens the editor.
            onSwitched: on => {
                if (on) {
                    SettingsBackend.openScriptIfMissing("global");
                }
            }
        }
    }

    // ADV-05 to ADV-07, KEY-40, KEY-41
    WyeGroupCard {
        title: qsTr("Keyboard Shortcuts")

        Repeater {
            model: page.shortcuts.rows

            WyeGlobalShortcutRow {
                required property var modelData

                configurable: page.shortcuts.configurable
                recordable: page.shortcuts.recordable
                shortcut: modelData
            }
        }
    }

    // ADV-08: a dimmed note whose links switch pages.
    WyeLinkText {
        Layout.fillWidth: true
        Layout.leftMargin: Kirigami.Units.largeSpacing + Kirigami.Units.smallSpacing
        Layout.rightMargin: Kirigami.Units.largeSpacing
        routed: false
        text: qsTr("Picker keys are on the <a href=\"page:picker\">Picker</a> page; the alternative browser key is on the <a href=\"page:browsers\">Browsers</a> page.")

        onLinkActivated: link => SettingsBackend.requestPage(link.substring("page:".length))
    }

    // ADV-09: turning history off asks whether to delete what is stored.
    WyeGroupCard {
        title: qsTr("History")

        WyeButtonRow {
            buttonText: qsTr("Show…")
            hasSwitch: true
            isOn: page.value(path, false)
            path: "advanced.history"
            title: qsTr("Store history of the last 100 opened links")

            onActivated: SettingsBackend.showWindow("history", "")
            onSwitched: on => {
                if (!on) {
                    clearHistory.open();
                }
            }
        }
    }

    // ADV-10, ADV-11
    WyeGroupCard {
        title: qsTr("Miscellaneous")

        WyeSwitchRow {
            isOn: page.value(path, true)
            path: "advanced.force-picker-from-extension"
            subtitle: qsTr("When opening, hold %1 to not force show the picker.").arg(page.bypassKey.length > 0 ? page.bypassKey.join("+") : qsTr("the bypass key"))
            title: qsTr("Force show picker when opening from browser extension")
        }

        WyeModifierRow {
            help: "bypass-key"
            modifiers: page.bypassKey
            path: "advanced.bypass-key"
            title: qsTr("Bypass key")
        }
    }

    overlays: [
        Loader {
            id: expansionLoader

            active: false

            sourceComponent: ExpansionSheet {
                onClosed: expansionLoader.active = false
            }
        },
        WyeConfirmDialog {
            id: clearHistory

            confirmText: qsTr("Delete History")
            declineText: qsTr("Keep History")
            message: qsTr("History is off now. Delete the links Wye stored while it was on?")
            title: qsTr("Delete Stored History?")

            onConfirmed: SettingsBackend.clearHistory()
        }
    ]
}
