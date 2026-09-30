// First run (18-onboarding.md, ONB-01 to ONB-06): a small window that walks through what Wye needs before it is useful:
// welcome, make Wye the default browser, choose browsers, launch at login, and the optional browser extension. Back on every
// step after the first, progress dots at the bottom, and closing the window early counts as done. It opens on first start and
// later from the tray's "Set Up Wye…". Surface contract (crates/wye-ui/src/route.rs):
//   handle("show", "first-run", argument)  open, or raise, the one window (SET-04), at the welcome step
// Under `wye-ui --self-test` the argument is a JSON object: {fixture, scheme, step, desktop} (fixtures/onboarding.json).
// The data is `OnboardingBackend` (crates/wye-ui/src/bridge/onboarding.rs).
pragma ComponentBehavior: Bound
import QtQuick
import org.kde.kirigami as Kirigami
import dev.soldunov.wye.ui

Kirigami.ApplicationWindow {
    id: window

    // The `window` and `argument` of the last ShowWindow.
    property string windowName
    property string argument
    readonly property var view: OnboardingBackend.viewJson === "" ? ({}) : JSON.parse(OnboardingBackend.viewJson)
    // The steps in order (ONB-01 to ONB-05); the page stack holds the pages up to the current one.
    readonly property var stepPages: [welcomePage, defaultPage, browsersPage, integrationPage, extensionPage]

    function parseArgument(text) {
        if (!text.startsWith("{")) {
            return {};
        }
        try {
            return JSON.parse(text);
        } catch (error) {
            console.warn("first-run: the argument is not JSON:", error);
            return {};
        }
    }

    // Push or pop pages until the stack ends at step `index` (ONB-06).
    function showStep(index) {
        while (pageStack.depth > index + 1) {
            pageStack.pop();
        }
        while (pageStack.depth < index + 1) {
            pageStack.push(window.stepPages[pageStack.depth]);
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
            if (!OnboardingBackend.loadFixture(JSON.stringify(request.fixture), request.desktop ?? "")) {
                console.error("first-run: the fixture is not valid service data");
            }
        }
        if (request.scheme !== undefined) {
            Qt.styleHints.colorScheme = request.scheme === "dark" ? Qt.Dark : Qt.Light;
        }
        // ONB-01: every opening starts at the welcome step.
        OnboardingBackend.restart();
        if (request.step !== undefined) {
            OnboardingBackend.go(request.step);
        }
        window.showStep(OnboardingBackend.step);
        show();
        raise();
        requestActivate();
    }

    title: qsTr("Welcome to Wye")
    minimumWidth: Kirigami.Units.gridUnit * 24
    minimumHeight: Kirigami.Units.gridUnit * 22
    width: Kirigami.Units.gridUnit * 30
    height: Kirigami.Units.gridUnit * 26

    // A step at a time, moved by the buttons only.
    pageStack.globalToolBar.style: Kirigami.ApplicationHeaderStyle.None
    pageStack.columnView.columnResizeMode: Kirigami.ColumnView.SingleColumn
    pageStack.columnView.interactive: false
    pageStack.initialPage: welcomePage

    // ONB-06: closing early counts as done; the General page keeps showing the default-browser state.
    onClosing: OnboardingBackend.finish()

    Connections {
        function onFinished() {
            if (window.visible) {
                window.close();
            }
        }

        target: OnboardingBackend
    }

    Connections {
        function onStepChanged() {
            window.showStep(OnboardingBackend.step);
        }

        target: OnboardingBackend
    }

    footer: OnboardingFooter {
        view: window.view

        onBackRequested: OnboardingBackend.back()
        onNextRequested: OnboardingBackend.next()
        onSkipRequested: OnboardingBackend.skipDefault()
    }

    // The step pages exist from the start, parked in a hidden item until the page stack takes them: pushing a Component
    // or a URL makes the stack create the page without a parent, which Qt warns about.
    Item {
        visible: false

        OnboardingWelcome {
            id: welcomePage
        }

        OnboardingDefault {
            id: defaultPage
        }

        OnboardingBrowsers {
            id: browsersPage
        }

        OnboardingIntegration {
            id: integrationPage
        }

        OnboardingExtension {
            id: extensionPage
        }
    }
}
