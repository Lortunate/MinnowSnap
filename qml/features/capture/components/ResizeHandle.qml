import QtQuick
import com.lortunate.minnow

MouseArea {
    id: root

    property string corner: ""
    property var overlayWindow: null

    height: 20
    scale: pressed ? 0.9 : 1.0
    width: 20
    z: 1

    Behavior on scale {
        NumberAnimation {
            duration: AppTheme.durationFast
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
        border.color: Qt.rgba((overlayWindow ? overlayWindow.selectionColor : AppTheme.primary).r, (overlayWindow ? overlayWindow.selectionColor : AppTheme.primary).g, (overlayWindow ? overlayWindow.selectionColor : AppTheme.primary).b, 0.3)
        border.width: 2
        color: "transparent"
        height: 20
        radius: 10
        width: 20
    }
    Rectangle {
        anchors.centerIn: parent
        border.color: overlayWindow ? overlayWindow.selectionColor : AppTheme.primary
        border.width: 2
        color: "white"
        height: 14
        radius: 7
        width: 14

        Rectangle {
            anchors.centerIn: parent
            color: Qt.lighter(overlayWindow ? overlayWindow.selectionColor : AppTheme.primary, 1.4)
            height: 6
            radius: 3
            width: 6
        }
    }
}
