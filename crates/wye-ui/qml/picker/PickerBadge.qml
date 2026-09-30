// A profile badge (PICK-06, DISC-08): the profile's picture in a circle,
// or its initial on its colour. Hidden when there is neither.
import QtQuick
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

Item {
    id: badge

    property int size: Kirigami.Units.iconSizes.small
    // An absolute path or URL of the picture.
    property string image
    property string initial
    // `#rrggbb`.
    property string tint

    width: size
    height: size
    visible: image !== "" || initial !== ""

    Rectangle {
        anchors.fill: parent
        radius: width / 2
        visible: badge.image === ""
        color: badge.tint !== "" ? badge.tint : Kirigami.Theme.highlightColor
        border.width: 1
        border.color: Kirigami.Theme.backgroundColor

        QQC2.Label {
            anchors.centerIn: parent
            text: badge.initial
            color: "white"
            font.bold: true
            font.pixelSize: Math.round(badge.size * 0.55)
        }
    }

    Kirigami.ShadowedImage {
        anchors.fill: parent
        visible: badge.image !== ""
        source: badge.image.startsWith("/") ? "file://" + badge.image : badge.image
        radius: width / 2
        fillMode: Image.PreserveAspectCrop
        border.width: 1
        border.color: Kirigami.Theme.backgroundColor
    }
}
