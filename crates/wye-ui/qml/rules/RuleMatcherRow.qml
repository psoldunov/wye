// RuleMatcherRow (RUL-14): one URL matcher of the rule editor: a kind popup, the Match entry with an example for the kind,
// and a remove button; an invalid pattern shows its error under the entry. A card row like WyeRow: the same padding, the
// inset hairline above every row but the first, no hover.
//
// API
//   kind: string               domain, prefix, contains, wildcard, regex
//   pattern: string
//   error: string              the pattern's problem, empty when it is fine
//   editable: bool
//   edited(string kind, string pattern)   the user changed the row
//   removeRequested()
//   focusEntry()               put the cursor in the entry ("+" adds a row and focuses it)
pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.kde.kirigamiaddons.formcard as FormCard

FormCard.AbstractFormDelegate {
    id: row

    // Required, so a Repeater over a model with `kind` and `pattern` roles fills them.
    required property string kind
    required property string pattern
    property string error
    property bool editable: true

    // The error shows once there is a pattern to judge; an empty row says nothing yet (Save stays disabled, RUL-18).
    readonly property bool showsError: row.error !== "" && row.pattern !== ""

    signal edited(string kind, string pattern)
    signal removeRequested

    // The kinds and an example pattern for each (19-help-texts.md, "URL matchers").
    readonly property var kinds: [
        {
            "value": "domain",
            "label": qsTr("Domain"),
            "example": "github.com"
        },
        {
            "value": "prefix",
            "label": qsTr("Starts with"),
            "example": "docs.google.com/spreadsheets"
        },
        {
            "value": "contains",
            "label": qsTr("Contains"),
            "example": "/pull/"
        },
        {
            "value": "wildcard",
            "label": qsTr("Wildcard"),
            "example": "*.atlassian.net/browse/*"
        },
        {
            "value": "regex",
            "label": qsTr("Regular expression"),
            "example": "^meet\\.google\\.com/[a-z]{3}-"
        }
    ]

    function focusEntry() {
        entry.forceActiveFocus();
    }

    // A row is not a button: its controls take the focus and the clicks.
    hoverEnabled: false
    focusPolicy: Qt.NoFocus
    background: Item {}
    Accessible.name: qsTr("URL matcher")

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

    contentItem: GridLayout {
        columns: 3
        columnSpacing: Kirigami.Units.smallSpacing
        rowSpacing: Kirigami.Units.smallSpacing

        QQC2.ComboBox {
            id: kindBox

            enabled: row.editable
            model: row.kinds
            textRole: "label"
            valueRole: "value"
            // As wide as the widest kind, so every row's entry starts at the same place.
            implicitContentWidthPolicy: QQC2.ComboBox.WidestText
            currentIndex: Math.max(0, row.kinds.findIndex(kind => kind.value === row.kind))
            Accessible.name: qsTr("Kind")
            onActivated: row.edited(currentValue, row.pattern)
        }

        QQC2.TextField {
            id: entry

            Layout.fillWidth: true
            Layout.minimumWidth: Kirigami.Units.gridUnit * 6
            enabled: row.editable
            text: row.pattern
            placeholderText: row.kinds.find(kind => kind.value === row.kind)?.example ?? ""
            Accessible.name: qsTr("Match")
            Accessible.description: row.showsError ? row.error : ""
            onTextEdited: row.edited(row.kind, text)
        }

        QQC2.ToolButton {
            display: QQC2.AbstractButton.IconOnly
            enabled: row.editable
            icon.name: "list-remove-symbolic"
            text: qsTr("Remove Matcher")
            QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
            QQC2.ToolTip.text: text
            QQC2.ToolTip.visible: hovered
            onClicked: row.removeRequested()
        }

        // RUL-14: the error, under the entry it is about.
        Item {
            visible: row.showsError
            implicitWidth: kindBox.implicitWidth
        }
        RowLayout {
            Layout.columnSpan: 2
            Layout.fillWidth: true
            visible: row.showsError
            spacing: Kirigami.Units.smallSpacing

            Kirigami.Icon {
                Layout.alignment: Qt.AlignTop
                implicitHeight: Kirigami.Units.iconSizes.small
                implicitWidth: Kirigami.Units.iconSizes.small
                // Breeze's own error glyph, already in the negative colour; it has no symbolic variant to tint.
                source: "dialog-error"
            }
            QQC2.Label {
                Layout.fillWidth: true
                color: Kirigami.Theme.negativeTextColor
                font: Kirigami.Theme.smallFont
                text: row.error
                wrapMode: Text.Wrap
            }
        }
    }
}
