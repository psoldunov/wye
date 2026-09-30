// The General page (04-general.md): default-browser status, startup behaviour, the tray icon, and the callout about links
// Wye cannot intercept. GEN-01 to GEN-05.
pragma ComponentBehavior: Bound
import QtQuick
import dev.soldunov.wye.ui

WyePage {
    id: page

    readonly property var defaultBrowser: page.status.defaultBrowser ?? ({})
    readonly property bool isDefault: defaultBrowser.isDefault ?? false
    readonly property string currentName: defaultBrowser.current?.name ?? ""

    title: qsTr("General")

    // GEN-05: the default-browser status and switch come first.
    WyeGroupCard {
        title: qsTr("Default Browser")

        WyeButtonRow {
            buttonText: page.isDefault ? qsTr("Stop Being Default") : qsTr("Make Default")
            needsConfig: false
            subtitle: page.isDefault ? "" : (page.currentName !== "" ? qsTr("Your default browser is %1.").arg(page.currentName) : "")
            title: page.isDefault ? qsTr("✓ Wye is your default browser") : qsTr("Wye is not your default browser")

            onActivated: SettingsBackend.act(page.isDefault ? "stop-being-default" : "make-default")
        }

        WyeSwitchRow {
            isOn: page.value(path, false)
            path: "general.open-local-html"
            title: qsTr("Also open local HTML files")
        }
    }

    // GEN-01
    WyeGroupCard {
        title: qsTr("Startup")

        WyeSwitchRow {
            isOn: page.value(path, true)
            path: "general.launch-at-login"
            title: qsTr("Launch at login")
        }
    }

    // GEN-02, GEN-03
    WyeGroupCard {
        title: qsTr("Tray")

        WyeChoiceRow {
            choices: [
                {
                    "value": "primary-browser",
                    "label": qsTr("Primary Browser")
                },
                {
                    "value": "wye",
                    "label": qsTr("Wye")
                }
            ]
            currentValue: page.value(path, "primary-browser")
            path: "general.tray-icon"
            title: qsTr("Tray icon")
        }

        WyeSwitchRow {
            isOn: page.value(path, true)
            path: "general.show-tray-icon"
            title: qsTr("Show tray icon")
        }
    }

    // GEN-04
    WyeCallout {
        calloutId: "general-links"
        closeLeading: true
        text: qsTr("<b>Wye cannot handle links clicked inside a browser.</b> You can either use the browser extension (see the website for more info), or copy the link and then choose “Open URL from Clipboard” in the Wye menu.")
    }
}
