// WyeRadioRow (BLK-06): a row with a horizontal radio group as its trailing control ("Small", "Medium", "Large").
//
// API (WyeRow's, plus)
//   choices: var           [{value: "small", label: qsTr("Small")}, …]
//   currentValue: var      the selected choice's value; bind it to the configuration
//   path: string           config key path to save the chosen value to; empty: only the signal
//   activated(var value)   the user picked a choice
pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls as QQC2
import dev.soldunov.wye.ui

WyeRow {
    id: row

    property var choices: []
    property var currentValue
    property string path
    signal activated(var value)

    Repeater {
        model: row.choices

        QQC2.RadioButton {
            id: radio

            required property var modelData

            checked: row.currentValue === radio.modelData.value
            text: radio.modelData.label
            onClicked: {
                row.activated(radio.modelData.value);
                if (row.path !== "") {
                    SettingsBackend.setValue(row.path, JSON.stringify(radio.modelData.value));
                }
                radio.checked = Qt.binding(() => row.currentValue === radio.modelData.value);
            }
        }
    }
}
