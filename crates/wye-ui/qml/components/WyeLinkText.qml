// WyeLinkText (BLK-17): dimmed, wrapping text that may hold accent-coloured links and inline `code`. A link opens through
// Wye's own pipeline, like any other link (SettingsBackend.openLink), never through the desktop's opener.
//
// API
//   text: string           rich text: <a href="https://…">label</a>, <code>…</code>, <b>…</b>
//   linkActivated(string)  also emitted, for a link the page wants to handle itself (accepted: set `routed: false`)
//   routed: bool           send links to Wye's pipeline (default true)
pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import dev.soldunov.wye.ui

QQC2.Label {
    id: label

    property bool routed: true

    color: Kirigami.Theme.disabledTextColor
    font: Kirigami.Theme.smallFont
    linkColor: Kirigami.Theme.linkColor
    textFormat: Text.RichText
    wrapMode: Text.WordWrap

    onLinkActivated: link => {
        if (label.routed) {
            SettingsBackend.openLink(link);
        }
    }

    HoverHandler {
        cursorShape: label.hoveredLink !== "" ? Qt.PointingHandCursor : Qt.ArrowCursor
    }
}
