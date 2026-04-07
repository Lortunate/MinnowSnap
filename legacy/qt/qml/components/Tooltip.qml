import QtQuick
import QtQuick.Controls
import com.lortunate.minnow

ToolTip {
    id: control

    property int position: Qt.AlignTop
    property int showDelay: 300

    delay: showDelay
    timeout: 3000

    y: {
        if (position === Qt.AlignTop) {
            return parent.hovered ? -32 : -25;
        } else if (position === Qt.AlignBottom) {
            return parent.height + 8;
        }
        return -32;
    }

    background: Rectangle {
        border.color: AppTheme.border
        border.width: 1
        color: AppTheme.surface
        radius: AppTheme.radiusSmall
    }

    contentItem: Text {
        color: AppTheme.text
        font.family: AppTheme.fontFamily
        font.pixelSize: AppTheme.fontSizeSmall
        text: control.text
    }

    Behavior on y {
        NumberAnimation {
            duration: AppTheme.durationNormal
            easing.type: Easing.OutCubic
        }
    }

    enter: Transition {
        NumberAnimation {
            property: "opacity"
            from: 0.0
            to: 1.0
            duration: AppTheme.durationFast
        }
    }

    exit: Transition {
        NumberAnimation {
            property: "opacity"
            from: 1.0
            to: 0.0
            duration: AppTheme.durationFast
        }
    }
}
