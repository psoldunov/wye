// RuleMatcherRow (RUL-14): one URL matcher of the rule editor: a kind popup, the Match entry with an example for the kind,
// and a remove button; an invalid pattern shows its error under the row.
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

ColumnLayout {
    id: row

    // Required, so a Repeater over a model with `kind` and `pattern` roles fills them.
    required property string kind
    required property string pattern
    property string error
    property bool editable: true

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

    spacing: 0

    RowLayout {
        Layout.fillWidth: true
        Layout.margins: Kirigami.Units.smallSpacing

        QQC2.ComboBox {
            enabled: row.editable
            model: row.kinds
            textRole: "label"
            valueRole: "value"
            currentIndex: Math.max(0, row.kinds.findIndex(kind => kind.value === row.kind))
            Accessible.name: qsTr("Kind")
            onActivated: row.edited(currentValue, row.pattern)
        }

        QQC2.TextField {
            id: entry

            Layout.fillWidth: true
            enabled: row.editable
            text: row.pattern
            placeholderText: row.kinds.find(kind => kind.value === row.kind)?.example ?? ""
            Accessible.name: qsTr("Match")
            onTextEdited: row.edited(row.kind, text)
        }

        QQC2.ToolButton {
            display: QQC2.AbstractButton.IconOnly
            enabled: row.editable
            icon.name: "list-remove"
            text: qsTr("Remove Matcher")
            QQC2.ToolTip.text: text
            QQC2.ToolTip.visible: hovered
            onClicked: row.removeRequested()
        }
    }

    QQC2.Label {
        Layout.fillWidth: true
        Layout.leftMargin: Kirigami.Units.smallSpacing
        Layout.bottomMargin: Kirigami.Units.smallSpacing
        visible: row.error !== "" && row.pattern !== ""
        color: Kirigami.Theme.negativeTextColor
        font: Kirigami.Theme.smallFont
        text: row.error
        wrapMode: Text.Wrap
    }
}
