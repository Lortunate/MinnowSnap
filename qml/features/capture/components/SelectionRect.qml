import QtQuick
import com.lortunate.minnow

Rectangle {
    id: root

    property bool bindToRect: false
    property bool enableHandles: true
    property var overlayWindow: null
    property rect rectProperty: Qt.rect(0, 0, 0, 0)

    border.color: overlayWindow ? overlayWindow.selectionColor : AppTheme.primary
    border.width: bindToRect ? 3 : 2
    color: bindToRect ? "transparent" : (overlayWindow ? overlayWindow.selectionFillColor : AppTheme.selectionFill)
    radius: AppTheme.radiusLarge
    visible: false

    Behavior on border.width {
        NumberAnimation {
            duration: AppTheme.durationNormal
        }
    }
    Behavior on opacity {
        NumberAnimation {
            duration: AppTheme.durationSlow
        }
    }

    // Inner highlight for depth effect
    Rectangle {
        anchors.fill: parent
        anchors.margins: parent.border.width + 1
        border.color: Qt.lighter(overlayWindow ? overlayWindow.selectionColor : AppTheme.primary, 1.4)
        border.width: 1
        color: "transparent"
        radius: parent.radius - parent.border.width / 2
        visible: bindToRect

        Behavior on opacity {
            NumberAnimation {
                duration: AppTheme.durationNormal
            }
        }
    }
    Binding {
        property: "x"
        target: root
        value: root.rectProperty.x
        when: root.bindToRect
    }
    Binding {
        property: "y"
        target: root
        value: root.rectProperty.y
        when: root.bindToRect
    }
    Binding {
        property: "width"
        target: root
        value: root.rectProperty.width
        when: root.bindToRect
    }
    Binding {
        property: "height"
        target: root
        value: root.rectProperty.height
        when: root.bindToRect
    }
    ResizeHandle {
        corner: "top-left"
        cursorShape: Qt.SizeFDiagCursor
        overlayWindow: root.overlayWindow
        visible: root.bindToRect && root.enableHandles
        x: -10
        y: -10
    }
    ResizeHandle {
        corner: "top-right"
        cursorShape: Qt.SizeBDiagCursor
        overlayWindow: root.overlayWindow
        visible: root.bindToRect && root.enableHandles
        x: parent.width - 10
        y: -10
    }
    ResizeHandle {
        corner: "bottom-left"
        cursorShape: Qt.SizeBDiagCursor
        overlayWindow: root.overlayWindow
        visible: root.bindToRect && root.enableHandles
        x: -10
        y: parent.height - 10
    }
    ResizeHandle {
        corner: "bottom-right"
        cursorShape: Qt.SizeFDiagCursor
        overlayWindow: root.overlayWindow
        visible: root.bindToRect && root.enableHandles
        x: parent.width - 10
        y: parent.height - 10
    }
    ResizeHandle {
        corner: "left"
        cursorShape: Qt.SizeHorCursor
        height: parent.height - 20
        overlayWindow: root.overlayWindow
        visible: root.bindToRect && root.enableHandles
        width: 20
        x: -10
        y: 10
    }
    ResizeHandle {
        corner: "right"
        cursorShape: Qt.SizeHorCursor
        height: parent.height - 20
        overlayWindow: root.overlayWindow
        visible: root.bindToRect && root.enableHandles
        width: 20
        x: parent.width - 10
        y: 10
    }
    ResizeHandle {
        corner: "top"
        cursorShape: Qt.SizeVerCursor
        height: 20
        overlayWindow: root.overlayWindow
        visible: root.bindToRect && root.enableHandles
        width: parent.width - 20
        x: 10
        y: -10
    }
    ResizeHandle {
        corner: "bottom"
        cursorShape: Qt.SizeVerCursor
        height: 20
        overlayWindow: root.overlayWindow
        visible: root.bindToRect && root.enableHandles
        width: parent.width - 20
        x: 10
        y: parent.height - 10
    }
}
