// Wye's one-shot KWin query (PICK-02, source-app step 4 in
// docs/spec/13-linux-platform.md).
//
// The service fills in the @…@ placeholders, loads this file with
// org.kde.kwin.Scripting.loadScript(path, "wye-query-<nonce>"), starts it and
// unloads it once the answer arrives. The script runs once: it reads the
// pointer, the output under it and the active window, and reports them back
// through dev.soldunov.wye.KWin1.Report.
//
// Every number is forced to an integer with `| 0`: KWin marshals a JavaScript
// number that is not an integer as a double, which the Report signature
// (s i i s i s s) refuses.
(function () {
    "use strict";

    var pointer = workspace.cursorPos;
    var output = workspace.screenAt(pointer);
    var origin = output ? output.geometry : null;
    var active = workspace.activeWindow;

    callDBus(
        "@SERVICE@",
        "@PATH@",
        "@INTERFACE@",
        "Report",
        "@NONCE@",
        (origin ? pointer.x - origin.x : pointer.x) | 0,
        (origin ? pointer.y - origin.y : pointer.y) | 0,
        output ? String(output.name) : "",
        active ? active.pid | 0 : 0,
        active && active.desktopFileName ? String(active.desktopFileName) : "",
        active && active.resourceClass ? String(active.resourceClass) : ""
    );
})();
