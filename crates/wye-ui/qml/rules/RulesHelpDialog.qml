// RulesHelpDialog (RUL-19, 19-help-texts.md "Rules help"): the rule editor's help, a dialog with seven sections.
//
// API
//   open() / close()
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import dev.soldunov.wye.ui

WyeSheet {
    id: dialog

    // One section: a heading and its text.
    component Part: ColumnLayout {
        property alias heading: title.text
        property alias body: text.text

        Layout.fillWidth: true
        spacing: Kirigami.Units.smallSpacing

        Kirigami.Heading {
            id: title

            Layout.fillWidth: true
            level: 4
            wrapMode: Text.Wrap
        }
        QQC2.Label {
            id: text

            Layout.fillWidth: true
            textFormat: Text.StyledText
            wrapMode: Text.Wrap
        }
    }

    title: qsTr("How Rules Work")
    primaryText: qsTr("Done")
    sheetWidth: Kirigami.Units.gridUnit * 26

    onPrimaryTriggered: close()

    Part {
        heading: qsTr("How rules work")
        body: qsTr("Rules are checked from the top of the list down; the first rule that matches decides where the link opens. A rule can check the link, the app it was clicked in, and the keys held while clicking.")
    }

    Part {
        heading: qsTr("URL matchers")
        body: qsTr("Wye removes <code>https://</code> and a leading <code>www.</code> before matching.") + "<br><br>" + qsTr("<b>Domain</b> <code>github.com</code> matches github.com/x and gist.github.com/y, not notgithub.com.") + "<br>" + qsTr("<b>Starts with</b> <code>docs.google.com/spreadsheets</code> matches docs.google.com/spreadsheets/d/1, not docs.google.com/document/d/1.") + "<br>" + qsTr("<b>Contains</b> <code>/pull/</code> matches github.com/a/b/pull/7, not github.com/a/b/issues/7.") + "<br>" + qsTr("<b>Wildcard</b> <code>*.atlassian.net/browse/*</code> matches team.atlassian.net/browse/ABC-1, not atlassian.net/wiki.") + "<br>" + qsTr("<b>Regular expression</b> <code>^meet\\.google\\.com/[a-z]{3}-</code> matches meet.google.com/abc-defg-hij, not meet.google.com/landing.")
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
        body: qsTr("Built-in rules are the mappings on the Apps page. \"Before\" lets a rule override them.")
    }

    Part {
        heading: qsTr("Transform URL")
        body: qsTr("A script that rewrites the link when this rule matches. See the script editor's Reference.")
    }

    Part {
        heading: qsTr("Testing")
        body: qsTr("Use <b>Test Rules…</b> in the Rules page menu to see which rule a link hits.")
    }
}
