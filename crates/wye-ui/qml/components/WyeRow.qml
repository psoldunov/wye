// WyeRow (BLK-02): a title on the left, an optional subtitle under it, a trailing control on the right. It is the base of
// every row below; use it directly for a row with a custom trailing control.
//
// API
//   title: string          the row title (bold-free, normal weight); it takes the width the trailing control leaves and
//                          wraps there
//   subtitle: string       rich text under the title, smaller and dimmed; wraps; may hold <a> links (routed through
//                          Wye, BLK-17) and <code>
//   titleFormat: int       Text.PlainText (default) or Text.RichText: a title with a link (Songlink, EXT-05); links go
//                          through Wye (BLK-17)
//   leadingIcon: string    optional icon before the title: a status (the General page's default-browser row)
//   leadingIconColor: color  tints a symbolic leadingIcon (a status colour); unset draws the icon as it is
//   help: string           help text ID (BLK-08, settings/help.rs); a "?" button follows the title text
//   helpText: string       or the help text itself
//   dimmed: bool           the setting does not apply (BLK-10): title, subtitle and trailing control are dimmed and the
//                          control is disabled; the help button still works, so it can say why
//   default property       the trailing control(s), centred on the title. They keep their implicit width: the title
//                          wraps first, so a control is never cut off.
//   trailingEnabled: bool  extra condition for the trailing controls
//   needsConfig: bool      the control changes the configuration (default true): it is disabled while the file is
//                          read-only (Status config.writable). Set false for a control that does something else, such as
//                          Make Default or Rescan.
pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.kde.kirigamiaddons.formcard as FormCard
import dev.soldunov.wye.ui

FormCard.AbstractFormDelegate {
    id: row

    property string title
    property string subtitle
    property int titleFormat: Text.PlainText
    property string leadingIcon
    property color leadingIconColor: "transparent"
    property string help
    property string helpText
    property bool dimmed: false
    property bool trailingEnabled: true
    property bool needsConfig: true
    default property alias trailing: trailingLayout.data

    // A row is not a button: no hover highlight, no press, and no stop in the Tab chain (its controls have their own).
    hoverEnabled: false
    focusPolicy: Qt.NoFocus
    background: Item {}
    text: title
    Accessible.name: title

    // The inset hairline above every row but the first (BLK-01).
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

        Kirigami.Icon {
            Layout.alignment: Qt.AlignVCenter
            implicitHeight: Kirigami.Units.iconSizes.smallMedium
            implicitWidth: Kirigami.Units.iconSizes.smallMedium
            color: row.leadingIconColor
            isMask: row.leadingIconColor.a > 0
            opacity: row.dimmed ? 0.5 : 1
            source: row.leadingIcon
            visible: row.leadingIcon !== ""
        }

        ColumnLayout {
            Layout.fillWidth: true
            Layout.minimumWidth: Kirigami.Units.gridUnit * 6
            spacing: 0

            RowLayout {
                Layout.fillWidth: true
                spacing: Kirigami.Units.smallSpacing

                // As wide as its text, and no wider than the room there is: the help button then sits right after the
                // last word, and a long title wraps instead of pushing the trailing control away (BLK-02, BLK-08).
                QQC2.Label {
                    id: titleLabel

                    Layout.fillWidth: true
                    Layout.maximumWidth: Math.ceil(titleMetrics.advanceWidth) + 1
                    linkColor: Kirigami.Theme.linkColor
                    opacity: row.dimmed ? 0.5 : 1
                    text: row.title
                    textFormat: row.titleFormat
                    wrapMode: Text.Wrap

                    onLinkActivated: link => SettingsBackend.openLink(link)

                    HoverHandler {
                        cursorShape: titleLabel.hoveredLink !== "" ? Qt.PointingHandCursor : Qt.ArrowCursor
                    }
                }

                // The title's one-line width; a wrapping label's own implicit width follows the width it was given.
                TextMetrics {
                    id: titleMetrics

                    font: titleLabel.font
                    text: row.titleFormat === Text.PlainText ? row.title : row.title.replace(/<[^>]*>/g, "")
                }

                WyeHelpButton {
                    Layout.alignment: Qt.AlignVCenter
                    body: row.helpText
                    helpId: row.help
                    opacity: row.dimmed ? 0.5 : 1
                }

                Item {
                    Layout.fillWidth: true
                }
            }

            WyeLinkText {
                Layout.fillWidth: true
                opacity: row.dimmed ? 0.5 : 1
                text: row.subtitle
                visible: row.subtitle !== ""
            }
        }

        RowLayout {
            id: trailingLayout

            Layout.alignment: Qt.AlignVCenter | Qt.AlignRight
            Layout.minimumWidth: implicitWidth
            enabled: !row.dimmed && row.trailingEnabled && (!row.needsConfig || SettingsBackend.writable)
            opacity: row.dimmed ? 0.5 : 1
            spacing: Kirigami.Units.smallSpacing
        }
    }
}
