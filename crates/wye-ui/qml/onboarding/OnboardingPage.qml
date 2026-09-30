// OnboardingPage (ONB-01 to ONB-05): the base of one step of the first-run window: a heading, a lead paragraph, and the
// step's own content below them. The step reads what to show from `view`, the window's state.
//
// API
//   heading: string        the step's title
//   lead: string           the paragraph under it; empty for none
//   view: var              `View` of crates/wye-ui/src/onboarding/view.rs (step, isDefault, currentDefault, primary,
//                          checklist, launchAtLogin, desktopNote, writable, …)
//   default property       the step's content, below the lead
pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import dev.soldunov.wye.ui

Kirigami.ScrollablePage {
    id: page

    property string heading
    property string lead
    default property alias content: body.data
    readonly property var view: OnboardingBackend.viewJson === "" ? ({}) : JSON.parse(OnboardingBackend.viewJson)

    ColumnLayout {
        id: body

        spacing: Kirigami.Units.largeSpacing

        Kirigami.Heading {
            Layout.fillWidth: true
            level: 1
            text: page.heading
            visible: page.heading !== ""
            wrapMode: Text.WordWrap
        }

        QQC2.Label {
            Layout.fillWidth: true
            text: page.lead
            visible: page.lead !== ""
            wrapMode: Text.WordWrap
        }
    }
}
