// WyeRow (BLK-02): a title on the left, an optional subtitle under it, a trailing control on the right. It is the base of
// every row below; use it directly for a row with a custom trailing control.
//
// API
//   title: string          the row title (bold-free, normal weight)
//   subtitle: string       rich text under the title, smaller and dimmed; wraps; may hold <a> links (routed through
//                          Wye, BLK-17) and <code>
//   titleFormat: int       Text.PlainText (default) or Text.RichText: a title with a link (Songlink, EXT-05); links go
//                          through Wye (BLK-17)
//   help: string           help text ID (BLK-08, settings/help.rs); a "?" button follows the title
//   helpText: string       or the help text itself
//   dimmed: bool           the setting does not apply (BLK-10): title, subtitle and trailing control are dimmed and the
//                          control is disabled; the help button still works, so it can say why
//   default property       the trailing control(s), centred on the title
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
    property string help
    property string helpText
    property bool dimmed: false
    property bool trailingEnabled: true
    property bool needsConfig: true
    default property alias trailing: trailingLayout.data

    // A row is not a button: no hover highlight, no press.
    hoverEnabled: false
    background: Item {}
    text: title

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

        ColumnLayout {
            Layout.fillWidth: true
            spacing: 0

            RowLayout {
                spacing: Kirigami.Units.smallSpacing

                QQC2.Label {
                    Layout.fillWidth: false
                    Layout.maximumWidth: Kirigami.Units.gridUnit * 14
                    linkColor: Kirigami.Theme.linkColor
                    text: row.title
                    textFormat: row.titleFormat
                    opacity: row.dimmed ? 0.5 : 1
                    wrapMode: Text.WordWrap

                    onLinkActivated: link => SettingsBackend.openLink(link)
                }

                WyeHelpButton {
                    Layout.maximumHeight: Kirigami.Units.iconSizes.smallMedium
                    Layout.preferredWidth: Kirigami.Units.iconSizes.smallMedium
                    helpId: row.help
                    body: row.helpText
                    opacity: row.dimmed ? 0.5 : 1
                }

                Item {
                    Layout.fillWidth: true
                }
            }

            WyeLinkText {
                Layout.fillWidth: true
                text: row.subtitle
                visible: row.subtitle !== ""
                opacity: row.dimmed ? 0.5 : 1
            }
        }

        RowLayout {
            id: trailingLayout

            Layout.alignment: Qt.AlignVCenter
            enabled: !row.dimmed && row.trailingEnabled && (!row.needsConfig || SettingsBackend.writable)
            opacity: row.dimmed ? 0.5 : 1
            spacing: Kirigami.Units.smallSpacing
        }
    }
}
