/*
    SPDX-FileCopyrightText: 2026 Philipp Soldunov <69530789+psoldunov@users.noreply.github.com>
    SPDX-License-Identifier: MIT

    Drives the applet's DaemonClient against a running `wye service` and
    prints what came back, so the D-Bus half is checked without a Plasma
    shell. Run through dbus-smoke.sh.
*/
pragma ComponentBehavior: Bound

import QtQuick

import "../dev.soldunov.wye/contents/ui" as Wye

Item {
    id: smoke

    property int failures: 0
    property bool started: false

    function report(label, ok, detail) {
        console.warn((ok ? "ok   " : "FAIL ") + label + (detail ? ": " + detail : ""));
        if (!ok) {
            smoke.failures += 1;
        }
    }

    function finish() {
        console.warn(smoke.failures === 0 ? "ALL CHECKS PASSED" : smoke.failures + " CHECK(S) FAILED");
        Qt.exit(smoke.failures === 0 ? 0 : 1);
    }

    function find(items, id) {
        for (const item of items || []) {
            if (item.id === id) {
                return item;
            }
            const inner = smoke.find(item.children, id);
            if (inner) {
                return inner;
            }
        }
        return null;
    }

    function check() {
        smoke.started = true;
        const tray = client.tray;
        smoke.report("service running", client.serviceRunning);
        smoke.report("registered as the tray host", client.registered, client.lastError);
        smoke.report("Tray parsed", !!tray && Array.isArray(tray.items) && tray.items.length > 0,
                     tray ? tray.items.length + " item(s)" : client.lastError);
        const picker = smoke.find(tray ? tray.items : [], "primary:picker");
        smoke.report("Picker radio item", !!picker && picker.kind === "radio", picker ? JSON.stringify(picker) : "");
        smoke.report("icon name", client.iconName(tray ? tray.icon : null).length > 0,
                     client.iconName(tray ? tray.icon : null));
        client.clipboardHasUrl(hasUrl => {
            smoke.report("ClipboardHasUrl answered", hasUrl === true || hasUrl === false, String(hasUrl));
            client.activate("primary:picker", error => {
                smoke.report("ActivateTrayItem(primary:picker)", error.length === 0, error);
                client.activate("separator:0", refused => {
                    smoke.report("a separator is refused", refused.length > 0, refused);
                    smoke.finish();
                });
            });
        });
    }

    Wye.DaemonClient {
        id: client

        onTrayChanged: {
            if (client.tray && client.registered && !smoke.started) {
                smoke.check();
            }
        }
        onRegisteredChanged: {
            if (client.tray && client.registered && !smoke.started) {
                smoke.check();
            }
        }
    }

    Timer {
        interval: 20000
        running: true

        onTriggered: {
            smoke.report("the service answered in time", false, client.lastError);
            smoke.finish();
        }
    }
}
