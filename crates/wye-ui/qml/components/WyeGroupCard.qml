// WyeGroupCard (BLK-01): a rounded card of rows, a step lighter than the window, with an optional bold title above.
//
// API
//   title: string          the group title ("Startup"); empty for none
//   default property       the rows: WyeRow and its variants (WyeSwitchRow, WyeTargetRow, …). Each row draws the inset
//                          hairline above itself, except the first.
//
//   WyeGroupCard {
//       title: qsTr("Startup")
//       WyeSwitchRow { title: qsTr("Launch at login"); path: "general.launch-at-login"; isOn: page.value(path, true) }
//   }
pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.kde.kirigamiaddons.formcard as FormCard

ColumnLayout {
    id: group

    property string title
    default property alias rows: card.delegates

    Layout.fillWidth: true
    // An untitled card keeps some of the room a title would take, so cards are evenly spaced down every page (BLK-01).
    Layout.topMargin: title === "" ? Kirigami.Units.largeSpacing : 0
    spacing: 0

    FormCard.FormHeader {
        title: group.title
        visible: group.title !== ""
        maximumWidth: card.maximumWidth
    }

    FormCard.FormCard {
        id: card

        // Narrower than the group, so the card is rounded and inset from the window's edges.
        maximumWidth: group.width - Kirigami.Units.largeSpacing * 2
    }
}
