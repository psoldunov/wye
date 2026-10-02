// The URL line (PICK-09): "from Slack", then the link with the host
// emphasised and the rest dimmed. The backend cuts a long link in the middle
// (URL_LINE_CHARS); what still does not fit the panel is cut in the middle of
// the rest, so the host and the end of the path stay visible. The full link
// is in the tooltip. The picker centres the line and caps its width.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

RowLayout {
    id: line

    property string host
    property string rest
    property string full
    property string sourceName
    property string sourceIcon

    spacing: 0
    Accessible.role: Accessible.StaticText
    Accessible.name: sourceName !== "" ? qsTr("From %1: %2").arg(sourceName).arg(full) : full

    Kirigami.Icon {
        Layout.preferredWidth: Kirigami.Units.iconSizes.small
        Layout.preferredHeight: Kirigami.Units.iconSizes.small
        Layout.rightMargin: Kirigami.Units.smallSpacing
        visible: line.sourceName !== "" && line.sourceIcon !== ""
        source: line.sourceIcon
        fallback: "application-x-executable"
    }

    QQC2.Label {
        Layout.preferredWidth: Math.ceil(implicitWidth)
        Layout.rightMargin: Kirigami.Units.largeSpacing
        visible: line.sourceName !== ""
        text: qsTr("from %1").arg(line.sourceName)
        textFormat: Text.PlainText
        font: Kirigami.Theme.smallFont
        opacity: 0.7
    }

    // The host gives way last. It stands out by weight alone (PICK-09): the
    // same small font as the rest, in bold. Styled text keeps it the very same
    // font, where a font built from the theme's could resolve to another family.
    // Widths are whole pixels: the layout rounds a fractional text width down,
    // which cut a short link that had room ("example.com/…" for "/a").
    QQC2.Label {
        Layout.preferredWidth: Math.ceil(implicitWidth)
        Layout.maximumWidth: Math.ceil(implicitWidth)
        Layout.minimumWidth: Math.min(Math.ceil(implicitWidth), Kirigami.Units.gridUnit * 12)
        Layout.fillWidth: true
        text: "<b>" + line.host.replace(/&/g, "&amp;").replace(/</g, "&lt;") + "</b>"
        textFormat: Text.StyledText
        font: Kirigami.Theme.smallFont
        elide: Text.ElideRight
    }

    QQC2.Label {
        Layout.preferredWidth: Math.ceil(implicitWidth)
        Layout.maximumWidth: Math.ceil(implicitWidth)
        Layout.fillWidth: true
        text: line.rest
        textFormat: Text.PlainText
        font: Kirigami.Theme.smallFont
        elide: Text.ElideMiddle
        opacity: 0.6
    }

    HoverHandler {
        id: hover
    }

    QQC2.ToolTip.visible: hover.hovered && full !== ""
    QQC2.ToolTip.text: full
    QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
}
