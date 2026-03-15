import QtQuick
import QtQuick.Window
import com.lortunate.minnow

Window {
    id: root

    readonly property color maskColor: AppTheme.overlayMask

    readonly property int padding: 4
    property rect selectionRect: Qt.rect(0, 0, 0, 0)
    property rect viewportRect: Qt.rect(0, 0, 0, 0)
    property string warningText: ""

    function flashSuccess() {
        successAnim.restart();
    }

    color: "transparent"

    flags: Qt.Window | Qt.FramelessWindowHint | Qt.WindowStaysOnTopHint | Qt.WindowTransparentForInput | Qt.Tool
    height: viewportRect.height
    width: viewportRect.width

    x: viewportRect.x
    y: viewportRect.y

    Rectangle {
        color: maskColor
        height: selectionRect.y - padding
        width: parent.width
        x: 0
        y: 0
    }
    Rectangle {
        color: maskColor
        height: parent.height - (selectionRect.y + selectionRect.height + padding)
        width: parent.width
        x: 0
        y: selectionRect.y + selectionRect.height + padding
    }
    Rectangle {
        color: maskColor
        height: selectionRect.height + (padding * 2)
        width: selectionRect.x - padding
        x: 0
        y: selectionRect.y - padding
    }
    Rectangle {
        color: maskColor
        height: selectionRect.height + (padding * 2)
        width: parent.width - (selectionRect.x + selectionRect.width + padding)
        x: selectionRect.x + selectionRect.width + padding
        y: selectionRect.y - padding
    }

    Rectangle {
        id: borderRect

        property color pulsedColor: AppTheme.selection

        border.color: warningText !== "" ? AppTheme.danger : (successFlash.opacity > 0 ? AppTheme.success : pulsedColor)
        border.width: warningText !== "" ? 3 : 2
        color: "transparent"
        height: selectionRect.height + (padding * 2)
        width: selectionRect.width + (padding * 2)
        x: selectionRect.x - padding
        y: selectionRect.y - padding

        SequentialAnimation on pulsedColor {
            loops: Animation.Infinite
            running: warningText === "" && successFlash.opacity === 0

            ColorAnimation {
                duration: 800
                easing.type: Easing.InOutQuad
                from: AppTheme.selection
                to: Qt.lighter(AppTheme.selection, 1.4)
            }
            ColorAnimation {
                duration: 800
                easing.type: Easing.InOutQuad
                to: AppTheme.selection
            }
        }

        Rectangle {
            id: successFlash

            anchors.fill: parent
            border.color: AppTheme.success
            border.width: 3
            color: "transparent"
            opacity: 0

            NumberAnimation on opacity {
                id: successAnim

                duration: 600
                easing.type: Easing.OutQuad
                from: 1.0
                running: false
                to: 0.0
            }
        }

        Rectangle {
            anchors.bottom: parent.top
            anchors.bottomMargin: 12
            anchors.horizontalCenter: parent.horizontalCenter
            color: AppTheme.danger
            height: warnTxt.contentHeight + 12
            radius: 6
            visible: warningText !== ""
            width: warnTxt.contentWidth + 24

            Text {
                id: warnTxt

                anchors.centerIn: parent
                color: "white"
                font.bold: true
                font.pixelSize: AppTheme.fontSizeBody
                text: warningText
            }
        }
    }
}
