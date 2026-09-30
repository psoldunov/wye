// WyeChoiceRow (BLK-04 for plain values): a row whose trailing control is a popup of named choices ("Tray icon": Primary
// Browser / Wye). For targets use WyeTargetRow.
//
// API (WyeRow's, plus)
//   choices: var           [{value: "primary-browser", label: qsTr("Primary Browser")}, …]
//   currentValue: var      the selected choice's value; bind it to the configuration
//   path: string           config key path to save the chosen value to; empty: only the signal
//   activated(var value)   the user picked a choice
pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import dev.soldunov.wye.ui

WyeRow {
    id: row

    property var choices: []
    property var currentValue
    property string path
    signal activated(var value)

    QQC2.ComboBox {
        id: combo

        Accessible.name: row.title
        Layout.preferredWidth: Kirigami.Units.gridUnit * 10
        currentIndex: row.choices.findIndex(choice => choice.value === row.currentValue)
        model: row.choices.map(choice => choice.label)
        onActivated: index => {
            const value = row.choices[index].value;
            row.activated(value);
            if (row.path !== "") {
                SettingsBackend.setValue(row.path, JSON.stringify(value));
            }
            combo.currentIndex = Qt.binding(() => row.choices.findIndex(choice => choice.value === row.currentValue));
        }
    }
}
