// WyeModifierChooser (BLK-18, KEY-01): four linked toggle buttons, Shift, Ctrl, Alt and Super. The pressed set is the
// binding; none pressed means off. Left and right modifiers count the same.
//
// API
//   modifiers: var         the pressed names, for example ["Shift"]; bind it to the configuration
//   modified(var names)    the user changed the set; `names` is in the order Shift, Ctrl, Alt, Super
pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

RowLayout {
    id: chooser

    readonly property var names: ["Shift", "Ctrl", "Alt", "Super"]
    property var modifiers: []
    signal modified(var names)

    spacing: 0

    Repeater {
        model: chooser.names

        QQC2.Button {
            id: button

            required property string modelData

            checkable: true
            horizontalPadding: Kirigami.Units.smallSpacing * 2
            implicitWidth: implicitContentWidth + leftPadding + rightPadding
            checked: chooser.modifiers.indexOf(button.modelData) >= 0
            text: button.modelData
            onToggled: {
                const pressed = chooser.names.filter(name => name === button.modelData ? button.checked : chooser.modifiers.indexOf(name) >= 0);
                chooser.modified(pressed);
                button.checked = Qt.binding(() => chooser.modifiers.indexOf(button.modelData) >= 0);
            }
        }
    }
}
