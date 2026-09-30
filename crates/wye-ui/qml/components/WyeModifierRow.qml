// WyeModifierRow (BLK-18, KEY-01, KEY-06): a row whose trailing control is the modifier chooser. Where the session cannot
// report held modifiers, the chooser is disabled, the subtitle says "Not available in this session" and the help button
// explains why.
//
// API (WyeRow's, plus)
//   modifiers: var         the pressed names, for example ["Shift"]; bind it: page.value(path, [])
//   path: string           config key path to save to ("browsers.alternative-key"); empty: only the signal
//   modified(var names)    the user changed the set
pragma ComponentBehavior: Bound
import QtQuick
import dev.soldunov.wye.ui

WyeRow {
    id: row

    property var modifiers: []
    property string path
    property string availableSubtitle
    signal modified(var names)

    readonly property bool available: SettingsBackend.heldKeysAvailable

    dimmed: !available
    help: available ? "" : "held-keys-unavailable"
    subtitle: available ? availableSubtitle : qsTr("Not available in this session")

    WyeModifierChooser {
        modifiers: row.modifiers
        onModified: names => {
            row.modified(names);
            if (row.path !== "") {
                SettingsBackend.setModifiers(row.path, JSON.stringify(names));
            }
        }
    }
}
