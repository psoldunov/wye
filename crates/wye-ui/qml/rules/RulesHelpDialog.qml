// RulesHelpDialog (RUL-19, 19-help-texts.md "Rules help"): the rule editor's help, a dialog with seven sections. The URL
// matchers section lists each kind with its example pattern, what it matches and what it does not (the spec's table).
//
// API
//   open() / close()
pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import dev.soldunov.wye.ui

WyeSheet {
    id: dialog

    // The kinds of URL matcher and their examples (19-help-texts.md, "URL matchers").
    readonly property var kinds: [
        {
            "kind": qsTr("Domain"),
            "pattern": "github.com",
            "matches": "github.com/x, gist.github.com/y",
            "misses": "notgithub.com"
        },
        {
            "kind": qsTr("Starts with"),
            "pattern": "docs.google.com/spreadsheets",
            "matches": "docs.google.com/spreadsheets/d/1",
            "misses": "docs.google.com/document/d/1"
        },
        {
            "kind": qsTr("Contains"),
            "pattern": "/pull/",
            "matches": "github.com/a/b/pull/7",
            "misses": "github.com/a/b/issues/7"
        },
        {
            "kind": qsTr("Wildcard"),
            "pattern": "*.atlassian.net/browse/*",
            "matches": "team.atlassian.net/browse/ABC-1",
            "misses": "atlassian.net/wiki"
        },
        {
            "kind": qsTr("Regular expression"),
            "pattern": "^meet\\.google\\.com/[a-z]{3}-",
            "matches": "meet.google.com/abc-defg-hij",
            "misses": "meet.google.com/landing"
        }
    ]

    // One section: a heading and its text, inset like the sheet's title.
    component Part: ColumnLayout {
        property alias heading: title.text
        property alias body: text.text
        default property alias extra: extraColumn.data

        Layout.fillWidth: true
        Layout.topMargin: Kirigami.Units.largeSpacing
        Layout.leftMargin: Kirigami.Units.largeSpacing * 2
        Layout.rightMargin: Kirigami.Units.largeSpacing * 2
        spacing: Kirigami.Units.smallSpacing

        Kirigami.Heading {
            id: title

            Layout.fillWidth: true
            level: 4
            type: Kirigami.Heading.Type.Primary
            wrapMode: Text.Wrap
        }
        // Rich text, so <code> is drawn in the fixed-width font.
        QQC2.Label {
            id: text

            Layout.fillWidth: true
            textFormat: Text.RichText
            wrapMode: Text.Wrap
        }
        ColumnLayout {
            id: extraColumn

            Layout.fillWidth: true
            spacing: Kirigami.Units.largeSpacing
            visible: children.length > 0
        }
    }

    title: qsTr("How Rules Work")
    primaryText: qsTr("Done")
    sheetWidth: Kirigami.Units.gridUnit * 30

    onPrimaryTriggered: close()

    Part {
        heading: qsTr("How rules work")
        body: qsTr("Rules are checked from the top of the list down; the first rule that matches decides where the link opens. A rule can check the link, the app it was clicked in, and the keys held while clicking.")
    }

    Part {
        heading: qsTr("URL matchers")
        body: qsTr("Wye removes <code>https://</code> and a leading <code>www.</code> before matching.")

        Repeater {
            model: dialog.kinds

            delegate: ColumnLayout {
                id: kindEntry

                required property var modelData

                Layout.fillWidth: true
                spacing: 0

                RowLayout {
                    Layout.fillWidth: true
                    spacing: Kirigami.Units.largeSpacing

                    QQC2.Label {
                        font.weight: Font.Bold
                        text: kindEntry.modelData.kind
                    }
                    QQC2.Label {
                        Layout.fillWidth: true
                        elide: Text.ElideRight
                        font: Kirigami.Theme.fixedWidthFont
                        text: kindEntry.modelData.pattern
                    }
                }
                QQC2.Label {
                    Layout.fillWidth: true
                    Layout.leftMargin: Kirigami.Units.gridUnit
                    color: Kirigami.Theme.disabledTextColor
                    text: qsTr("Matches %1").arg(kindEntry.modelData.matches)
                    wrapMode: Text.Wrap
                }
                QQC2.Label {
                    Layout.fillWidth: true
                    Layout.leftMargin: Kirigami.Units.gridUnit
                    color: Kirigami.Theme.disabledTextColor
                    text: qsTr("Does not match %1").arg(kindEntry.modelData.misses)
                    wrapMode: Text.Wrap
                }
            }
        }
    }

    Part {
        heading: qsTr("Source apps")
        body: qsTr("The rule matches only links opened from one of these apps. Some apps (for example sandboxed Flatpak apps) cannot always be identified; rules with source apps then do not match.")
    }

    Part {
        heading: qsTr("Held keys")
        body: qsTr("The rule matches only while exactly these modifier keys are held.")
    }

    Part {
        heading: qsTr("Before or after built-in rules")
        body: qsTr("Built-in rules are the mappings on the Apps page. “Before” lets a rule override them.")
    }

    Part {
        heading: qsTr("Transform URL")
        body: qsTr("A script that rewrites the link when this rule matches. See the script editor's Reference.")
    }

    Part {
        heading: qsTr("Testing")
        body: qsTr("Use <b>Test Rules…</b> in the Rules page menu to see which rule a link hits.")
    }

    Item {
        implicitHeight: Kirigami.Units.largeSpacing
    }
}
