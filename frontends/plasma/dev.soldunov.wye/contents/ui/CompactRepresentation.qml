/*
    SPDX-FileCopyrightText: 2026 Philipp Soldunov <69530789+psoldunov@users.noreply.github.com>
    SPDX-License-Identifier: MIT

    The tray icon: Wye's icon, the picker glyph or the primary browser's icon
    (TRAY-02, GEN-02), a warning emblem while Wye is not the default browser
    (TRAY-18, ONB-11), highlighted while the menu is open (TRAY-06). A primary
    click opens the menu (TRAY-07).
*/
pragma ComponentBehavior: Bound

import QtQuick

import org.kde.kirigami as Kirigami

MouseArea {
    id: root

    /*! Icon theme name or path. */
    property string iconName: "dev.soldunov.wye-symbolic"
    /*! Draw the warning emblem. */
    property bool warning: false
    /*! The menu is open. */
    property bool pressedState: false
    property string accessibleName: ""
    property string accessibleDescription: ""

    signal menuRequested

    acceptedButtons: Qt.LeftButton
    hoverEnabled: true
    activeFocusOnTab: true

    Accessible.name: root.accessibleName
    Accessible.description: root.accessibleDescription
    Accessible.role: Accessible.ButtonMenu

    onClicked: root.menuRequested()
    Keys.onPressed: event => {
        if (event.key === Qt.Key_Space || event.key === Qt.Key_Enter || event.key === Qt.Key_Return) {
            root.menuRequested();
            event.accepted = true;
        }
    }

    Kirigami.Icon {
        id: icon

        anchors.fill: parent
        source: root.iconName
        active: root.containsMouse || root.pressedState
        // Symbolic icons take the panel's colour (GEN-02 "Wye").
        isMask: root.iconName.endsWith("-symbolic")
    }

    Kirigami.Icon {
        anchors.right: icon.right
        anchors.bottom: icon.bottom
        width: Math.round(icon.width / 2)
        height: Math.round(icon.width / 2)
        visible: root.warning
        source: "emblem-warning"
    }
}
