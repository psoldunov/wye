// WyeGlobalShortcutRow (BLK-16, ADV-05 to ADV-07, KEY-40, KEY-41): a row for one global shortcut. With a shortcuts mechanism
// it holds a recorder (and a clear button once a shortcut is set); on the portal a "Change…" button opens the desktop's own
// dialog, and the row shows the binding the portal reports. Without a mechanism it shows the command to bind in the
// compositor's configuration, with a Copy button.
//
// API (WyeRow's, plus)
//   shortcut: var          the row's data from SettingsBackend.shortcutsJson: {action, title, binding, label, command}
//   recordable: bool       the session has a shortcuts mechanism
//   configurable: bool     the mechanism has its own dialog (the portal)
// Bindings are saved with SettingsBackend.setShortcut.
pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls as QQC2
import dev.soldunov.wye.ui

WyeRow {
    id: row

    property var shortcut: ({
            "action": "",
            "title": "",
            "binding": "",
            "label": "",
            "command": ""
        })
    property bool recordable: false
    property bool configurable: false

    // KEY-41: without a mechanism, say what to bind and where.
    help: recordable ? "" : "global-shortcuts"
    needsConfig: false
    subtitle: recordable ? "" : qsTr("Bind <code>%1</code> in your compositor's configuration.").arg(shortcut.command)
    title: shortcut.title

    WyeCopyButton {
        payload: row.shortcut.command
        visible: !row.recordable
    }

    WyeShortcutRecorder {
        binding: row.shortcut.binding
        visible: row.recordable

        onCleared: SettingsBackend.setShortcut(row.shortcut.action, "")
        onRecorded: binding => SettingsBackend.setShortcut(row.shortcut.action, binding)
    }

    QQC2.ToolButton {
        display: QQC2.AbstractButton.IconOnly
        icon.name: "edit-clear"
        text: qsTr("Clear shortcut")
        visible: row.recordable && row.shortcut.binding !== ""
        QQC2.ToolTip.text: text
        QQC2.ToolTip.visible: hovered

        onClicked: SettingsBackend.setShortcut(row.shortcut.action, "")
    }

    QQC2.Button {
        text: qsTr("Change…")
        visible: row.recordable && row.configurable

        onClicked: SettingsBackend.configureShortcuts()
    }
}
