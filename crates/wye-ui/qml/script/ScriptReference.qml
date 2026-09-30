// The script API and examples (SCR-06, 16-script-editor.md "Script API").
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

Kirigami.OverlaySheet {
    id: sheet

    // One code sample, selectable.
    component Code: QQC2.TextArea {
        Layout.fillWidth: true
        readOnly: true
        selectByMouse: true
        wrapMode: TextEdit.NoWrap
        textFormat: TextEdit.PlainText
        font: Kirigami.Theme.fixedWidthFont
    }

    title: qsTr("Script Reference")

    ColumnLayout {
        spacing: Kirigami.Units.largeSpacing

        QQC2.Label {
            Layout.fillWidth: true
            wrapMode: Text.Wrap
            text: qsTr("The script exports a default function. It receives the link after expansion and cleaning as a URL object, which it may change, and a context. Return a URL or a string to open a different link, or nothing to keep it. The result must be an http or https link.")
        }

        Code {
            text: "/**\n * @param {URL} url           The link. Mutable.\n * @param {object} context\n * @param {string|null} context.sourceApp   Desktop ID of the source app, if known.\n * @param {string} context.entryPoint       \"handler\" | \"clipboard\" | \"extension\" | \"cli\"\n * @param {string[]} context.heldKeys       e.g. [\"Ctrl\"]\n * @param {string|null} context.rule        Matched rule name (per-rule scripts only).\n * @returns {URL|string|undefined}\n */\nexport default function transform(url, context) {}"
        }

        QQC2.Label {
            Layout.fillWidth: true
            wrapMode: Text.Wrap
            text: qsTr("Available: URL, URLSearchParams and console.log, which writes to Wye's log and to the result below the editor. There is no network, file access or timers. A run may take 50 ms and 16 MB of memory.")
        }

        Kirigami.Heading {
            level: 3
            text: qsTr("Examples")
        }

        Code {
            text: "// Open Reddit links on old.reddit.com\nexport default function transform(url) {\n  if (url.hostname.endsWith(\"reddit.com\")) url.hostname = \"old.reddit.com\";\n  return url;\n}"
        }

        Code {
            text: "// Send YouTube links to a self-hosted front end\nexport default function transform(url) {\n  if (url.hostname === \"youtu.be\") {\n    return `https://invidious.example.org/watch?v=${url.pathname.slice(1)}`;\n  }\n}"
        }
    }
}
