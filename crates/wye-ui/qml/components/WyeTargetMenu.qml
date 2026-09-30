// WyeTargetMenu (TGT-01 to TGT-07): the popup every target popup row opens. It draws the rows Rust built
// (crates/wye-ui/src/settings/menu.rs): sections separated by lines, dimmed headers ("Private Browsing", "Profiles:
// Chrome"), an icon on every item (TGT-03), a checkmark on the current value, a warning icon on a target whose app is gone
// (APP-10), and "Other…" last. A menu taller than the window scrolls (TGT-04).
//
// API
//   rows: var              the menu rows: JSON of SettingsBackend.targetMenu(surface, current, service)
//   chosen(var target)     the user picked a target (its configuration shape: {"app": "firefox.desktop"})
//   otherRequested()       "Other…": open the app chooser (TGT-06)
//   popup(): open below the parent item; the menu closes itself after a choice.
pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import dev.soldunov.wye.ui

QQC2.Popup {
    id: menu

    property var rows: []
    signal chosen(var target)
    signal otherRequested

    readonly property real maximumHeight: (QQC2.Overlay.overlay?.height ?? Kirigami.Units.gridUnit * 30) - Kirigami.Units.gridUnit * 4

    y: parent ? parent.height : 0
    width: Math.max(Kirigami.Units.gridUnit * 14, parent?.width ?? 0)
    height: Math.min(list.contentHeight + topPadding + bottomPadding, maximumHeight)
    padding: Kirigami.Units.smallSpacing
    closePolicy: QQC2.Popup.CloseOnEscape | QQC2.Popup.CloseOnPressOutside

    function popup() {
        open();
    }

    onOpened: SettingsBackend.popupOpened()
    onClosed: SettingsBackend.popupClosed()
    Component.onDestruction: {
        if (opened) {
            SettingsBackend.popupClosed();
        }
    }

    contentItem: ListView {
        id: list

        clip: true
        currentIndex: -1
        model: menu.rows
        boundsBehavior: Flickable.StopAtBounds

        QQC2.ScrollBar.vertical: QQC2.ScrollBar {}

        delegate: QQC2.ItemDelegate {
            id: item

            required property var modelData
            required property int index

            readonly property string kind: item.modelData.kind
            readonly property bool selectable: kind === "item" || kind === "other"

            width: ListView.view.width
            height: kind === "separator" ? Kirigami.Units.smallSpacing * 2 + 1 : implicitHeight
            enabled: selectable
            hoverEnabled: selectable
            highlighted: selectable && hovered
            padding: Kirigami.Units.smallSpacing

            background: Rectangle {
                color: item.highlighted ? Qt.alpha(Kirigami.Theme.highlightColor, 0.2) : "transparent"
                radius: Kirigami.Units.cornerRadius
            }

            onClicked: {
                menu.close();
                if (kind === "other") {
                    menu.otherRequested();
                } else {
                    menu.chosen(item.modelData.target);
                }
            }

            contentItem: RowLayout {
                spacing: Kirigami.Units.smallSpacing

                Kirigami.Separator {
                    Layout.fillWidth: true
                    visible: item.kind === "separator"
                }

                QQC2.Label {
                    Layout.fillWidth: true
                    Layout.leftMargin: Kirigami.Units.smallSpacing
                    color: Kirigami.Theme.disabledTextColor
                    font: Kirigami.Theme.smallFont
                    text: item.modelData.label
                    visible: item.kind === "header"
                }

                Kirigami.Icon {
                    Layout.preferredHeight: Kirigami.Units.iconSizes.smallMedium
                    Layout.preferredWidth: Kirigami.Units.iconSizes.smallMedium
                    source: item.modelData.missing ? "dialog-warning" : item.modelData.icon
                    visible: item.kind === "item" && source !== ""
                }

                Item {
                    Layout.preferredWidth: Kirigami.Units.iconSizes.smallMedium
                    visible: item.kind === "item" && item.modelData.icon === "" && !item.modelData.missing
                }

                QQC2.Label {
                    Layout.fillWidth: true
                    color: item.modelData.missing ? Kirigami.Theme.negativeTextColor : Kirigami.Theme.textColor
                    elide: Text.ElideRight
                    text: item.modelData.label
                    visible: item.kind === "item" || item.kind === "other"
                }

                Kirigami.Icon {
                    Layout.preferredHeight: Kirigami.Units.iconSizes.small
                    Layout.preferredWidth: Kirigami.Units.iconSizes.small
                    source: "checkmark"
                    visible: item.kind === "item" && item.modelData.checked
                }
            }
        }
    }
}
