// WyeModifierChooser (BLK-18, KEY-01): four linked toggle buttons, Shift, Ctrl, Alt and Super. The pressed set is the
// binding; none pressed means off. Left and right modifiers count the same.
//
// API
//   modifiers: var         the pressed names, for example ["Shift"]; bind it to the configuration
//   modified(var names)    the user changed the set; `names` is in the order Shift, Ctrl, Alt, Super
// Each button keeps the width of its label (a layout never squeezes it, so "Shift" never reads "Shi"), and all four are as
// wide as the widest, so the group reads as one control.
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

    // The widest label plus a button's padding, so the four buttons share one width.
    readonly property real buttonWidth: Math.ceil(Math.max(...names.map(name => metrics.advanceWidth(name)))) + Kirigami.Units.gridUnit * 1.5

    Layout.minimumWidth: implicitWidth
    spacing: Kirigami.Units.smallSpacing / 2
    Accessible.role: Accessible.Grouping

    FontMetrics {
        id: metrics
    }

    Repeater {
        model: chooser.names

        QQC2.Button {
            id: button

            required property string modelData

            // Narrower than a dialog button (Breeze gives those a minimum width), never narrower than the label.
            implicitWidth: chooser.buttonWidth
            Layout.minimumWidth: chooser.buttonWidth
            Accessible.name: button.modelData
            checkable: true
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
