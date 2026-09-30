// RuleTesterSheet (DLG-TST-01 to DLG-TST-03): "Test Rules". A link, an optional source app and held keys go through the
// pipeline with TestLink (nothing opens); the steps that changed the link or decided the target are listed, ending with
// the target and its options. The result updates as the inputs change; the matched rule opens in the rule editor.
//
// API
//   openWith(string url, string traceJson)   open with this link; a non-empty trace (the self-test) is shown instead of
//                                            asking the service
//   ruleRequested(int index)                 the matched rule was clicked (DLG-TST-03)
pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import dev.soldunov.wye.ui

WyeSheet {
    id: sheet

    property string sourceApp
    property var held: []
    property bool skipNetwork: false
    // Debounce before a run (DLG-TST-01).
    readonly property int runDelay: 300
    readonly property var view: tester.viewJson !== "" ? JSON.parse(tester.viewJson) : null
    readonly property var apps: {
        const text = SettingsBackend.offline ? SettingsBackend.fixtureAppsJson : RulesBackend.appsJson;
        try {
            return [
                {
                    "id": "",
                    "name": qsTr("None")
                }
            ].concat(JSON.parse(text).apps ?? []);
        } catch (error) {
            return [
                {
                    "id": "",
                    "name": qsTr("None")
                }
            ];
        }
    }

    signal ruleRequested(int index)

    function openWith(url: string, traceJson: string) {
        urlField.text = url;
        open();
        urlField.forceActiveFocus();
        if (traceJson !== "") {
            tester.loadTrace(traceJson);
        } else {
            run();
        }
    }

    function run() {
        tester.test(urlField.text, sheet.sourceApp, JSON.stringify(sheet.held), sheet.skipNetwork);
    }

    title: qsTr("Test Rules")
    sheetWidth: Kirigami.Units.gridUnit * 26
    primaryText: qsTr("Done")

    onPrimaryTriggered: close()

    TesterBackend {
        id: tester
    }

    Timer {
        id: runLater

        interval: sheet.runDelay
        onTriggered: sheet.run()
    }

    WyeGroupCard {
        Layout.fillWidth: true

        WyeRow {
            title: qsTr("Link")
            needsConfig: false

            QQC2.TextField {
                id: urlField

                Layout.preferredWidth: Kirigami.Units.gridUnit * 15
                placeholderText: "https://bit.ly/abc123"
                Accessible.name: qsTr("Link")
                onTextEdited: runLater.restart()
            }
        }

        WyeRow {
            title: qsTr("Source app")
            needsConfig: false

            QQC2.ComboBox {
                Layout.preferredWidth: Kirigami.Units.gridUnit * 12
                model: sheet.apps
                textRole: "name"
                valueRole: "id"
                Accessible.name: qsTr("Source app")
                onActivated: {
                    sheet.sourceApp = currentValue;
                    runLater.restart();
                }
            }
        }

        WyeModifierRow {
            title: qsTr("Held keys")
            needsConfig: false
            modifiers: sheet.held
            onModified: names => {
                sheet.held = names;
                runLater.restart();
            }
        }

        WyeSwitchRow {
            title: qsTr("Skip network")
            subtitle: qsTr("Do not ask short-link services where a link leads.")
            needsConfig: false
            isOn: sheet.skipNetwork
            onSwitched: on => {
                sheet.skipNetwork = on;
                runLater.restart();
            }
        }
    }

    Kirigami.Heading {
        Layout.fillWidth: true
        level: 4
        text: qsTr("Steps")
    }

    QQC2.Label {
        Layout.fillWidth: true
        visible: tester.error !== ""
        color: Kirigami.Theme.negativeTextColor
        text: tester.error
        wrapMode: Text.Wrap
    }

    QQC2.BusyIndicator {
        Layout.alignment: Qt.AlignHCenter
        visible: tester.busy
        running: tester.busy
    }

    WyeGroupCard {
        Layout.fillWidth: true
        visible: sheet.view !== null

        Repeater {
            model: sheet.view?.steps ?? []

            delegate: RowLayout {
                id: stepRow

                required property var modelData

                Layout.fillWidth: true
                Layout.margins: Kirigami.Units.smallSpacing

                QQC2.Label {
                    Layout.preferredWidth: Kirigami.Units.gridUnit * 6
                    Layout.alignment: Qt.AlignTop
                    color: Kirigami.Theme.disabledTextColor
                    text: stepRow.modelData.label
                }
                QQC2.Label {
                    Layout.fillWidth: true
                    text: stepRow.modelData.text
                    wrapMode: Text.WrapAnywhere
                }
            }
        }

        // The outcome: opens in, the picker, or refused.
        RowLayout {
            id: outcome

            readonly property var shown: sheet.view !== null && sheet.view.target !== null && SettingsBackend.generation >= 0 ? JSON.parse(SettingsBackend.targetLabel("rule", JSON.stringify(sheet.view.target), "")) : ({
                    "label": sheet.view?.targetName ?? "",
                    "icon": ""
                })

            Layout.fillWidth: true
            Layout.margins: Kirigami.Units.smallSpacing

            QQC2.Label {
                Layout.preferredWidth: Kirigami.Units.gridUnit * 6
                Layout.alignment: Qt.AlignTop
                color: Kirigami.Theme.disabledTextColor
                text: sheet.view?.decision === "rejected" ? qsTr("Refused") : sheet.view?.decision === "picker" ? qsTr("Shows") : qsTr("Opens in")
            }
            Kirigami.Icon {
                Layout.preferredHeight: Kirigami.Units.iconSizes.small
                Layout.preferredWidth: Kirigami.Units.iconSizes.small
                visible: sheet.view?.decision === "open"
                source: outcome.shown.icon !== "" ? outcome.shown.icon : "application-x-executable"
            }
            ColumnLayout {
                Layout.fillWidth: true
                spacing: 0

                QQC2.Label {
                    Layout.fillWidth: true
                    wrapMode: Text.Wrap
                    text: sheet.view?.decision === "rejected" ? (sheet.view?.rejected ?? "") : sheet.view?.decision === "picker" ? qsTr("the picker") : (sheet.view?.targetName || outcome.shown.label)
                }
                QQC2.Label {
                    Layout.fillWidth: true
                    visible: text !== ""
                    color: Kirigami.Theme.disabledTextColor
                    font: Kirigami.Theme.smallFont
                    text: sheet.view?.options ?? ""
                }
                QQC2.Label {
                    Layout.fillWidth: true
                    visible: text !== ""
                    color: Kirigami.Theme.disabledTextColor
                    font: Kirigami.Theme.smallFont
                    text: sheet.view?.finalUrl ?? ""
                    wrapMode: Text.WrapAnywhere
                }
            }
        }

        // DLG-TST-03
        WyeButtonRow {
            visible: (sheet.view?.ruleIndex ?? -1) >= 0
            title: qsTr("Matched rule")
            buttonText: qsTr("Edit Rule…")
            needsConfig: false
            onActivated: sheet.ruleRequested(sheet.view.ruleIndex)
        }
    }
}
