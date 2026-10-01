// The Browsers page (05-browsers.md): the primary and the alternative browser, the alternative-browser key, the shown
// browsers sheet, and the browser profiles' detection status. BRW-01 to BRW-06.
pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import dev.soldunov.wye.ui

WyePage {
    id: page

    // BRW-06: the profiles the service found.
    readonly property int profileCount: {
        if (SettingsBackend.generation < 0 || SettingsBackend.targetsJson === "") {
            return 0;
        }
        return JSON.parse(SettingsBackend.targetsJson).targets.filter(target => target.kind === "profile" && !target.missing).length;
    }
    readonly property var alternativeKey: page.value("browsers.alternative-key", ["Shift"])

    title: qsTr("Browsers")

    function showShownBrowsers() {
        shownLoader.active = true;
        (shownLoader.item as ShownBrowsersSheet)?.open();
    }

    Connections {
        function onSheetRequested(name) {
            if (name === "shown-browsers") {
                page.showShownBrowsers();
            } else if (name === "target-menu") {
                // Self-test only: the primary browser's menu, to see the target menu open.
                primaryRow.popup();
            }
        }

        target: SettingsBackend
    }

    WyeGroupCard {
        // BRW-01
        WyeTargetRow {
            id: primaryRow

            current: page.value(path, {
                "picker": true
            })
            path: "browsers.primary"
            surface: "browsers"
            title: qsTr("Browser")
        }

        // BRW-02: the subtitle names the key set in BRW-03.
        WyeTargetRow {
            current: page.value(path, {
                "picker": true
            })
            help: "alternative-browser"
            path: "browsers.alternative"
            subtitle: page.alternativeKey.length > 0 ? qsTr("Hold %1 while opening a link to open it in the alternative browser.").arg(page.alternativeKey.join("+")) : qsTr("Set a key below to open links in the alternative browser.")
            surface: "browsers"
            title: qsTr("Alternative browser")
        }

        // BRW-03
        WyeModifierRow {
            modifiers: page.alternativeKey
            path: "browsers.alternative-key"
            title: qsTr("Alternative browser key")
        }

        // BRW-04
        WyeButtonRow {
            buttonText: qsTr("Choose…")
            subtitle: qsTr("Browsers shown in the picker and the tray menu.")
            title: qsTr("Shown browsers")

            onActivated: page.showShownBrowsers()
        }

        // BRW-05, BRW-06
        WyeRow {
            help: "browser-profiles"
            needsConfig: false
            subtitle: qsTr("Profiles of Chromium-based and Firefox-based browsers are found automatically. <a href=\"https://github.com/psoldunov/wye#browser-profiles\">Learn more</a>")
            title: qsTr("Browser profiles")

            // BRW-06: the count. Wye ships no translations, so a numerus string (`%n`) would print "profile(s)"; each
            // English form is its own string instead.
            QQC2.Label {
                readonly property int count: page.profileCount

                color: Kirigami.Theme.disabledTextColor
                text: count === 0 ? qsTr("No profiles found") : count === 1 ? qsTr("1 profile found") : qsTr("%1 profiles found").arg(count)
            }

            QQC2.Button {
                text: qsTr("Rescan")
                onClicked: SettingsBackend.act("rescan")
            }
        }
    }

    // The sheet exists only once it was asked for (SHOWN-01).
    overlays: [
        Loader {
            id: shownLoader

            active: false

            sourceComponent: ShownBrowsersSheet {
                onClosed: shownLoader.active = false
            }
        }
    ]
}
