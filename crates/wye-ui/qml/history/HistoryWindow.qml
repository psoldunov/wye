// The History window (17-dialogs.md, DLG-HIS-01 to DLG-HIS-04): the last opened links, newest first, with a search toggle
// and Clear History in the header, a context menu per row, and an empty state that says why the list is empty. It reloads
// when the service's HistoryRevision moves. Surface contract (crates/wye-ui/src/route.rs):
//   handle("show", "history", argument)  open, or raise, the one window (SET-04)
// Under `wye-ui --self-test` the argument is a JSON object: {fixture, scheme, query} (fixtures/history.json). The data is
// `HistoryBackend` (crates/wye-ui/src/bridge/history.rs).
pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import dev.soldunov.wye.ui

Kirigami.ApplicationWindow {
    id: window

    // The `window` and `argument` of the last ShowWindow.
    property string windowName
    property string argument
    readonly property var view: backend.viewJson === "" ? ({
            "enabled": false,
            "total": 0,
            "rows": []
        }) : JSON.parse(backend.viewJson)

    function parseArgument(text) {
        if (!text.startsWith("{")) {
            return {};
        }
        try {
            return JSON.parse(text);
        } catch (error) {
            console.warn("history: the argument is not JSON:", error);
            return {};
        }
    }

    function handle(action, key, argument) {
        if (action !== "show") {
            return;
        }
        windowName = key;
        window.argument = argument;
        const request = parseArgument(argument);
        if (request.fixture !== undefined) {
            if (!backend.loadFixture(JSON.stringify(request.fixture))) {
                console.error("history: the fixture is not valid service data");
            }
        }
        if (request.scheme !== undefined) {
            Qt.styleHints.colorScheme = request.scheme === "dark" ? Qt.Dark : Qt.Light;
        }
        if (request.query !== undefined) {
            page.searching = true;
            searchField.text = request.query;
        }
        backend.refresh();
        show();
        raise();
        requestActivate();
    }

    // DLG-HIS-01: about 560 × 480 px, resizable.
    title: qsTr("History")
    width: 560
    height: 480
    minimumWidth: Kirigami.Units.gridUnit * 20
    minimumHeight: Kirigami.Units.gridUnit * 16

    HistoryBackend {
        id: backend
    }

    CopyHelper {
        id: clipboard
    }

    // DLG-HIS-01: changes made elsewhere (a link opened, another window's Clear) show without a restart.
    Timer {
        interval: 2000
        repeat: true
        running: window.visible && !backend.offline

        onTriggered: backend.poll()
    }

    onVisibleChanged: {
        if (visible) {
            backend.refresh();
        }
    }

    Shortcut {
        enabled: !clearDialog.opened
        sequence: "Ctrl+W"

        onActivated: window.close()
    }

    Shortcut {
        enabled: !clearDialog.opened && !page.searching
        sequence: "Escape"

        onActivated: window.close()
    }

    Shortcut {
        sequence: "Ctrl+F"

        onActivated: page.searching = !page.searching
    }

    // DLG-HIS-01: Clear History asks first.
    Kirigami.PromptDialog {
        id: clearDialog

        dialogType: Kirigami.PromptDialog.Warning
        title: qsTr("Clear History?")
        subtitle: qsTr("Wye forgets every link in the history. This cannot be undone.")
        standardButtons: Kirigami.Dialog.NoButton
        customFooterActions: [
            Kirigami.Action {
                icon.name: "edit-clear-history"
                text: qsTr("Clear History")

                onTriggered: {
                    backend.clearHistory();
                    clearDialog.close();
                }
            },
            Kirigami.Action {
                icon.name: "dialog-cancel"
                text: qsTr("Cancel")

                onTriggered: clearDialog.close()
            }
        ]
    }

    pageStack.initialPage: Kirigami.Page {
        id: page

        property bool searching: false

        title: window.title
        padding: 0

        // DLG-HIS-01: title, search toggle, Clear History.
        actions: [
            Kirigami.Action {
                checkable: true
                checked: page.searching
                enabled: window.view.total > 0
                icon.name: "edit-find"
                text: qsTr("Search")

                onTriggered: page.searching = checked
            },
            Kirigami.Action {
                enabled: window.view.total > 0
                icon.name: "edit-clear-history"
                text: qsTr("Clear History")

                onTriggered: clearDialog.open()
            }
        ]

        onSearchingChanged: {
            if (searching) {
                searchField.forceActiveFocus();
            } else {
                searchField.text = "";
            }
        }

        header: ColumnLayout {
            spacing: 0

            Kirigami.SearchField {
                id: searchField

                Layout.fillWidth: true
                Layout.margins: Kirigami.Units.smallSpacing
                visible: page.searching

                onTextChanged: backend.search(text)
                Keys.onEscapePressed: page.searching = false
            }

            Kirigami.InlineMessage {
                Layout.fillWidth: true
                showCloseButton: true
                text: backend.error
                type: Kirigami.MessageType.Error
                visible: backend.error !== ""

                onVisibleChanged: if (!visible) {
                    backend.clearError()
                }
            }
        }

        ListView {
            id: list

            anchors.fill: parent
            activeFocusOnTab: true
            clip: true
            currentIndex: -1
            keyNavigationEnabled: true
            model: window.view.rows

            delegate: HistoryRow {
                highlighted: ListView.isCurrentItem && list.activeFocus

                onCopyRequested: text => clipboard.copyText(text)
                onDeleteRequested: id => backend.deleteEntry(id)
                onPickerRequested: id => backend.reopen(id, "picker")
                onRuleRequested: id => backend.createRule(id)
                onSameTargetRequested: id => backend.reopen(id, "same-target")
            }

            // DLG-HIS-04: why the list is empty.
            Kirigami.PlaceholderMessage {
                anchors.centerIn: parent
                helpfulAction: Kirigami.Action {
                    icon.name: "list-add"
                    text: qsTr("Turn On")

                    onTriggered: backend.turnOn()
                }
                text: qsTr("History Is Off")
                explanation: qsTr("Wye does not keep the links you open.")
                visible: backend.loaded && !window.view.enabled
                width: parent.width - Kirigami.Units.gridUnit * 4
            }

            Kirigami.PlaceholderMessage {
                anchors.centerIn: parent
                explanation: qsTr("Links you open appear here.")
                text: qsTr("No History")
                visible: backend.loaded && window.view.enabled && window.view.total === 0
                width: parent.width - Kirigami.Units.gridUnit * 4
            }

            Kirigami.PlaceholderMessage {
                anchors.centerIn: parent
                explanation: qsTr("No link matches “%1”.").arg(searchField.text)
                text: qsTr("No Matches")
                visible: window.view.total > 0 && window.view.rows.length === 0
                width: parent.width - Kirigami.Units.gridUnit * 4
            }
        }
    }
}
