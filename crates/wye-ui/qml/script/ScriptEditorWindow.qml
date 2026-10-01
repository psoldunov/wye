// The transform script editor (16-script-editor.md, SCR-01 to SCR-09).
// Surface contract (crates/wye-ui/src/route.rs):
//   handle("show", window name, argument)  open, or raise the one window (SET-04)
// The argument is a scope (`global`, `rule:<id>`) or JSON with `scope`,
// `ruleName` and, for the self-test, `fixture` (its `reference` opens the
// Reference too).
//
// Closing with unsaved changes (Cancel, the window's close button) asks first: Save, Discard or Cancel (SCR-10).
// Quitting Wye closes without asking.
//
// Layout: a tool bar over the code (Undo, Redo, and the Reference, SCR-06),
// the code area, the Test group (SCR-04) as a form with right-aligned
// labels, and the dialog buttons in KDE's order: Revert at the left, Save
// (the default button) then Cancel at the right. The page has no title bar
// of its own: the window's title already names the script (SCR-01).
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import dev.soldunov.wye.ui

Kirigami.ApplicationWindow {
    id: window

    // Debounce before a test run (SCR-04).
    readonly property int runDelay: 300
    // SCR-10: an answer was given, so the close that follows is not asked about again.
    property bool closeConfirmed: false
    // SCR-10: Save was chosen in the question; the window closes once the save has gone through.
    property bool closeAfterSave: false
    // SCR-10: Wye is quitting (Main.qml `quitApp`). Nothing can be saved, and a window that refuses to close would
    // abort the quit, so no question is asked.
    property bool quitting: false

    function handle(action, key, argument) {
        if (action !== "show") {
            return;
        }
        closeConfirmed = false;
        closeAfterSave = false;
        backend.open(argument);
        show();
        raise();
        requestActivate();
        const fixture = window.fixtureOf(argument);
        if (fixture.edit !== undefined) {
            editor.type(fixture.edit);
        }
        if (fixture.reference === true) {
            reference.open();
        }
        if (fixture.confirmClose === true) {
            close();
        }
    }

    // The self-test's `fixture` (crates/wye-ui/src/script_editor/opening.rs): `edit`, `reference` and `confirmClose` are the window's.
    function fixtureOf(argument: string): var {
        if (!argument.startsWith("{")) {
            return {};
        }
        try {
            return JSON.parse(argument).fixture ?? {};
        } catch (error) {
            return {};
        }
    }

    // SCR-10: called after a save's state settled: close if it went through, stay open if it failed.
    function finishSave() {
        if (!closeAfterSave || backend.busy) {
            return;
        }
        closeAfterSave = false;
        if (!backend.dirty) {
            closeConfirmed = true;
            close();
        }
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
        const mark = "<span style=\"background-color: " + Kirigami.Theme.positiveBackgroundColor + "; color: " + Kirigami.Theme.positiveTextColor + "; font-weight: bold\">";
        return segments.map(segment => segment.changed ? mark + escape(segment.text) + "</span>" : escape(segment.text)).join("");
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

    // SCR-10: Cancel and the close button both end here; unsaved changes are asked about first. Quitting Wye does not ask.
    onClosing: close => {
        if (!closeConfirmed && !quitting && backend.dirty) {
            close.accepted = false;
            unsaved.open();
        }
    }

    ScriptEditorBackend {
        id: backend

        onSourceLoaded: text => editor.load(text)
        // Save answers later (busy, then dirty), so look once the change is done.
        onBusyChanged: Qt.callLater(window.finishSave)
        onDirtyChanged: Qt.callLater(window.finishSave)
    }

    ScriptUnsavedDialog {
        id: unsaved

        canSave: backend.canSave
        scriptName: window.title
        onDiscardRequested: {
            window.closeConfirmed = true;
            window.close();
        }
        onSaveRequested: {
            window.closeAfterSave = true;
            backend.save();
        }
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

    Shortcut {
        sequence: "F1"
        onActivated: reference.open()
    }

    ScriptReference {
        id: reference
    }

    pageStack.initialPage: Kirigami.Page {
        title: window.title
        globalToolBarStyle: Kirigami.ApplicationHeaderStyle.None
        padding: Kirigami.Units.largeSpacing
        topPadding: Kirigami.Units.smallSpacing

        header: QQC2.ToolBar {
            contentItem: RowLayout {
                spacing: Kirigami.Units.smallSpacing

                QQC2.ToolButton {
                    display: QQC2.AbstractButton.IconOnly
                    enabled: editor.canUndo
                    icon.name: "edit-undo-symbolic"
                    text: qsTr("Undo")
                    QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
                    QQC2.ToolTip.text: qsTr("Undo (Ctrl+Z)")
                    QQC2.ToolTip.visible: hovered
                    onClicked: editor.undo()
                }

                QQC2.ToolButton {
                    display: QQC2.AbstractButton.IconOnly
                    enabled: editor.canRedo
                    icon.name: "edit-redo-symbolic"
                    text: qsTr("Redo")
                    QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
                    QQC2.ToolTip.text: qsTr("Redo (Ctrl+Shift+Z)")
                    QQC2.ToolTip.visible: hovered
                    onClicked: editor.redo()
                }

                Item {
                    Layout.fillWidth: true
                }

                // SCR-06
                QQC2.ToolButton {
                    display: QQC2.AbstractButton.TextBesideIcon
                    icon.name: "help-contents-symbolic"
                    text: qsTr("Reference")
                    QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
                    QQC2.ToolTip.text: qsTr("The script API and examples (F1)")
                    QQC2.ToolTip.visible: hovered
                    onClicked: reference.open()
                }
            }
        }

        ColumnLayout {
            anchors.fill: parent
            spacing: Kirigami.Units.largeSpacing

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

            // SCR-04
            ColumnLayout {
                Layout.fillWidth: true
                spacing: Kirigami.Units.smallSpacing

                Kirigami.Heading {
                    level: 5
                    font.weight: Font.DemiBold
                    text: qsTr("Test")
                }

                GridLayout {
                    Layout.fillWidth: true
                    columns: 2
                    columnSpacing: Kirigami.Units.largeSpacing
                    rowSpacing: Kirigami.Units.smallSpacing

                    QQC2.Label {
                        Layout.alignment: Qt.AlignRight | Qt.AlignVCenter
                        text: qsTr("Link:")
                    }
                    QQC2.TextField {
                        Layout.fillWidth: true
                        text: backend.testUrl
                        placeholderText: "https://example.com/"
                        Accessible.name: qsTr("Test link")
                        onTextEdited: {
                            backend.testUrl = text;
                            runLater.restart();
                        }
                    }

                    QQC2.Label {
                        Layout.alignment: Qt.AlignRight | Qt.AlignVCenter
                        text: qsTr("Source app:")
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
                        Layout.alignment: Qt.AlignRight | Qt.AlignTop
                        text: qsTr("Result:")
                    }
                    RowLayout {
                        Layout.fillWidth: true
                        spacing: Kirigami.Units.smallSpacing

                        Kirigami.Icon {
                            Layout.alignment: Qt.AlignTop
                            implicitWidth: Kirigami.Units.iconSizes.small
                            implicitHeight: Kirigami.Units.iconSizes.small
                            visible: backend.status !== "idle" && backend.status !== ""
                            source: backend.status === "failed" ? "dialog-error" : "dialog-ok"
                        }
                        QQC2.Label {
                            Layout.fillWidth: true
                            Accessible.name: qsTr("Result")
                            wrapMode: Text.WrapAnywhere
                            textFormat: backend.status === "changed" ? Text.RichText : Text.PlainText
                            color: backend.status === "failed" ? Kirigami.Theme.negativeTextColor : backend.status === "unchanged" ? Kirigami.Theme.disabledTextColor : Kirigami.Theme.textColor
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
                        QQC2.Label {
                            Layout.alignment: Qt.AlignTop
                            visible: backend.resultTime !== ""
                            text: backend.resultTime
                            color: Kirigami.Theme.disabledTextColor
                            font: Kirigami.Theme.smallFont
                            Accessible.name: qsTr("Run time: %1").arg(backend.resultTime)
                        }
                    }

                    QQC2.Label {
                        Layout.alignment: Qt.AlignRight | Qt.AlignTop
                        visible: backend.logs !== ""
                        text: qsTr("Log:")
                    }
                    QQC2.Label {
                        Layout.fillWidth: true
                        visible: backend.logs !== ""
                        wrapMode: Text.WrapAnywhere
                        maximumLineCount: 6
                        elide: Text.ElideRight
                        font: Kirigami.Theme.fixedWidthFont
                        color: Kirigami.Theme.disabledTextColor
                        text: backend.logs
                        Accessible.name: qsTr("Log")
                    }
                }
            }
        }

        // KDE's button order: Revert (a reset) at the left; Save, the default button, then Cancel at the right.
        footer: QQC2.ToolBar {
            position: QQC2.ToolBar.Footer

            contentItem: RowLayout {
                spacing: Kirigami.Units.smallSpacing

                QQC2.Button {
                    text: qsTr("Revert")
                    icon.name: "document-revert"
                    enabled: backend.dirty && !backend.busy
                    QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
                    QQC2.ToolTip.text: qsTr("Discard the changes and read the saved script again")
                    QQC2.ToolTip.visible: hovered
                    onClicked: backend.reload()
                }

                // SCR-07: why Save is off. The error itself, with its icon, is in the Result row above.
                QQC2.Label {
                    Layout.fillWidth: true
                    Layout.leftMargin: Kirigami.Units.largeSpacing
                    horizontalAlignment: Text.AlignRight
                    color: Kirigami.Theme.negativeTextColor
                    text: backend.syntaxError ? qsTr("Fix the syntax error to save.") : ""
                    elide: Text.ElideRight
                }

                QQC2.Button {
                    Accessible.defaultButton: true
                    text: qsTr("Save")
                    icon.name: "document-save"
                    enabled: backend.canSave
                    highlighted: enabled
                    onClicked: backend.save()
                }
                QQC2.Button {
                    text: qsTr("Cancel")
                    icon.name: "dialog-cancel"
                    onClicked: window.close()
                }
            }
        }
    }
}
