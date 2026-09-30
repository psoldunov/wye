// AboutContent (DLG-ABT-01, DLG-ABT-02): the About page, laid out like Kirigami Addons' `FormCard.AboutPage`: Wye's icon,
// name, version and description; copyright; the licence (its text opens in a dialog); homepage and issue tracker; authors and
// credits; then the Troubleshooting section. It is built from the same FormCard delegates inside WyeGroupCard because
// `AboutPage` needs the `i18nd` functions KDE's `KLocalizedContext` installs, which this host does not have, and because
// FormCards placed straight into a ScrollablePage never settle their width here (each card's inset depends on the width the
// layout gives it, and the layout's width depends on the cards); WyeGroupCard sizes the card from its own width. Links go
// through Wye (BLK-17).
//
// API
//   aboutData: var         the shape of KAboutData (crates/wye-ui/src/about/info.rs): displayName, version, shortDescription,
//                          homepage, bugAddress, copyrightStatement, licenses, authors, credits
//   troubleshooting: string  what Wye detected in this session (GetTroubleshooting)
//   error: string          why something could not be read; empty for none
//   linkRequested(url)     a link was chosen: open it through Wye
//   copyRequested(text)    Copy: put `text` on the clipboard
pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.kde.kirigamiaddons.formcard as FormCard
import dev.soldunov.wye.ui

Kirigami.Page {
    id: page

    property var aboutData: ({})
    property string troubleshooting
    property string error
    signal linkRequested(string url)
    signal copyRequested(string text)

    padding: 0
    title: qsTr("About %1").arg(page.aboutData.displayName ?? "Wye")

    QQC2.ScrollView {
        id: scroll

        anchors.fill: parent
        contentWidth: availableWidth

        ColumnLayout {
            spacing: Kirigami.Units.largeSpacing
            width: scroll.availableWidth

            // The name, version and one-line description (DLG-ABT-01).
            ColumnLayout {
                Layout.fillWidth: true
                Layout.margins: Kirigami.Units.gridUnit
                spacing: Kirigami.Units.smallSpacing

                Kirigami.Icon {
                    Layout.alignment: Qt.AlignHCenter
                    Layout.preferredHeight: Kirigami.Units.iconSizes.enormous
                    Layout.preferredWidth: Kirigami.Units.iconSizes.enormous
                    source: page.aboutData.desktopFileName ?? "dev.soldunov.wye"
                }

                Kirigami.Heading {
                    Layout.fillWidth: true
                    horizontalAlignment: Text.AlignHCenter
                    text: (page.aboutData.displayName ?? "Wye") + " " + (page.aboutData.version ?? "")
                    wrapMode: Text.WordWrap
                }

                Kirigami.Heading {
                    Layout.fillWidth: true
                    horizontalAlignment: Text.AlignHCenter
                    level: 3
                    text: page.aboutData.shortDescription ?? ""
                    type: Kirigami.Heading.Type.Secondary
                    wrapMode: Text.WordWrap
                }

                QQC2.Label {
                    Layout.fillWidth: true
                    horizontalAlignment: Text.AlignHCenter
                    text: page.aboutData.copyrightStatement ?? ""
                    visible: text !== ""
                    wrapMode: Text.WordWrap
                }
            }

            // The licence: one row per licence; its text opens in a dialog.
            WyeGroupCard {
                title: qsTr("License")
                visible: (page.aboutData.licenses ?? []).length > 0

                Repeater {
                    model: page.aboutData.licenses ?? []

                    FormCard.FormButtonDelegate {
                        required property var modelData

                        description: modelData.spdx
                        text: modelData.name

                        onClicked: {
                            licenseDialog.title = modelData.name;
                            licenseText.text = modelData.text;
                            licenseDialog.open();
                        }
                    }
                }
            }

            // The homepage and the issue tracker (DLG-ABT-01).
            WyeGroupCard {
                FormCard.FormButtonDelegate {
                    description: page.aboutData.homepage ?? ""
                    icon.name: "globe-symbolic"
                    text: qsTr("Homepage")
                    visible: (page.aboutData.homepage ?? "") !== ""

                    onClicked: page.linkRequested(page.aboutData.homepage)
                }

                FormCard.FormButtonDelegate {
                    description: page.aboutData.bugAddress ?? ""
                    icon.name: "tools-report-bug-symbolic"
                    text: qsTr("Report a Bug")
                    visible: (page.aboutData.bugAddress ?? "") !== ""

                    onClicked: page.linkRequested(page.aboutData.bugAddress)
                }
            }

            WyeGroupCard {
                title: qsTr("Authors")
                visible: (page.aboutData.authors ?? []).length > 0

                Repeater {
                    model: page.aboutData.authors ?? []

                    FormCard.FormButtonDelegate {
                        required property var modelData

                        description: modelData.task
                        enabled: (modelData.webAddress ?? "") !== ""
                        text: modelData.name

                        onClicked: page.linkRequested(modelData.webAddress)
                    }
                }
            }

            WyeGroupCard {
                title: qsTr("Credits")
                visible: (page.aboutData.credits ?? []).length > 0

                Repeater {
                    model: page.aboutData.credits ?? []

                    FormCard.FormButtonDelegate {
                        required property var modelData

                        description: modelData.task
                        enabled: (modelData.webAddress ?? "") !== ""
                        text: modelData.name

                        onClicked: page.linkRequested(modelData.webAddress)
                    }
                }
            }

            // DLG-ABT-02: what Wye detected in this session, one fact per line, and a button that copies it for bug reports.
            WyeGroupCard {
                title: qsTr("Troubleshooting")

                Kirigami.InlineMessage {
                    Layout.fillWidth: true
                    text: page.error
                    type: Kirigami.MessageType.Error
                    visible: page.error !== ""
                }

                Kirigami.SelectableLabel {
                    Layout.fillWidth: true
                    Layout.margins: Kirigami.Units.largeSpacing
                    font.family: "monospace"
                    font.pointSize: Kirigami.Theme.smallFont.pointSize
                    text: page.troubleshooting === "" ? qsTr("Reading what Wye detected…") : page.troubleshooting
                    wrapMode: Text.WrapAtWordBoundaryOrAnywhere
                }

                FormCard.FormButtonDelegate {
                    enabled: page.troubleshooting !== ""
                    icon.name: "edit-copy-symbolic"
                    text: qsTr("Copy")

                    onClicked: page.copyRequested(page.troubleshooting)
                }
            }
        }
    }

    Kirigami.Dialog {
        id: licenseDialog

        implicitHeight: Kirigami.Units.gridUnit * 24
        implicitWidth: Kirigami.Units.gridUnit * 32
        padding: 0
        standardButtons: Kirigami.Dialog.Close

        QQC2.ScrollView {
            Kirigami.SelectableLabel {
                id: licenseText

                textMargin: Kirigami.Units.gridUnit
                width: parent.width
                wrapMode: Text.WordWrap
            }
        }
    }
}
