// The Apps page (06-apps.md): routes links to well-known web services. A callout, a heading, and one card with a target
// popup row per service, sorted by name. APP-01 to APP-10.
pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import dev.soldunov.wye.ui

WyePage {
    id: page

    // APP-03: alphabetical by service name.
    readonly property var sortedServices: page.services.services.slice().sort((a, b) => a.name.localeCompare(b.name))

    title: qsTr("Apps")

    // APP-01
    WyeCallout {
        calloutId: "apps-read-first"
        text: qsTr("This lets you open links <b>to</b> certain websites directly in their desktop app or in a specific browser. To open links <b>clicked in</b> a certain app in a specific browser, create a custom rule with “Source Apps” matching.")
        title: qsTr("Please Read")
    }

    // APP-02
    QQC2.Label {
        Layout.fillWidth: true
        Layout.leftMargin: Kirigami.Units.largeSpacing + Kirigami.Units.smallSpacing
        Layout.rightMargin: Kirigami.Units.largeSpacing
        font.weight: Font.Bold
        text: qsTr("Open links to web apps in their desktop app or a specific browser")
        wrapMode: Text.WordWrap
    }

    // APP-03 to APP-06, APP-10: every row starts at Default (<primary>); a mapping to a missing app shows a warning.
    WyeGroupCard {
        Repeater {
            model: page.sortedServices

            WyeTargetRow {
                required property var modelData

                current: modelData.target
                service: modelData.id
                surface: "apps"
                title: modelData.name
            }
        }
    }
}
