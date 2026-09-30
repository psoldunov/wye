// CopyHelper: puts text on the clipboard. Qt Quick has no clipboard object of its own, so a hidden TextEdit selects the
// text and copies it. Used by the History and About windows.
//
// API
//   copyText(value)        the clipboard holds `value`
import QtQuick

TextEdit {
    id: helper

    function copyText(value) {
        helper.text = value;
        helper.selectAll();
        helper.copy();
        helper.text = "";
    }

    visible: false
}
