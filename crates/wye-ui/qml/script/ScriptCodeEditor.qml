// The code area of the script editor (SCR-02, SCR-05): monospace text with
// JavaScript highlighting (KSyntaxHighlighting), line numbers, auto-indent,
// bracket matching, undo/redo (TextArea's own), and the error line marked.
// It is drawn as a text field is: the View colours, a rounded frame that
// turns to the focus colour while the text has focus, and a gutter in the
// same colour as the text, set off by a hairline.
pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import org.kde.syntaxhighlighting
import dev.soldunov.wye.ui

QQC2.Frame {
    id: root

    // The backend that knows brackets and indentation.
    required property ScriptEditorBackend backend
    // The line to mark, counted from 1; 0 for none.
    property int errorLine: 0
    readonly property alias text: area.text
    readonly property alias canUndo: area.canUndo
    readonly property alias canRedo: area.canRedo

    // The user changed the text.
    signal edited(string text)

    // Replace the text (a load); not reported as an edit.
    function load(text: string) {
        loading = true;
        area.text = text;
        area.cursorPosition = 0;
        loading = false;
    }

    // Replace the text as the user would; reported as an edit (the self-test's unsaved state).
    function type(text: string) {
        area.text = text;
    }

    function undo() {
        area.undo();
        area.forceActiveFocus();
    }

    function redo() {
        area.redo();
        area.forceActiveFocus();
    }

    property bool loading: false
    readonly property real lineHeight: area.lineCount > 0 ? area.contentHeight / area.lineCount : metrics.height
    readonly property real gutterWidth: metrics.advanceWidth * Math.max(2, String(area.lineCount).length) + Kirigami.Units.largeSpacing * 2
    // The line the cursor is on, counted from 1.
    readonly property int cursorLine: Math.floor(area.positionToRectangle(area.cursorPosition).y / Math.max(1, lineHeight)) + 1

    Kirigami.Theme.colorSet: Kirigami.Theme.View
    Kirigami.Theme.inherit: false

    background: Rectangle {
        color: Kirigami.Theme.backgroundColor
        radius: Kirigami.Units.cornerRadius
        border.width: 1
        border.color: area.activeFocus ? Kirigami.Theme.focusColor : Kirigami.ColorUtils.linearInterpolation(Kirigami.Theme.backgroundColor, Kirigami.Theme.textColor, Kirigami.Theme.frameContrast)
    }

    // UTF-16 offset of the start of `line` (from 1) in `text`.
    function lineStart(text: string, line: int): int {
        let position = 0;
        for (let current = 1; current < line; ++current) {
            const next = text.indexOf("\n", position);
            if (next < 0) {
                return position;
            }
            position = next + 1;
        }
        return position;
    }

    // Inside the frame's border.
    padding: 1

    TextMetrics {
        id: metrics
        font: Kirigami.Theme.fixedWidthFont
        text: "0"
    }

    Flickable {
        id: flick
        anchors.fill: parent
        clip: true

        QQC2.ScrollBar.vertical: QQC2.ScrollBar {}
        QQC2.ScrollBar.horizontal: QQC2.ScrollBar {}

        QQC2.TextArea.flickable: QQC2.TextArea {
            id: area

            // The bracket paired with the one at the cursor, or -1.
            readonly property int partner: root.backend.matchingBracket(text, cursorPosition)
            // The bracket the partner belongs to: at the cursor, else just
            // before it (the backend's rule).
            readonly property int bracket: partner < 0 ? -1 : ("()[]{}".indexOf(text.charAt(cursorPosition)) >= 0 ? cursorPosition : cursorPosition - 1)

            font: Kirigami.Theme.fixedWidthFont
            textFormat: TextEdit.PlainText
            wrapMode: TextEdit.NoWrap
            leftPadding: root.gutterWidth + Kirigami.Units.smallSpacing
            persistentSelection: true
            selectByMouse: true
            background: null
            Accessible.name: qsTr("Script")

            onTextChanged: {
                if (!root.loading) {
                    root.edited(text);
                }
            }

            Keys.onReturnPressed: event => {
                area.remove(area.selectionStart, area.selectionEnd);
                area.insert(area.cursorPosition, root.backend.newlineAt(area.text, area.cursorPosition));
                event.accepted = true;
            }
            Keys.onEnterPressed: event => {
                area.remove(area.selectionStart, area.selectionEnd);
                area.insert(area.cursorPosition, root.backend.newlineAt(area.text, area.cursorPosition));
                event.accepted = true;
            }
            Keys.onTabPressed: event => {
                area.remove(area.selectionStart, area.selectionEnd);
                area.insert(area.cursorPosition, root.backend.indentText());
                event.accepted = true;
            }

            // SCR-05: the reported line, tinted and underlined.
            Rectangle {
                readonly property rect at: area.positionToRectangle(root.lineStart(area.text, root.errorLine))

                visible: root.errorLine > 0
                x: area.leftPadding
                y: at.y
                width: Math.max(area.width, flick.width) - area.leftPadding
                height: root.lineHeight
                color: Qt.alpha(Kirigami.Theme.negativeTextColor, 0.12)

                Rectangle {
                    anchors.bottom: parent.bottom
                    width: parent.width
                    height: 2
                    color: Kirigami.Theme.negativeTextColor
                }
            }

            // SCR-02: the bracket at the cursor and its partner.
            Repeater {
                model: area.partner >= 0 ? [area.bracket, area.partner] : []

                delegate: Rectangle {
                    required property int modelData
                    readonly property rect box: area.positionToRectangle(modelData)

                    x: box.x
                    y: box.y
                    width: metrics.advanceWidth
                    height: box.height
                    color: "transparent"
                    border.color: Kirigami.Theme.highlightColor
                    radius: 2
                }
            }
        }
    }

    // Line numbers, scrolled with the text: dimmed, the cursor's line in the
    // text colour, the error line in the negative colour.
    Item {
        x: 0
        y: 0
        width: root.gutterWidth
        height: flick.height
        clip: true

        Column {
            y: area.topPadding - flick.contentY

            Repeater {
                model: area.lineCount

                delegate: QQC2.Label {
                    required property int index

                    width: root.gutterWidth - Kirigami.Units.largeSpacing
                    height: root.lineHeight
                    horizontalAlignment: Text.AlignRight
                    verticalAlignment: Text.AlignVCenter
                    font: Kirigami.Theme.fixedWidthFont
                    text: index + 1
                    color: index + 1 === root.errorLine ? Kirigami.Theme.negativeTextColor : index + 1 === root.cursorLine && area.activeFocus ? Kirigami.Theme.textColor : Kirigami.Theme.disabledTextColor
                }
            }
        }

        Kirigami.Separator {
            anchors.right: parent.right
            anchors.top: parent.top
            anchors.bottom: parent.bottom
        }
    }

    SyntaxHighlighter {
        textEdit: area
        definition: "JavaScript"
        theme: Kirigami.Theme.backgroundColor.hslLightness < 0.5 ? Repository.defaultTheme(Repository.DarkTheme) : Repository.defaultTheme(Repository.LightTheme)
    }
}
