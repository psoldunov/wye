// RuleEditorSheet (RUL-10 to RUL-28): the rule editor. A new rule starts from RulesBackend.newDraft (PICK-31 prefills a
// Domain matcher and the source app); an existing one from RulesBackend.draftFor. Save stays disabled until
// RulesBackend.check says the rule is valid (RUL-18).
//
// API
//   editable: bool                         false for a read-only configuration: nothing can be saved or deleted
//   openNew(string argument)               a new rule; `argument` is the rule-editor ShowWindow argument (may be empty)
//   openRule(int index)                    edit rule `index`
//   saveRequested(int index, var rule)     Save: `index` is -1 for a new rule (RUL-20)
//   deleteRequested(int index)             Delete Rule (RUL-28)
//   testRequested(string url)              Test… (RUL-19): open the tester with this link
pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import dev.soldunov.wye.ui

WyeSheet {
    id: sheet

    property bool editable: true
    property int ruleIndex: -1
    // The draft's fields; the matchers live in `matchers` so their rows keep focus while typing.
    property string ruleId
    property bool ruleEnabled: true
    property string name
    property var target: ({
            "default": true
        })
    property var sources: []
    property var held: []
    property bool openInBackground: false
    property bool newWindow: false
    property string run: "before"
    property bool transform: false
    // Bumped on every change, so `checked` is worked out again.
    property int revision: 0

    readonly property var checked: sheet.revision >= 0 ? JSON.parse(RulesBackend.check(JSON.stringify(sheet.compose()), SettingsBackend.configJson)) : ({})
    readonly property bool targetIsDefault: sheet.target["default"] === true || sheet.target["picker"] === true
    readonly property bool helpSeen: SettingsBackend.statusJson !== "" && (JSON.parse(SettingsBackend.statusJson).uiState?.helpArrowSeen ?? false)

    signal saveRequested(int index, var rule)
    signal deleteRequested(int index)
    signal testRequested(string url)

    function compose() {
        const matcherList = [];
        for (let i = 0; i < matchers.count; ++i) {
            const matcher = matchers.get(i);
            matcherList.push({
                "kind": matcher.kind,
                "pattern": matcher.pattern
            });
        }
        return {
            "id": sheet.ruleId,
            "name": sheet.name,
            "enabled": sheet.ruleEnabled,
            "target": sheet.target,
            "url-matchers": matcherList,
            "source-apps": sheet.sources,
            "held-keys": sheet.held,
            "open-in-background": sheet.openInBackground,
            "force-new-window": sheet.newWindow,
            "run": sheet.run,
            "transform": sheet.transform
        };
    }

    function load(draft) {
        ruleId = draft.id ?? "";
        ruleEnabled = draft.enabled ?? true;
        name = draft.name ?? "";
        target = draft.target ?? ({
                "default": true
            });
        sources = draft["source-apps"] ?? [];
        held = draft["held-keys"] ?? [];
        openInBackground = draft["open-in-background"] ?? false;
        newWindow = draft["force-new-window"] ?? false;
        run = draft.run ?? "before";
        transform = draft.transform ?? false;
        matchers.clear();
        (draft["url-matchers"] ?? []).forEach(matcher => matchers.append({
                "kind": matcher.kind ?? "domain",
                "pattern": matcher.pattern ?? ""
            }));
        nameField.text = name;
        changed();
    }

    function openNew(argument: string) {
        ruleIndex = -1;
        load(JSON.parse(RulesBackend.newDraft(SettingsBackend.configJson, argument)));
        open();
        nameField.forceActiveFocus();
    }

    function openRule(index: int) {
        const text = RulesBackend.draftFor(SettingsBackend.configJson, index);
        if (text === "") {
            return;
        }
        ruleIndex = index;
        load(JSON.parse(text));
        open();
        nameField.forceActiveFocus();
    }

    function changed() {
        revision += 1;
    }

    function addSources(targets) {
        const ids = targets.map(target => Object.values(target)[0]).filter(id => typeof id === "string" && sources.indexOf(id) < 0);
        sources = sources.concat(ids);
        changed();
    }

    title: ruleIndex < 0 ? qsTr("New Rule") : qsTr("Edit Rule")
    sheetWidth: Kirigami.Units.gridUnit * 27
    note: qsTr("If you specify both types, at least one of the URL matchers AND one of the source apps must match.")
    primaryText: qsTr("Save")
    primaryEnabled: sheet.editable && (sheet.checked.valid ?? false)
    secondaryText: qsTr("Cancel")

    onPrimaryTriggered: {
        sheet.saveRequested(sheet.ruleIndex, sheet.compose());
        sheet.close();
    }
    onSecondaryTriggered: sheet.close()

    // RUL-19: help, the first-use arrow pointing at it, and Test….
    footerLeading: RowLayout {
        QQC2.ToolButton {
            display: QQC2.AbstractButton.IconOnly
            icon.name: "help-contextual"
            text: qsTr("How Rules Work")
            QQC2.ToolTip.text: text
            QQC2.ToolTip.visible: hovered
            onClicked: {
                helpDialog.open();
                if (!sheet.helpSeen) {
                    SettingsBackend.updateUiState(JSON.stringify({
                        "helpArrowSeen": true
                    }));
                }
            }
        }
        Kirigami.Icon {
            visible: !sheet.helpSeen
            Layout.preferredHeight: Kirigami.Units.iconSizes.small
            Layout.preferredWidth: Kirigami.Units.iconSizes.small
            color: Kirigami.Theme.neutralTextColor
            isMask: true
            source: "arrow-left"
        }
        QQC2.Button {
            text: qsTr("Test…")
            onClicked: sheet.testRequested(RulesBackend.testUrl(JSON.stringify(sheet.compose())))
        }
    }

    ListModel {
        id: matchers
    }

    RulesHelpDialog {
        id: helpDialog
    }

    WyeAppChooser {
        id: chooser

        multiple: true
        showRecent: true
        onChosenTargets: targets => sheet.addSources(targets)
    }

    // RUL-11
    WyeCallout {
        Layout.fillWidth: true
        calloutId: "rules-order"
        text: qsTr("Rules are matched in order from top to bottom of the list.") + "<br><br>" + qsTr("The scheme (<code>https://</code>) and <code>www.</code> are removed from the URL before matching, so you do not need to include those in the “Match” field.") + "<br><br>" + qsTr("Click the (?) button for more info.")
    }

    // RUL-12
    WyeGroupCard {
        Layout.fillWidth: true

        WyeRow {
            title: qsTr("Name")

            QQC2.TextField {
                id: nameField

                Layout.preferredWidth: Kirigami.Units.gridUnit * 12
                horizontalAlignment: TextInput.AlignRight
                placeholderText: qsTr("Required")
                Accessible.name: qsTr("Name")
                onTextEdited: {
                    sheet.name = text;
                    sheet.changed();
                }
            }
        }

        WyeTargetRow {
            title: qsTr("Open in")
            surface: "rule"
            current: sheet.target
            onChosen: chosen => {
                sheet.target = chosen;
                sheet.changed();
            }
        }
    }

    // RUL-13, RUL-14
    WyeSection {
        Layout.fillWidth: true
        title: qsTr("URL Matchers")
        subtitle: qsTr("The link must match one of these patterns")
        emptyText: qsTr("No Matchers")
        addText: qsTr("Add Matcher")
        addEnabled: sheet.editable
        count: matchers.count

        onAddTriggered: {
            matchers.append({
                "kind": "domain",
                "pattern": ""
            });
            sheet.changed();
            Qt.callLater(() => (matcherRepeater.itemAt(matchers.count - 1) as RuleMatcherRow)?.focusEntry());
        }

        Repeater {
            id: matcherRepeater

            model: matchers

            delegate: RuleMatcherRow {
                required property int index

                Layout.fillWidth: true
                editable: sheet.editable
                error: sheet.checked.matcherErrors?.[index] ?? ""

                onEdited: (kind, pattern) => {
                    matchers.set(index, {
                        "kind": kind,
                        "pattern": pattern
                    });
                    sheet.changed();
                }
                onRemoveRequested: {
                    matchers.remove(index);
                    sheet.changed();
                }
            }
        }
    }

    // RUL-16
    WyeSection {
        Layout.fillWidth: true
        title: qsTr("Source Apps")
        subtitle: qsTr("The link must have been clicked in one of these apps")
        emptyText: qsTr("No Source Apps")
        addText: qsTr("Add Source App")
        addEnabled: sheet.editable
        count: sheet.sources.length

        onAddTriggered: chooser.open()

        Repeater {
            model: JSON.parse(RulesBackend.sourceRows(JSON.stringify(sheet.sources), SettingsBackend.offline ? SettingsBackend.fixtureAppsJson : RulesBackend.appsJson))

            delegate: RowLayout {
                id: sourceRow

                required property var modelData

                Layout.fillWidth: true
                Layout.margins: Kirigami.Units.smallSpacing

                Kirigami.Icon {
                    Layout.preferredHeight: Kirigami.Units.iconSizes.small
                    Layout.preferredWidth: Kirigami.Units.iconSizes.small
                    source: sourceRow.modelData.icon !== "" ? sourceRow.modelData.icon : "application-x-executable"
                }
                QQC2.Label {
                    Layout.fillWidth: true
                    elide: Text.ElideRight
                    text: sourceRow.modelData.name
                }
                QQC2.ToolButton {
                    display: QQC2.AbstractButton.IconOnly
                    enabled: sheet.editable
                    icon.name: "list-remove"
                    text: qsTr("Remove Source App")
                    QQC2.ToolTip.text: text
                    QQC2.ToolTip.visible: hovered
                    onClicked: {
                        sheet.sources = sheet.sources.filter(spec => spec !== sourceRow.modelData.spec);
                        sheet.changed();
                    }
                }
            }
        }
    }

    // RUL-21 to RUL-25, RUL-27
    WyeGroupCard {
        Layout.fillWidth: true
        title: qsTr("Advanced")

        WyeSwitchRow {
            title: qsTr("Open in background")
            help: "open-in-background"
            isOn: sheet.openInBackground
            onSwitched: on => {
                sheet.openInBackground = on;
                sheet.changed();
            }
        }

        WyeSwitchRow {
            title: qsTr("Force new window")
            help: "force-new-window"
            dimmed: sheet.targetIsDefault
            isOn: sheet.newWindow
            onSwitched: on => {
                sheet.newWindow = on;
                sheet.changed();
            }
        }

        WyeModifierRow {
            title: qsTr("Only when keys are held")
            modifiers: sheet.held
            availableSubtitle: (sheet.checked.alternativeKeyWins ?? false) ? qsTr("These are the alternative-browser keys, which take precedence over rules.") : ""
            onModified: names => {
                sheet.held = names;
                sheet.changed();
            }
        }

        WyeChoiceRow {
            title: qsTr("Run")
            choices: [
                {
                    "value": "before",
                    "label": qsTr("before built-in rules")
                },
                {
                    "value": "after",
                    "label": qsTr("after built-in rules")
                }
            ]
            currentValue: sheet.run
            onActivated: value => {
                sheet.run = value;
                sheet.changed();
            }
        }

        WyeButtonRow {
            title: qsTr("Transform URL")
            buttonText: qsTr("Edit Script…")
            needsConfig: false
            hasSwitch: true
            isOn: sheet.transform
            onSwitched: on => {
                sheet.transform = on;
                sheet.changed();
                // SCR-09: turning it on for a script that is still empty opens the editor.
                if (on) {
                    SettingsBackend.openScriptIfMissing("rule:" + sheet.ruleId);
                }
            }
            onActivated: SettingsBackend.showWindow("script-editor", JSON.stringify({
                "scope": "rule:" + sheet.ruleId,
                "ruleName": sheet.name
            }))
        }
    }

    // RUL-28
    QQC2.Button {
        Layout.alignment: Qt.AlignRight
        visible: sheet.ruleIndex >= 0
        enabled: sheet.editable
        icon.name: "edit-delete"
        text: qsTr("Delete Rule")
        palette.buttonText: Kirigami.Theme.negativeTextColor
        onClicked: {
            sheet.deleteRequested(sheet.ruleIndex);
            sheet.close();
        }
    }
}
