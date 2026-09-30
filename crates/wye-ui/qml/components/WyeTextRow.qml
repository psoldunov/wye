// WyeTextRow (BLK-07): a title and an entry whose text is right-aligned ("Name").
//
// API (WyeRow's, plus)
//   entryText: string      the entry's text; bind it to the configuration
//   placeholder: string    dimmed text while the entry is empty
//   path: string           config key path to save to when editing finishes; empty: only the signal
//   edited(string text)    editing finished (Enter, or focus left the entry)
pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import dev.soldunov.wye.ui

WyeRow {
    id: row

    property string entryText
    property string placeholder
    property string path
    signal edited(string text)

    QQC2.TextField {
        id: entry

        Layout.preferredWidth: Kirigami.Units.gridUnit * 12
        Accessible.name: row.title
        horizontalAlignment: Text.AlignRight
        placeholderText: row.placeholder
        text: row.entryText
        onEditingFinished: {
            if (entry.text === row.entryText) {
                return;
            }
            row.edited(entry.text);
            if (row.path !== "") {
                SettingsBackend.setValue(row.path, JSON.stringify(entry.text));
            }
        }
    }
}
