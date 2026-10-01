// The question the script editor asks before it closes with unsaved changes (SCR-10): Save, Discard or Cancel, in KDE's
// order. Built on WyeSheet, so it dims the window; Escape and clicking outside cancel (the window stays open).
//
// API
//   scriptName: string     what the changes are to ("Transform Script — Global")
//   canSave: bool          false while the script has a syntax error (SCR-07): Save is off, Discard and Cancel stay
//   saveRequested() / discardRequested()   the answer; Cancel gives none, the sheet only closes
//   open()
pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import dev.soldunov.wye.ui

WyeSheet {
    id: dialog

    property string scriptName
    property bool canSave: true
    signal saveRequested
    signal discardRequested

    primaryEnabled: canSave
    primaryIcon: "document-save"
    primaryText: qsTr("Save")
    secondaryIcon: "edit-delete"
    secondaryText: qsTr("Discard")
    tertiaryText: qsTr("Cancel")
    sheetWidth: Kirigami.Units.gridUnit * 26
    title: qsTr("Discard unsaved changes?")

    onPrimaryTriggered: {
        close();
        saveRequested();
    }
    onSecondaryTriggered: {
        close();
        discardRequested();
    }
    onTertiaryTriggered: close()

    QQC2.Label {
        Layout.fillWidth: true
        Layout.margins: Kirigami.Units.largeSpacing * 2
        text: dialog.canSave ? qsTr("“%1” has changes that are not saved. Save them, or discard them and close the editor.").arg(dialog.scriptName) : qsTr("“%1” has changes that are not saved, and the script has a syntax error, so it cannot be saved. Discard the changes and close the editor, or go back and fix it.").arg(dialog.scriptName)
        wrapMode: Text.WordWrap
    }
}
