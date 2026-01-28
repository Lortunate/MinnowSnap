import QtQuick
import QtQuick.Effects
import com.lortunate.minnow

MouseArea {
    id: root

    property string corner: ""
    property var overlayWindow: null

    height: 20
    scale: pressed ? 1.1 : 1.0
    width: 20
    z: 1

    Behavior on scale {
        NumberAnimation {
            duration: AppTheme.durationFast
            easing.type: Easing.OutCubic
        }
    }

    onPositionChanged: function (mouse) {
        if (pressed && overlayWindow) {
            var pt = mapToItem(overlayWindow.contentItem, mouse.x, mouse.y);
            overlayWindow.updateResize(pt.x, pt.y);
        }
    }
    onPressed: function (mouse) {
        mouse.accepted = true;
        if (overlayWindow) {
            var pt = mapToItem(overlayWindow.contentItem, mouse.x, mouse.y);
            overlayWindow.startResize(root.corner, pt.x, pt.y);
        }
    }
    onReleased: {
        if (overlayWindow) {
            overlayWindow.endResize();
        }
    }

    Rectangle {
        anchors.centerIn: parent
        width: 20
        height: 20
        radius: 10
        color: "transparent"
        border.color: Qt.rgba((root.overlayWindow ? root.overlayWindow.selectionColor : AppTheme.primary).r, (root.overlayWindow ? root.overlayWindow.selectionColor : AppTheme.primary).g, (root.overlayWindow ? root.overlayWindow.selectionColor : AppTheme.primary).b, 0.15)
        border.width: 1
        visible: root.containsMouse

        Behavior on border.color {
            ColorAnimation {
                duration: AppTheme.durationNormal
            }
        }
    }

    Rectangle {
        anchors.centerIn: parent
        width: 12
        height: 12
        radius: 6
        color: "#FFFFFF"
        border.color: root.overlayWindow ? root.overlayWindow.selectionColor : AppTheme.primary
        border.width: 1.5

        Behavior on border.color {
            ColorAnimation {
                duration: AppTheme.durationNormal
            }
        }

        Rectangle {
            anchors.centerIn: parent
            width: 4
            height: 4
            radius: 2
            color: root.overlayWindow ? root.overlayWindow.selectionColor : AppTheme.primary
        }

        layer.enabled: root.pressed
        layer.effect: MultiEffect {
            shadowColor: Qt.rgba(0, 0, 0, 0.3)
            shadowHorizontalOffset: 0
            shadowVerticalOffset: 2
            shadowBlur: 4
        }
    }
}
