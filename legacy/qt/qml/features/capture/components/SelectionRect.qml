import QtQuick
import com.lortunate.minnow

Rectangle {
    id: root

    property bool bindToRect: false
    property bool enableHandles: true
    property var overlayWindow: null
    property rect rectProperty: Qt.rect(0, 0, 0, 0)

    readonly property int handleSize: 20
    readonly property int handleOffset: 10
    readonly property int innerBorderMargin: 1
    readonly property int outerBorderMargin: 2
    readonly property real innerBorderWidth: 0.5
    readonly property real outerBorderWidth: 0.3
    readonly property real lightnerRatio1: 1.6
    readonly property real lightnerRatio2: 2.0
    readonly property real innerBorderOpacity: 0.6
    readonly property real outerBorderOpacity: 0.3

    border.color: overlayWindow ? overlayWindow.selectionColor : AppTheme.primary
    border.width: 2
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
    Behavior on border.color {
        ColorAnimation {
            duration: AppTheme.durationNormal
        }
    }

    Rectangle {
        anchors.fill: parent
        anchors.margins: root.innerBorderMargin
        border.color: Qt.lighter(root.overlayWindow ? root.overlayWindow.selectionColor : AppTheme.primary, root.lightnerRatio1)
        border.width: root.innerBorderWidth
        color: "transparent"
        radius: root.radius - root.innerBorderMargin
        visible: root.bindToRect
        opacity: root.innerBorderOpacity
    }

    Rectangle {
        anchors.fill: parent
        anchors.margins: root.outerBorderMargin
        border.color: Qt.lighter(root.overlayWindow ? root.overlayWindow.selectionColor : AppTheme.primary, root.lightnerRatio2)
        border.width: root.outerBorderWidth
        color: "transparent"
        radius: root.radius - root.outerBorderMargin
        visible: root.bindToRect
        opacity: root.outerBorderOpacity
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
        x: -root.handleOffset
        y: -root.handleOffset
    }
    ResizeHandle {
        corner: "top-right"
        cursorShape: Qt.SizeBDiagCursor
        overlayWindow: root.overlayWindow
        visible: root.bindToRect && root.enableHandles
        x: parent.width - root.handleOffset
        y: -root.handleOffset
    }
    ResizeHandle {
        corner: "bottom-left"
        cursorShape: Qt.SizeBDiagCursor
        overlayWindow: root.overlayWindow
        visible: root.bindToRect && root.enableHandles
        x: -root.handleOffset
        y: parent.height - root.handleOffset
    }
    ResizeHandle {
        corner: "bottom-right"
        cursorShape: Qt.SizeFDiagCursor
        overlayWindow: root.overlayWindow
        visible: root.bindToRect && root.enableHandles
        x: parent.width - root.handleOffset
        y: parent.height - root.handleOffset
    }
    ResizeHandle {
        corner: "left"
        cursorShape: Qt.SizeHorCursor
        height: parent.height - root.handleSize
        overlayWindow: root.overlayWindow
        visible: root.bindToRect && root.enableHandles
        width: root.handleSize
        x: -root.handleOffset
        y: root.handleOffset
    }
    ResizeHandle {
        corner: "right"
        cursorShape: Qt.SizeHorCursor
        height: parent.height - root.handleSize
        overlayWindow: root.overlayWindow
        visible: root.bindToRect && root.enableHandles
        width: root.handleSize
        x: parent.width - root.handleOffset
        y: root.handleOffset
    }
    ResizeHandle {
        corner: "top"
        cursorShape: Qt.SizeVerCursor
        height: root.handleSize
        overlayWindow: root.overlayWindow
        visible: root.bindToRect && root.enableHandles
        width: parent.width - root.handleSize
        x: root.handleOffset
        y: -root.handleOffset
    }
    ResizeHandle {
        corner: "bottom"
        cursorShape: Qt.SizeVerCursor
        height: root.handleSize
        overlayWindow: root.overlayWindow
        visible: root.bindToRect && root.enableHandles
        width: parent.width - root.handleSize
        x: root.handleOffset
        y: parent.height - root.handleOffset
    }
}
