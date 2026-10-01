// AboutPerson (DLG-ABT-01): one author or credit on the About page, drawn like Kirigami Addons' `AboutPage` does: an avatar
// with the initials, the name, the task under it, and a button for the person's web page when there is one. The row itself
// is not a button, so a person without a web page does not look disabled.
//
// API
//   modelData: var         an entry of `authors` or `credits` (crates/wye-ui/src/about/info.rs): name, task, webAddress
//   linkRequested(url)     the web page button was pressed
pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.kde.kirigamiaddons.components as Components
import org.kde.kirigamiaddons.formcard as FormCard

FormCard.AbstractFormDelegate {
    id: person

    required property var modelData
    readonly property string webAddress: person.modelData.webAddress ?? ""
    signal linkRequested(string url)

    Accessible.name: person.modelData.task ? qsTr("%1, %2").arg(person.modelData.name).arg(person.modelData.task) : person.modelData.name
    Layout.fillWidth: true
    background: null
    focusPolicy: Qt.NoFocus
    hoverEnabled: false

    contentItem: RowLayout {
        spacing: Kirigami.Units.largeSpacing

        Components.Avatar {
            Layout.rightMargin: Kirigami.Units.smallSpacing
            implicitHeight: Kirigami.Units.iconSizes.medium
            implicitWidth: Kirigami.Units.iconSizes.medium
            name: person.modelData.name
        }

        ColumnLayout {
            Layout.fillWidth: true
            spacing: 0

            QQC2.Label {
                Layout.fillWidth: true
                elide: Text.ElideRight
                text: person.modelData.name
            }

            QQC2.Label {
                Layout.fillWidth: true
                color: Kirigami.Theme.disabledTextColor
                elide: Text.ElideRight
                font: Kirigami.Theme.smallFont
                text: person.modelData.task ?? ""
                visible: text !== ""
            }
        }

        QQC2.ToolButton {
            Accessible.name: qsTr("Visit %1’s web page").arg(person.modelData.name)
            display: QQC2.AbstractButton.IconOnly
            icon.name: "globe-symbolic"
            text: Accessible.name
            visible: person.webAddress !== ""
            QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
            QQC2.ToolTip.text: person.webAddress
            QQC2.ToolTip.visible: hovered

            onClicked: person.linkRequested(person.webAddress)
        }
    }
}
