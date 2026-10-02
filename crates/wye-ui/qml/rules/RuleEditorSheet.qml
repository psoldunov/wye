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
//   showHelp()                             open the rules help (RUL-19)
pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.kde.kirigamiaddons.formcard as FormCard
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
    // RUL-18: the user has changed something since the sheet opened. A new rule is incomplete by nature, so the reason
    // Save is disabled waits for the first edit rather than greeting the user; an existing rule's shows at once.
    property bool touched: false

    readonly property var checked: sheet.revision >= 0 ? JSON.parse(RulesBackend.check(JSON.stringify(sheet.compose()), SettingsBackend.configJson)) : ({})
    readonly property bool targetIsDefault: sheet.target["default"] === true || sheet.target["picker"] === true
    readonly property bool helpSeen: SettingsBackend.statusJson !== "" && (JSON.parse(SettingsBackend.statusJson).uiState?.helpArrowSeen ?? false)
    // RUL-18: why Save is disabled, in words; empty when the rule can be saved. A matcher's own error shows under it.
    readonly property string blocker: {
        const check = sheet.checked;
        if (!sheet.editable || !sheet.touched || (check.valid ?? false)) {
            return "";
        }
        if ((check.error ?? "") !== "") {
            return check.error;
        }
        const parts = [];
        if (check.nameMissing) {
            parts.push(qsTr("Give the rule a name."));
        }
        if (check.noCondition) {
            parts.push(qsTr("Add a URL matcher, a source app or held keys."));
        }
        const errors = check.matcherErrors ?? [];
        const empty = errors.some((error, index) => error !== "" && index < matchers.count && matchers.get(index).pattern === "");
        if (empty) {
            parts.push(qsTr("Fill in or remove the empty URL matcher."));
        } else if (errors.some(error => error !== "")) {
            parts.push(qsTr("Correct the URL matcher marked below."));
        }
        return parts.join(" ");
    }

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
        revision += 1;
    }

    function openNew(argument: string) {
        ruleIndex = -1;
        touched = false;
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
        touched = true;
        load(JSON.parse(text));
        open();
        nameField.forceActiveFocus();
        // A long name shows from its start, not scrolled to the cursor at its end.
        nameField.cursorPosition = 0;
    }

    // Open the rules help (RUL-19) without the button: the self-test's "rules-help" sheet.
    function showHelp() {
        helpDialog.open();
    }

    // The width of the Name field and the "Open in" box (RUL-12).
    readonly property real controlWidth: Kirigami.Units.gridUnit * 16

    // Every edit the user makes comes through here.
    function changed() {
        touched = true;
        revision += 1;
    }

    function addSources(targets) {
        const ids = targets.map(target => Object.values(target)[0]).filter(id => typeof id === "string" && sources.indexOf(id) < 0);
        sources = sources.concat(ids);
        changed();
    }

    title: ruleIndex < 0 ? qsTr("New Rule") : qsTr("Edit Rule")
    // About as large as the settings window (08-rules.md, "Rule editor sheet"); WyeSheet keeps it inside the window.
    sheetWidth: Kirigami.Units.gridUnit * 32
    note: qsTr("If you specify both types, at least one of the URL matchers AND one of the source apps must match.")
    primaryText: qsTr("Save")
    primaryIcon: "document-save"
    primaryEnabled: sheet.editable && (sheet.checked.valid ?? false)
    secondaryText: qsTr("Cancel")

    onPrimaryTriggered: {
        sheet.saveRequested(sheet.ruleIndex, sheet.compose());
        sheet.close();
    }
    onSecondaryTriggered: sheet.close()

    // RUL-19: help, the first-use arrow pointing at it, and Test….
    footerLeading: RowLayout {
        spacing: Kirigami.Units.smallSpacing

        QQC2.ToolButton {
            display: QQC2.AbstractButton.IconOnly
            icon.name: "help-contextual-symbolic"
            text: qsTr("How Rules Work")
            QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
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
            icon.name: "system-run-symbolic"
            text: qsTr("Test…")
            QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
            QQC2.ToolTip.text: qsTr("Open the rule tester with a link this rule's first URL matcher matches")
            QQC2.ToolTip.visible: hovered
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
        Layout.topMargin: Kirigami.Units.largeSpacing
        calloutId: "rules-order"
        text: qsTr("Rules are matched in order from top to bottom of the list.") + "<br><br>" + qsTr("The scheme (<code>https://</code>) and <code>www.</code> are removed from the URL before matching, so you do not need to include those in the “Match” field.") + "<br><br>" + qsTr("Click the (?) button for more info.")
    }

    // RUL-18: what is missing before Save works.
    Kirigami.InlineMessage {
        Layout.fillWidth: true
        Layout.topMargin: Kirigami.Units.smallSpacing
        Layout.leftMargin: Kirigami.Units.largeSpacing
        Layout.rightMargin: Kirigami.Units.largeSpacing
        type: Kirigami.MessageType.Information
        visible: sheet.blocker !== ""
        text: sheet.blocker
    }

    // RUL-12
    WyeGroupCard {
        Layout.fillWidth: true

        WyeRow {
            title: qsTr("Name")

            QQC2.TextField {
                id: nameField

                // The same width as the "Open in" box below, so the two trailing controls line up.
                Layout.preferredWidth: sheet.controlWidth
                placeholderText: qsTr("Required")
                Accessible.name: qsTr("Name")
                onTextEdited: {
                    sheet.name = text;
                    sheet.changed();
                }
            }
        }

        WyeTargetRow {
            controlWidth: sheet.controlWidth
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

            // A card row like WyeRow: its padding, the inset hairline above every row but the first, no hover.
            delegate: FormCard.AbstractFormDelegate {
                id: sourceRow

                required property var modelData

                hoverEnabled: false
                focusPolicy: Qt.NoFocus
                background: Item {}
                Accessible.name: sourceRow.modelData.name

                Kirigami.Separator {
                    anchors {
                        top: parent.top
                        left: parent.left
                        right: parent.right
                        leftMargin: sourceRow.leftPadding
                        rightMargin: sourceRow.rightPadding
                    }
                    visible: sourceRow.y > 0
                }

                contentItem: RowLayout {
                    spacing: Kirigami.Units.largeSpacing

                    Kirigami.Icon {
                        Layout.preferredHeight: Kirigami.Units.iconSizes.smallMedium
                        Layout.preferredWidth: Kirigami.Units.iconSizes.smallMedium
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
                        icon.name: "list-remove-symbolic"
                        text: qsTr("Remove Source App")
                        Accessible.name: qsTr("Remove “%1”").arg(sourceRow.modelData.name)
                        QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
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
                    SettingsBackend.openScriptIfMissing("rule:" + sheet.ruleId, sheet.name);
                }
            }
            onActivated: SettingsBackend.showWindow("script-editor", JSON.stringify({
                "scope": "rule:" + sheet.ruleId,
                "ruleName": sheet.name
            }))
        }
    }

    // RUL-28, at the end of the body, lined up with the cards' right edge.
    QQC2.Button {
        id: deleteRuleButton

        Layout.alignment: Qt.AlignRight
        Layout.topMargin: Kirigami.Units.largeSpacing
        Layout.rightMargin: Kirigami.Units.largeSpacing
        visible: sheet.ruleIndex >= 0
        enabled: sheet.editable
        Accessible.name: qsTr("Delete Rule")
        leftPadding: Kirigami.Units.largeSpacing
        rightPadding: Kirigami.Units.largeSpacing
        // Breeze draws a button's text from the style, not from `palette.buttonText`, so the content is drawn here (and the
        // button has no `text` or `icon` of its own, which the style would draw as well).
        contentItem: RowLayout {
            spacing: Kirigami.Units.smallSpacing

            Kirigami.Icon {
                Layout.preferredHeight: Kirigami.Units.iconSizes.small
                Layout.preferredWidth: Kirigami.Units.iconSizes.small
                color: Kirigami.Theme.negativeTextColor
                isMask: true
                opacity: deleteRuleButton.enabled ? 1 : 0.5
                source: "edit-delete-symbolic"
            }
            QQC2.Label {
                color: Kirigami.Theme.negativeTextColor
                opacity: deleteRuleButton.enabled ? 1 : 0.5
                text: qsTr("Delete Rule")
            }
        }
        onClicked: {
            sheet.deleteRequested(sheet.ruleIndex);
            sheet.close();
        }
    }

    // The same room below the last item as above the first.
    Item {
        implicitHeight: Kirigami.Units.largeSpacing
    }
}
