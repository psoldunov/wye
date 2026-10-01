// HistoryBadge (DLG-HIS-02): a small rounded label on a history row's second line: why the link went where it did ("rule
// “Meetings”", "picker choice") or what changed it ("cleaned", "expanded").
//
// API
//   text: string           the label
//   tint: color            the badge's colour; the label and outline follow it
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

Rectangle {
    id: badge

    property string text
    property color tint: Kirigami.Theme.disabledTextColor

    Accessible.name: badge.text
    Accessible.role: Accessible.StaticText
    Layout.alignment: Qt.AlignVCenter
    Layout.maximumWidth: implicitWidth
    color: Qt.alpha(badge.tint, 0.12)
    border.color: Qt.alpha(badge.tint, 0.35)
    border.width: 1
    implicitHeight: label.implicitHeight + Kirigami.Units.smallSpacing / 2
    implicitWidth: label.implicitWidth + Kirigami.Units.largeSpacing * 1.5
    radius: height / 2

    QQC2.Label {
        id: label

        anchors.centerIn: parent
        color: badge.tint
        elide: Text.ElideRight
        font: Kirigami.Theme.smallFont
        text: badge.text
        width: Math.min(implicitWidth, badge.width - Kirigami.Units.largeSpacing)
    }
}
