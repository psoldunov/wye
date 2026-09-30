// WyeAppChooser (DLG-APP-01 to DLG-APP-04): the "Choose App" sheet. A search entry at the top (focused on open) filters by
// name, desktop ID and keywords; sections Recent Sources (only when choosing source apps), Browsers and All Apps, each row
// with an icon, a name and a dimmed packaging badge ("Flatpak", "Snap"). Single choice: a click chooses and closes.
// Multiple choice: checkboxes and an Add button. Browse… picks an executable or a .desktop file.
//
// API
//   multiple: bool         checkboxes and Add (source apps); default false (a click chooses)
//   showRecent: bool       list Recent Sources first (source apps)
//   chosenTarget(var target)      single choice: the target in its configuration shape ({"custom": "slack.desktop"})
//   chosenTargets(var targets)    multiple choice: the targets of the checked rows
//   open() / closed()      as any popup
// Used by "Other…" of every target menu (TGT-06), "+" of the shown browsers sheet (SHOWN-05) and "+" under Source Apps.
pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Dialogs
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import dev.soldunov.wye.ui

WyeSheet {
    id: chooser

    property bool multiple: false
    property bool showRecent: false
    property var selected: []
    signal chosenTarget(var target)
    signal chosenTargets(var targets)

    readonly property var rows: JSON.parse(backend.rows === "" ? "[]" : backend.rows)

    function choose(id) {
        const target = backend.targetFor(id);
        if (target === "") {
            return;
        }
        chooser.chosenTarget(JSON.parse(target));
        chooser.close();
    }

    function toggle(id, checked) {
        const rest = selected.filter(existing => existing !== id);
        selected = checked ? rest.concat([id]) : rest;
    }

    function confirm() {
        const targets = selected.map(id => backend.targetFor(id)).filter(text => text !== "").map(text => JSON.parse(text));
        chooser.chosenTargets(targets);
        chooser.close();
    }

    function firstApp() {
        const app = rows.find(row => row.kind === "app");
        return app === undefined ? "" : app.id;
    }

    sheetWidth: Kirigami.Units.gridUnit * 26
    title: qsTr("Choose App")
    primaryEnabled: selected.length > 0
    primaryText: multiple ? qsTr("Add") : ""
    secondaryText: qsTr("Cancel")

    onAboutToShow: {
        selected = [];
        search.text = "";
        if (SettingsBackend.offline) {
            backend.loadJson(SettingsBackend.fixtureAppsJson);
        } else {
            backend.reload();
        }
        backend.filter("", chooser.showRecent);
    }
    onOpened: search.forceActiveFocus()
    onPrimaryTriggered: confirm()
    onSecondaryTriggered: close()

    AppChooserBackend {
        id: backend
    }

    footerLeading: QQC2.Button {
        icon.name: "document-open"
        text: qsTr("Browse…")
        onClicked: fileDialog.open()
    }

    FileDialog {
        id: fileDialog

        title: qsTr("Choose an app")

        onAccepted: {
            const path = decodeURIComponent(fileDialog.selectedFile.toString().replace(/^file:\/\//, ""));
            const target = backend.browseTarget(path);
            if (target === "") {
                return;
            }
            if (chooser.multiple) {
                chooser.chosenTargets([JSON.parse(target)]);
            } else {
                chooser.chosenTarget(JSON.parse(target));
            }
            chooser.close();
        }
    }

    Kirigami.SearchField {
        id: search

        Layout.fillWidth: true
        Layout.margins: Kirigami.Units.largeSpacing
        placeholderText: qsTr("Search apps")
        Accessible.name: qsTr("Search apps")

        // Down from the search field goes into the list.
        Keys.onDownPressed: {
            list.forceActiveFocus();
            list.currentIndex = chooser.rows.findIndex(row => row.kind === "app");
        }

        onAccepted: {
            if (!chooser.multiple && chooser.firstApp() !== "") {
                chooser.choose(chooser.firstApp());
            }
        }
        onTextChanged: backend.filter(search.text, chooser.showRecent)
    }

    QQC2.Label {
        Layout.fillWidth: true
        Layout.margins: Kirigami.Units.largeSpacing
        color: Kirigami.Theme.negativeTextColor
        text: backend.error
        visible: backend.error !== ""
        wrapMode: Text.WordWrap
    }

    ListView {
        id: list

        Layout.fillWidth: true
        // As tall as the sheet allows next to its search field and buttons, so only the list scrolls, not the sheet too.
        Layout.preferredHeight: Math.max(Kirigami.Units.gridUnit * 6, Math.min(Kirigami.Units.gridUnit * 20, chooser.available - Kirigami.Units.gridUnit * 9))
        Layout.bottomMargin: Kirigami.Units.smallSpacing
        clip: true
        keyNavigationEnabled: true
        model: chooser.rows

        // Enter or Space on the current row does what a click does.
        Keys.onReturnPressed: (list.currentItem as QQC2.ItemDelegate)?.click()
        Keys.onSpacePressed: (list.currentItem as QQC2.ItemDelegate)?.click()

        QQC2.ScrollBar.vertical: QQC2.ScrollBar {
            id: listScroll
        }

        delegate: QQC2.ItemDelegate {
            id: entry

            required property var modelData

            readonly property bool isHeader: modelData.kind === "header"

            width: ListView.view.width
            enabled: !isHeader
            hoverEnabled: !isHeader
            leftPadding: Kirigami.Units.largeSpacing
            // Clear of the scroll bar, so the packaging badge is never under it.
            rightPadding: Kirigami.Units.largeSpacing + (listScroll.visible ? listScroll.width : 0)
            topPadding: isHeader ? Kirigami.Units.largeSpacing : Kirigami.Units.smallSpacing
            bottomPadding: Kirigami.Units.smallSpacing
            Accessible.name: modelData.label
            highlighted: ListView.isCurrentItem && list.activeFocus

            onClicked: {
                if (chooser.multiple) {
                    chooser.toggle(modelData.id, chooser.selected.indexOf(modelData.id) < 0);
                } else {
                    chooser.choose(modelData.id);
                }
            }

            contentItem: RowLayout {
                spacing: Kirigami.Units.smallSpacing

                // A section title, as Kirigami draws list sections.
                Kirigami.Heading {
                    Layout.fillWidth: true
                    color: Kirigami.Theme.disabledTextColor
                    level: 5
                    text: entry.modelData.label
                    visible: entry.isHeader
                }

                QQC2.CheckBox {
                    checked: chooser.selected.indexOf(entry.modelData.id) >= 0
                    visible: !entry.isHeader && chooser.multiple
                    onToggled: chooser.toggle(entry.modelData.id, checked)
                }

                Kirigami.Icon {
                    Layout.preferredHeight: Kirigami.Units.iconSizes.smallMedium
                    Layout.preferredWidth: Kirigami.Units.iconSizes.smallMedium
                    fallback: "application-x-executable"
                    source: entry.modelData.icon !== "" ? entry.modelData.icon : "application-x-executable"
                    visible: !entry.isHeader
                }

                QQC2.Label {
                    Layout.fillWidth: true
                    elide: Text.ElideRight
                    text: entry.modelData.label
                    visible: !entry.isHeader
                }

                QQC2.Label {
                    color: Kirigami.Theme.disabledTextColor
                    font: Kirigami.Theme.smallFont
                    text: entry.modelData.packaging
                    visible: !entry.isHeader && entry.modelData.packaging !== ""
                }
            }
        }
    }
}
