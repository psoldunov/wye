// The Rules page (08-rules.md, RUL-01 to RUL-07): the rules in order with their summaries, targets and switches, dragging
// to reorder, the list toolbar (add; Test Rules…, Import Rules…, Export Rules…, Delete All Rules…), the rule editor and the
// rule tester. Every change is a merge patch from RulesBackend saved with SettingsBackend.applyPatch (SET-06).
// ShowWindow("rule-editor" | "test-rules") reaches the page through RulesBackend.request (SettingsWindow.qml).
pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Dialogs as Dialogs
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import dev.soldunov.wye.ui

WyePage {
    id: page

    // How long the undo toast stays (RUL-06).
    readonly property int undoTime: 8000
    readonly property string appsJson: SettingsBackend.offline ? SettingsBackend.fixtureAppsJson : RulesBackend.appsJson
    readonly property var rows: SettingsBackend.generation >= 0 ? JSON.parse(RulesBackend.rows(SettingsBackend.configJson, page.appsJson)) : []

    function save(patch: string) {
        if (patch !== "") {
            SettingsBackend.applyPatch(patch);
        }
    }

    function parse(text: string): var {
        try {
            return JSON.parse(text);
        } catch (error) {
            return {};
        }
    }

    // A ShowWindow request for the editor (PICK-31, `index` from the tester) or the tester (`url`, and `trace` in the
    // self-test).
    function handleRequest() {
        const text = RulesBackend.takeRequest();
        if (text === "") {
            return;
        }
        const request = parse(text);
        const argument = parse(request.argument ?? "");
        if (request.key === "rule-editor") {
            if (argument.index !== undefined) {
                editor.openRule(argument.index);
            } else {
                editor.openNew(request.argument ?? "");
            }
        } else if (request.key === "test-rules") {
            tester.openWith(argument.url ?? "", argument.trace !== undefined ? JSON.stringify(argument.trace) : "");
        }
    }

    title: qsTr("Rules")

    Component.onCompleted: {
        if (!SettingsBackend.offline) {
            RulesBackend.loadApps();
        }
        page.handleRequest();
    }

    Connections {
        function onRequested() {
            page.handleRequest();
        }

        function onImported(count) {
            SettingsBackend.refresh();
        }

        target: RulesBackend
    }

    Connections {
        function onSheetRequested(name) {
            if (name === "rule-editor") {
                editor.openNew("");
            } else if (name === "tester") {
                tester.openWith("", "");
            }
        }

        target: SettingsBackend
    }

    Timer {
        id: undoTimer

        interval: page.undoTime
        running: RulesBackend.canUndo
        onTriggered: RulesBackend.dropUndo()
    }

    // RUL-06: the undo toast.
    Kirigami.InlineMessage {
        Layout.fillWidth: true
        Layout.leftMargin: Kirigami.Units.largeSpacing
        Layout.rightMargin: Kirigami.Units.largeSpacing
        type: Kirigami.MessageType.Information
        visible: RulesBackend.canUndo
        text: RulesBackend.notice
        actions: [
            Kirigami.Action {
                icon.name: "edit-undo"
                text: qsTr("Undo")
                onTriggered: page.save(RulesBackend.undoPatch(SettingsBackend.configJson))
            }
        ]
    }

    Kirigami.InlineMessage {
        Layout.fillWidth: true
        Layout.leftMargin: Kirigami.Units.largeSpacing
        Layout.rightMargin: Kirigami.Units.largeSpacing
        type: Kirigami.MessageType.Error
        visible: RulesBackend.error !== ""
        text: RulesBackend.error
        actions: [
            Kirigami.Action {
                text: qsTr("Dismiss")
                onTriggered: RulesBackend.error = ""
            }
        ]
    }

    WyeGroupCard {
        // RUL-01
        Item {
            Layout.fillWidth: true
            implicitHeight: Kirigami.Units.gridUnit * 9
            visible: page.rows.length === 0

            WyeEmptyState {
                title: qsTr("No Rules")
                explanation: qsTr("A rule lets you open a specific app based on the URL and source app")
                hint: qsTr("Click a rule to edit it. Drag to reorder.")
            }
        }

        // RUL-03 to RUL-07
        RuleList {
            Layout.fillWidth: true
            visible: page.rows.length > 0
            rows: page.rows
            editable: page.editable

            onEditRequested: index => editor.openRule(index)
            onToggled: (index, on) => page.save(RulesBackend.togglePatch(SettingsBackend.configJson, index, on))
            onDeleteRequested: index => page.save(RulesBackend.deletePatch(SettingsBackend.configJson, index))
            onDuplicateRequested: index => page.save(RulesBackend.duplicatePatch(SettingsBackend.configJson, index))
            onMoved: (from, to) => page.save(RulesBackend.movePatch(SettingsBackend.configJson, from, to))
        }

        // RUL-02
        WyeListToolbar {
            addText: qsTr("Add Rule")
            addEnabled: page.editable

            onAddTriggered: editor.openNew("")

            QQC2.MenuItem {
                icon.name: "system-run"
                text: qsTr("Test Rules…")
                onTriggered: tester.openWith("", "")
            }
            QQC2.MenuSeparator {}
            QQC2.MenuItem {
                enabled: page.editable && !RulesBackend.busy
                icon.name: "document-import"
                text: qsTr("Import Rules…")
                onTriggered: importDialog.open()
            }
            QQC2.MenuItem {
                enabled: page.rows.length > 0 && !RulesBackend.busy
                icon.name: "document-export"
                text: qsTr("Export Rules…")
                onTriggered: exportDialog.open()
            }
            QQC2.MenuSeparator {}
            QQC2.MenuItem {
                enabled: page.editable && page.rows.length > 0
                icon.name: "edit-delete"
                text: qsTr("Delete All Rules…")
                onTriggered: deleteAll.open()
            }
        }
    }

    overlays: [
        RuleEditorSheet {
            id: editor

            editable: page.editable

            onSaveRequested: (index, rule) => page.save(RulesBackend.savePatch(SettingsBackend.configJson, index, JSON.stringify(rule)))
            onDeleteRequested: index => page.save(RulesBackend.deletePatch(SettingsBackend.configJson, index))
            onTestRequested: url => tester.openWith(url, "")
        },
        RuleTesterSheet {
            id: tester

            onRuleRequested: index => {
                tester.close();
                editor.openRule(index);
            }
        },
        WyeConfirmDialog {
            id: deleteAll

            title: qsTr("Delete All Rules?")
            message: qsTr("Every rule is removed. Their scripts stay on disk.")
            confirmText: qsTr("Delete All Rules")
            declineText: qsTr("Cancel")

            onConfirmed: page.save(RulesBackend.deleteAllPatch(SettingsBackend.configJson))
        },
        Dialogs.FileDialog {
            id: importDialog

            fileMode: Dialogs.FileDialog.OpenFile
            nameFilters: [qsTr("Wye rules (*.toml)"), qsTr("All files (*)")]
            title: qsTr("Import Rules")

            onAccepted: RulesBackend.importFrom(selectedFile.toString())
        },
        Dialogs.FileDialog {
            id: exportDialog

            fileMode: Dialogs.FileDialog.SaveFile
            nameFilters: [qsTr("Wye rules (*.toml)")]
            title: qsTr("Export Rules")

            onAccepted: RulesBackend.exportTo(selectedFile.toString())
        }
    ]
}
