// WyeTargetRow (BLK-04, TGT-01): a row whose trailing control is a combo box showing the current target's icon and name.
// It opens the target menu (TGT-02 to TGT-07): sections separated by lines, dimmed headers ("Private Browsing", "Profiles:
// Chrome"), an icon on every item (TGT-03), a checkmark on the current value, a warning icon on a target whose app is gone
// (APP-10), and "Other…" last, which opens the app chooser (WyeAppChooser, TGT-06). The menu stays inside the Settings
// window: a menu in a window of its own takes the keyboard focus on Wayland, and the combo box closes it again as soon as
// it loses the focus. Its height is capped, and a longer menu scrolls (TGT-04). Its rows are built when it opens, and the
// chooser on first use, so a page with many rows stays light.
//
// API (WyeRow's, plus)
//   surface: string        "browsers" (no Default), "apps" (Default and the service's own app), "rule" (Default)
//   current: var           the current target in its configuration shape, for example {"picker": true}; bind it to the
//                          configuration: page.value(path, {picker: true})
//   path: string           config key path to save the choice to ("browsers.primary"); empty: only the signal
//   service: string        a web service ID ("discord"): saves the choice as that service's mapping (APP-04)
//   controlWidth: real     the combo box's width; default: as wide as its label, between 10 and 16 grid units (a row that
//                          must line up with another control, such as a text field, sets it)
//   chosen(var target)     the user chose a target
//   popup()                open the menu (as a click would)
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
    property real controlWidth: -1
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

    function popup() {
        combo.popup.open();
    }

    // A theme icon name, or a file (an app's own icon).
    function isFile(source) {
        return source.startsWith("/") || source.startsWith("file:");
    }

    QQC2.ComboBox {
        id: combo

        readonly property string shownIcon: row.shown.missing ? "dialog-warning" : (row.shown.icon ?? "")
        // Wide enough for the label, and within the same bounds on every row, so the trailing edge lines up.
        readonly property real wanted: labelMetrics.advanceWidth + Kirigami.Units.iconSizes.small + Kirigami.Units.gridUnit * 3

        Accessible.description: qsTr("Opens the list of targets")
        Accessible.name: row.title
        Layout.maximumWidth: row.controlWidth > 0 ? row.controlWidth : Kirigami.Units.gridUnit * 16
        Layout.minimumWidth: row.controlWidth > 0 ? row.controlWidth : Kirigami.Units.gridUnit * 10
        Layout.preferredWidth: row.controlWidth > 0 ? row.controlWidth : Math.max(Kirigami.Units.gridUnit * 10, Math.min(wanted, Kirigami.Units.gridUnit * 16))
        Kirigami.StyleHints.iconName: row.isFile(shownIcon) ? "" : shownIcon
        Kirigami.StyleHints.iconSource: row.isFile(shownIcon) ? shownIcon : ""
        displayText: row.shown.label
        model: []
        textRole: "label"
        // Scrolling the page over the box must not change the target.
        wheelEnabled: false

        QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
        QQC2.ToolTip.text: row.shown.missing ? qsTr("%1 is not installed").arg(row.shown.label) : row.shown.label
        QQC2.ToolTip.visible: hovered && !popup.visible && (row.shown.missing || combo.wanted > combo.width)

        // The arrow, Home and End keys of a closed box would step through targets and section headers and save the one
        // they land on without showing it: they only move in the menu (TGT-02). Type-ahead on a closed box finds nothing,
        // since the rows exist only while the menu is open (onClosed below).
        Keys.onDownPressed: event => event.accepted = !combo.popup.visible
        Keys.onUpPressed: event => event.accepted = !combo.popup.visible
        Keys.onPressed: event => {
            if (!combo.popup.visible && (event.key === Qt.Key_Home || event.key === Qt.Key_End || event.key === Qt.Key_PageUp || event.key === Qt.Key_PageDown)) {
                event.accepted = true;
            }
        }

        popup.height: Math.min(combo.popup.implicitHeight, Kirigami.Units.gridUnit * 26)

        // TGT-04: the desktop style's list scrolls only when it is taller than the whole window, so under the cap above
        // the mouse wheel would do nothing. It scrolls as soon as it is taller than the menu.
        Binding {
            property: "interactive"
            target: combo.popup.contentItem
            value: {
                const list = combo.popup.contentItem as Flickable;
                return list !== null && list.contentHeight > list.height;
            }
            when: combo.popup.contentItem instanceof Flickable
        }

        onActivated: index => {
            const entry = combo.model[index];
            if (entry === undefined) {
                return;
            }
            if (entry.kind === "other") {
                chooserLoader.active = true;
                (chooserLoader.item as WyeAppChooser)?.open();
            } else if (entry.kind === "item") {
                row.pick(entry.target);
            }
        }

        delegate: QQC2.MenuItem {
            id: entry

            required property var modelData
            required property int index

            readonly property string kind: entry.modelData.kind
            readonly property string source: entry.modelData.missing ? "dialog-warning" : (entry.modelData.icon ?? "")
            // TGT-02: a checkmark at the end marks the current target, as in the desktop's own combo boxes; there are no
            // check boxes, since only one target can be chosen.
            readonly property bool current: kind === "item" && entry.modelData.checked

            implicitHeight: kind === "separator" ? Kirigami.Units.smallSpacing * 2 + 1 : Math.max(implicitBackgroundHeight + topInset + bottomInset, implicitContentHeight + topPadding + bottomPadding, implicitIndicatorHeight + topPadding + bottomPadding)
            rightPadding: Kirigami.Units.largeSpacing * 2 + Kirigami.Units.iconSizes.small
            enabled: kind === "item" || kind === "other"
            highlighted: combo.highlightedIndex === index
            icon.name: row.isFile(source) ? "" : source
            icon.source: row.isFile(source) ? source : ""
            text: kind === "separator" ? "" : entry.modelData.label
            font: kind === "header" ? Kirigami.Theme.smallFont : Kirigami.Theme.defaultFont
            hoverEnabled: enabled

            // A separator is a line; a header a dimmed caption (disabled, so it cannot be chosen).
            Kirigami.Separator {
                anchors {
                    left: parent.left
                    right: parent.right
                    verticalCenter: parent.verticalCenter
                }
                visible: entry.kind === "separator"
            }

            Kirigami.Icon {
                anchors {
                    right: parent.right
                    rightMargin: Kirigami.Units.largeSpacing
                    verticalCenter: parent.verticalCenter
                }
                implicitHeight: Kirigami.Units.iconSizes.small
                implicitWidth: Kirigami.Units.iconSizes.small
                source: "checkmark"
                visible: entry.current
            }
        }

        // TGT-02: the rows are built when the menu opens; the current target is highlighted.
        Connections {
            function onAboutToShow() {
                const rows = JSON.parse(SettingsBackend.targetMenu(row.surface, JSON.stringify(row.current), row.service));
                combo.model = rows;
                combo.currentIndex = rows.findIndex(entry => entry.kind === "item" && entry.checked);
            }

            // The rows go with the menu, so a letter typed on the closed box cannot choose a target from a stale list
            // (TGT-02). Later, so `onActivated` still reads the row that was chosen.
            function onClosed() {
                Qt.callLater(() => {
                    if (!combo.popup.visible) {
                        combo.model = [];
                    }
                });
            }

            target: combo.popup
        }

        TextMetrics {
            id: labelMetrics

            font: combo.font
            text: row.shown.label
        }

        // While the menu is open, Escape closes it and not the window (SET-07).
        WyePopupTracker {
            popup: combo.popup
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
}
