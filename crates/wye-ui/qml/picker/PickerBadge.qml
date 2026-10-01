// A profile badge (PICK-06, DISC-08): the profile's picture in a circle,
// or its initial on its colour. Hidden when there is neither. A ring in the
// panel's colour cuts it out of the icon under it.
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
    readonly property color fill: tint !== "" ? tint : Kirigami.Theme.highlightColor
    readonly property int ring: Math.max(1, Math.round(size / 16))
    // Legible on any profile colour: the theme's darker of text and background
    // on a light fill, its lighter on a dark one.
    readonly property color initialColor: {
        const text = Kirigami.Theme.textColor;
        const background = Kirigami.Theme.backgroundColor;
        const textIsDark = Kirigami.ColorUtils.grayForColor(text) < Kirigami.ColorUtils.grayForColor(background);
        const wantDark = Kirigami.ColorUtils.brightnessForColor(fill) === Kirigami.ColorUtils.Light;
        return wantDark === textIsDark ? text : background;
    }

    width: size
    height: size
    visible: image !== "" || initial !== ""

    Rectangle {
        anchors.fill: parent
        radius: width / 2
        visible: badge.image === ""
        color: badge.fill
        border.width: badge.ring
        border.color: Kirigami.Theme.backgroundColor

        QQC2.Label {
            anchors.centerIn: parent
            text: badge.initial
            color: badge.initialColor
            font.weight: Font.Bold
            font.pixelSize: Math.round(badge.size * 0.5)
        }
    }

    Kirigami.ShadowedImage {
        anchors.fill: parent
        visible: badge.image !== ""
        source: badge.image.startsWith("/") ? "file://" + badge.image : badge.image
        radius: width / 2
        fillMode: Image.PreserveAspectCrop
        sourceSize.width: badge.size * 2
        sourceSize.height: badge.size * 2
        border.width: badge.ring
        border.color: Kirigami.Theme.backgroundColor
    }
}
