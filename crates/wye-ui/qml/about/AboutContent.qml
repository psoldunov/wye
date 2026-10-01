// AboutContent (DLG-ABT-01, DLG-ABT-02): the About page, laid out like Kirigami Addons' `FormCard.AboutPage`: a card with
// Wye's icon, name, version and description and the copyright under them; the licence (its text opens in a dialog); homepage
// and issue tracker; authors and credits; then the Troubleshooting section with its Copy button. It is built from the same
// FormCard delegates inside AboutSection because `AboutPage` needs the `i18nd` functions KDE's `KLocalizedContext` installs,
// which this host does not have, and because FormCards placed straight into a ScrollablePage never settle their width here
// (each card's inset depends on the width the layout gives it, and the layout's width depends on the cards); AboutSection
// sizes the card from its own width. The page has no header of its own: the window's title already says "About Wye". Links
// go through Wye (BLK-17).
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
    readonly property string displayName: page.aboutData.displayName ?? "Wye"
    signal linkRequested(string url)
    signal copyRequested(string text)

    globalToolBarStyle: Kirigami.ApplicationHeaderStyle.None
    padding: 0
    title: qsTr("About %1").arg(page.displayName)

    QQC2.ScrollView {
        id: scroll

        anchors.fill: parent
        contentWidth: availableWidth

        ColumnLayout {
            spacing: Kirigami.Units.largeSpacing
            width: scroll.availableWidth

            // The name, version and one-line description, then the copyright (DLG-ABT-01).
            AboutSection {
                Layout.topMargin: Kirigami.Units.largeSpacing * 4

                FormCard.AbstractFormDelegate {
                    Accessible.name: qsTr("%1 %2, %3").arg(page.displayName).arg(page.aboutData.version ?? "").arg(page.aboutData.shortDescription ?? "")
                    Layout.fillWidth: true
                    background: null
                    focusPolicy: Qt.NoFocus
                    hoverEnabled: false

                    contentItem: RowLayout {
                        spacing: Kirigami.Units.largeSpacing * 2

                        Kirigami.Icon {
                            Layout.preferredHeight: Kirigami.Units.iconSizes.huge
                            Layout.preferredWidth: Kirigami.Units.iconSizes.huge
                            source: page.aboutData.desktopFileName ?? "dev.soldunov.wye"
                        }

                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: Kirigami.Units.smallSpacing

                            Kirigami.Heading {
                                Layout.fillWidth: true
                                text: (page.displayName + " " + (page.aboutData.version ?? "")).trim()
                                wrapMode: Text.WordWrap
                            }

                            Kirigami.Heading {
                                Layout.fillWidth: true
                                level: 3
                                text: page.aboutData.shortDescription ?? ""
                                type: Kirigami.Heading.Type.Secondary
                                visible: text !== ""
                                wrapMode: Text.WordWrap
                            }
                        }
                    }
                }

                FormCard.FormDelegateSeparator {
                    visible: copyright.description !== ""
                }

                FormCard.FormTextDelegate {
                    id: copyright

                    description: page.aboutData.copyrightStatement ?? ""
                    text: qsTr("Copyright")
                    visible: description !== ""
                }
            }

            // The licence: one row per licence; its text opens in a dialog. The SPDX identifier is not repeated under the
            // name ("MIT License" / "MIT"), as KDE's own About page does not.
            AboutSection {
                title: (page.aboutData.licenses ?? []).length > 1 ? qsTr("Licenses") : qsTr("License")
                visible: (page.aboutData.licenses ?? []).length > 0

                Repeater {
                    model: page.aboutData.licenses ?? []

                    FormCard.FormButtonDelegate {
                        required property var modelData

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
            AboutSection {
                // Not `homepage.visible`: a child of a hidden item reports itself hidden, so that would never turn true.
                visible: (page.aboutData.homepage ?? "") !== "" || (page.aboutData.bugAddress ?? "") !== ""

                FormCard.FormButtonDelegate {
                    id: homepage

                    description: page.aboutData.homepage ?? ""
                    icon.name: "globe-symbolic"
                    text: qsTr("Homepage")
                    visible: description !== ""

                    onClicked: page.linkRequested(page.aboutData.homepage)
                }

                FormCard.FormDelegateSeparator {
                    visible: homepage.description !== "" && bugs.description !== ""
                }

                // Breeze's `tools-report-bug` is a pixel-art face that reads as a missing image at fractional scales; the
                // flag says "report" in the same monochrome style as the globe above it.
                FormCard.FormButtonDelegate {
                    id: bugs

                    description: page.aboutData.bugAddress ?? ""
                    icon.name: "flag-symbolic"
                    text: qsTr("Report a Bug")
                    visible: description !== ""

                    onClicked: page.linkRequested(page.aboutData.bugAddress)
                }
            }

            AboutSection {
                title: qsTr("Authors")
                visible: (page.aboutData.authors ?? []).length > 0

                Repeater {
                    model: page.aboutData.authors ?? []

                    AboutPerson {
                        onLinkRequested: url => page.linkRequested(url)
                    }
                }
            }

            AboutSection {
                title: qsTr("Credits")
                visible: (page.aboutData.credits ?? []).length > 0

                Repeater {
                    model: page.aboutData.credits ?? []

                    AboutPerson {
                        onLinkRequested: url => page.linkRequested(url)
                    }
                }
            }

            // DLG-ABT-02: what Wye detected in this session, one fact per line, and a button that copies it for bug reports.
            AboutSection {
                Layout.bottomMargin: Kirigami.Units.largeSpacing * 2
                title: qsTr("Troubleshooting")

                actions: Kirigami.Action {
                    enabled: page.troubleshooting !== ""
                    icon.name: "edit-copy-symbolic"
                    text: qsTr("Copy")
                    tooltip: qsTr("Copy the troubleshooting information for a bug report")

                    onTriggered: page.copyRequested(page.troubleshooting)
                }

                Kirigami.InlineMessage {
                    Layout.fillWidth: true
                    Layout.margins: Kirigami.Units.largeSpacing
                    text: page.error
                    type: Kirigami.MessageType.Error
                    visible: page.error !== ""
                }

                FormCard.AbstractFormDelegate {
                    Layout.fillWidth: true
                    background: null
                    focusPolicy: Qt.NoFocus
                    hoverEnabled: false

                    contentItem: Kirigami.SelectableLabel {
                        Accessible.name: qsTr("Troubleshooting information")
                        color: page.troubleshooting !== "" ? Kirigami.Theme.textColor : Kirigami.Theme.disabledTextColor
                        font.family: Kirigami.Theme.fixedWidthFont.family
                        font.pointSize: Kirigami.Theme.smallFont.pointSize
                        text: page.troubleshooting !== "" ? page.troubleshooting.trim() : page.error !== "" ? qsTr("Not available.") : qsTr("Reading what Wye detected…")
                        wrapMode: Text.WrapAtWordBoundaryOrAnywhere
                    }
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
