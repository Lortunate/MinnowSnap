import QtQuick
import QtQuick.Controls
import com.lortunate.minnow

AbstractButton {
    id: control

    property string iconSource: ""
    property bool showIcon: iconSource !== ""

    implicitHeight: AppTheme.buttonHeight
    implicitWidth: 120

    hoverEnabled: true

    background: Rectangle {
        implicitHeight: AppTheme.buttonHeight
        color: control.down ? AppTheme.itemPress : (control.hovered ? AppTheme.itemHover : "transparent")
        radius: AppTheme.radiusSmall

        Behavior on color {
            ColorAnimation {
                duration: AppTheme.durationFast
            }
        }
    }

    contentItem: Row {
        spacing: AppTheme.spacingSmall
        leftPadding: 12
        rightPadding: AppTheme.spacingSmall

        Image {
            visible: control.showIcon
            source: control.iconSource
            width: AppTheme.spacingMedium
            height: AppTheme.spacingMedium
            sourceSize.width: AppTheme.spacingMedium
            sourceSize.height: AppTheme.spacingMedium
            fillMode: Image.PreserveAspectFit
            anchors.verticalCenter: parent.verticalCenter
        }

        Text {
            text: control.text
            color: AppTheme.text
            font.family: AppTheme.fontFamily
            font.pixelSize: AppTheme.fontSizeBody
            verticalAlignment: Text.AlignVCenter
            elide: Text.ElideRight
            anchors.verticalCenter: parent.verticalCenter
        }
    }
}
