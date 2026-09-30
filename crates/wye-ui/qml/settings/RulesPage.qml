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
    // The undo toast's text (RUL-06) and what an import or export did (RUL-02).
    property string undoText
    property string transferText
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
            // The editor replaces the tester, as the tester's own "Edit Rule…" does (DLG-TST-03).
            tester.close();
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
            // No translations ship, so each English form is its own string (a `%n` string would print "rule(s)").
            page.transferText = count === 1 ? qsTr("Imported 1 rule") : qsTr("Imported %1 rules").arg(count);
            SettingsBackend.refresh();
        }

        function onExported() {
            page.transferText = qsTr("Rules exported");
        }

        target: RulesBackend
    }

    Connections {
        function onSheetRequested(name) {
            if (name === "rule-editor") {
                editor.openNew("");
            } else if (name === "tester") {
                tester.openWith("", "");
            } else if (name === "rules-help") {
                // The self-test's view of the rules help (RUL-19), over the editor it belongs to.
                editor.openNew("");
                editor.showHelp();
            }
        }

        target: SettingsBackend
    }

    // RUL-06: each deletion gets the whole undo time, also one made while
    // the previous toast still shows.
    Timer {
        id: undoTimer

        interval: page.undoTime
        onTriggered: RulesBackend.dropUndo()
    }

    Connections {
        function onUndoArmed(count, name) {
            page.undoText = count === 1 ? qsTr("Deleted \u201c%1\u201d").arg(name) : qsTr("Deleted %1 rules").arg(count);
            undoTimer.restart();
        }

        function onCanUndoChanged() {
            if (!RulesBackend.canUndo) {
                undoTimer.stop();
            }
        }

        target: RulesBackend
    }

    // RUL-06: the undo toast.
    Kirigami.InlineMessage {
        Layout.fillWidth: true
        Layout.leftMargin: Kirigami.Units.largeSpacing
        Layout.rightMargin: Kirigami.Units.largeSpacing
        type: Kirigami.MessageType.Information
        visible: RulesBackend.canUndo
        text: page.undoText
        actions: [
            Kirigami.Action {
                icon.name: "edit-undo"
                text: qsTr("Undo")
                onTriggered: page.save(RulesBackend.undoPatch(SettingsBackend.configJson))
            }
        ]
    }

    // RUL-02: what an import or export did.
    Kirigami.InlineMessage {
        Layout.fillWidth: true
        Layout.leftMargin: Kirigami.Units.largeSpacing
        Layout.rightMargin: Kirigami.Units.largeSpacing
        type: Kirigami.MessageType.Positive
        visible: page.transferText !== ""
        text: page.transferText
        showCloseButton: true
        onVisibleChanged: if (!visible) {
            page.transferText = "";
        }
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
        // RUL-01: the empty state, with the action it asks for. The usage hint ("Click a rule to edit it…") only makes
        // sense once there are rules, so it sits under the card then.
        // The message sits in a plain item that takes the card's width: laid out by the card itself, its wrapped text
        // and the card's width chase each other and the layout never settles.
        Item {
            Layout.fillWidth: true
            implicitHeight: emptyMessage.implicitHeight + Kirigami.Units.gridUnit * 3
            visible: page.rows.length === 0

            Kirigami.PlaceholderMessage {
                id: emptyMessage

                x: Kirigami.Units.gridUnit
                y: Kirigami.Units.gridUnit * 1.5
                width: parent.width - Kirigami.Units.gridUnit * 2
                icon.name: "vcs-branch-symbolic"
                text: qsTr("No Rules")
                explanation: qsTr("A rule lets you open a specific app based on the URL and source app")
                helpfulAction: Kirigami.Action {
                    enabled: page.editable
                    icon.name: "list-add-symbolic"
                    text: qsTr("Add Rule…")
                    onTriggered: editor.openNew("")
                }
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

        // RUL-02: the list toolbar (BLK-13) at the bottom of the card: add at the left, the "⋯" menu at the right. Both
        // carry their words, as KDE's list toolbars do; the menu button's are in its tooltip.
        QQC2.ToolBar {
            Layout.fillWidth: true
            position: QQC2.ToolBar.Footer

            RowLayout {
                anchors.fill: parent
                spacing: Kirigami.Units.smallSpacing

                QQC2.ToolButton {
                    enabled: page.editable
                    icon.name: "list-add-symbolic"
                    text: qsTr("Add Rule…")
                    QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
                    QQC2.ToolTip.text: qsTr("Add a rule at the end of the list")
                    QQC2.ToolTip.visible: hovered
                    onClicked: editor.openNew("")
                }

                QQC2.ToolButton {
                    icon.name: "system-run-symbolic"
                    text: qsTr("Test Rules…")
                    QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
                    QQC2.ToolTip.text: qsTr("See which rule a link matches, without opening it")
                    QQC2.ToolTip.visible: hovered
                    onClicked: tester.openWith("", "")
                }

                Item {
                    Layout.fillWidth: true
                }

                QQC2.ToolButton {
                    id: moreButton

                    display: QQC2.AbstractButton.IconOnly
                    icon.name: "overflow-menu"
                    text: qsTr("More Actions")
                    Accessible.role: Accessible.ButtonMenu
                    QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
                    QQC2.ToolTip.text: text
                    QQC2.ToolTip.visible: hovered && !moreMenu.visible
                    down: pressed || moreMenu.visible
                    // RUL-02: the button toggles its menu. A press on the button does not count as a press outside the
                    // menu (its parent), so it reaches the button, which then closes the menu instead of opening it again.
                    onPressed: moreMenu.visible ? moreMenu.close() : moreMenu.popup(moreButton, 0, moreButton.height)

                    QQC2.Menu {
                        id: moreMenu

                        closePolicy: QQC2.Popup.CloseOnEscape | QQC2.Popup.CloseOnPressOutsideParent

                        QQC2.MenuItem {
                            icon.name: "system-run-symbolic"
                            text: qsTr("Test Rules…")
                            onTriggered: tester.openWith("", "")
                        }
                        QQC2.MenuSeparator {}
                        QQC2.MenuItem {
                            enabled: page.editable && !RulesBackend.busy
                            icon.name: "document-import-symbolic"
                            text: qsTr("Import Rules…")
                            onTriggered: importDialog.open()
                        }
                        QQC2.MenuItem {
                            enabled: page.rows.length > 0 && !RulesBackend.busy
                            icon.name: "document-export-symbolic"
                            text: qsTr("Export Rules…")
                            onTriggered: exportDialog.open()
                        }
                        QQC2.MenuSeparator {}
                        QQC2.MenuItem {
                            enabled: page.editable && page.rows.length > 0
                            icon.name: "edit-delete-symbolic"
                            text: qsTr("Delete All Rules…")
                            onTriggered: deleteAll.open()
                        }
                    }
                }
            }
        }
    }

    // RUL-01's usage hint, under the list it explains. The label sits in a plain item: as a layout child its width and
    // the page's never settle.
    Item {
        Layout.fillWidth: true
        implicitHeight: usageHint.implicitHeight
        visible: page.rows.length > 0

        QQC2.Label {
            id: usageHint

            anchors {
                left: parent.left
                right: parent.right
                leftMargin: Kirigami.Units.largeSpacing * 2
                rightMargin: Kirigami.Units.largeSpacing * 2
            }
            color: Kirigami.Theme.disabledTextColor
            font: Kirigami.Theme.smallFont
            text: qsTr("Click a rule to edit it. Drag to reorder.")
            wrapMode: Text.Wrap
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
