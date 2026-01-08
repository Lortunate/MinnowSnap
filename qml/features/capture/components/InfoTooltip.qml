import QtQuick
import com.lortunate.minnow

Rectangle {
    id: root

    property alias text: label.text

    border.color: AppTheme.border
    border.width: 1
    color: AppTheme.surface
    height: label.contentHeight + AppTheme.tooltipPadding
    opacity: visible ? 1.0 : 0.0
    radius: AppTheme.radiusSmall
    visible: text !== ""
    width: label.contentWidth + 16

    Behavior on opacity {
        NumberAnimation {
            duration: AppTheme.durationNormal
        }
    }

    Text {
        id: label

        anchors.centerIn: parent
        color: AppTheme.text
        font.family: AppTheme.fontFamily
        font.pixelSize: AppTheme.fontSizeSmall
    }
}
