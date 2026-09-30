// ONB-05: the browser extension step (optional): why the extension exists and where to get it for Chromium-based and
// Firefox-based browsers. Done (the window's footer button) closes the window and opens nothing else.
pragma ComponentBehavior: Bound
import QtQuick
import org.kde.kirigamiaddons.formcard as FormCard
import dev.soldunov.wye.ui

OnboardingPage {
    id: page

    // The project page describes how to install the extension (GEN-04 points there too).
    readonly property string installUrl: "https://github.com/psoldunov/wye"

    heading: qsTr("Browser extension")
    lead: qsTr("A link you click inside a browser is opened by that browser and never reaches Wye. The optional Wye browser extension sends those links to Wye.")

    FormCard.FormCard {
        FormCard.FormButtonDelegate {
            description: qsTr("Chrome, Chromium, Brave, Vivaldi, Edge")
            icon.name: "internet-web-browser-symbolic"
            text: qsTr("Install for Chromium-based browsers")

            onClicked: OnboardingBackend.openLink(page.installUrl)
        }

        FormCard.FormButtonDelegate {
            description: qsTr("Firefox, Zen, LibreWolf, Floorp")
            icon.name: "internet-web-browser-symbolic"
            text: qsTr("Install for Firefox-based browsers")

            onClicked: OnboardingBackend.openLink(page.installUrl)
        }
    }
}
