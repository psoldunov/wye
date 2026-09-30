/*
    SPDX-FileCopyrightText: 2026 Philipp Soldunov <69530789+psoldunov@users.noreply.github.com>
    SPDX-License-Identifier: MIT

    Renders the applet's tray icon in every state (TRAY-02, TRAY-06, TRAY-18,
    GEN-02) into one PNG. Run through render.sh.

    Arguments after `--`: output PNG path.
*/
pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts

import org.kde.kirigami as Kirigami

import "../dev.soldunov.wye/contents/ui" as Wye

Rectangle {
    id: sheet

    readonly property var samples: [
        {
            "label": "Picker glyph",
            "icon": "dev.soldunov.wye-picker-symbolic",
            "warning": false,
            "pressed": false
        },
        {
            "label": "Wye (GEN-02)",
            "icon": "dev.soldunov.wye-symbolic",
            "warning": false,
            "pressed": false
        },
        {
            "label": "Primary browser",
            "icon": "internet-web-browser",
            "warning": false,
            "pressed": false
        },
        {
            "label": "Not default (ONB-11)",
            "icon": "dev.soldunov.wye-picker-symbolic",
            "warning": true,
            "pressed": false
        },
        {
            "label": "Menu open (TRAY-06)",
            "icon": "dev.soldunov.wye-symbolic",
            "warning": false,
            "pressed": true
        }
    ]

    width: row.implicitWidth + Kirigami.Units.gridUnit * 2
    height: row.implicitHeight + Kirigami.Units.gridUnit * 2
    color: Kirigami.Theme.backgroundColor

    RowLayout {
        id: row

        anchors.centerIn: parent
        spacing: Kirigami.Units.gridUnit

        Repeater {
            model: sheet.samples

            delegate: ColumnLayout {
                id: cell

                required property var modelData

                Wye.CompactRepresentation {
                    Layout.alignment: Qt.AlignHCenter
                    Layout.preferredWidth: Kirigami.Units.iconSizes.medium
                    Layout.preferredHeight: Kirigami.Units.iconSizes.medium
                    iconName: cell.modelData.icon
                    warning: cell.modelData.warning
                    pressedState: cell.modelData.pressed
                }

                Text {
                    Layout.alignment: Qt.AlignHCenter
                    text: cell.modelData.label
                    color: Kirigami.Theme.textColor
                    font.pixelSize: 11
                }
            }
        }
    }

    Timer {
        interval: 1500
        running: true

        onTriggered: {
            const out = Qt.application.arguments[Qt.application.arguments.length - 1];
            sheet.grabToImage(result => {
                const saved = result.saveToFile(out);
                console.warn(saved ? "wrote " + out : "could not write " + out);
                Qt.exit(saved ? 0 : 1);
            });
        }
    }
}
