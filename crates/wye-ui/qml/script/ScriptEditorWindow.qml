// The transform script editor (16-script-editor.md, SCR-01 to SCR-09).
// Surface contract (crates/wye-ui/src/route.rs):
//   handle("show", window name, argument)  open, or raise the one window (SET-04)
// The argument is a scope (`global`, `rule:<id>`) or JSON with `scope`,
// `ruleName` and, for the self-test, `fixture`.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import dev.soldunov.wye.ui

Kirigami.ApplicationWindow {
    id: window

    // Debounce before a test run (SCR-04).
    readonly property int runDelay: 300

    function handle(action, key, argument) {
        if (action !== "show") {
            return;
        }
        backend.open(argument);
        show();
        raise();
        requestActivate();
    }

    // The result link as rich text, changed parts marked (SCR-04).
    function resultHtml(segmentsJson: string): string {
        const escape = text => text.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
        let segments = [];
        try {
            segments = JSON.parse(segmentsJson);
        } catch (error) {
            return "";
        }
        return segments.map(segment => segment.changed ? "<b><span style=\"background-color: " + Kirigami.Theme.positiveBackgroundColor + "\">" + escape(segment.text) + "</span></b>" : escape(segment.text)).join("");
    }

    function sourceChoices(json: string): var {
        let apps = [];
        try {
            apps = JSON.parse(json);
        } catch (error) {
            apps = [];
        }
        return [
            {
                "id": "",
                "name": qsTr("None")
            }
        ].concat(apps);
    }

    title: backend.title !== "" ? backend.title : qsTr("Transform Script")
    // SCR-01: about 680 × 520, resizable.
    width: 680
    height: 520
    minimumWidth: Kirigami.Units.gridUnit * 24
    minimumHeight: Kirigami.Units.gridUnit * 20

    ScriptEditorBackend {
        id: backend

        onSourceLoaded: text => editor.load(text)
    }

    Timer {
        id: runLater
        interval: window.runDelay
        onTriggered: backend.runTest()
    }

    Shortcut {
        sequences: [StandardKey.Save]
        onActivated: backend.save()
    }

    ScriptReference {
        id: reference
    }

    pageStack.initialPage: Kirigami.Page {
        title: window.title
        padding: Kirigami.Units.largeSpacing

        actions: [
            Kirigami.Action {
                text: qsTr("Reference")
                icon.name: "help-contents"
                onTriggered: reference.open()
            }
        ]

        ColumnLayout {
            anchors.fill: parent
            spacing: Kirigami.Units.smallSpacing

            // SCR-08: the file changed elsewhere.
            Kirigami.InlineMessage {
                Layout.fillWidth: true
                type: Kirigami.MessageType.Warning
                visible: backend.externalChange
                text: backend.dirty ? qsTr("The script changed on disk. Reloading discards your changes.") : qsTr("The script changed on disk.")
                actions: [
                    Kirigami.Action {
                        text: qsTr("Reload")
                        icon.name: "view-refresh"
                        onTriggered: backend.reload()
                    }
                ]
            }

            Kirigami.InlineMessage {
                Layout.fillWidth: true
                type: Kirigami.MessageType.Error
                visible: backend.error !== ""
                text: backend.error
            }

            ScriptCodeEditor {
                id: editor
                Layout.fillWidth: true
                Layout.fillHeight: true
                backend: backend
                errorLine: backend.errorLine
                onEdited: text => {
                    backend.edit(text);
                    runLater.restart();
                }
            }

            Kirigami.Heading {
                level: 4
                text: qsTr("Test")
            }

            GridLayout {
                Layout.fillWidth: true
                columns: 2
                columnSpacing: Kirigami.Units.largeSpacing

                QQC2.Label {
                    text: qsTr("Link")
                }
                QQC2.TextField {
                    Layout.fillWidth: true
                    text: backend.testUrl
                    Accessible.name: qsTr("Test link")
                    onTextEdited: {
                        backend.testUrl = text;
                        runLater.restart();
                    }
                }

                QQC2.Label {
                    text: qsTr("Source app")
                }
                QQC2.ComboBox {
                    Layout.fillWidth: true
                    model: window.sourceChoices(backend.sourcesJson)
                    textRole: "name"
                    valueRole: "id"
                    Accessible.name: qsTr("Source app")
                    onActivated: {
                        backend.sourceApp = currentValue;
                        runLater.restart();
                    }
                }

                QQC2.Label {
                    Layout.alignment: Qt.AlignTop
                    text: qsTr("Result")
                }
                RowLayout {
                    Layout.fillWidth: true

                    QQC2.Label {
                        Layout.fillWidth: true
                        wrapMode: Text.WrapAnywhere
                        textFormat: backend.status === "changed" ? Text.StyledText : Text.PlainText
                        color: backend.status === "failed" ? Kirigami.Theme.negativeTextColor : Kirigami.Theme.textColor
                        text: {
                            switch (backend.status) {
                            case "changed":
                                return window.resultHtml(backend.segmentsJson);
                            case "unchanged":
                                return qsTr("Unchanged");
                            case "failed":
                                return backend.errorLine > 0 ? qsTr("Line %1: %2").arg(backend.errorLine).arg(backend.resultMessage) : backend.resultMessage;
                            default:
                                return "";
                            }
                        }
                    }
                    Kirigami.Icon {
                        visible: backend.status !== "idle"
                        implicitWidth: Kirigami.Units.iconSizes.small
                        implicitHeight: Kirigami.Units.iconSizes.small
                        source: backend.status === "failed" ? "dialog-error" : "dialog-ok"
                    }
                    QQC2.Label {
                        visible: backend.resultTime !== ""
                        text: backend.resultTime
                        color: Kirigami.Theme.disabledTextColor
                    }
                }

                QQC2.Label {
                    Layout.alignment: Qt.AlignTop
                    visible: backend.logs !== ""
                    text: qsTr("Log")
                }
                QQC2.Label {
                    Layout.fillWidth: true
                    visible: backend.logs !== ""
                    wrapMode: Text.WrapAnywhere
                    maximumLineCount: 6
                    elide: Text.ElideRight
                    font: Kirigami.Theme.fixedWidthFont
                    text: backend.logs
                }
            }
        }

        footer: QQC2.ToolBar {
            contentItem: RowLayout {
                QQC2.Button {
                    text: qsTr("Revert")
                    icon.name: "document-revert"
                    enabled: backend.dirty && !backend.busy
                    onClicked: backend.reload()
                }
                QQC2.Label {
                    Layout.fillWidth: true
                    visible: backend.syntaxError
                    horizontalAlignment: Text.AlignRight
                    color: Kirigami.Theme.negativeTextColor
                    text: qsTr("Fix the syntax error to save.")
                    elide: Text.ElideRight
                }
                Item {
                    Layout.fillWidth: true
                    visible: !backend.syntaxError
                }
                QQC2.Button {
                    text: qsTr("Cancel")
                    icon.name: "dialog-cancel"
                    onClicked: window.close()
                }
                QQC2.Button {
                    text: qsTr("Save")
                    icon.name: "document-save"
                    enabled: backend.canSave
                    onClicked: backend.save()
                }
            }
        }
    }
}
