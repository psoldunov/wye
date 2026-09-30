// The URL expansion sheet (17-dialogs.md, DLG-EXP-01 to DLG-EXP-05), opened by "Configure…" on the Advanced page: which redirect
// wrappers Wye unwraps on this computer, which short-link services it asks for the target of a link (and a "+" to add a
// domain of your own), and the limits: timeout, maximum redirects, and whether to notify when a link cannot be expanded.
// Changes apply at once; Done closes the sheet. The list is the service's (GetExpansionCatalogue); only what differs from it
// is stored (advanced.expansion in the configuration).
pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import dev.soldunov.wye.ui

WyeSheet {
    id: sheet

    // The rows follow the catalogue and the configuration (DLG-EXP-01, DLG-EXP-02).
    readonly property var rows: SettingsBackend.generation >= 0 && SettingsBackend.expansionLoaded ? JSON.parse(SettingsBackend.expansionRows()) : []
    readonly property var wrappers: rows.filter(row => row.kind === "wrapper")
    readonly property var shortLinks: rows.filter(row => row.kind === "short-link")
    readonly property var config: SettingsBackend.configJson === "" ? ({}) : JSON.parse(SettingsBackend.configJson)
    readonly property var expansion: config.advanced?.expansion ?? ({})

    // The entry that adds a domain (DLG-EXP-02).
    property bool adding: false
    property string addError

    function addDomain() {
        const problem = SettingsBackend.expansionAddDomain(domainField.text);
        addError = problem;
        if (problem === "") {
            domainField.text = "";
            adding = false;
        }
    }

    primaryText: qsTr("Done")
    sheetWidth: Kirigami.Units.gridUnit * 24
    title: qsTr("URL Expansion")

    onAboutToShow: {
        adding = false;
        addError = "";
        SettingsBackend.loadExpansion();
    }
    onPrimaryTriggered: close()

    WyeCallout {
        calloutId: "expansion-info"
        closeLeading: true
        text: qsTr("Redirect wrappers are unwrapped on your computer. Short links need one request to the short-link service to find where they lead.")
    }

    // DLG-EXP-01
    WyeGroupCard {
        title: qsTr("Redirect Wrappers")

        Repeater {
            model: sheet.wrappers

            WyeSwitchRow {
                required property var modelData

                isOn: modelData.enabled
                subtitle: modelData.detail !== "" ? "<code>" + modelData.detail + "</code>" : ""
                title: modelData.label

                onSwitched: on => SettingsBackend.expansionToggle(modelData.id, on)
            }
        }
    }

    // DLG-EXP-02
    WyeSection {
        addEnabled: SettingsBackend.writable
        addText: qsTr("Add Domain")
        count: sheet.shortLinks.length + (sheet.adding ? 1 : 0)
        emptyText: qsTr("No short-link domains")
        title: qsTr("Short Links")

        onAddTriggered: {
            sheet.adding = true;
            domainField.forceActiveFocus();
        }

        RowLayout {
            Layout.fillWidth: true
            Layout.margins: Kirigami.Units.largeSpacing
            visible: sheet.adding

            ColumnLayout {
                Layout.fillWidth: true

                QQC2.TextField {
                    id: domainField

                    Layout.fillWidth: true
                    placeholderText: qsTr("example.link")

                    onAccepted: sheet.addDomain()
                    onTextChanged: sheet.addError = ""
                }

                QQC2.Label {
                    Layout.fillWidth: true
                    color: Kirigami.Theme.negativeTextColor
                    text: sheet.addError
                    visible: sheet.addError !== ""
                    wrapMode: Text.WordWrap
                }
            }

            QQC2.Button {
                Layout.alignment: Qt.AlignTop
                text: qsTr("Add")

                onClicked: sheet.addDomain()
            }

            QQC2.Button {
                Layout.alignment: Qt.AlignTop
                text: qsTr("Cancel")

                onClicked: {
                    sheet.adding = false;
                    sheet.addError = "";
                }
            }
        }

        Repeater {
            model: sheet.shortLinks

            WyeSwitchRow {
                id: link

                required property var modelData

                isOn: modelData.enabled
                title: modelData.label

                onSwitched: on => SettingsBackend.expansionToggle(modelData.id, on)

                // DLG-EXP-02: a domain you added can be removed again.
                QQC2.ToolButton {
                    display: QQC2.AbstractButton.IconOnly
                    icon.name: "edit-delete"
                    text: qsTr("Remove")
                    visible: link.modelData.removable

                    onClicked: SettingsBackend.expansionRemoveDomain(link.modelData.id)
                }
            }
        }
    }

    // DLG-EXP-04
    WyeGroupCard {
        title: qsTr("Behaviour")

        WyeRow {
            title: qsTr("Timeout")

            QQC2.SpinBox {
                id: timeout

                readonly property int units: Math.round((sheet.expansion["timeout-ms"] ?? 1500) / 500)

                Accessible.name: qsTr("Timeout")
                from: 1
                to: 10
                textFromValue: value => qsTr("%1 s").arg((value * 0.5).toFixed(1))
                value: units

                onValueModified: {
                    SettingsBackend.setValue("advanced.expansion.timeout-ms", JSON.stringify(timeout.value * 500));
                    timeout.value = Qt.binding(() => timeout.units);
                }
            }
        }

        WyeRow {
            title: qsTr("Maximum redirects")

            QQC2.SpinBox {
                id: redirects

                readonly property int current: sheet.expansion["max-redirects"] ?? 5

                Accessible.name: qsTr("Maximum redirects")
                from: 1
                to: 10
                value: current

                onValueModified: {
                    SettingsBackend.setValue("advanced.expansion.max-redirects", JSON.stringify(redirects.value));
                    redirects.value = Qt.binding(() => redirects.current);
                }
            }
        }

        WyeSwitchRow {
            isOn: sheet.expansion["notify-on-failure"] ?? false
            path: "advanced.expansion.notify-on-failure"
            title: qsTr("Notify when a link cannot be expanded")
        }
    }
}
