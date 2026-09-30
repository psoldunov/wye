// The shown browsers sheet (05-browsers.md, SHOWN-01 to SHOWN-08): which targets the picker and the tray menu list, in
// which order, and with which hotkey. One row per candidate: every installed browser, every profile, every app added with
// "+", then private windows. Checked rows sit at the top in the user's order with a drag handle; a hotkey popup on every
// row. Checkbox, order and hotkey changes apply at once; Done closes the sheet.
pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import dev.soldunov.wye.ui

WyeSheet {
    id: sheet

    // The rows follow the configuration and the inventory (SHOWN-06: changes apply at once).
    readonly property var rows: SettingsBackend.generation >= 0 ? JSON.parse(SettingsBackend.shownRows()) : []
    readonly property int checkedCount: rows.filter(row => row.checked).length

    note: rows.length === 0 ? qsTr("No browsers found. Click Rescan on the Browsers page.") : ""
    primaryText: qsTr("Done")
    sheetWidth: Kirigami.Units.gridUnit * 21
    title: qsTr("Shown Browsers")

    function showChooser() {
        chooserLoader.active = true;
        (chooserLoader.item as WyeAppChooser)?.open();
    }

    onPrimaryTriggered: close()

    Connections {
        function onSheetRequested(name) {
            if (name === "app-chooser") {
                sheet.showChooser();
            }
        }

        target: SettingsBackend
    }

    // SHOWN-05: "+" at the bottom left adds any app through the app chooser.
    footerLeading: QQC2.ToolButton {
        display: QQC2.AbstractButton.IconOnly
        enabled: SettingsBackend.writable
        icon.name: "list-add"
        text: qsTr("Add App")
        QQC2.ToolTip.text: text
        QQC2.ToolTip.visible: hovered

        onClicked: sheet.showChooser()
    }

    WyeChecklist {
        Layout.fillWidth: true
        Layout.preferredHeight: implicitHeight
        enabled: SettingsBackend.writable
        rows: sheet.rows

        trailing: ShownHotkeyChooser {}

        onMoved: (from, to) => SettingsBackend.shownMove(from, to)
        onRemoveRequested: key => SettingsBackend.shownRemove(key)
        onToggled: (key, checked) => SettingsBackend.shownToggle(key, checked)
    }

    Loader {
        id: chooserLoader

        active: false

        sourceComponent: WyeAppChooser {
            multiple: false
            onChosenTarget: target => SettingsBackend.shownAdd(JSON.stringify(target))
            onClosed: chooserLoader.active = false
        }
    }
}
