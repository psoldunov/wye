// The Settings window (03-settings-window.md): one window with seven pages, titled with the current page (SET-01), a page
// switcher across the top (SET-02, SET-03), one size for every page with the page scrolling inside (SET-05), instant
// apply (SET-06), Escape and Ctrl+W to close (SET-07, KEY-50), and the last page reopened (SET-08). Surface contract
// (crates/wye-ui/src/route.rs):
//   handle("show", "settings", page)    open, or raise, the one window (SET-04) on `page`; an empty page is the last one
//   handle("show", "rule-editor", json) the Rules page; `windowRequested` tells it to open the rule editor
//   handle("show", "test-rules", "")    the Rules page; `windowRequested` tells it to open the tester
// Under `wye-ui --self-test` the argument may instead be a JSON object: {page, fixture, scheme, sheet} (fixtures/settings.json);
// `sheet` is shown-browsers, app-chooser, picker-keys, expansion, history-confirm, rule-editor, rules-help or tester, or
// target-menu (the primary browser's target menu, open).
// The data of every page is `SettingsBackend` (crates/wye-ui/src/bridge/settings.rs).
pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import dev.soldunov.wye.ui

Kirigami.ApplicationWindow {
    id: window

    // The `window` and `argument` of the last ShowWindow.
    property string windowName
    property string argument
    property string currentPage: "general"
    // The user chose a page before the last page was known (SET-08).
    property bool navigated: false

    // Tells the Rules page (U11) that the rule editor or the tester was asked for.
    signal windowRequested(string name, string argument)

    readonly property var pageIds: ["general", "browsers", "apps", "picker", "rules", "extras", "advanced"]
    readonly property var components: ({
            "general": generalPage,
            "browsers": browsersPage,
            "apps": appsPage,
            "picker": pickerPage,
            "rules": rulesPage,
            "extras": extrasPage,
            "advanced": advancedPage
        })
    // SET-05, adapted: one size for every page, so switching pages never resizes the window. It fits the densest page
    // (Advanced, the Apps list scrolls) at the default font; the user can resize it, and the page scrolls inside.
    readonly property real availableHeight: screen?.desktopAvailableHeight ?? Kirigami.Units.gridUnit * 50
    readonly property real defaultWidth: Kirigami.Units.gridUnit * 34
    readonly property real defaultHeight: Math.min(Kirigami.Units.gridUnit * 38, Math.round(availableHeight * 0.85))

    function pageTitle(id) {
        switch (id) {
        case "browsers":
            return qsTr("Browsers");
        case "apps":
            return qsTr("Apps");
        case "picker":
            return qsTr("Picker");
        case "rules":
            return qsTr("Rules");
        case "extras":
            return qsTr("Extras");
        case "advanced":
            return qsTr("Advanced");
        default:
            return qsTr("General");
        }
    }

    // Show `id` without remembering it: the page the last session ended on is not the user's new choice yet.
    function selectPage(id) {
        currentPage = pageIds.indexOf(id) >= 0 ? id : "general";
    }

    // SET-01, SET-08: the user goes to a page, and the window reopens on it next time.
    function showPage(id) {
        selectPage(id);
        SettingsBackend.rememberPage(currentPage);
    }

    // The page that owns each sheet a self-test fixture can ask for.
    readonly property var sheetPages: ({
            "shown-browsers": "browsers",
            "target-menu": "browsers",
            "app-chooser": "browsers",
            "picker-keys": "picker",
            "expansion": "advanced",
            "history-confirm": "advanced",
            "rule-editor": "rules",
            "rules-help": "rules",
            "tester": "rules"
        })

    // Open a sheet: go to its page, which creates it, and ask for it. The app chooser opens from the shown browsers sheet.
    function openSheet(name) {
        selectPage(sheetPages[name] ?? currentPage);
        if (name === "app-chooser") {
            SettingsBackend.requestSheet("shown-browsers");
        }
        SettingsBackend.requestSheet(name);
    }

    function lastPage() {
        if (SettingsBackend.statusJson === "") {
            return "";
        }
        return JSON.parse(SettingsBackend.statusJson).uiState?.lastPage ?? "";
    }

    function parseArgument(text) {
        if (!text.startsWith("{")) {
            return {
                "page": text
            };
        }
        try {
            return JSON.parse(text);
        } catch (error) {
            console.warn("settings: the argument is not JSON:", error);
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
            if (!SettingsBackend.loadFixture(JSON.stringify(request.fixture))) {
                console.error("settings: the fixture is not valid service data");
            }
        }
        if (request.scheme !== undefined) {
            Qt.styleHints.colorScheme = request.scheme === "dark" ? Qt.Dark : Qt.Light;
        }
        let page = request.page ?? "";
        if (key === "rule-editor" || key === "test-rules") {
            page = "rules";
        }
        if (page === "") {
            // SET-08: reopen on the last page, once it is known.
            page = lastPage();
            navigated = page !== "";
            selectPage(page === "" ? currentPage : page);
        } else {
            navigated = true;
            showPage(page);
        }
        if (key === "rule-editor" || key === "test-rules") {
            windowRequested(key, argument);
            // The Rules page takes the request, now or once it is created.
            RulesBackend.request(key, argument);
        }
        if (request.sheet !== undefined) {
            openSheet(request.sheet);
        }
        show();
        raise();
        requestActivate();
    }

    title: pageTitle(currentPage)
    width: defaultWidth
    height: defaultHeight
    minimumWidth: Kirigami.Units.gridUnit * 26
    minimumHeight: Math.min(Kirigami.Units.gridUnit * 20, defaultHeight)

    // The last page, once it is known and the user has not chosen another.
    Connections {
        function onLoadedChanged() {
            if (!window.navigated && SettingsBackend.loaded && window.lastPage() !== "") {
                window.selectPage(window.lastPage());
            }
        }

        target: SettingsBackend
    }

    // A link in a note that goes to another page (ADV-08).
    Connections {
        function onPageRequested(name) {
            window.showPage(name);
        }

        target: SettingsBackend
    }

    // Changes made elsewhere (`wye default`, a new browser) show without a
    // restart: while the window is on screen, the backend reads what the
    // service announces (PropertiesChanged); when it comes back, all of it.
    onVisibleChanged: {
        SettingsBackend.live = visible;
        if (visible) {
            SettingsBackend.refresh();
        }
    }
    // Anything the service changed without announcing it shows once the
    // user returns to the window.
    onActiveChanged: {
        if (active && !SettingsBackend.offline) {
            SettingsBackend.poll();
        }
    }

    WyeErrorText {
        id: errors
    }

    // SET-07, KEY-50. While a sheet or menu is open, Escape closes that first.
    Shortcut {
        enabled: SettingsBackend.popups === 0
        sequence: "Escape"

        onActivated: window.close()
    }

    Shortcut {
        sequence: "Ctrl+W"

        onActivated: window.close()
    }

    // KEY-50: Ctrl+, opens Settings; from the window itself it raises it.
    Shortcut {
        sequence: "Ctrl+,"

        onActivated: {
            window.raise();
            window.requestActivate();
        }
    }

    // KEY-50: Ctrl+Q quits Wye.
    Shortcut {
        sequence: "Ctrl+Q"

        onActivated: {
            SettingsBackend.act("quit");
            window.close();
        }
    }

    Component {
        id: generalPage

        GeneralPage {}
    }

    Component {
        id: browsersPage

        BrowsersPage {}
    }

    Component {
        id: appsPage

        AppsPage {}
    }

    Component {
        id: pickerPage

        PickerPage {}
    }

    Component {
        id: rulesPage

        RulesPage {}
    }

    Component {
        id: extrasPage

        ExtrasPage {}
    }

    Component {
        id: advancedPage

        AdvancedPage {}
    }

    // SET-02, SET-03: seven pages, each with its icon above its label, in this order. The icons are the theme's symbolic ones,
    // so they read as one set at the tab bar's size and follow the colour scheme.
    header: Kirigami.NavigationTabBar {
        id: tabs

        actions: [
            Kirigami.Action {
                checked: window.currentPage === "general"
                icon.name: "configure-symbolic"
                text: qsTr("General")

                onTriggered: window.showPage("general")
            },
            Kirigami.Action {
                checked: window.currentPage === "browsers"
                icon.name: "globe-symbolic"
                text: qsTr("Browsers")

                onTriggered: window.showPage("browsers")
            },
            Kirigami.Action {
                checked: window.currentPage === "apps"
                icon.name: "applications-all-symbolic"
                text: qsTr("Apps")

                onTriggered: window.showPage("apps")
            },
            Kirigami.Action {
                checked: window.currentPage === "picker"
                icon.name: "view-list-text-symbolic"
                text: qsTr("Picker")

                onTriggered: window.showPage("picker")
            },
            Kirigami.Action {
                checked: window.currentPage === "rules"
                icon.name: "vcs-branch-symbolic"
                text: qsTr("Rules")

                onTriggered: window.showPage("rules")
            },
            Kirigami.Action {
                checked: window.currentPage === "extras"
                icon.name: "starred-symbolic"
                text: qsTr("Extras")

                onTriggered: window.showPage("extras")
            },
            Kirigami.Action {
                checked: window.currentPage === "advanced"
                icon.name: "tools-symbolic"
                text: qsTr("Advanced")

                onTriggered: window.showPage("advanced")
            }
        ]
    }

    // SET-02, SET-03: the wheel never switches pages, only a click or a shortcut does. NavigationTabBar flips to the previous or
    // next tab on a wheel notch, and a target menu that opened over the bar leaves the pointer there, so the next notch sent the
    // window to General. It is parented to the bar here rather than declared in it: the bar treats its children as tab buttons.
    // No buttons are accepted, so clicks, hover and tooltips still reach the tabs.
    MouseArea {
        id: tabsWheelGuard

        acceptedButtons: Qt.NoButton
        anchors.fill: parent
        parent: tabs
        z: 1

        onWheel: wheel => wheel.accepted = true
    }

    pageStack.globalToolBar.style: Kirigami.ApplicationHeaderStyle.None
    pageStack.initialPage: Kirigami.Page {
        id: frame

        padding: 0
        title: window.title

        QQC2.ScrollView {
            id: scroll

            anchors.fill: parent
            contentWidth: availableWidth

            ColumnLayout {
                id: column

                spacing: Kirigami.Units.largeSpacing
                width: scroll.availableWidth

                // SET-06: a read-only file (home-manager) keeps the window usable but nothing can be saved.
                Kirigami.InlineMessage {
                    Layout.fillWidth: true
                    Layout.leftMargin: Kirigami.Units.largeSpacing
                    Layout.rightMargin: Kirigami.Units.largeSpacing
                    Layout.topMargin: Kirigami.Units.largeSpacing
                    text: qsTr("The configuration file is read-only, so changes cannot be saved. It is probably managed by Nix, home-manager or another tool: change it there.")
                    type: Kirigami.MessageType.Information
                    visible: SettingsBackend.loaded && !SettingsBackend.writable
                }

                Kirigami.InlineMessage {
                    Layout.fillWidth: true
                    Layout.leftMargin: Kirigami.Units.largeSpacing
                    Layout.rightMargin: Kirigami.Units.largeSpacing
                    Layout.topMargin: Kirigami.Units.largeSpacing
                    text: errors.describe(SettingsBackend.errorKind, SettingsBackend.error)
                    type: Kirigami.MessageType.Error
                    visible: SettingsBackend.error !== ""

                    actions: [
                        Kirigami.Action {
                            text: qsTr("Dismiss")

                            onTriggered: SettingsBackend.clearError()
                        }
                    ]
                }

                Kirigami.InlineMessage {
                    Layout.fillWidth: true
                    Layout.leftMargin: Kirigami.Units.largeSpacing
                    Layout.rightMargin: Kirigami.Units.largeSpacing
                    Layout.topMargin: Kirigami.Units.largeSpacing
                    text: errors.describe(SettingsBackend.connectionErrorKind, SettingsBackend.connectionError)
                    type: Kirigami.MessageType.Warning
                    visible: SettingsBackend.connectionError !== ""
                }

                // The same space above the first group on every page, titled or not, and below the last.
                Loader {
                    id: pageLoader

                    Layout.fillWidth: true
                    Layout.topMargin: Kirigami.Units.largeSpacing
                    Layout.bottomMargin: Kirigami.Units.gridUnit
                    sourceComponent: window.components[window.currentPage]
                }
            }
        }
    }
}
