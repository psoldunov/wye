// One row of the tray-menu popup (TRAY-08): a dimmed header, a separator,
// or an item with its check mark (TRAY-11), icon (TRAY-14), label and fixed
// shortcut (TRAY-13); a submenu row ends in an arrow (TRAY-15).
//
// The metrics and colours are those of a Plasma menu (qqc2-desktop-style's
// MenuItem and MenuSeparator, and Breeze's QMenu): the check column, then
// the icon, the label, the shortcut dimmed at the right; the hovered or
// keyboard-current row is a rounded focus-coloured frame over a light tint
// of it, and the text keeps its colour. Icons are drawn at the small-medium
// size, the largest a menu row takes without growing past a Plasma menu's
// row height (TRAY-14 asks for "large where the host allows").
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

Item {
    id: row

    // A row of TrayMenuBackend.rows.
    required property var entry
    property bool current: false
    // Whether any row of the list has a check mark: then every row keeps that
    // column, so the labels line up (as Plasma's menus do).
    property bool checkColumn: false
    // TRAY-21: Ctrl or Shift is held, so choosing a radio row opens it rather
    // than making it primary (TRAY-20): its mark hides, its column stays.
    property bool openHeld: false

    readonly property bool separator: entry.kind === "separator"
    readonly property bool header: entry.kind === "header"
    readonly property bool interactive: entry.selectable || entry.opens
    readonly property bool disabled: entry.enabled === false || (!interactive && !header && !separator)
    readonly property bool highlighted: current && interactive
    readonly property bool pressed: mouse.pressed && interactive
    readonly property color textColor: pressed ? Kirigami.Theme.highlightedTextColor : Kirigami.Theme.textColor
    // Breeze's menu item padding (qqc2-desktop-style's MenuItem hard-codes the same 4 px).
    readonly property real verticalPadding: Kirigami.Units.smallSpacing
    // What the row needs to show its label and shortcut in full; the list sizes itself from the widest row.
    readonly property real contentWidth: separator ? 0 : layout.implicitWidth + layout.anchors.leftMargin + layout.anchors.rightMargin

    signal chosen
    signal pointed
    // The pointer moved over the row with these keyboard modifiers (TRAY-21).
    signal modifiersMoved(int modifiers)

    Accessible.role: separator ? Accessible.Separator : header ? Accessible.StaticText : Accessible.MenuItem
    Accessible.name: separator ? "" : row.entry.label
    Accessible.checkable: row.entry.kind === "radio"
    Accessible.checked: row.entry.checked === true
    implicitHeight: separator ? Kirigami.Units.smallSpacing * 2 + 1 : Math.max(Kirigami.Units.iconSizes.smallMedium, label.implicitHeight) + verticalPadding * 2

    // The highlight: as qqc2-desktop-style's MenuItem.
    Rectangle {
        anchors.fill: parent
        visible: row.highlighted || row.pressed
        radius: Kirigami.Units.cornerRadius
        color: row.pressed ? Kirigami.Theme.focusColor : Qt.alpha(Kirigami.Theme.focusColor, 0.3)
        border.color: Kirigami.Theme.focusColor
        border.width: 1
    }

    Kirigami.Separator {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        anchors.leftMargin: Kirigami.Units.smallSpacing
        anchors.rightMargin: Kirigami.Units.smallSpacing
        visible: row.separator
    }

    RowLayout {
        id: layout

        anchors.fill: parent
        anchors.leftMargin: Kirigami.Units.smallSpacing
        anchors.rightMargin: Kirigami.Units.smallSpacing * 2
        visible: !row.separator
        spacing: Kirigami.Units.smallSpacing

        // TRAY-11: the checked radio item. A header starts at the check column's left edge, as a menu section does.
        Kirigami.Icon {
            Layout.preferredWidth: Kirigami.Units.iconSizes.small
            Layout.preferredHeight: Kirigami.Units.iconSizes.small
            Layout.rightMargin: Kirigami.Units.smallSpacing / 2
            visible: !row.header && row.checkColumn
            source: row.entry.checked && !(row.openHeld && row.entry.kind === "radio") ? "checkmark" : ""
            color: row.textColor
        }

        // TRAY-14: the browser's full-colour icon; a browser whose icon the theme lacks still shows a browser.
        Kirigami.Icon {
            Layout.preferredWidth: Kirigami.Units.iconSizes.smallMedium
            Layout.preferredHeight: Kirigami.Units.iconSizes.smallMedium
            visible: !row.header && (row.entry.icon ?? "") !== ""
            enabled: !row.disabled
            fallback: "internet-web-browser"
            source: row.entry.icon ?? ""
            selected: row.pressed
        }

        QQC2.Label {
            id: label

            Layout.fillWidth: true
            text: row.entry.label ?? ""
            elide: Text.ElideRight
            color: row.header || row.disabled ? Kirigami.Theme.disabledTextColor : row.textColor
        }

        // TRAY-13, KEY-51: dimmed, at the right.
        QQC2.Label {
            Layout.leftMargin: Kirigami.Units.gridUnit
            visible: (row.entry.shortcut ?? "") !== ""
            text: row.entry.shortcut ?? ""
            color: row.pressed ? row.textColor : Kirigami.Theme.disabledTextColor
            horizontalAlignment: Text.AlignRight
        }

        // TRAY-15
        Kirigami.Icon {
            Layout.leftMargin: Kirigami.Units.smallSpacing
            Layout.preferredWidth: Kirigami.Units.iconSizes.small
            Layout.preferredHeight: Kirigami.Units.iconSizes.small
            visible: row.entry.kind === "submenu"
            source: row.LayoutMirroring.enabled ? "go-next-symbolic-rtl" : "go-next-symbolic"
            color: row.disabled ? Kirigami.Theme.disabledTextColor : row.textColor
        }
    }

    MouseArea {
        id: mouse

        anchors.fill: parent
        enabled: row.interactive
        hoverEnabled: true
        onEntered: row.pointed()
        onPositionChanged: mouse => row.modifiersMoved(mouse.modifiers)
        onClicked: row.chosen()
    }
}
