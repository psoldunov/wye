// TesterRow (DLG-TST-02): one row of the rule tester's Steps card: a dimmed label in a column of its own, then what
// happened. A card row like WyeRow: the same padding, the inset hairline above every row but the first, no hover.
//
// API
//   label: string          the step's label ("Expanded", "Opens in")
//   labelWidth: real       the label column's width, the same for every row of the card
//   labelEmphasis: bool    the outcome row: the label in the normal text colour
//   default property       what happened: labels, icons, laid out in a row after the label
pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.kde.kirigamiaddons.formcard as FormCard

FormCard.AbstractFormDelegate {
    id: row

    property string label
    property real labelWidth: Kirigami.Units.gridUnit * 5
    property bool labelEmphasis: false
    default property alias content: contentRow.data

    hoverEnabled: false
    focusPolicy: Qt.NoFocus
    background: Item {}
    Accessible.name: row.label

    Kirigami.Separator {
        anchors {
            top: parent.top
            left: parent.left
            right: parent.right
            leftMargin: row.leftPadding
            rightMargin: row.rightPadding
        }
        visible: row.y > 0
    }

    contentItem: RowLayout {
        spacing: Kirigami.Units.largeSpacing

        QQC2.Label {
            Layout.alignment: Qt.AlignTop
            Layout.preferredWidth: row.labelWidth
            color: row.labelEmphasis ? Kirigami.Theme.textColor : Kirigami.Theme.disabledTextColor
            text: row.label
        }

        RowLayout {
            id: contentRow

            Layout.fillWidth: true
            spacing: Kirigami.Units.smallSpacing
        }
    }
}
