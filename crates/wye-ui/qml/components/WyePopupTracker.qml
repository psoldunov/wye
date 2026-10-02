// WyePopupTracker (SET-07): counts one popup, menu or sheet in SettingsBackend.popups while it is on screen, so Escape
// closes it and not the Settings window, and in SettingsBackend.settling while it fades in or out, so a help popover
// opens only once no other popup is moving (BLK-08). Put one inside each such popup's owner.
//
// API
//   popup: var             the QQC2.Popup to follow (a combo box's `popup`, a sheet itself)
//
// It follows `visible` and `opened`, not the opened and closed signals: a popup closed during its enter transition never
// says `opened`, and one destroyed during its exit transition (its row rebuilt, its Loader turned off) never says
// `closed`, and either left the shared count wrong for the rest of the session. The tracker counts its popup at most
// once in each and gives the counts back when the popup hides or the tracker goes.
import QtQml
import dev.soldunov.wye.ui

QtObject {
    id: tracker

    property var popup
    property bool counted: false
    property bool settlingCounted: false
    readonly property bool shown: tracker.popup?.visible ?? false
    // On screen but not opened: its enter or exit transition is running.
    readonly property bool moving: tracker.shown && !(tracker.popup?.opened ?? false)

    function follow() {
        if (tracker.shown && !tracker.counted) {
            tracker.counted = true;
            SettingsBackend.popupOpened();
        } else if (!tracker.shown && tracker.counted) {
            tracker.counted = false;
            SettingsBackend.popupClosed();
        }
        if (tracker.moving && !tracker.settlingCounted) {
            tracker.settlingCounted = true;
            SettingsBackend.popupSettling();
        } else if (!tracker.moving && tracker.settlingCounted) {
            tracker.settlingCounted = false;
            SettingsBackend.popupSettled();
        }
    }

    onShownChanged: follow()
    onMovingChanged: follow()
    Component.onCompleted: follow()
    Component.onDestruction: {
        if (tracker.counted) {
            tracker.counted = false;
            SettingsBackend.popupClosed();
        }
        if (tracker.settlingCounted) {
            tracker.settlingCounted = false;
            SettingsBackend.popupSettled();
        }
    }
}
