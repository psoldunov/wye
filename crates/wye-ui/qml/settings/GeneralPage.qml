// The General page (04-general.md): default-browser status, startup behaviour, the tray icon, and the callout about links
// Wye cannot intercept. GEN-01 to GEN-05.
pragma ComponentBehavior: Bound
import QtQuick
import org.kde.kirigami as Kirigami
import dev.soldunov.wye.ui

WyePage {
    id: page

    readonly property var defaultBrowser: page.status.defaultBrowser ?? ({})
    readonly property bool isDefault: defaultBrowser.isDefault ?? false
    readonly property string currentName: defaultBrowser.current?.name ?? ""
    // GEN-01: the Nix modules decide login start (`loginManagedOn`); the
    // switch shows what they set and changes nothing. Absent from an older
    // service: not managed, and not started.
    readonly property bool loginManaged: page.status.loginManaged ?? false
    readonly property bool loginManagedOn: page.status.loginManagedOn ?? false

    title: qsTr("General")

    // GEN-05: the default-browser status and switch come first.
    WyeGroupCard {
        title: qsTr("Default Browser")

        // The status reads as a status (GEN-05): a plain glyph, not a box, so it cannot pass for a checkbox, then the text.
        // Default: a check tinted in the positive colour. Not default: Breeze's warning triangle (`dialog-warning`), which
        // the theme already draws in the neutral colour; the symbolic `emblem-important` has a fixed red fill that no tint
        // reaches. The action that changes it is the row's button.
        WyeButtonRow {
            buttonHighlighted: !page.isDefault
            buttonText: page.isDefault ? qsTr("Stop Being Default") : qsTr("Make Default")
            leadingIcon: page.isDefault ? "checkmark-symbolic" : "dialog-warning"
            leadingIconColor: page.isDefault ? Kirigami.Theme.positiveTextColor : "transparent"
            needsConfig: false
            subtitle: page.isDefault ? "" : (page.currentName !== "" ? qsTr("Your default browser is %1.").arg(page.currentName) : "")
            title: page.isDefault ? qsTr("Wye is your default browser") : qsTr("Wye is not your default browser")

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
            dimmed: page.loginManaged
            isOn: page.loginManaged ? page.loginManagedOn : page.value(path, true)
            path: "general.launch-at-login"
            subtitle: !page.loginManaged ? "" : page.loginManagedOn ? qsTr("Your Nix configuration starts Wye at login. Change it there.") : qsTr("Your Nix configuration does not start Wye at login. Change it there.")
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
        text: qsTr("<b>Wye cannot handle links clicked inside a browser.</b> You can either use the browser extension (see the website for more info), or copy the link and then choose “Open URL from Clipboard” in the Wye menu.")
    }
}
