// WyeTargetRow (BLK-04, TGT-01): a row whose trailing control shows the current target's icon and name, then a round
// up/down chevron button. It opens the target menu (WyeTargetMenu); "Other…" opens the app chooser (WyeAppChooser). The
// menu and the chooser are created on first use, so a page with many rows stays light.
//
// API (WyeRow's, plus)
//   surface: string        "browsers" (no Default), "apps" (Default and the service's own app), "rule" (Default)
//   current: var           the current target in its configuration shape, for example {"picker": true}; bind it to the
//                          configuration: page.value(path, {picker: true})
//   path: string           config key path to save the choice to ("browsers.primary"); empty: only the signal
//   service: string        a web service ID ("discord"): saves the choice as that service's mapping (APP-04)
//   chosen(var target)     the user chose a target
pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import dev.soldunov.wye.ui

WyeRow {
    id: row

    property string surface: "browsers"
    property var current: ({
            "picker": true
        })
    property string path
    property string service
    signal chosen(var target)

    // Reads the generation first, so the label follows the configuration and the inventory (APP-04: "Default (<primary>)"
    // changes when the primary browser does).
    readonly property var shown: SettingsBackend.generation >= 0 ? JSON.parse(SettingsBackend.targetLabel(row.surface, JSON.stringify(row.current), row.service)) : ({
            "label": "",
            "icon": "",
            "missing": false
        })

    function pick(target) {
        row.chosen(target);
        if (row.service !== "") {
            SettingsBackend.setServiceTarget(row.service, JSON.stringify(target));
        } else if (row.path !== "") {
            SettingsBackend.setTarget(row.path, JSON.stringify(target));
        }
    }

    function openMenu() {
        menuLoader.active = true;
        const menu = menuLoader.item as WyeTargetMenu;
        if (menu !== null) {
            menu.rows = JSON.parse(SettingsBackend.targetMenu(row.surface, JSON.stringify(row.current), row.service));
            menu.popup();
        }
    }

    QQC2.Button {
        id: opener

        Accessible.description: qsTr("Opens the list of targets")
        Accessible.name: row.title + ": " + row.shown.label
        flat: true
        onClicked: row.openMenu()

        contentItem: RowLayout {
            spacing: Kirigami.Units.smallSpacing

            Kirigami.Icon {
                Layout.preferredHeight: Kirigami.Units.iconSizes.smallMedium
                Layout.preferredWidth: Kirigami.Units.iconSizes.smallMedium
                source: row.shown.missing ? "dialog-warning" : row.shown.icon
                visible: source !== ""
            }

            QQC2.Label {
                color: row.shown.missing ? Kirigami.Theme.negativeTextColor : Kirigami.Theme.textColor
                elide: Text.ElideRight
                text: row.shown.label
            }
        }

        // Neither is laid out: a Button positions only its own content.
        Loader {
            id: menuLoader

            active: false

            sourceComponent: WyeTargetMenu {
                parent: opener
                onChosen: target => row.pick(target)
                onOtherRequested: {
                    chooserLoader.active = true;
                    (chooserLoader.item as WyeAppChooser)?.open();
                }
            }
        }

        Loader {
            id: chooserLoader

            active: false

            sourceComponent: WyeAppChooser {
                multiple: false
                onChosenTarget: target => row.pick(target)
                onClosed: chooserLoader.active = false
            }
        }
    }

    QQC2.RoundButton {
        Accessible.name: qsTr("Choose a target")
        display: QQC2.AbstractButton.IconOnly
        flat: true
        icon.name: "go-down-symbolic"
        text: qsTr("Choose a target")
        onClicked: row.openMenu()
    }
}
