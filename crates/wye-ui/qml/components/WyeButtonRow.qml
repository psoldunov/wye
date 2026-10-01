// WyeButtonRow (BLK-05): a row with a push button ("Choose…", "Configure…", "Edit Script…", "Record Shortcut"). A switch
// can sit next to the button in the same row.
//
// API (WyeRow's, plus)
//   buttonText: string     the button's label
//   buttonIcon: string     optional icon name
//   activated()            the button was pressed
//   hasSwitch: bool        show a switch left of the button
//   isOn: bool             the switch's state
//   path: string           config key path the switch saves to; empty: only the signal
//   switched(bool on)      the user flipped the switch
//   buttonEnabled: bool    the button alone can be disabled (for example "Configure…" while its switch is off)
//   buttonHighlighted: bool  the button is the action the row suggests ("Make Default"): drawn as a default button, in
//                          the accent colour (BLK-05)
pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls as QQC2
import dev.soldunov.wye.ui

WyeRow {
    id: row

    property string buttonText
    property string buttonIcon
    property bool buttonEnabled: true
    property bool buttonHighlighted: false
    property bool hasSwitch: false
    property bool isOn: false
    property string path
    signal activated
    signal switched(bool on)

    QQC2.Switch {
        id: control

        visible: row.hasSwitch
        checked: row.isOn
        Accessible.name: row.title
        onToggled: {
            row.switched(control.checked);
            if (row.path !== "") {
                SettingsBackend.setValue(row.path, JSON.stringify(control.checked));
            }
            control.checked = Qt.binding(() => row.isOn);
        }
    }

    QQC2.Button {
        Accessible.name: row.buttonText
        enabled: row.buttonEnabled
        // qqc2-desktop-style draws the accent-coloured default look only for a default button; `highlighted` alone
        // looks like keyboard focus.
        Accessible.defaultButton: row.buttonHighlighted
        highlighted: row.buttonHighlighted
        icon.name: row.buttonIcon
        text: row.buttonText
        onClicked: row.activated()
    }
}
