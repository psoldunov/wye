// WyeSwitchRow (BLK-03): a row with an on/off switch. Saves at once when `path` is set (SET-06).
//
// API (WyeRow's, plus)
//   isOn: bool             the switch's state; bind it to the configuration: isOn: page.value(path, true)
//   path: string           config key path to save to ("general.launch-at-login"); empty: only the signal
//   switched(bool on)      the user flipped the switch
//   default property       more trailing controls, left of the switch
pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls as QQC2
import dev.soldunov.wye.ui

WyeRow {
    id: row

    property bool isOn: false
    property string path
    property alias switchControl: control
    signal switched(bool on)

    QQC2.Switch {
        id: control

        checked: row.isOn
        Accessible.name: row.title
        onToggled: {
            row.switched(control.checked);
            if (row.path !== "") {
                SettingsBackend.setValue(row.path, JSON.stringify(control.checked));
            }
            // The user's click replaced the binding; take it back so the switch follows the configuration.
            control.checked = Qt.binding(() => row.isOn);
        }
    }
}
