// The URL line (PICK-09): "from Slack", then the link with the host
// emphasised and the rest dimmed, cut in the middle by the backend. The full
// link is in the tooltip.
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

    // `text` with the characters StyledText reads as markup escaped.
    function escaped(text) {
        return text.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
    }

    spacing: Kirigami.Units.smallSpacing

    Kirigami.Icon {
        Layout.preferredWidth: Kirigami.Units.iconSizes.small
        Layout.preferredHeight: Kirigami.Units.iconSizes.small
        visible: line.sourceName !== "" && line.sourceIcon !== ""
        source: line.sourceIcon
    }

    QQC2.Label {
        visible: line.sourceName !== ""
        text: qsTr("from %1").arg(line.sourceName)
        font: Kirigami.Theme.smallFont
        opacity: 0.7
    }

    QQC2.Label {
        Layout.fillWidth: true
        textFormat: Text.StyledText
        text: "<b>" + line.escaped(line.host) + "</b><font color=\"" + Qt.alpha(Kirigami.Theme.textColor, 0.6) + "\">" + line.escaped(line.rest) + "</font>"
        font: Kirigami.Theme.smallFont
        elide: Text.ElideMiddle

        HoverHandler {
            id: hover
        }

        QQC2.ToolTip.visible: hover.hovered
        QQC2.ToolTip.text: line.full
    }
}
