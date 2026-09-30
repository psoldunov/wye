/*
    SPDX-FileCopyrightText: 2026 Philipp Soldunov <69530789+psoldunov@users.noreply.github.com>
    SPDX-License-Identifier: MIT

    Wye in the Plasma system tray (01-tray-menu.md). The service owns every
    decision; the applet draws the `Tray` model it publishes and sends back
    the ID of the item chosen. While the applet is loaded it is the tray host,
    so the service shows no StatusNotifierItem of its own (decision 8).
*/
pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts

import org.kde.plasma.plasmoid
import org.kde.plasma.core as PlasmaCore
import org.kde.kirigami as Kirigami

PlasmoidItem {
    id: root

    readonly property var tray: daemon.tray
    readonly property var items: tray && tray.items ? tray.items : []
    readonly property bool warning: !!tray && tray.overlay === "warning"
    /*! The checked item of the primary-browser group (TRAY-11). */
    readonly property var primary: {
        for (const item of root.items) {
            if (item.kind === "radio" && item.checked) {
                return item;
            }
        }
        return null;
    }

    function openMenu() {
        if (!daemon.serviceRunning || !root.tray) {
            // Starts the service through D-Bus activation; the menu comes
            // with its first `Tray`.
            daemon.register();
            return;
        }
        if (trayMenu.open) {
            trayMenu.close();
            return;
        }
        // TRAY-10: the model's clipboard state is as old as its last change.
        daemon.clipboardHasUrl(hasUrl => {
            trayMenu.clipboardHasUrl = hasUrl;
            trayMenu.popup();
        });
    }

    preferredRepresentation: compactRepresentation

    // TRAY-04: hidden (moved to the hidden items; Wye keeps running) when
    // the tray icon is off, and while no service runs (TRAY-17).
    Plasmoid.status: {
        if (!daemon.serviceRunning || !root.tray || root.tray.visible === false) {
            return PlasmaCore.Types.HiddenStatus;
        }
        return root.warning ? PlasmaCore.Types.NeedsAttentionStatus : PlasmaCore.Types.ActiveStatus;
    }
    Plasmoid.icon: daemon.iconName(root.tray ? root.tray.icon : null)

    toolTipMainText: i18nc("@title Applet name", "Wye")
    toolTipSubText: {
        if (!daemon.serviceRunning) {
            return i18nc("@info:tooltip", "The Wye service is not running.");
        }
        if (root.warning) {
            return i18nc("@info:tooltip", "Wye is not the default browser.");
        }
        if (root.primary) {
            return i18nc("@info:tooltip %1 is a browser, a profile or Picker", "Primary browser: %1", root.primary.label);
        }
        return "";
    }

    compactRepresentation: CompactRepresentation {
        iconName: Plasmoid.icon
        warning: root.warning
        pressedState: trayMenu.open
        accessibleName: root.toolTipMainText
        accessibleDescription: root.toolTipSubText
        onMenuRequested: root.openMenu()
    }

    // Shown only where the tray lists the applet among its hidden items and
    // opens it there; the menu takes its place.
    fullRepresentation: Item {
        Layout.preferredWidth: Kirigami.Units.gridUnit * 12
        Layout.preferredHeight: Kirigami.Units.gridUnit * 4
    }

    onExpandedChanged: {
        if (root.expanded) {
            root.expanded = false;
            root.openMenu();
        }
    }

    DaemonClient {
        id: daemon
    }

    TrayMenu {
        id: trayMenu

        visualParent: root.compactRepresentationItem
        items: root.items
        onChosen: id => daemon.activate(id)
    }
}
