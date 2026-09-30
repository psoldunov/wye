/*
    SPDX-FileCopyrightText: 2026 Philipp Soldunov <69530789+psoldunov@users.noreply.github.com>
    SPDX-License-Identifier: MIT

    The only place in the applet that talks D-Bus. Everything else renders the
    parsed `tray` model it exposes.

    Contract: docs/dbus-api.md (`Tray`, `RegisterTray`, `ActivateTrayItem`,
    `ClipboardHasUrl`).
*/
pragma ComponentBehavior: Bound

import QtQuick

import org.kde.plasma.workspace.dbus as DBus

Item {
    id: client

    readonly property string service: "dev.soldunov.wye"
    readonly property string objectPath: "/dev/soldunov/wye"
    readonly property string ifaceName: "dev.soldunov.wye1"

    /*! Parsed `Tray` JSON (wye_api::tray::TrayMenu), or null before it arrives. */
    property var tray: null
    /*! The service owns its bus name right now. */
    readonly property bool serviceRunning: serviceWatcher.registered
    /*! RegisterTray answered on the current service. */
    property bool registered: false
    /*! Last transport or parse failure; empty after a good exchange. */
    property string lastError: ""

    visible: false
    implicitWidth: 0
    implicitHeight: 0

    function buildMessage(member, args) {
        return {
            "service": client.service,
            "path": client.objectPath,
            "iface": client.ifaceName,
            "member": member,
            "arguments": args || []
        };
    }

    function errorText(reply) {
        if (reply && reply.error && reply.error.isValid && reply.error.message) {
            return reply.error.message;
        }
        return i18n("The Wye service did not answer.");
    }

    /*!
       Announce the applet as the tray host, so the service hides its own
       StatusNotifierItem (decision 8). The bus name is D-Bus activatable, so
       this also starts the service.
    */
    function register() {
        DBus.SessionBus.asyncCall(buildMessage("RegisterTray", ["plasma-applet"]), reply => {
            client.registered = true;
            client.lastError = "";
            properties.updateAll();
        }, reply => {
            client.registered = false;
            client.lastError = client.errorText(reply);
        });
    }

    /*!
       Stop being the tray host. The applet's connection is plasmashell's and
       outlives the applet, so removing or disabling the applet has to say so
       for the service's own tray icon to come back.
    */
    function unregister() {
        if (!client.registered) {
            return;
        }
        client.registered = false;
        DBus.SessionBus.asyncCall(buildMessage("UnregisterTray"), reply => {}, reply => {});
    }

    /*! Carry out the menu item `id`; `callback(errorMessage)` is optional. */
    function activate(id, callback) {
        DBus.SessionBus.asyncCall(buildMessage("ActivateTrayItem", [String(id)]), reply => {
            client.lastError = "";
            if (callback) {
                callback("");
            }
        }, reply => {
            client.lastError = client.errorText(reply);
            if (callback) {
                callback(client.lastError);
            }
        });
    }

    /*! TRAY-10: `callback(hasUrl)`; false when the service cannot tell. */
    function clipboardHasUrl(callback) {
        DBus.SessionBus.asyncCall(buildMessage("ClipboardHasUrl"), reply => {
            callback(reply.value === true);
        }, reply => {
            callback(false);
        });
    }

    /*! The icon theme name or path to draw for a `Tray.icon` value. */
    function iconName(icon) {
        if (!icon || icon.kind === "app") {
            return "dev.soldunov.wye-symbolic";
        }
        if (icon.kind === "picker") {
            return "dev.soldunov.wye-picker-symbolic";
        }
        return icon.name || "dev.soldunov.wye-symbolic";
    }

    function applyTray() {
        const raw = properties.properties.Tray;
        if (raw === undefined || raw === null || String(raw).length === 0) {
            return;
        }
        try {
            client.tray = JSON.parse(String(raw));
            client.lastError = "";
        } catch (error) {
            client.lastError = i18n("The Wye service sent an unreadable menu.");
        }
    }

    DBus.DBusServiceWatcher {
        id: serviceWatcher

        busType: DBus.BusType.Session
        watchedService: client.service

        onRegisteredChanged: {
            if (registered) {
                // A new service knows no hosts yet.
                client.register();
            } else {
                client.registered = false;
                client.tray = null;
            }
        }
    }

    DBus.Properties {
        id: properties

        busType: DBus.BusType.Session
        service: client.service
        path: client.objectPath
        iface: client.ifaceName

        onRefreshed: client.applyTray()
        onPropertiesChanged: client.applyTray()
    }

    Component.onCompleted: client.register()
    Component.onDestruction: client.unregister()
}
