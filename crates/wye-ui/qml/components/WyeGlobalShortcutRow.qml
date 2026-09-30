// WyeGlobalShortcutRow (BLK-16, ADV-05 to ADV-07, KEY-40, KEY-41): a row for one global shortcut, with one control that
// shows the current binding and a way to change it, as the session's mechanism allows:
//   portal (configurable)  the binding the portal reports ("None" when unset) and "Change…", which opens the desktop's own
//                          shortcut dialog; the portal owns the binding, so that dialog is also where it is cleared
//   X11 (recordable)       a recorder button showing the binding ("None" when unset; click, press the keys), and a clear
//                          button once a shortcut is set
//   none                   the command to bind in the compositor's configuration, with a Copy button (KEY-41)
//
// API (WyeRow's, plus)
//   shortcut: var          the row's data from SettingsBackend.shortcutsJson: {action, title, binding, label, command}
//   recordable: bool       the session has a shortcuts mechanism
//   configurable: bool     the mechanism has its own dialog (the portal)
// Bindings are saved with SettingsBackend.setShortcut.
pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
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

    readonly property bool bound: (shortcut.binding ?? "") !== ""

    // KEY-41: without a mechanism, say what to bind and where.
    help: recordable ? "" : "global-shortcuts"
    needsConfig: false
    subtitle: recordable ? "" : qsTr("Bind <code>%1</code> in your compositor's configuration.").arg(shortcut.command)
    title: shortcut.title

    WyeCopyButton {
        payload: row.shortcut.command
        visible: !row.recordable
    }

    // KEY-40: the portal's binding, as the desktop reports it.
    QQC2.Label {
        Accessible.name: row.bound ? qsTr("Shortcut: %1").arg(row.shortcut.label) : qsTr("No shortcut")
        color: row.bound ? Kirigami.Theme.textColor : Kirigami.Theme.disabledTextColor
        text: row.bound ? row.shortcut.label : qsTr("None")
        visible: row.recordable && row.configurable
    }

    QQC2.Button {
        Accessible.description: qsTr("Opens the desktop's shortcut settings")
        icon.name: "configure-shortcuts"
        text: qsTr("Change…")
        visible: row.recordable && row.configurable

        QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
        QQC2.ToolTip.text: qsTr("Change or remove this shortcut in the desktop's shortcut settings")
        QQC2.ToolTip.visible: hovered

        onClicked: SettingsBackend.configureShortcuts()
    }

    WyeShortcutRecorder {
        binding: row.shortcut.binding
        emptyText: qsTr("None")
        visible: row.recordable && !row.configurable

        onCleared: SettingsBackend.setShortcut(row.shortcut.action, "")
        onRecorded: binding => SettingsBackend.setShortcut(row.shortcut.action, binding)
    }

    QQC2.ToolButton {
        Accessible.name: text
        display: QQC2.AbstractButton.IconOnly
        enabled: row.bound
        icon.name: "edit-clear-symbolic"
        opacity: row.bound ? 1 : 0
        text: qsTr("Clear shortcut")
        visible: row.recordable && !row.configurable

        QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
        QQC2.ToolTip.text: text
        QQC2.ToolTip.visible: hovered

        onClicked: SettingsBackend.setShortcut(row.shortcut.action, "")
    }
}
