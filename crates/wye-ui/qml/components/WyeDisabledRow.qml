// WyeDisabledRow (BLK-10): a row for a setting that does not apply ("Force new window" when the target is not a browser):
// dimmed title, help button and control. Same API as WyeRow; `reason` is the help text that says why (KEY-06).
//
// API
//   reason: string         help popover text; or set `help` to a help text ID
pragma ComponentBehavior: Bound
import QtQuick

WyeRow {
    property string reason

    dimmed: true
    helpText: reason
}
