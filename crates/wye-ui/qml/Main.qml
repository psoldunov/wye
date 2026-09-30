// The engine's root object. It has no window of its own: it creates each
// surface's root file on first use, keeps it (one of each, SET-04), and
// calls its `handle(action, key, argument)` for every D-Bus request the
// App object routes here (crates/wye-ui/src/route.rs has the table).
//
// Under `wye-ui --self-test` it instead feeds one surface the cases of its
// fixture (crates/wye-ui/fixtures/<surface>.json), prints the pass line and
// exits.
import QtQuick
import dev.soldunov.wye.ui

QtObject {
    id: root

    // Set by `wye-ui --self-test-child` (src/qt_app.rs); empty otherwise.
    property string selfTestSurface
    property string selfTestCases
    property string selfTestPassLine
    property bool selfTestFailed: false

    // Surface name to its root object.
    property var surfaces: ({})

    readonly property App app: App {
        onRouted: (surface, action, key, argument) => root.deliver(surface, action, key, argument)
        onQuitRequested: Qt.quit()
    }

    // How long the self-test lets a surface render after its last case.
    readonly property Timer selfTestTimer: Timer {
        interval: 300
        onTriggered: root.finishSelfTest()
    }

    function surface(name) {
        const existing = surfaces[name];
        if (existing) {
            return existing;
        }
        const url = app.surfaceUrl(name);
        if (url === "") {
            console.error("unknown surface", name);
            return null;
        }
        const component = Qt.createComponent(url);
        if (component.status !== Component.Ready) {
            console.error("cannot load", url, component.errorString());
            return null;
        }
        const created = component.createObject(root);
        if (created === null) {
            console.error("cannot create", url);
            return null;
        }
        surfaces = Object.assign({}, surfaces, { [name]: created });
        return created;
    }

    function deliver(name, action, key, argument) {
        const target = surface(name);
        if (target === null) {
            return false;
        }
        target.handle(action, key, argument);
        return true;
    }

    function runSelfTest() {
        // Started first and failed until proven otherwise, so a surface that
        // throws still ends the run (Qt.exit() is ignored before the event
        // loop runs, so even a failure waits for the timer).
        selfTestFailed = true;
        selfTestTimer.start();
        const cases = JSON.parse(selfTestCases);
        selfTestFailed = !cases.every(c => deliver(selfTestSurface, c.action, c.key, c.argument));
    }

    function finishSelfTest() {
        if (selfTestFailed) {
            Qt.exit(1);
            return;
        }
        console.info(selfTestPassLine, selfTestSurface);
        Qt.exit(0);
    }

    Component.onCompleted: {
        if (selfTestSurface !== "") {
            runSelfTest();
        } else if (!app.attach()) {
            // Another App already receives the routes; nothing would reach
            // this one. Qt.exit() only works once the event loop runs.
            Qt.callLater(() => Qt.exit(1));
        }
    }
}
