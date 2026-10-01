// AboutSection (DLG-ABT-01, DLG-ABT-02): one card of the About page with an optional header above it, as WyeGroupCard, plus
// the header's actions (the Troubleshooting section's Copy), which FormCard.FormHeader draws at the header's right. The card
// takes its width from the section, not the other way round (see AboutContent.qml).
//
// API
//   title: string          the header; empty for none
//   actions: list<Action>  buttons at the header's right
//   default property       the card's delegates
pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import QtQuick.Templates as T
import org.kde.kirigami as Kirigami
import org.kde.kirigamiaddons.formcard as FormCard

ColumnLayout {
    id: section

    property string title
    property list<T.Action> actions
    default property alias delegates: card.delegates

    Layout.fillWidth: true
    spacing: 0

    FormCard.FormHeader {
        actions: section.actions
        maximumWidth: card.maximumWidth
        title: section.title
        visible: section.title !== ""
    }

    FormCard.FormCard {
        id: card

        // Narrower than the section, so the card is rounded and inset from the window's edges.
        maximumWidth: section.width - Kirigami.Units.largeSpacing * 2
    }
}
