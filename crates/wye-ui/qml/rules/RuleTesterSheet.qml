// RuleTesterSheet (DLG-TST-01 to DLG-TST-03): "Test Rules". A link, an optional source app and held keys go through the
// pipeline with TestLink (nothing opens; the footer's "Skip network" leaves short links unexpanded); the steps that
// changed the link or decided the target are listed, ending with the target and its options. The result updates as the
// inputs change; the matched rule opens in the rule editor.
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
    // The inputs' trailing controls share one width, so the entry and the popup line up.
    readonly property real fieldWidth: Kirigami.Units.gridUnit * 16
    readonly property var view: tester.viewJson !== "" ? JSON.parse(tester.viewJson) : null
    readonly property string decision: sheet.view?.decision ?? ""
    readonly property int ruleIndex: sheet.view?.ruleIndex ?? -1
    // The matched rule's name, for the row that opens it (DLG-TST-03).
    readonly property string ruleName: {
        if (sheet.ruleIndex < 0 || SettingsBackend.configJson === "") {
            return "";
        }
        const rules = JSON.parse(SettingsBackend.configJson).rules ?? [];
        return rules[sheet.ruleIndex]?.name ?? "";
    }
    readonly property string outcomeLabel: sheet.decision === "rejected" ? qsTr("Refused") : sheet.decision === "picker" ? qsTr("Shows") : qsTr("Opens in")
    // The step labels' column: as wide as the widest label, so every step's text starts at the same place.
    readonly property real labelWidth: {
        const labels = (sheet.view?.steps ?? []).map(step => step.label).concat([sheet.outcomeLabel]);
        return Math.ceil(Math.max(Kirigami.Units.gridUnit * 4, ...labels.map(label => labelMetrics.advanceWidth(label))));
    }
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
        // A long link shows from its start, not scrolled to the cursor at its end.
        urlField.cursorPosition = 0;
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
    sheetWidth: Kirigami.Units.gridUnit * 32
    primaryText: qsTr("Done")

    onPrimaryTriggered: close()

    // DLG-TST-02: the "Skip network" switch sits in the footer, left of Done, and explains itself in its tooltip. As a row
    // of the inputs card it pushed the steps, the point of the sheet, below the fold at the window's default size.
    footerLeading: QQC2.Switch {
        checked: sheet.skipNetwork
        text: qsTr("Skip network")
        QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
        QQC2.ToolTip.text: qsTr("Do not ask short-link services where a link leads.")
        QQC2.ToolTip.visible: hovered
        onToggled: {
            sheet.skipNetwork = checked;
            checked = Qt.binding(() => sheet.skipNetwork);
            runLater.restart();
        }
    }

    TesterBackend {
        id: tester
    }

    Timer {
        id: runLater

        interval: sheet.runDelay
        onTriggered: sheet.run()
    }

    FontMetrics {
        id: labelMetrics
    }

    // DLG-TST-01: the inputs.
    WyeGroupCard {
        Layout.fillWidth: true
        Layout.topMargin: Kirigami.Units.largeSpacing

        WyeRow {
            title: qsTr("Link")
            needsConfig: false

            QQC2.TextField {
                id: urlField

                Layout.preferredWidth: sheet.fieldWidth
                placeholderText: qsTr("Paste or type a link")
                inputMethodHints: Qt.ImhUrlCharactersOnly | Qt.ImhNoAutoUppercase
                Accessible.name: qsTr("Link")
                onTextEdited: runLater.restart()
                onAccepted: {
                    runLater.stop();
                    sheet.run();
                }
            }
        }

        WyeRow {
            title: qsTr("Source app")
            needsConfig: false

            QQC2.ComboBox {
                Layout.preferredWidth: sheet.fieldWidth
                model: sheet.apps
                textRole: "name"
                valueRole: "id"
                // Scrolling the sheet over the box must not change the source app.
                wheelEnabled: false
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
    }

    Kirigami.InlineMessage {
        Layout.fillWidth: true
        Layout.leftMargin: Kirigami.Units.largeSpacing
        Layout.rightMargin: Kirigami.Units.largeSpacing
        type: Kirigami.MessageType.Error
        visible: tester.error !== ""
        text: tester.error
    }

    // DLG-TST-02: the steps, ending with the outcome.
    WyeGroupCard {
        Layout.fillWidth: true
        title: qsTr("Steps")

        // Nothing to show yet: no link, or the service is still working on it. In a plain item that takes the card's
        // width: laid out by the card itself, the wrapped text and the card's width never settle.
        Item {
            Layout.fillWidth: true
            visible: sheet.view === null
            implicitHeight: (tester.busy ? busy.implicitHeight : waiting.implicitHeight) + Kirigami.Units.gridUnit * 2

            QQC2.BusyIndicator {
                id: busy

                anchors.centerIn: parent
                visible: tester.busy
                running: tester.busy
            }
            QQC2.Label {
                id: waiting

                anchors {
                    left: parent.left
                    right: parent.right
                    verticalCenter: parent.verticalCenter
                    margins: Kirigami.Units.gridUnit
                }
                visible: !tester.busy
                color: Kirigami.Theme.disabledTextColor
                horizontalAlignment: Text.AlignHCenter
                text: urlField.text === "" ? qsTr("Enter a link to see which rule it matches and where it opens.") : qsTr("No result yet.")
                wrapMode: Text.Wrap
            }
        }

        Repeater {
            model: sheet.view?.steps ?? []

            delegate: TesterRow {
                id: stepRow

                required property var modelData

                label: stepRow.modelData.label
                labelWidth: sheet.labelWidth

                QQC2.Label {
                    Layout.fillWidth: true
                    text: stepRow.modelData.text
                    wrapMode: Text.Wrap
                }
            }
        }

        // The outcome: where the link opens, the picker, or why it was refused.
        TesterRow {
            id: outcome

            readonly property var shown: sheet.view !== null && sheet.view.target !== null && SettingsBackend.generation >= 0 ? JSON.parse(SettingsBackend.targetLabel("rule", JSON.stringify(sheet.view.target), "")) : ({
                    "label": sheet.view?.targetName ?? "",
                    "icon": ""
                })

            visible: sheet.view !== null
            label: sheet.outcomeLabel
            labelWidth: sheet.labelWidth
            labelEmphasis: true

            Kirigami.Icon {
                Layout.alignment: Qt.AlignTop
                implicitHeight: Kirigami.Units.iconSizes.medium
                implicitWidth: Kirigami.Units.iconSizes.medium
                visible: sheet.decision !== "rejected"
                // The Picker's glyph, as its row in every target menu draws it (TGT-03).
                source: sheet.decision === "picker" ? "view-list-text" : outcome.shown.icon !== "" ? outcome.shown.icon : "application-x-executable"
            }
            Kirigami.Icon {
                Layout.alignment: Qt.AlignTop
                implicitHeight: Kirigami.Units.iconSizes.smallMedium
                implicitWidth: Kirigami.Units.iconSizes.smallMedium
                visible: sheet.decision === "rejected"
                // Breeze's own error glyph, already in the negative colour; it has no symbolic variant to tint.
                source: "dialog-error"
            }
            ColumnLayout {
                Layout.fillWidth: true
                spacing: 0

                QQC2.Label {
                    Layout.fillWidth: true
                    font.weight: Font.DemiBold
                    color: sheet.decision === "rejected" ? Kirigami.Theme.negativeTextColor : Kirigami.Theme.textColor
                    text: sheet.decision === "rejected" ? (sheet.view?.rejected ?? "") : sheet.decision === "picker" ? qsTr("the picker") : (sheet.view?.targetName || outcome.shown.label)
                    wrapMode: Text.Wrap
                }
                QQC2.Label {
                    Layout.fillWidth: true
                    visible: text !== ""
                    color: Kirigami.Theme.disabledTextColor
                    font: Kirigami.Theme.smallFont
                    text: sheet.view?.options ?? ""
                    wrapMode: Text.Wrap
                }
                QQC2.Label {
                    Layout.fillWidth: true
                    visible: text !== ""
                    color: Kirigami.Theme.disabledTextColor
                    font: Kirigami.Theme.smallFont
                    text: sheet.view?.finalUrl ?? ""
                    wrapMode: Text.WrapAtWordBoundaryOrAnywhere
                }
            }
        }

        // DLG-TST-03
        WyeButtonRow {
            visible: sheet.ruleIndex >= 0
            title: qsTr("Matched rule")
            subtitle: sheet.ruleName !== "" ? "“" + sheet.ruleName.replace(/&/g, "&amp;").replace(/</g, "&lt;") + "”" : ""
            buttonText: qsTr("Edit Rule…")
            buttonIcon: "document-edit-symbolic"
            needsConfig: false
            onActivated: sheet.ruleRequested(sheet.ruleIndex)
        }
    }

    // The same room below the last card as above the first.
    Item {
        implicitHeight: Kirigami.Units.largeSpacing
    }
}
